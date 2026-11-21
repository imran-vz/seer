//! Copying and moving files and folders: within the Mac, within a device,
//! and between the two in either direction.
//!
//! Nothing is ever overwritten. An item whose name is taken at the
//! destination arrives as "name 2". A move removes each original only after
//! its copy is complete.

use crate::mtp::{self, Place};
use anyhow::{Context as _, anyhow, bail};
use bytes::Bytes;
use futures::executor::block_on;
use mtp_rs::mtp::{ByteRange, DEFAULT_CANCEL_TIMEOUT, NewObjectInfo, ObjectHandle, Storage};
use std::collections::HashSet;
use std::io::{Read as _, Write as _};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::SystemTime;

/// Bytes read from disk per write to a device. Large enough that the USB
/// link, not the reads, sets the pace.
const UPLOAD_CHUNK: usize = 1 << 20;
/// Bytes per read when copying between disks on the Mac.
const COPY_CHUNK: usize = 4 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Copy,
    Move,
}

/// Shared with the UI: how far a transfer is, and the way to stop it.
#[derive(Default)]
pub struct Progress {
    bytes_done: AtomicU64,
    bytes_total: AtomicU64,
    files_done: AtomicU64,
    files_total: AtomicU64,
    cancel: AtomicBool,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    /// One per item that was asked for, in the order given.
    items: Vec<ItemStatus>,
    active: Option<usize>,
    /// The file in hand, which is inside the active item when that is a folder.
    file: String,
    file_size: u64,
    file_done: u64,
}

/// How far one of the items asked for has got.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stage {
    Waiting,
    Active,
    Done,
    /// Nothing to do: it is already where it was being moved to.
    Skipped,
    Failed(String),
}

/// One of the items asked for: a file, or a folder with all that is in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemStatus {
    pub name: String,
    pub bytes_done: u64,
    /// Zero until it has been sized, and for items that move without their
    /// data being read (a rename, or a device moving its own file).
    pub bytes_total: u64,
    /// Files in it; one for a file.
    pub files: u64,
    pub stage: Stage,
}

/// Where a transfer is right now, for showing to the user.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Detail {
    /// The file being copied or moved. Empty before the first one starts.
    pub name: String,
    pub file_done: u64,
    /// Zero when no data is read, as when a device moves its own file.
    pub file_size: u64,
    pub files_done: u64,
    pub files_total: u64,
    pub bytes_done: u64,
    pub bytes_total: u64,
    /// How many items were asked for, and which of them is in hand.
    pub items: usize,
    pub active: Option<usize>,
}

impl Progress {
    /// By bytes where data is read, otherwise by files. Zero until the
    /// items have been sized.
    pub fn fraction(&self) -> f32 {
        let part = |done: &AtomicU64, total: u64| {
            (done.load(Ordering::Relaxed) as f64 / total as f64).min(1.) as f32
        };
        match (
            self.bytes_total.load(Ordering::Relaxed),
            self.files_total.load(Ordering::Relaxed),
        ) {
            (0, 0) => 0.,
            (0, files) => part(&self.files_done, files),
            (bytes, _) => part(&self.bytes_done, bytes),
        }
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    pub fn detail(&self) -> Detail {
        let state = self.state();
        Detail {
            name: state.file.clone(),
            file_size: state.file_size,
            file_done: state.file_done.min(state.file_size),
            files_done: self.files_done.load(Ordering::Relaxed),
            files_total: self.files_total.load(Ordering::Relaxed),
            bytes_done: self.bytes_done.load(Ordering::Relaxed),
            bytes_total: self.bytes_total.load(Ordering::Relaxed),
            items: state.items.len(),
            active: state.active,
        }
    }

    /// The items asked for, from `start`, at most `count` of them. A window
    /// rather than all, so showing a transfer of thousands stays cheap.
    pub fn items(&self, start: usize, count: usize) -> Vec<ItemStatus> {
        let state = self.state();
        state
            .items
            .iter()
            .skip(start)
            .take(count)
            .cloned()
            .collect()
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn list(&self, sources: &[PathBuf]) {
        self.state().items = sources
            .iter()
            .map(|source| ItemStatus {
                name: mtp::name_of(source),
                bytes_done: 0,
                bytes_total: 0,
                files: 0,
                stage: Stage::Waiting,
            })
            .collect();
    }

    fn size(&self, index: usize, bytes: u64, files: u64) {
        self.bytes_total.fetch_add(bytes, Ordering::Relaxed);
        self.files_total.fetch_add(files, Ordering::Relaxed);
        if let Some(item) = self.state().items.get_mut(index) {
            item.bytes_total = bytes;
            item.files = files;
        }
    }

    fn stage(&self, index: usize, stage: Stage) {
        let mut state = self.state();
        state.active = match stage {
            Stage::Active => Some(index),
            _ => state.active.filter(|active| *active != index),
        };
        if let Some(item) = state.items.get_mut(index) {
            if stage == Stage::Done {
                item.bytes_done = item.bytes_total;
            }
            item.stage = stage;
        }
    }

    fn add(&self, amount: u64) {
        self.bytes_done.fetch_add(amount, Ordering::Relaxed);
        let mut state = self.state();
        state.file_done += amount;
        if let Some(active) = state.active
            && let Some(item) = state.items.get_mut(active)
        {
            item.bytes_done = (item.bytes_done + amount).min(item.bytes_total);
        }
    }

    fn begin_file(&self, name: &str, size: u64) {
        let mut state = self.state();
        state.file = name.to_owned();
        state.file_size = size;
        state.file_done = 0;
    }

    fn end_file(&self) {
        self.files_done.fetch_add(1, Ordering::Relaxed);
    }
}

#[derive(Debug, Default)]
pub struct Outcome {
    /// Top-level items that arrived.
    pub done: usize,
    /// Name and reason for each top-level item that did not.
    pub failed: Vec<(String, String)>,
    pub cancelled: bool,
}

#[derive(Debug)]
struct Cancelled;

impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Cancelled.")
    }
}

