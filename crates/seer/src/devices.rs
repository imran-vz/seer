//! Connected Android devices, the items waiting to be pasted, and the copies
//! and moves in progress. Views observe this entity and repaint.

use crate::platform;
use crate::theme::{self, Theme, motion, size, text};
use gpui_kit::prelude::*;
use gpui_kit::{App, Context, Entity, EventEmitter, Global, SharedString, Task, Window, div};
use seer_core::format;
use seer_core::mtp::{self, Device};
use seer_core::transfer::{self, Mode, Outcome, Progress};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub enum DevicesEvent {
    /// A device was plugged in or unplugged.
    Changed,
    /// A copy or move ended. How it went is in [`Devices::finished`].
    Finished,
}

/// Rows being dragged from the file list, on their way to a folder.
#[derive(Clone)]
pub struct Dragged(pub Rc<Vec<PathBuf>>);

impl Dragged {
    /// What dropping these on `dest` does, as in Finder: a move within one
    /// disk or one device, a copy when they cross to another.
    pub fn mode(&self, dest: &Path) -> Mode {
        use std::os::unix::fs::MetadataExt as _;
        let Some(first) = self.0.first() else {
            return Mode::Copy;
        };
        let same_place = match (mtp::is_device_path(first), mtp::is_device_path(dest)) {
            (true, true) => first.components().nth(2) == dest.components().nth(2),
            (false, false) => {
                let disk = |path: &Path| std::fs::metadata(path).map(|m| m.dev()).ok();
                disk(first).is_some() && disk(first) == disk(dest)
            }
            _ => false,
        };
        match same_place {
            true => Mode::Move,
            false => Mode::Copy,
        }
    }
}

/// The label that follows the pointer during a drag.
impl Render for Dragged {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::of(cx);
        let label = match self.0.as_slice() {
            [only] => name(only),
            many => format!("{} items", many.len()),
        };
        div()
            .flex()
            .items_center()
            .h(size::CONTROL)
            .px(size::ROW_INSET)
            .rounded(size::RADIUS_CONTROL)
            .bg(t.selection)
            .text_color(t.on_selection)
            .font_family(theme::font(cx))
            .text_size(text::SMALL)
            .child(label)
    }
}

pub struct Transfer {
    pub id: usize,
    /// "Copying 3 items to Download". Never a file name, so it stays short.
    pub label: SharedString,
    /// "Copying 3 items": the label without its destination, for the
    /// status bar.
    pub action: SharedString,
    /// The folder the items are going into.
    pub dest: PathBuf,
    /// Copying or moving, so Stop can say which it stops.
    pub mode: Mode,
    pub progress: Arc<Progress>,
    _task: Task<()>,
}

impl Transfer {
    /// What the folder it goes into says while it runs: "Copying 4 items
    /// here…".
    pub fn arriving(&self) -> String {
        format!("{} here…", self.action)
    }
}

/// How a transfer ended.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ending {
    /// Everything arrived.
    Done,
    /// Stopped before the end; nothing failed.
    Stopped,
    /// At least one item did not arrive.
    Failed,
}

/// A transfer that has ended, kept so its outcome can be read: briefly
/// when it went well, until it is dismissed when it was stopped or
/// something failed.
pub struct Finished {
    pub id: usize,
    pub ending: Ending,
    /// What happened, in a sentence: "Copied 4 items to Archive".
    pub summary: SharedString,
    /// Every item and how far it got.
    pub progress: Arc<Progress>,
    /// When it ended.
    pub at: Instant,
    /// Put away with Dismiss in the activity popover.
    pub dismissed: bool,
}

impl Finished {
    /// Whether the status bar still shows it. Success says so briefly; a
    /// stop or a failure stays until it is dismissed, so someone who looked
    /// away during a long copy still finds out.
    fn current(&self) -> bool {
        match self.ending {
            _ if self.dismissed => false,
            Ending::Done => self.at.elapsed() < motion::NOTICE,
            Ending::Stopped | Ending::Failed => true,
        }
    }

