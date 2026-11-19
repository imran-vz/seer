//! What Seer knows about the media files it has seen. Probing and analysis
//! run on background threads; views observe this entity and repaint.

use gpui_kit::{App, AppContext as _, Context, Entity, Global, SharedString, Task};
use seer_core::bitrate::{self, Analysis};
use seer_core::cache::Cache;
use seer_core::fingerprint::fingerprint;
use seer_core::fs::Entry;
use seer_core::probe::{self, MediaInfo};
use seer_core::remux;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::SystemTime;

const PROBE_KIND: &str = "probe";
const BITRATE_KIND: &str = "bitrate";

#[derive(Clone)]
pub enum Probe {
    Pending,
    Ready(Arc<MediaInfo>),
    /// Not something FFmpeg can read.
    Unreadable,
}

#[derive(Clone)]
pub enum Bitrate {
    Idle,
    Running(Arc<Progress>),
    Ready(Arc<Analysis>),
    Failed(SharedString),
}

/// Removal of streams from a file.
#[derive(Clone)]
pub enum Prune {
    Running(Arc<Progress>),
    Failed(SharedString),
}

pub struct Progress {
    thousandths: AtomicU32,
    cancel: AtomicBool,
}

impl Progress {
    fn new() -> Arc<Progress> {
        Arc::new(Progress {
            thousandths: AtomicU32::new(0),
            cancel: AtomicBool::new(false),
        })
    }

    fn set(&self, fraction: f32) {
        self.thousandths
            .store((fraction * 1000.) as u32, Ordering::Relaxed);
    }

    pub fn fraction(&self) -> f32 {
        self.thousandths.load(Ordering::Relaxed) as f32 / 1000.
    }
}

/// Identifies one state of a file without reading it.
type Stamp = (u64, Option<SystemTime>);

pub struct Library {
    cache: Option<Arc<Cache>>,
    probes: HashMap<PathBuf, (Stamp, Probe)>,
    bitrates: HashMap<PathBuf, Bitrate>,
    prunes: HashMap<PathBuf, Prune>,
    scan: Option<Task<()>>,
    analyses: HashMap<PathBuf, Task<()>>,
    removals: HashMap<PathBuf, Task<()>>,
}

struct GlobalLibrary(Entity<Library>);
impl Global for GlobalLibrary {}

impl Library {
    pub fn init(cx: &mut App) {
        let cache = dirs::data_dir()
            .map(|dir| dir.join("Seer").join("cache.sqlite"))
            .and_then(|file| {
                Cache::open(&file)
                    .inspect_err(|e| log::warn!("cache: {e:#}"))
                    .ok()
            })
            .map(Arc::new);
        let library = cx.new(|_| Library {
            cache,
            probes: HashMap::new(),
            bitrates: HashMap::new(),
            prunes: HashMap::new(),
            scan: None,
            analyses: HashMap::new(),
            removals: HashMap::new(),
        });
        cx.set_global(GlobalLibrary(library));
    }

    pub fn global(cx: &App) -> Entity<Library> {
        cx.global::<GlobalLibrary>().0.clone()
    }

    pub fn cache(&self) -> Option<Arc<Cache>> {
        self.cache.clone()
    }

    pub fn probe(&self, path: &Path) -> Option<&Probe> {
        self.probes.get(path).map(|(_, probe)| probe)
    }

    pub fn info(&self, path: &Path) -> Option<&Arc<MediaInfo>> {
        match self.probe(path)? {
            Probe::Ready(info) => Some(info),
            _ => None,
        }
    }