impl std::error::Error for Cancelled {}

/// A file or folder to transfer, with everything inside it.
struct Item {
    name: String,
    path: PathBuf,
    /// Set for items on a device.
    handle: Option<ObjectHandle>,
    size: u64,
    modified: Option<SystemTime>,
    /// `Some` for folders.
    children: Option<Vec<Item>>,
}

impl Item {
    fn bytes(&self) -> u64 {
        match &self.children {
            Some(children) => children.iter().map(Item::bytes).sum(),
            None => self.size,
        }
    }

    fn files(&self) -> u64 {
        match &self.children {
            Some(children) => children.iter().map(Item::files).sum(),
            None => 1,
        }
    }
}

/// A folder to put things in.
#[derive(Clone, Copy)]
enum Target<'a> {
    Local(&'a Path),
    Device(&'a Storage, Option<ObjectHandle>),
}

/// Copies or moves `sources` into the folder `dest`. Blocks until done or
/// cancelled through `progress`.
pub fn run(sources: &[PathBuf], dest: &Path, mode: Mode, progress: &Progress) -> Outcome {
    let mut outcome = Outcome::default();
    progress.list(sources);
    let mut destination = match Destination::open(dest) {
        Ok(destination) => destination,
        Err(e) => {
            let reason = reason(&e);
            for (index, source) in sources.iter().enumerate() {
                progress.stage(index, Stage::Failed(reason.clone()));
                outcome.failed.push((mtp::name_of(source), reason.clone()));
            }
            return outcome;
        }
    };

    // Sizes first, so the bar is honest from the start. Listings made here
    // are the ones the transfer needs anyway.
    let mut planned = Vec::new();
    for (index, source) in sources.iter().enumerate() {
        if progress.cancelled() {
            outcome.cancelled = true;
            return outcome;
        }
        match Plan::new(source, dest, mode, &destination) {
            Ok(Some(plan)) => {
                progress.size(index, plan.bytes(), plan.files());
                planned.push((index, plan));
            }
            // Already where it is being moved to.
            Ok(None) => progress.stage(index, Stage::Skipped),
            Err(e) => {
                let reason = reason(&e);
                progress.stage(index, Stage::Failed(reason.clone()));
                outcome.failed.push((mtp::name_of(source), reason));
            }
        }
    }

    for (index, plan) in planned {
        if progress.cancelled() {
            outcome.cancelled = true;
            break;
        }
        let name = plan.name().to_owned();
        progress.stage(index, Stage::Active);
        match plan.execute(&mut destination, mode, progress) {
            Ok(()) => {
                progress.stage(index, Stage::Done);
                outcome.done += 1;
            }
            Err(e) if e.is::<Cancelled>() => {
                progress.stage(index, Stage::Waiting);
                outcome.cancelled = true;
                break;
            }
            Err(e) => {
                let reason = reason(&e);
                progress.stage(index, Stage::Failed(reason.clone()));
                outcome.failed.push((name, reason));
            }
        }
    }
    outcome
}

/// The folder being transferred into, and the names already in it.
struct Destination {
    path: PathBuf,
    device: Option<Place>,
    /// Lowercased; Android's storage ignores case, as most Macs do.
    names: HashSet<String>,
}

impl Destination {
    fn open(path: &Path) -> anyhow::Result<Destination> {
        let (device, names) = if mtp::is_device_path(path) {
            let path = mtp::landing(path)?;
            let place = mtp::place(&path)?;
            let names = place.children()?;
            let names = names.iter().map(|o| o.filename.to_lowercase()).collect();
            (Some(place), names)
        } else {
            let names = std::fs::read_dir(path)
                .with_context(|| format!("“{}” can’t be opened", path.display()))?
                .flatten()
                .map(|e| e.file_name().to_string_lossy().to_lowercase())
                .collect();
            (None, names)
        };
        Ok(Destination {
            path: path.to_owned(),
            device,
            names,
        })
    }

    fn target(&self) -> Target<'_> {
        match &self.device {
            Some(place) => Target::Device(&place.storage, place.handle),
            None => Target::Local(&self.path),
        }
    }

    fn has(&self, name: &str) -> bool {
        self.names.contains(&name.to_lowercase())
    }

    /// `name`, or "name 2", "name 3"… if that is taken. Claims the result.
    fn claim(&mut self, name: &str, is_dir: bool) -> String {
        let (stem, extension) = match name.rfind('.').filter(|i| *i > 0 && !is_dir) {
            Some(dot) => name.split_at(dot),
            None => (name, ""),
        };
        let mut candidate = name.to_owned();
        let mut n = 2;
        while self.has(&candidate) {
            candidate = format!("{stem} {n}{extension}");
            n += 1;
        }
        self.names.insert(candidate.to_lowercase());
        candidate
    }
}