    /// Whether it waits for Dismiss rather than going by itself.
    pub fn needs_dismissing(&self) -> bool {
        self.ending != Ending::Done
    }

    /// A stop or a failure nobody has dismissed yet.
    fn unresolved(&self) -> bool {
        self.needs_dismissing() && !self.dismissed
    }
}

/// Stops and failures not yet dismissed, beyond the one the status bar is
/// already showing: the cue that keeps them in sight behind a newer
/// transfer or a newer success.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Unresolved {
    pub failed: usize,
    pub stopped: usize,
}

impl Unresolved {
    pub fn any(self) -> bool {
        self.failed + self.stopped > 0
    }

    /// "1 failed", "2 stopped", "1 failed, 1 stopped".
    pub fn label(self) -> String {
        let parts: Vec<String> = [(self.failed, "failed"), (self.stopped, "stopped")]
            .into_iter()
            .filter(|(n, _)| *n > 0)
            .map(|(n, what)| format!("{n} {what}"))
            .collect();
        parts.join(", ")
    }
}

pub struct Devices {
    devices: Vec<Device>,
    /// What Copy last picked up.
    clipboard: Vec<PathBuf>,
    transfers: Vec<Transfer>,
    /// Ended transfers whose outcome is still shown, oldest first. The
    /// latest one is kept even after its moment has passed, as the receipt
    /// Show Transfers reopens.
    finished: Vec<Finished>,
    /// The activity popover is open: nothing in it is taken away meanwhile.
    details_open: bool,
    next_id: usize,
    /// Watchers are being repainted a few times a second while transfers run.
    ticking: bool,
    _watch: Task<()>,
}

struct GlobalDevices(Entity<Devices>);
impl Global for GlobalDevices {}

impl EventEmitter<DevicesEvent> for Devices {}

impl Devices {
    pub fn init(cx: &mut App) {
        let devices = cx.new(|cx| {
            let (tx, rx) = smol::channel::unbounded();
            // USB arrival and removal are reported on a thread of their own
            // for as long as the app runs.
            std::thread::spawn(move || {
                let watched = mtp::watch(|devices| {
                    tx.send_blocking(devices).ok();
                });
                if let Err(e) = watched {
                    log::warn!("devices are not being watched: {e:#}");
                }
            });
            let watch = cx.spawn(async move |this, cx| {
                while let Ok(devices) = rx.recv().await {
                    let alive = this.update(cx, |this: &mut Devices, cx| {
                        this.devices = devices;
                        cx.emit(DevicesEvent::Changed);
                        cx.notify();
                    });
                    if alive.is_err() {
                        break;
                    }
                }
            });
            Devices {
                devices: Vec::new(),
                clipboard: Vec::new(),
                transfers: Vec::new(),
                finished: Vec::new(),
                details_open: false,
                next_id: 0,
                ticking: false,
                _watch: watch,
            }
        });
        cx.set_global(GlobalDevices(devices));
    }

    pub fn global(cx: &App) -> Entity<Devices> {
        cx.global::<GlobalDevices>().0.clone()
    }

    pub fn connected(&self) -> &[Device] {
        &self.devices
    }

    pub fn copy(&mut self, paths: Vec<PathBuf>) {
        self.clipboard = paths;
    }

    pub fn clipboard(&self) -> &[PathBuf] {
        &self.clipboard
    }

    pub fn transfers(&self) -> &[Transfer] {
        &self.transfers
    }

    /// The transfer running into `dir`, if one is: the folder is about to
    /// have what it lacks.
    pub fn incoming(&self, dir: &Path) -> Option<&Transfer> {
        self.transfers.iter().find(|t| t.dest == dir)
    }

    /// Ended transfers whose outcome is still shown, oldest first. While
    /// the activity popover is open, everything that was in it stays,
    /// except what was dismissed there, and so does the last receipt.
    pub fn finished(&self) -> impl Iterator<Item = &Finished> {
        let open = self.details_open;
        self.finished
            .iter()
            .filter(move |f| !f.dismissed && (open || f.current()))
    }

