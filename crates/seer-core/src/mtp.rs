//! Android phones and other MTP devices connected over USB.
//!
//! A device has no paths of its own, only numbered objects, so this module
//! gives it some: `/Devices/<device>/<storage>/<folders…>`. The rest of Seer
//! can then treat a phone as one more place to browse. Nothing under
//! [`ROOT`] exists on disk; pass such paths only to this module and to
//! [`crate::transfer`].
//!
//! Every function blocks on USB. Call them from a background thread.

use crate::fs::{Entry, Kind};
use anyhow::{Context as _, anyhow, bail};
use futures::StreamExt as _;
use futures::executor::block_on;
use mtp_rs::mtp::{
    HotplugEvent, MtpDevice, MtpDeviceInfo, ObjectHandle, ObjectInfo, Storage, watch_devices,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};
use std::time::{Duration, SystemTime};

/// Where connected devices appear.
pub const ROOT: &str = "/Devices";

pub fn is_device_path(path: &Path) -> bool {
    path.starts_with(ROOT)
}

/// A connected device. `name` is unique among the devices connected now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub name: String,
    pub path: PathBuf,
}

/// An open connection. A device allows only one, so it is shared.
pub(crate) struct Session {
    device: MtpDevice,
    /// Storages by the name they are listed under.
    storages: Mutex<Vec<(String, Arc<Storage>)>>,
    /// Object handles of the paths seen so far.
    handles: Mutex<HashMap<PathBuf, ObjectHandle>>,
}

#[derive(Default)]
struct State {
    known: Vec<(String, MtpDeviceInfo)>,
    sessions: HashMap<String, Arc<Session>>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

// Devices

/// The devices connected now.
pub fn devices() -> Vec<Device> {
    let found = MtpDevice::list_devices().unwrap_or_default();
    let mut state = lock(&STATE);
    state.known = name_all(found);
    let names: Vec<&String> = state.known.iter().map(|(name, _)| name).collect();
    let gone: Vec<String> = state
        .sessions
        .keys()
        .filter(|name| !names.contains(name))
        .cloned()
        .collect();
    for name in gone {
        state.sessions.remove(&name);
    }
    state
        .known
        .iter()
        .map(|(name, _)| Device {
            name: name.clone(),
            path: Path::new(ROOT).join(name),
        })
        .collect()
}

/// Calls `on_change` with the connected devices now and again whenever one
/// is plugged in or unplugged. Returns only if the OS stops reporting.
pub fn watch(mut on_change: impl FnMut(Vec<Device>)) -> anyhow::Result<()> {
    let mut events = watch_devices().map_err(|e| anyhow!("{e}"))?;
    // Devices already connected are the first events.
    block_on(async {
        while let Some(event) = events.next().await {
            let (HotplugEvent::Arrived(_) | HotplugEvent::Left(_)) = event;
            on_change(devices());
        }
    });
    Ok(())
}

/// Gives every device a name that is safe as a path component and unique.
fn name_all(found: Vec<MtpDeviceInfo>) -> Vec<(String, MtpDeviceInfo)> {
    let base = |info: &MtpDeviceInfo| {
        info.product
            .clone()
            .or_else(|| info.manufacturer.clone())
            .map(|name| name.replace(['/', ':'], "-").trim().to_owned())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "Android Device".to_owned())
    };
    let mut named: Vec<(String, MtpDeviceInfo)> = Vec::new();
    for info in found {
        let mut name = base(&info);
        if named.iter().any(|(taken, _)| *taken == name) {
            // Two of the same model: tell them apart by serial number.
            let serial = info.serial_number.clone().unwrap_or_default();
            let skip = serial.chars().count().saturating_sub(4);
            let tail: String = serial.chars().skip(skip).collect();
            name = format!("{name} ({tail})");
            let mut n = 2;
            while named.iter().any(|(taken, _)| *taken == name) {
                name = format!("{} {n}", base(&info));
                n += 1;
            }
        }
        named.push((name, info));
    }
    named
}