/// How one selected item will get to the destination.
enum Plan {
    /// Read from one side and written to the other, item by item.
    Stream { item: Item, source: Option<Place> },
    /// Done by the device itself; no data crosses the cable.
    OnDevice { source: Place },
    /// A rename on the same disk.
    Rename { source: PathBuf },
}

impl Plan {
    fn new(
        source: &Path,
        dest: &Path,
        mode: Mode,
        destination: &Destination,
    ) -> anyhow::Result<Option<Plan>> {
        if mode == Mode::Move && source.parent() == Some(dest) {
            return Ok(None);
        }
        if dest.starts_with(source) {
            bail!("A folder can’t be put inside itself.");
        }
        let on_device = mtp::is_device_path(source);
        Ok(Some(match (on_device, &destination.device) {
            (true, Some(to)) => {
                let from = mtp::place(source)?;
                if from.device != to.device {
                    bail!("Copy it to the Mac first, then to the other device.");
                }
                Plan::OnDevice { source: from }
            }
            (true, None) => {
                let from = mtp::place(source)?;
                let item = device_item(&from)?;
                Plan::Stream {
                    item,
                    source: Some(from),
                }
            }
            // On one disk a move is a rename. Across disks the data has to
            // be read and written, so it is sized and shown like a copy.
            (false, None) if mode == Mode::Move && same_disk(source, dest) => Plan::Rename {
                source: source.to_owned(),
            },
            (false, _) => Plan::Stream {
                item: local_item(source)?,
                source: None,
            },
        }))
    }

    fn name(&self) -> &str {
        match self {
            Plan::Stream { item, .. } => &item.name,
            Plan::OnDevice { source } => source
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default(),
            Plan::Rename { source } => source
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default(),
        }
    }

    /// The bytes that will be read. None where the item moves whole.
    fn bytes(&self) -> u64 {
        match self {
            Plan::Stream { item, .. } => item.bytes(),
            _ => 0,
        }
    }

    fn files(&self) -> u64 {
        match self {
            Plan::Stream { item, .. } => item.files(),
            _ => 1,
        }
    }

    fn execute(
        self,
        destination: &mut Destination,
        mode: Mode,
        progress: &Progress,
    ) -> anyhow::Result<()> {
        match self {
            Plan::Stream { item, source } => {
                let name = destination.claim(&item.name, item.children.is_some());
                let handle = transfer(
                    &item,
                    source.as_ref(),
                    destination.target(),
                    &name,
                    mode,
                    progress,
                )?;
                if let (Some(place), Some(handle)) = (&destination.device, handle) {
                    place.remember(&name, handle);
                }
                if let (Mode::Move, Some(source)) = (mode, &source) {
                    source.forget();
                }
                Ok(())
            }
            Plan::OnDevice { source } => {
                let to = destination.device.as_ref().expect("planned for a device");
                let name = source.name();
                if destination.has(&name) {
                    bail!("An item with that name is already there.");
                }
                let handle = source.handle.context("A storage can’t be copied.")?;
                progress.begin_file(&name, 0);
                let parent = to.handle.unwrap_or(ObjectHandle::ROOT);
                let storage = Some(to.storage.id());
                match mode {
                    Mode::Copy => {
                        block_on(source.storage.copy_object(handle, parent, storage))
                            .map_err(|e| source.fail(e))?;
                    }
                    Mode::Move => {
                        block_on(source.storage.move_object(handle, parent, storage))
                            .map_err(|e| source.fail(e))?;
                        source.forget();
                        to.remember(&name, handle);
                    }
                }
                destination.names.insert(name.to_lowercase());
                progress.end_file();
                Ok(())
            }
            Plan::Rename { source } => {
                let is_dir = source.is_dir();
                let name = destination.claim(&mtp::name_of(&source), is_dir);
                let target = destination.path.join(&name);
                progress.begin_file(&name, 0);
                match std::fs::rename(&source, &target) {
                    Ok(()) => {
                        progress.end_file();
                        Ok(())
                    }
                    // Another disk after all: copy across, then remove the
                    // original. Each file counts itself as it finishes.
                    Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
                        let item = local_item(&source)?;
                        transfer(&item, None, destination.target(), &name, mode, progress)?;
                        Ok(())
                    }
                    Err(e) => Err(e.into()),
                }
            }
        }
    }
}