    /// Stops and failures still waiting for Dismiss, apart from the latest
    /// thing `finished` shows, which the status bar names itself.
    pub fn unresolved(&self) -> Unresolved {
        let shown = match self.transfers.is_empty() {
            true => self.finished().last().map(|f| f.id),
            false => None,
        };
        self.finished
            .iter()
            .filter(|f| f.unresolved() && Some(f.id) != shown)
            .fold(Unresolved::default(), |mut sum, f| {
                match f.ending {
                    Ending::Failed => sum.failed += 1,
                    _ => sum.stopped += 1,
                }
                sum
            })
    }

    /// The last ended transfer, kept after its notice has gone until it is
    /// dismissed: what the status bar's quiet "Last Transfer" entry opens
    /// when nothing newer is showing.
    pub fn receipt(&self) -> Option<&Finished> {
        self.finished.last().filter(|f| !f.dismissed)
    }

    /// Whether the activity popover is open. The status bar's popover
    /// follows this, so a command can open it.
    pub fn details_open(&self) -> bool {
        self.details_open
    }

    /// Whether there is anything for the activity popover to show: a
    /// transfer running, or an outcome nobody has dismissed, down to the
    /// last receipt after its notice has gone. View › Show Transfers is
    /// available exactly when this is true.
    pub fn has_details(&self) -> bool {
        !self.transfers.is_empty() || self.finished.iter().any(|f| !f.dismissed)
    }

    /// Opens the activity popover from a command (View › Show Transfers):
    /// what is running, then what ended, down to the last receipt. Returns
    /// false, and opens nothing, when [`Devices::has_details`] is false.
    pub fn show_details(&mut self, cx: &mut Context<Self>) -> bool {
        let any = self.has_details();
        if any {
            self.details_open = true;
            cx.notify();
        }
        any
    }

    /// The activity popover opened or closed. Closing it lets go of
    /// successes whose moment has passed, all but the last receipt; stops
    /// and failures stay until they are dismissed.
    pub fn set_details_open(&mut self, open: bool, cx: &mut Context<Self>) {
        self.details_open = open;
        if !open {
            self.prune();
        }
        cx.notify();
    }

    /// Lets go of outcomes no longer shown, keeping the latest as the
    /// receipt Show Transfers reopens.
    fn prune(&mut self) {
        let last = self.finished.last().map(|f| f.id);
        self.finished
            .retain(|f| f.current() || (Some(f.id) == last && !f.dismissed));
    }

    /// Puts away the outcome of an ended transfer.
    pub fn dismiss(&mut self, id: usize, cx: &mut Context<Self>) {
        if let Some(finished) = self.finished.iter_mut().find(|f| f.id == id) {
            finished.dismissed = true;
        }
        // With nothing left to show, the trigger and its popover go, and
        // the popover can no longer say it closed.
        if self.transfers.is_empty() && self.finished().next().is_none() {
            self.details_open = false;
        }
        if !self.details_open {
            self.prune();
        }
        cx.notify();
    }

    /// Copies or moves `sources` into `dest` in the background.
    pub fn transfer(
        &mut self,
        sources: Vec<PathBuf>,
        dest: PathBuf,
        mode: Mode,
        cx: &mut Context<Self>,
    ) {
        if sources.is_empty() {
            return;
        }
        let id = self.next_id;
        self.next_id += 1;
        let progress = Arc::new(Progress::default());
        let count = sources.len();
        let place = name(&dest);
        // The file names are in the popover; this stays short.
        let action = format!(
            "{} {}",
            match mode {
                Mode::Copy => "Copying",
                Mode::Move => "Moving",
            },
            format::count(count, "item"),
        );
        let label = format!("{action} to {place}");
        let dest_path = dest.clone();
        let task = cx.spawn({
            let progress = progress.clone();
            async move |this, cx| {
                let run = progress.clone();
                let outcome = cx
                    .background_spawn(async move { transfer::run(&sources, &dest, mode, &run) })
                    .await;
                this.update(cx, |this, cx| {
                    this.transfers.retain(|t| t.id != id);
                    if mode == Mode::Move && outcome.failed.is_empty() && !outcome.cancelled {
                        // Those items are no longer where they were copied from.
                        this.clipboard.clear();
                    }
                    this.finish(id, &outcome, mode, count, &place, progress, cx);
                    cx.emit(DevicesEvent::Finished);
                    cx.notify();
                })
                .ok();
            }
        });
        self.transfers.push(Transfer {
            id,
            label: label.into(),
            action: action.into(),
            dest: dest_path,
            mode,
            progress,
            _task: task,
        });
        self.tick(cx);
        cx.notify();
    }