fn session(device: &str) -> anyhow::Result<Arc<Session>> {
    if let Some(session) = lock(&STATE).sessions.get(device) {
        return Ok(session.clone());
    }
    let known = |state: &State| {
        state
            .known
            .iter()
            .find(|(name, _)| name == device)
            .map(|(_, info)| info.location_id)
    };
    let location = known(&lock(&STATE));
    let location = match location {
        Some(location) => location,
        None => {
            devices();
            known(&lock(&STATE)).ok_or_else(|| anyhow!("“{device}” is not connected."))?
        }
    };
    // Held while opening so two threads never open the same device.
    let mut state = lock(&STATE);
    if let Some(session) = state.sessions.get(device) {
        return Ok(session.clone());
    }
    let opened = block_on(MtpDevice::builder().open_by_location(location)).map_err(explain)?;
    let session = Arc::new(Session {
        device: opened,
        storages: Mutex::default(),
        handles: Mutex::default(),
    });
    state.sessions.insert(device.to_owned(), session.clone());
    Ok(session)
}

/// Says what went wrong in plain words, with what to do about it.
pub(crate) fn explain(error: mtp_rs::Error) -> anyhow::Error {
    use mtp_rs::Error as E;
    let message = match &error {
        _ if error.is_exclusive_access() => {
            "Another app is using this device. Quit it (another Seer window, Android File Transfer, Image Capture or Photos), then reconnect the device."
        }
        _ if error.is_disconnected() => "The device was disconnected.",
        E::NoDevice => "The device is not connected.",
        E::StorageFull => "The device is full.",
        E::AccessDenied => "The device does not allow that here.",
        E::Unsupported => "The device does not support that.",
        E::Busy => "The device is busy. Try again in a moment.",
        E::Timeout => "The device did not answer. Unlock it and try again.",
        E::AlreadyExists => "An item with that name is already there.",
        E::NotFound | E::StaleHandle => "It is no longer on the device.",
        _ => return anyhow!("{error}"),
    };
    anyhow!(message)
}

/// Converts a device error, and forgets the connection if it is dead so the
/// next call opens a new one.
fn fail(device: &str, error: mtp_rs::Error) -> anyhow::Error {
    if error.is_disconnected() || matches!(error, mtp_rs::Error::DeviceReset) {
        lock(&STATE).sessions.remove(device);
    }
    explain(error)
}

// Paths

/// A folder or file on a device, found from its path.
pub(crate) struct Place {
    pub device: String,
    pub storage: Arc<Storage>,
    /// `None` is the top of the storage.
    pub handle: Option<ObjectHandle>,
    pub path: PathBuf,
    session: Arc<Session>,
}

/// Splits a device path into device, storage and the storage's own path.
fn split(path: &Path) -> Option<(String, Option<String>)> {
    let rest = path.strip_prefix(ROOT).ok()?;
    let mut parts = rest.components();
    let device = parts.next()?.as_os_str().to_str()?.to_owned();
    let storage = parts
        .next()
        .and_then(|s| s.as_os_str().to_str())
        .map(str::to_owned);
    Some((device, storage))
}

fn storages(device: &str, session: &Session) -> anyhow::Result<Vec<(String, Arc<Storage>)>> {
    let found = block_on(session.device.storages()).map_err(|e| fail(device, e))?;
    let mut named: Vec<(String, Arc<Storage>)> = Vec::new();
    for storage in found {
        let base = match storage.info().description.replace(['/', ':'], "-").trim() {
            "" => "Storage".to_owned(),
            name => name.to_owned(),
        };
        let mut name = base.clone();
        let mut n = 2;
        while named.iter().any(|(taken, _)| *taken == name) {
            name = format!("{base} {n}");
            n += 1;
        }
        named.push((name, Arc::new(storage)));
    }
    *lock(&session.storages) = named.clone();
    Ok(named)
}

/// Where things sent to `path` should land. A device with a single storage
/// stands for that storage, so files can be dropped on the device itself.
pub(crate) fn landing(path: &Path) -> anyhow::Result<PathBuf> {
    if let Some((device, None)) = split(path)
        && let [(only, _)] = storages(&device, &*session(&device)?)?.as_slice()
    {
        return Ok(path.join(only));
    }
    Ok(path.to_owned())
}