    /// Probes the media files among `entries` that are new or have changed.
    /// Replaces any scan already running.
    pub fn scan(&mut self, entries: &[Entry], cx: &mut Context<Self>) {
        let mut wanted = Vec::new();
        for entry in entries.iter().filter(|e| e.kind.is_probeable()) {
            let stamp = (entry.size, entry.modified);
            match self.probes.get(&entry.path) {
                Some((known, Probe::Ready(_) | Probe::Unreadable)) if *known == stamp => {}
                _ => {
                    self.probes
                        .insert(entry.path.clone(), (stamp, Probe::Pending));
                    wanted.push((entry.path.clone(), stamp));
                }
            }
        }
        if wanted.is_empty() {
            self.scan = None;
            cx.notify();
            return;
        }

        let cache = self.cache.clone();
        self.scan = Some(cx.spawn(async move |this, cx| {
            // Small batches keep the list filling in steadily.
            for batch in wanted.chunks(24) {
                let (batch, cache) = (batch.to_vec(), cache.clone());
                let results = cx
                    .background_spawn(async move { probe_batch(batch, cache) })
                    .await;
                let alive = this.update(cx, |this, cx| {
                    for (path, stamp, probe) in results {
                        this.probes.insert(path, (stamp, probe));
                    }
                    cx.notify();
                });
                if alive.is_err() {
                    return;
                }
            }
            this.update(cx, |this, cx| {
                this.scan = None;
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    pub fn bitrate(&self, path: &Path) -> Bitrate {
        self.bitrates.get(path).cloned().unwrap_or(Bitrate::Idle)
    }

    /// Shows an earlier analysis of this file if one is cached.
    pub fn restore_bitrate(&mut self, path: &Path, cx: &mut Context<Self>) {
        let Some(cache) = self.cache.clone() else {
            return;
        };
        if self.bitrates.contains_key(path) {
            return;
        }
        let path = path.to_owned();
        cx.spawn(async move |this, cx| {
            let lookup = path.clone();
            let found = cx
                .background_spawn(async move {
                    let fp = fingerprint(&lookup).ok()?;
                    cache.get::<Analysis>(&lookup, BITRATE_KIND, &fp)
                })
                .await;
            if let Some(analysis) = found {
                this.update(cx, |this, cx| {
                    this.bitrates
                        .entry(path)
                        .or_insert(Bitrate::Ready(Arc::new(analysis)));
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    pub fn analyze(&mut self, path: &Path, cx: &mut Context<Self>) {
        if matches!(self.bitrate(path), Bitrate::Running(_)) {
            return;
        }
        let progress = Progress::new();
        self.bitrates
            .insert(path.to_owned(), Bitrate::Running(progress.clone()));

        let (cache, path) = (self.cache.clone(), path.to_owned());
        let task = cx.spawn({
            let path = path.clone();
            async move |this, cx| {
                let work = path.clone();
                let result = cx
                    .background_spawn(async move {
                        let fp = fingerprint(&work).ok();
                        let analysis = bitrate::analyze(&work, &progress.cancel, |fraction| {
                            progress.set(fraction)
                        })?;
                        if let (Some(cache), Some(fp)) = (cache, fp) {
                            cache.put(&work, BITRATE_KIND, &fp, &analysis).ok();
                        }
                        anyhow::Ok(analysis)
                    })
                    .await;
                this.update(cx, |this, cx| {
                    let state = match result {
                        Ok(analysis) => Bitrate::Ready(Arc::new(analysis)),
                        Err(e) if e.is::<bitrate::Cancelled>() => Bitrate::Idle,
                        Err(e) => Bitrate::Failed(format!("{e:#}").into()),
                    };
                    this.bitrates.insert(path.clone(), state);
                    this.analyses.remove(&path);
                    cx.notify();
                })
                .ok();
            }
        });
        self.analyses.insert(path, task);
        cx.notify();
    }

    pub fn cancel_analysis(&mut self, path: &Path, cx: &mut Context<Self>) {
        if let Bitrate::Running(progress) = self.bitrate(path) {
            // The worker notices the flag within a few hundred packets and
            // the task then resets the state to idle.
            progress.cancel.store(true, Ordering::Relaxed);
            cx.notify();
        }
    }

    pub fn prune(&self, path: &Path) -> Option<Prune> {
        self.prunes.get(path).cloned()
    }

    /// Rewrites the file without the given streams. The original goes to the
    /// Trash once the new file is complete.
    pub fn remove_streams(&mut self, path: &Path, remove: Vec<usize>, cx: &mut Context<Self>) {
        self.rewrite(
            path,
            remux::Edits {
                remove,
                ..Default::default()
            },
            cx,
        );
    }

    /// Rewrites the file with `edits` applied, without re-encoding.
    pub fn rewrite(&mut self, path: &Path, edits: remux::Edits, cx: &mut Context<Self>) {
        if matches!(self.prune(path), Some(Prune::Running(_))) || edits.is_empty() {
            return;
        }
        let progress = Progress::new();
        self.prunes
            .insert(path.to_owned(), Prune::Running(progress.clone()));

        let path = path.to_owned();
        let task = cx.spawn({
            let path = path.clone();
            async move |this, cx| {
                let work = path.clone();
                let result = cx
                    .background_spawn(async move {
                        remux::rewrite(&work, &edits, &progress.cancel, |fraction| {
                            progress.set(fraction)
                        })
                    })
                    .await;
                this.update(cx, |this, cx| {
                    this.removals.remove(&path);
                    match result {
                        Ok(()) => {
                            // The file is new: forget what was known about the old one.
                            this.prunes.remove(&path);
                            this.bitrates.remove(&path);
                            this.probes.remove(&path);
                        }
                        Err(e) if e.is::<bitrate::Cancelled>() => {
                            this.prunes.remove(&path);
                        }
                        Err(e) => {
                            this.prunes
                                .insert(path.clone(), Prune::Failed(format!("{e:#}").into()));
                        }
                    }
                    cx.notify();
                })
                .ok();
            }
        });
        self.removals.insert(path, task);
        cx.notify();
    }

    pub fn cancel_prune(&mut self, path: &Path, cx: &mut Context<Self>) {
        if let Some(Prune::Running(progress)) = self.prune(path) {
            progress.cancel.store(true, Ordering::Relaxed);
            cx.notify();
        }
    }

    /// Number of analyses and rewrites in progress.
    pub fn running_jobs(&self) -> usize {
        self.analyses.len() + self.removals.len()
    }
}

fn probe_batch(
    batch: Vec<(PathBuf, Stamp)>,
    cache: Option<Arc<Cache>>,
) -> Vec<(PathBuf, Stamp, Probe)> {
    let workers = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .min(8);
    let per_worker = batch.len().div_ceil(workers).max(1);
    let cache = cache.as_deref();
    std::thread::scope(|scope| {
        let handles: Vec<_> = batch
            .chunks(per_worker)
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .iter()
                        .map(|(path, stamp)| (path.clone(), *stamp, probe_one(path, cache)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap_or_default())
            .collect()
    })
}

/// Probes one file, through the cache. Blocks; call it off the main thread.
pub fn probe_one(path: &Path, cache: Option<&Cache>) -> Probe {
    let fp = fingerprint(path).ok();
    if let (Some(cache), Some(fp)) = (cache, &fp)
        && let Some(info) = cache.get::<MediaInfo>(path, PROBE_KIND, fp)
    {
        return Probe::Ready(Arc::new(info));
    }
    match probe::probe(path) {
        Ok(info) => {
            if let (Some(cache), Some(fp)) = (cache, &fp) {
                cache.put(path, PROBE_KIND, fp, &info).ok();
            }
            Probe::Ready(Arc::new(info))
        }
        Err(_) => Probe::Unreadable,
    }
}