    /// Stops a transfer. It ends within a moment, once the device lets go.
    pub fn cancel(&mut self, id: usize, cx: &mut Context<Self>) {
        if let Some(transfer) = self.transfers.iter().find(|t| t.id == id) {
            transfer.progress.cancel();
            cx.notify();
        }
    }

    /// Keeps the outcome of a transfer that just ended, and lets it go
    /// once it has been shown for long enough.
    #[allow(clippy::too_many_arguments)]
    fn finish(
        &mut self,
        id: usize,
        outcome: &Outcome,
        mode: Mode,
        count: usize,
        place: &str,
        progress: Arc<Progress>,
        cx: &mut Context<Self>,
    ) {
        let (past, verb) = match mode {
            Mode::Copy => ("Copied", "copied"),
            Mode::Move => ("Moved", "moved"),
        };
        let (ending, summary) = if !outcome.failed.is_empty() {
            let failed = outcome.failed.len();
            let summary = match (count, outcome.failed.first()) {
                (1, Some((name, _))) => format!("“{name}” not {verb}"),
                _ if failed == count => {
                    format!("{} not {verb} to {place}", format::count(failed, "item"))
                }
                _ => format!("{failed} of {count} items not {verb} to {place}"),
            };
            (Ending::Failed, summary)
        } else if outcome.cancelled {
            let summary = match outcome.done {
                0 => format!("Stopped, nothing {verb}"),
                done => format!("Stopped after {done} of {}", format::count(count, "item")),
            };
            (Ending::Stopped, summary)
        } else {
            (
                Ending::Done,
                format!("{past} {} to {place}", format::count(count, "item")),
            )
        };
        if !self.details_open {
            self.prune();
        }
        self.finished.push(Finished {
            id,
            ending,
            summary: summary.into(),
            progress,
            at: Instant::now(),
            dismissed: false,
        });
        if ending == Ending::Done {
            // Let the notice go once it has been read.
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(motion::NOTICE).await;
                this.update(cx, |this, cx| {
                    if !this.details_open {
                        this.prune();
                    }
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
    }

    /// Repaints whatever shows transfers a few times a second while any
    /// run, rather than on every frame.
    fn tick(&mut self, cx: &mut Context<Self>) {
        if std::mem::replace(&mut self.ticking, true) {
            return;
        }
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                let running = this.update(cx, |this, cx| {
                    cx.notify();
                    this.ticking = !this.transfers.is_empty();
                    this.ticking
                });
                if !matches!(running, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
    }
}

/// How often running transfers are redrawn.
const TICK: Duration = Duration::from_millis(100);

/// What a folder is called; the root of the disk has a name of its own.
fn name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| platform::root_name().into())
}

#[cfg(test)]
mod tests {
    use super::Unresolved;

    #[test]
    fn unresolved_transfers_are_counted_by_how_they_ended() {
        let cue = |failed, stopped| Unresolved { failed, stopped };
        assert!(!cue(0, 0).any());
        assert_eq!(cue(1, 0).label(), "1 failed");
        assert_eq!(cue(0, 2).label(), "2 stopped");
        assert_eq!(cue(1, 1).label(), "1 failed, 1 stopped");
    }
}