fn local_item(path: &Path) -> anyhow::Result<Item> {
    // Follows links, so a linked folder is copied as a folder.
    let meta = std::fs::metadata(path)?;
    let children = if meta.is_dir() {
        let mut children = Vec::new();
        for child in std::fs::read_dir(path)? {
            children.push(local_item(&child?.path())?);
        }
        Some(children)
    } else {
        None
    };
    Ok(Item {
        name: mtp::name_of(path),
        path: path.to_owned(),
        handle: None,
        size: meta.len(),
        modified: meta.modified().ok(),
        children,
    })
}

fn device_item(place: &Place) -> anyhow::Result<Item> {
    let handle = place.handle.context("A storage can’t be copied.")?;
    let info = block_on(place.storage.get_object_info(handle)).map_err(|e| place.fail(e))?;
    let dir = place.path.parent().unwrap_or(Path::new(mtp::ROOT));
    device_tree(place, mtp::entry(dir, &info), handle)
}

fn device_tree(
    place: &Place,
    entry: crate::fs::Entry,
    handle: ObjectHandle,
) -> anyhow::Result<Item> {
    let children = if entry.is_dir() {
        let objects =
            block_on(place.storage.list_objects(Some(handle))).map_err(|e| place.fail(e))?;
        let mut children = Vec::new();
        for object in objects {
            children.push(device_tree(
                place,
                mtp::entry(&entry.path, &object),
                object.handle,
            )?);
        }
        Some(children)
    } else {
        None
    };
    Ok(Item {
        name: entry.name,
        path: entry.path,
        handle: Some(handle),
        size: entry.size,
        modified: entry.modified,
        children,
    })
}

/// Sends `item` into `target` under `name`. `source` is the device it is on,
/// if it is on one. Returns the new object's handle when the target is a
/// device.
fn transfer(
    item: &Item,
    source: Option<&Place>,
    target: Target<'_>,
    name: &str,
    mode: Mode,
    progress: &Progress,
) -> anyhow::Result<Option<ObjectHandle>> {
    if progress.cancelled() {
        return Err(Cancelled.into());
    }
    let Some(children) = &item.children else {
        progress.begin_file(&item.name, item.size);
        let made = match (source, target) {
            (None, Target::Local(dir)) => {
                copy_local(item, &dir.join(name), progress)?;
                None
            }
            (Some(from), Target::Local(dir)) => {
                download(from, item, &dir.join(name), progress)?;
                None
            }
            (None, Target::Device(storage, parent)) => {
                Some(upload(storage, parent, name, item, progress)?)
            }
            (Some(_), Target::Device(..)) => bail!("The device copies its own files."),
        };
        if mode == Mode::Move {
            remove(item, source)?;
        }
        progress.end_file();
        return Ok(made);
    };

    let (inside, made) = match target {
        Target::Local(dir) => {
            let path = dir.join(name);
            std::fs::create_dir(&path)?;
            (Some(path), None)
        }
        Target::Device(storage, parent) => {
            let handle = block_on(storage.create_folder(parent, name)).map_err(mtp::explain)?;
            (None, Some(handle))
        }
    };
    let target = match (&inside, target) {
        (Some(path), _) => Target::Local(path),
        (None, Target::Device(storage, _)) => Target::Device(storage, made),
        (None, Target::Local(_)) => unreachable!("a local folder has a path"),
    };
    for child in children {
        transfer(child, source, target, &child.name, mode, progress)
            .with_context(|| format!("“{}”", child.name))?;
    }
    if mode == Mode::Move {
        remove(item, source)?;
    }
    Ok(made)
}

/// Removes an original that has been moved. Folders are empty by now.
fn remove(item: &Item, source: Option<&Place>) -> anyhow::Result<()> {
    match (source, item.handle) {
        (Some(from), Some(handle)) => {
            block_on(from.storage.delete(handle)).map_err(|e| from.fail(e))
        }
        _ if item.children.is_some() => Ok(std::fs::remove_dir(&item.path)?),
        _ => Ok(std::fs::remove_file(&item.path)?),
    }
}

/// Why an item did not arrive, in plain words. A file inside a folder is
/// named ahead of the reason.
fn reason(error: &anyhow::Error) -> String {
    use std::io::ErrorKind as Kind;
    let mut parts: Vec<String> = error.chain().map(|cause| cause.to_string()).collect();
    if let Some(io) = error.root_cause().downcast_ref::<std::io::Error>() {
        let plain = match io.kind() {
            Kind::NotFound => "It is no longer there. It may have been moved or deleted.",
            Kind::PermissionDenied => {
                "Seer doesn’t have permission. Allow access in System Settings › Privacy & Security › Files and Folders."
            }
            Kind::StorageFull | Kind::QuotaExceeded => "The disk is full.",
            Kind::ReadOnlyFilesystem => "The disk can’t be written to.",
            Kind::FileTooLarge => "The file is too large for that disk’s format.",
            Kind::InvalidFilename => "The disk’s format doesn’t allow that name.",
            Kind::AlreadyExists => "An item with that name is already there.",
            _ => "",
        };
        if !plain.is_empty()
            && let Some(last) = parts.last_mut()
        {
            *last = plain.to_owned();
        }
    }
    parts.join(": ")
}