pub(crate) fn place(path: &Path) -> anyhow::Result<Place> {
    let (device, Some(storage_name)) = split(path).context("Not a place on a device.")? else {
        bail!("Open one of the device’s storages first.");
    };
    let session = session(&device)?;
    let find = |list: &[(String, Arc<Storage>)]| {
        list.iter()
            .find(|(name, _)| *name == storage_name)
            .map(|(_, storage)| storage.clone())
    };
    let cached = find(&lock(&session.storages));
    let storage = match cached {
        Some(storage) => storage,
        None => find(&storages(&device, &session)?)
            .ok_or_else(|| anyhow!("“{storage_name}” is no longer available."))?,
    };
    let top = Path::new(ROOT).join(&device).join(&storage_name);
    let handle = handle(&device, &session, &storage, &top, path)?;
    Ok(Place {
        device,
        storage,
        handle,
        path: path.to_owned(),
        session,
    })
}

/// Finds the handle of `path`, listing parent folders that are not known yet.
fn handle(
    device: &str,
    session: &Session,
    storage: &Storage,
    top: &Path,
    path: &Path,
) -> anyhow::Result<Option<ObjectHandle>> {
    if path == top {
        return Ok(None);
    }
    if let Some(handle) = lock(&session.handles).get(path) {
        return Ok(Some(*handle));
    }
    let parent = path.parent().context("Not a place on a device.")?;
    let parent_handle = handle(device, session, storage, top, parent)?;
    list(device, session, storage, parent, parent_handle)?;
    let found = lock(&session.handles).get(path).copied();
    match found {
        Some(handle) => Ok(Some(handle)),
        None => bail!("It is no longer on the device."),
    }
}

/// Lists one folder and remembers the handles of what is in it.
fn list(
    device: &str,
    session: &Session,
    storage: &Storage,
    dir: &Path,
    handle: Option<ObjectHandle>,
) -> anyhow::Result<Vec<ObjectInfo>> {
    let objects = block_on(storage.list_objects(handle)).map_err(|e| fail(device, e))?;
    let mut handles = lock(&session.handles);
    handles.retain(|path, _| path.parent() != Some(dir));
    for object in &objects {
        handles.insert(dir.join(&object.filename), object.handle);
    }
    Ok(objects)
}

impl Place {
    /// What is in this folder, straight from the device.
    pub(crate) fn children(&self) -> anyhow::Result<Vec<ObjectInfo>> {
        list(
            &self.device,
            &self.session,
            &self.storage,
            &self.path,
            self.handle,
        )
    }

    pub(crate) fn fail(&self, error: mtp_rs::Error) -> anyhow::Error {
        fail(&self.device, error)
    }

    /// Records a new object so it can be found without listing again.
    pub(crate) fn remember(&self, name: &str, handle: ObjectHandle) {
        lock(&self.session.handles).insert(self.path.join(name), handle);
    }

    /// Forgets this object and everything under it.
    pub(crate) fn forget(&self) {
        lock(&self.session.handles).retain(|path, _| !path.starts_with(&self.path));
    }

    pub(crate) fn name(&self) -> String {
        name_of(&self.path)
    }
}

pub(crate) fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

// Listing

/// Lists a folder on a device. [`ROOT`] lists the devices, and a device
/// lists its storages.
pub fn list_dir(dir: &Path) -> anyhow::Result<Vec<Entry>> {
    let folder = |name: String, path: PathBuf| Entry {
        hidden: false,
        size: 0,
        modified: None,
        kind: Kind::Folder,
        name,
        path,
    };
    let Some((device, storage)) = split(dir) else {
        return Ok(devices()
            .into_iter()
            .map(|d| folder(d.name, d.path))
            .collect());
    };
    if storage.is_none() {
        let session = session(&device)?;
        let found = storages(&device, &session)?;
        if found.is_empty() {
            bail!(
                "The device is not sharing its files. Unlock it, open the USB notification and choose File Transfer."
            );
        }
        return Ok(found
            .into_iter()
            .map(|(name, _)| folder(name.clone(), dir.join(name)))
            .collect());
    }
    let objects = match place(dir).and_then(|place| place.children()) {
        Ok(objects) => objects,
        Err(_) => {
            // The device may have renumbered its objects; look once more
            // from the top.
            if let Ok(session) = session(&device) {
                lock(&session.handles).clear();
            }
            place(dir)?.children()?
        }
    };
    Ok(objects.iter().map(|object| entry(dir, object)).collect())
}

pub(crate) fn entry(dir: &Path, object: &ObjectInfo) -> Entry {
    let path = dir.join(&object.filename);
    let kind = if object.is_folder() {
        Kind::Folder
    } else {
        path.extension()
            .and_then(|e| e.to_str())
            .map(Kind::from_extension)
            .unwrap_or(Kind::Other)
    };
    Entry {
        hidden: object.filename.starts_with('.'),
        size: if object.is_folder() { 0 } else { object.size },
        modified: object.modified.and_then(system_time),
        name: object.filename.clone(),
        path,
        kind,
    }
}

/// Devices report wall-clock time, usually without a zone. Read it in the
/// Mac's zone, which is right unless the phone is set to another one.
fn system_time(date: mtp_rs::mtp::DateTime) -> Option<SystemTime> {
    use chrono::TimeZone as _;
    let seconds = match date.to_unix_seconds() {
        Some(seconds) => seconds,
        None => {
            let naive = chrono::NaiveDate::from_ymd_opt(
                date.year.into(),
                date.month.into(),
                date.day.into(),
            )?
            .and_hms_opt(date.hour.into(), date.minute.into(), date.second.into())?;
            chrono::Local
                .from_local_datetime(&naive)
                .earliest()?
                .timestamp()
        }
    };
    let seconds = u64::try_from(seconds).ok()?;
    Some(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds))
}

/// The reverse of [`system_time`], for files sent to a device.
pub(crate) fn device_time(time: SystemTime) -> Option<mtp_rs::mtp::DateTime> {
    use chrono::TimeZone as _;
    let seconds = time.duration_since(SystemTime::UNIX_EPOCH).ok()?.as_secs() as i64;
    let local = chrono::Local.timestamp_opt(seconds, 0).single()?;
    let minutes = i16::try_from(local.offset().local_minus_utc() / 60).ok()?;
    let offset = mtp_rs::mtp::UtcOffset::from_minutes(minutes)?;
    // Sent as wall-clock time without a zone, as Android itself writes it.
    let mut date = mtp_rs::mtp::DateTime::from_unix_seconds(seconds, offset)?;
    date.offset = None;
    Some(date)
}

// Changes

/// Creates a folder. Its parent must exist.
pub fn create_dir(path: &Path) -> anyhow::Result<()> {
    let parent = place(path.parent().context("Not a place on a device.")?)?;
    let name = name_of(path);
    let handle =
        block_on(parent.storage.create_folder(parent.handle, &name)).map_err(|e| parent.fail(e))?;
    parent.remember(&name, handle);
    Ok(())
}

/// Renames a file or folder in place.
pub fn rename(path: &Path, name: &str) -> anyhow::Result<()> {
    let target = place(path)?;
    let handle = target.handle.context("A storage can’t be renamed.")?;
    block_on(target.storage.rename(handle, name)).map_err(|e| target.fail(e))?;
    target.forget();
    lock(&target.session.handles).insert(path.with_file_name(name), handle);
    Ok(())
}

/// Deletes files and folders for good; a device has no Trash. Stops at the
/// first failure or when `cancel` is set.
pub fn delete(paths: &[PathBuf], cancel: &AtomicBool) -> anyhow::Result<()> {
    for path in paths {
        let target = place(path)?;
        let handle = target.handle.context("A storage can’t be deleted.")?;
        delete_tree(&target, handle, cancel)
            .with_context(|| format!("“{}” was not deleted", target.name()))?;
    }
    Ok(())
}