/// True when both are on one disk, so a move between them is a rename.
fn same_disk(a: &Path, b: &Path) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    let disk = |path: &Path| std::fs::metadata(path).map(|meta| meta.dev()).ok();
    disk(a).is_some() && disk(a) == disk(b)
}

/// Where a file is written until it is complete, so an interrupted copy
/// never looks finished.
fn partial_path(to: &Path) -> PathBuf {
    to.with_file_name(format!(".{}.seer-partial", mtp::name_of(to)))
}

/// Copies a file on the Mac. On one APFS disk that is an instant clone;
/// anywhere else the data is streamed, so it can be followed and stopped.
fn copy_local(item: &Item, to: &Path, progress: &Progress) -> anyhow::Result<()> {
    if clone_file(&item.path, to) {
        progress.add(item.size);
        return Ok(());
    }
    stream_local(&item.path, to, progress)
}

/// Reads one file and writes another, reporting as it goes.
fn stream_local(from: &Path, to: &Path, progress: &Progress) -> anyhow::Result<()> {
    let partial = partial_path(to);
    let result = (|| -> anyhow::Result<()> {
        let mut source = std::fs::File::open(from)?;
        let mut file = std::fs::File::create(&partial)?;
        let mut chunk = vec![0u8; COPY_CHUNK];
        loop {
            let read = source.read(&mut chunk)?;
            if read == 0 {
                break;
            }
            file.write_all(&chunk[..read])?;
            progress.add(read as u64);
            if progress.cancelled() {
                return Err(Cancelled.into());
            }
        }
        drop(file);
        copy_attributes(from, &partial);
        Ok(())
    })();
    match result {
        Ok(()) => Ok(std::fs::rename(&partial, to)?),
        Err(e) => {
            std::fs::remove_file(&partial).ok();
            Err(e)
        }
    }
}

/// Asks the file system for a copy that shares the original's blocks.
/// False when it can't, as across disks or on a disk that is not APFS.
fn clone_file(from: &Path, to: &Path) -> bool {
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::ffi::OsStrExt as _;
        let path = |p: &Path| std::ffi::CString::new(p.as_os_str().as_bytes());
        let (Ok(from), Ok(to)) = (path(from), path(to)) else {
            return false;
        };
        // SAFETY: both are valid NUL-terminated paths that outlive the call.
        unsafe { libc::clonefile(from.as_ptr(), to.as_ptr(), 0) == 0 }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (from, to);
        false
    }
}

/// Carries dates, permissions, tags and other attributes over to a copy.
/// Best effort: a disk that can't hold some of them keeps the rest.
fn copy_attributes(from: &Path, to: &Path) {
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::ffi::OsStrExt as _;
        let path = |p: &Path| std::ffi::CString::new(p.as_os_str().as_bytes());
        if let (Ok(from), Ok(to)) = (path(from), path(to)) {
            // SAFETY: both are valid NUL-terminated paths that outlive the
            // call, and a null state is allowed.
            unsafe {
                libc::copyfile(
                    from.as_ptr(),
                    to.as_ptr(),
                    std::ptr::null_mut(),
                    libc::COPYFILE_METADATA,
                );
            }
        }
    }
    if let Ok(meta) = std::fs::metadata(from) {
        let file = std::fs::File::options().write(true).open(to);
        if let (Ok(file), Ok(modified)) = (file, meta.modified()) {
            file.set_modified(modified).ok();
        }
        std::fs::set_permissions(to, meta.permissions()).ok();
    }
}

/// Streams a file from the device to `to`, through a hidden partial file so
/// an interrupted copy never looks complete.
fn download(from: &Place, item: &Item, to: &Path, progress: &Progress) -> anyhow::Result<()> {
    let handle = item.handle.context("Not on a device.")?;
    let partial = partial_path(to);
    let result = block_on(async {
        let mut incoming = from
            .storage
            .download(handle, ByteRange::Full)
            .await
            .map_err(|e| from.fail(e))?;
        let mut file = std::fs::File::create(&partial)?;
        let mut written = 0u64;
        let mut copy = async || -> anyhow::Result<()> {
            while let Some(chunk) = incoming.next_chunk().await {
                let chunk = chunk.map_err(|e| from.fail(e))?;
                file.write_all(&chunk)?;
                written += chunk.len() as u64;
                progress.add(chunk.len() as u64);
                if progress.cancelled() {
                    return Err(Cancelled.into());
                }
            }
            Ok(())
        };
        if let Err(e) = copy().await {
            // Leaves the device ready for the next request.
            incoming.cancel(DEFAULT_CANCEL_TIMEOUT).await.ok();
            return Err(e);
        }
        if written != incoming.size() {
            bail!("The device sent {written} of {} bytes.", incoming.size());
        }
        if let Some(modified) = item.modified {
            file.set_modified(modified).ok();
        }
        Ok(())
    });
    match result {
        Ok(()) => Ok(std::fs::rename(&partial, to)?),
        Err(e) => {
            std::fs::remove_file(&partial).ok();
            Err(e)
        }
    }
}