pub(crate) fn delete_tree(
    target: &Place,
    handle: ObjectHandle,
    cancel: &AtomicBool,
) -> anyhow::Result<()> {
    if cancel.load(Ordering::Relaxed) {
        bail!("Cancelled.");
    }
    // Most devices delete a folder with everything in it. Those that refuse
    // get it emptied first.
    let first = block_on(target.storage.delete(handle));
    if let Err(error) = first {
        let children = match block_on(target.storage.list_objects(Some(handle))) {
            Ok(children) if !children.is_empty() => children,
            _ => return Err(target.fail(error)),
        };
        for child in children {
            delete_tree(target, child.handle, cancel)?;
        }
        block_on(target.storage.delete(handle)).map_err(|e| target.fail(e))?;
    }
    target.forget();
    Ok(())
}

#[cfg(test)]
pub(crate) mod testing {
    use super::*;
    use mtp_rs::virtual_device::{
        VirtualDeviceConfig, VirtualStorageConfig, register_virtual_device,
        unregister_virtual_device,
    };

    /// A pretend phone whose storage is a temporary folder on disk.
    pub struct Phone {
        pub dir: tempfile::TempDir,
        /// The storage's path, as Seer sees it.
        pub top: PathBuf,
        location: u64,
    }

    impl Phone {
        pub fn new(model: &str) -> Phone {
            let dir = tempfile::tempdir().unwrap();
            let info = register_virtual_device(&VirtualDeviceConfig {
                manufacturer: "Seer".into(),
                model: model.into(),
                serial: format!("serial-{model}"),
                storages: vec![VirtualStorageConfig {
                    description: "Internal storage".into(),
                    capacity: 1 << 30,
                    backing_dir: dir.path().to_owned(),
                    read_only: false,
                }],
                watch_backing_dirs: false,
                ..Default::default()
            });
            let device = devices()
                .into_iter()
                .find(|d| d.name.contains(model))
                .expect("the virtual device is listed");
            Phone {
                dir,
                top: device.path.join("Internal storage"),
                location: info.location_id,
            }
        }
    }

    impl Drop for Phone {
        fn drop(&mut self) {
            unregister_virtual_device(self.location);
            devices();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::Phone;
    use super::*;

    #[test]
    fn browses_a_device_by_path() {
        let phone = Phone::new("Browse");
        std::fs::create_dir_all(phone.dir.path().join("DCIM/Camera")).unwrap();
        std::fs::write(phone.dir.path().join("DCIM/Camera/clip.mp4"), [0u8; 42]).unwrap();

        let root = list_dir(Path::new(ROOT)).unwrap();
        assert!(root.iter().any(|e| e.name.contains("Browse") && e.is_dir()));

        let device = phone.top.parent().unwrap();
        let storages = list_dir(device).unwrap();
        assert_eq!(storages[0].path, phone.top);

        // A deep folder is found without visiting the ones above it first.
        let camera = list_dir(&phone.top.join("DCIM/Camera")).unwrap();
        assert_eq!(camera.len(), 1);
        assert_eq!(camera[0].name, "clip.mp4");
        assert_eq!(camera[0].size, 42);
        assert_eq!(camera[0].kind, Kind::Video);
        assert!(camera[0].modified.is_some());

        assert!(list_dir(&phone.top.join("Missing")).is_err());
    }

    #[test]
    fn creates_renames_and_deletes() {
        let phone = Phone::new("Edit");
        let cancel = AtomicBool::new(false);

        create_dir(&phone.top.join("Albums")).unwrap();
        assert!(phone.dir.path().join("Albums").is_dir());

        rename(&phone.top.join("Albums"), "Music").unwrap();
        assert!(phone.dir.path().join("Music").is_dir());
        assert!(!phone.dir.path().join("Albums").exists());

        std::fs::write(phone.dir.path().join("Music/a.mp3"), b"a").unwrap();
        delete(&[phone.top.join("Music")], &cancel).unwrap();
        assert!(!phone.dir.path().join("Music").exists());
        assert!(list_dir(&phone.top).unwrap().is_empty());
    }

    #[test]
    fn device_times_round_trip() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_790_000_000);
        assert_eq!(device_time(now).and_then(system_time), Some(now));
    }
}