/// Streams a file from disk to the device.
fn upload(
    storage: &Storage,
    parent: Option<ObjectHandle>,
    name: &str,
    item: &Item,
    progress: &Progress,
) -> anyhow::Result<ObjectHandle> {
    let file = std::fs::File::open(&item.path)?;
    let size = file.metadata()?.len();
    let mut info = NewObjectInfo::file(name, size);
    if let Some(date) = item.modified.and_then(mtp::device_time) {
        info = info.with_modified(date);
    }
    let chunks = futures::stream::unfold(file, |mut file| async move {
        let mut chunk = vec![0u8; UPLOAD_CHUNK];
        match file.read(&mut chunk) {
            Ok(0) => None,
            Ok(read) => {
                chunk.truncate(read);
                progress.add(read as u64);
                Some((Ok(Bytes::from(chunk)), file))
            }
            Err(e) => Some((Err(e), file)),
        }
    });
    let sent = block_on(
        storage.upload_with_progress(parent, info, Box::pin(chunks), |_| {
            match progress.cancelled() {
                true => ControlFlow::Break(()),
                false => ControlFlow::Continue(()),
            }
        }),
    );
    sent.map_err(|e| {
        // The device keeps what it received; a broken file helps no one.
        if let Some(partial) = e.partial {
            block_on(storage.delete(partial)).ok();
        }
        match e.source {
            mtp_rs::Error::Cancelled => anyhow!(Cancelled),
            source => mtp::explain(source),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mtp::testing::Phone;

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    fn ok(outcome: &Outcome, done: usize) {
        assert!(outcome.failed.is_empty(), "{:?}", outcome.failed);
        assert!(!outcome.cancelled);
        assert_eq!(outcome.done, done);
    }

    #[test]
    fn copies_to_the_device_and_back() {
        let phone = Phone::new("RoundTrip");
        let mac = tempfile::tempdir().unwrap();
        let album = mac.path().join("Album");
        std::fs::create_dir_all(album.join("Disc 1")).unwrap();
        let song: Vec<u8> = (0..3_000_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(album.join("Disc 1/song.flac"), &song).unwrap();
        std::fs::write(album.join("cover.jpg"), b"jpeg").unwrap();
        std::fs::write(mac.path().join("note.txt"), b"hello").unwrap();

        let progress = Progress::default();
        let sources = [album.clone(), mac.path().join("note.txt")];
        ok(&run(&sources, &phone.top, Mode::Copy, &progress), 2);
        assert_eq!(progress.fraction(), 1.);
        let detail = progress.detail();
        assert_eq!((detail.files_done, detail.files_total), (3, 3));
        assert_eq!(
            (detail.name.as_str(), detail.file_done, detail.file_size),
            ("note.txt", 5, 5)
        );
        assert_eq!((detail.items, detail.active), (2, None));
        assert_eq!(detail.bytes_done, detail.bytes_total);
        // Every item asked for is reported, not just the one in hand.
        let items = progress.items(0, 10);
        assert_eq!(items.len(), 2);
        assert_eq!((items[0].name.as_str(), items[0].files), ("Album", 2));
        assert_eq!(items[0].bytes_total, song.len() as u64 + 4);
        assert_eq!(
            (items[1].name.as_str(), items[1].bytes_total),
            ("note.txt", 5)
        );
        assert!(
            items
                .iter()
                .all(|i| i.stage == Stage::Done && i.bytes_done == i.bytes_total)
        );
        assert_eq!(progress.items(1, 10).len(), 1);
        let on_phone = phone.dir.path();
        assert_eq!(
            std::fs::read(on_phone.join("Album/Disc 1/song.flac")).unwrap(),
            song
        );
        assert_eq!(std::fs::read(on_phone.join("note.txt")).unwrap(), b"hello");
        assert!(album.exists(), "a copy leaves the original");

        let back = tempfile::tempdir().unwrap();
        // Sent to the device itself, a file lands in its only storage.
        let device = phone.top.parent().unwrap();
        ok(
            &run(
                &[album.join("cover.jpg")],
                device,
                Mode::Copy,
                &Progress::default(),
            ),
            1,
        );
        assert!(on_phone.join("cover.jpg").exists());
        std::fs::remove_file(on_phone.join("cover.jpg")).unwrap();

        let sources = [phone.top.join("Album"), phone.top.join("note.txt")];
        ok(
            &run(&sources, back.path(), Mode::Copy, &Progress::default()),
            2,
        );
        assert_eq!(
            std::fs::read(back.path().join("Album/Disc 1/song.flac")).unwrap(),
            song
        );
        assert_eq!(
            std::fs::read(back.path().join("Album/cover.jpg")).unwrap(),
            b"jpeg"
        );
        assert_eq!(
            names(back.path()),
            ["Album", "note.txt"],
            "no partial files remain"
        );
    }

    #[test]
    fn moves_remove_the_original() {
        let phone = Phone::new("Move");
        let mac = tempfile::tempdir().unwrap();
        std::fs::create_dir(mac.path().join("Clips")).unwrap();
        std::fs::write(mac.path().join("Clips/a.mp4"), b"aaaa").unwrap();

        let sources = [mac.path().join("Clips")];
        ok(
            &run(&sources, &phone.top, Mode::Move, &Progress::default()),
            1,
        );
        assert!(names(mac.path()).is_empty());
        assert_eq!(
            std::fs::read(phone.dir.path().join("Clips/a.mp4")).unwrap(),
            b"aaaa"
        );

        let sources = [phone.top.join("Clips")];
        ok(
            &run(&sources, mac.path(), Mode::Move, &Progress::default()),
            1,
        );
        assert_eq!(
            std::fs::read(mac.path().join("Clips/a.mp4")).unwrap(),
            b"aaaa"
        );
        assert!(names(phone.dir.path()).is_empty());
    }

    #[test]
    fn never_overwrites() {
        let phone = Phone::new("Collide");
        let mac = tempfile::tempdir().unwrap();
        std::fs::write(mac.path().join("a.txt"), b"new").unwrap();
        std::fs::write(phone.dir.path().join("A.txt"), b"old").unwrap();

        ok(
            &run(
                &[mac.path().join("a.txt")],
                &phone.top,
                Mode::Copy,
                &Progress::default(),
            ),
            1,
        );
        assert_eq!(
            std::fs::read(phone.dir.path().join("A.txt")).unwrap(),
            b"old"
        );
        assert_eq!(
            std::fs::read(phone.dir.path().join("a 2.txt")).unwrap(),
            b"new"
        );

        ok(
            &run(
                &[phone.top.join("A.txt")],
                mac.path(),
                Mode::Copy,
                &Progress::default(),
            ),
            1,
        );
        assert_eq!(std::fs::read(mac.path().join("a.txt")).unwrap(), b"new");
        assert_eq!(std::fs::read(mac.path().join("A 2.txt")).unwrap(), b"old");
    }

    #[test]
    fn moves_and_copies_within_the_device() {
        let phone = Phone::new("Within");
        std::fs::create_dir(phone.dir.path().join("Download")).unwrap();
        std::fs::create_dir(phone.dir.path().join("Movies")).unwrap();
        std::fs::write(phone.dir.path().join("Download/film.mkv"), b"film").unwrap();

        let film = phone.top.join("Download/film.mkv");
        let movies = phone.top.join("Movies");
        ok(
            &run(
                std::slice::from_ref(&film),
                &movies,
                Mode::Copy,
                &Progress::default(),
            ),
            1,
        );
        assert!(phone.dir.path().join("Download/film.mkv").exists());
        assert_eq!(
            std::fs::read(phone.dir.path().join("Movies/film.mkv")).unwrap(),
            b"film"
        );

        // The name is taken now, and the device cannot rename as it copies.
        let again = run(
            std::slice::from_ref(&film),
            &movies,
            Mode::Move,
            &Progress::default(),
        );
        assert_eq!(again.failed.len(), 1);
        assert!(phone.dir.path().join("Download/film.mkv").exists());

        ok(
            &run(&[film], &phone.top, Mode::Move, &Progress::default()),
            1,
        );
        assert!(phone.dir.path().join("film.mkv").exists());
        assert!(!phone.dir.path().join("Download/film.mkv").exists());
    }

    #[test]
    fn local_copy_and_move() {
        let dir = tempfile::tempdir().unwrap();
        let (from, to) = (dir.path().join("from"), dir.path().join("to"));
        std::fs::create_dir_all(from.join("sub")).unwrap();
        std::fs::create_dir(&to).unwrap();
        std::fs::write(from.join("sub/x.bin"), b"x").unwrap();

        ok(
            &run(&[from.join("sub")], &to, Mode::Copy, &Progress::default()),
            1,
        );
        ok(
            &run(&[from.join("sub")], &to, Mode::Move, &Progress::default()),
            1,
        );
        assert_eq!(names(&to), ["sub", "sub 2"]);
        assert!(names(&from).is_empty());

        let inside = run(
            std::slice::from_ref(&to),
            &to.join("sub"),
            Mode::Copy,
            &Progress::default(),
        );
        assert_eq!(inside.failed.len(), 1);
    }

    #[test]
    fn cancelling_leaves_nothing_half_done() {
        let phone = Phone::new("Cancel");
        let mac = tempfile::tempdir().unwrap();
        std::fs::write(phone.dir.path().join("big.bin"), vec![7u8; 4_000_000]).unwrap();

        let progress = Progress::default();
        progress.cancel();
        let outcome = run(
            &[phone.top.join("big.bin")],
            mac.path(),
            Mode::Move,
            &progress,
        );
        assert!(outcome.cancelled);
        assert!(names(mac.path()).is_empty());
        assert!(phone.dir.path().join("big.bin").exists());
    }

    /// Needs a phone plugged in and sharing files. Writes one file to its
    /// Download folder and removes it again. Run with
    /// `cargo test -p seer-core --release -- --ignored --nocapture device_speed`.
    #[test]
    #[ignore = "timing, and needs a real device"]
    fn device_speed() {
        const SIZE: usize = 256 << 20;
        let device = mtp::devices()
            .into_iter()
            .next()
            .expect("a device is connected");
        let storage = mtp::list_dir(&device.path).unwrap().remove(0).path;
        let download = storage.join("Download");

        let start = std::time::Instant::now();
        let listed = mtp::list_dir(&download).unwrap().len();
        println!("listed {listed} items in {:?}", start.elapsed());

        let mac = tempfile::tempdir().unwrap();
        let file = mac.path().join("seer-speed-test.bin");
        let data: Vec<u8> = (0..SIZE as u32)
            .map(|i| (i.wrapping_mul(2_654_435_761) >> 24) as u8)
            .collect();
        std::fs::write(&file, &data).unwrap();
        let rate = |start: std::time::Instant| SIZE as f64 / 1e6 / start.elapsed().as_secs_f64();

        let start = std::time::Instant::now();
        ok(
            &run(&[file], &download, Mode::Move, &Progress::default()),
            1,
        );
        println!("to the device: {:.1} MB/s", rate(start));

        let on_device = download.join("seer-speed-test.bin");
        let start = std::time::Instant::now();
        ok(
            &run(&[on_device], mac.path(), Mode::Move, &Progress::default()),
            1,
        );
        println!("from the device: {:.1} MB/s", rate(start));

        assert!(std::fs::read(mac.path().join("seer-speed-test.bin")).unwrap() == data);
        assert!(
            !mtp::list_dir(&download)
                .unwrap()
                .iter()
                .any(|e| e.name == "seer-speed-test.bin")
        );
    }

    #[test]
    fn reports_each_item_as_it_goes() {
        let dir = tempfile::tempdir().unwrap();
        let (from, to) = (dir.path().join("from"), dir.path().join("to"));
        std::fs::create_dir_all(&from).unwrap();
        std::fs::create_dir(&to).unwrap();
        std::fs::write(from.join("a.bin"), b"aaa").unwrap();
        std::fs::write(to.join("stays.bin"), b"s").unwrap();

        // One arrives, one is missing, one is already where it is going.
        let progress = Progress::default();
        let sources = [
            from.join("a.bin"),
            from.join("gone.bin"),
            to.join("stays.bin"),
        ];
        let outcome = run(&sources, &to, Mode::Move, &progress);
        assert_eq!((outcome.done, outcome.failed.len()), (1, 1));
        let stages: Vec<Stage> = progress.items(0, 3).into_iter().map(|i| i.stage).collect();
        assert_eq!(stages[0], Stage::Done);
        assert_eq!(
            stages[1],
            Stage::Failed("It is no longer there. It may have been moved or deleted.".into())
        );
        assert_eq!(stages[2], Stage::Skipped);
        // Nothing was read for a rename, so the bar goes by files.
        assert_eq!(progress.fraction(), 1.);

        // Before anything has run, every item is listed as waiting.
        let cancelled = Progress::default();
        cancelled.cancel();
        run(&sources, &to, Mode::Copy, &cancelled);
        assert!(
            cancelled
                .items(0, 3)
                .iter()
                .all(|i| i.stage == Stage::Waiting)
        );
    }

    #[test]
    fn streams_a_copy_that_cannot_be_cloned() {
        let dir = tempfile::tempdir().unwrap();
        let data: Vec<u8> = (0..9_000_000u32).map(|i| (i % 253) as u8).collect();
        let source = dir.path().join("film.mkv");
        std::fs::write(&source, &data).unwrap();
        let then = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        let file = std::fs::File::options().write(true).open(&source).unwrap();
        file.set_modified(then).unwrap();
        drop(file);

        // A name that is taken stops a clone, as another disk would.
        let target = dir.path().join("copy.mkv");
        std::fs::write(&target, b"in the way").unwrap();
        assert!(!clone_file(&source, &target));
        std::fs::remove_file(&target).unwrap();

        // The streaming path is what runs across disks.
        let progress = Progress::default();
        stream_local(&source, &target, &progress).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), data);
        assert_eq!(
            std::fs::metadata(&target).unwrap().modified().unwrap(),
            then
        );
        assert_eq!(progress.detail().bytes_done, data.len() as u64);

        // Stopped part-way, it leaves nothing behind.
        let stopped = Progress::default();
        stopped.cancel();
        let other = dir.path().join("other.mkv");
        assert!(stream_local(&source, &other, &stopped).is_err());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);

        // On one APFS disk the same copy is a clone.
        let item = local_item(&source).unwrap();
        let cloned = dir.path().join("clone.mkv");
        copy_local(&item, &cloned, &Progress::default()).unwrap();
        assert_eq!(std::fs::read(&cloned).unwrap(), data);
    }
}
