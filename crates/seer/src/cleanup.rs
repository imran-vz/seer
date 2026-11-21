//! Folder-wide review of stream names: every video under a folder, at any
//! depth, with the names that carry a web address or don't describe their
//! stream, and a way to put them right in one go. Shown in a sheet over the
//! window (`ui::Sheet`).

use crate::browser::folder_name;
use crate::library::{Library, Probe, probe_one};
use crate::platform::{self, Command};
use crate::theme::{self, Theme, size, text};
use crate::ui::{self, Tip as _};
use gpui_kit::prelude::*;
use gpui_kit::*;
use seer_core::bitrate::Cancelled;
use seer_core::format;
use seer_core::fs::{Entry, Kind};
use seer_core::probe::MediaInfo;
use seer_core::remux;
use seer_core::titles::{self, Fix, Issue, Target, Unread};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

pub enum CleanupEvent {
    /// The person is finished with the review.
    Done,
}

/// Whether Clean Up can find anything under a folder holding `items`: a
/// video, or a visible folder that may hold one, since the review looks at
/// every depth. The one rule behind the toolbar button, the command, the
/// folder's context menu and the Main Menu; re-evaluate it on every
/// listing. Reads nothing from disk.
pub fn worth_reviewing<'a>(items: impl IntoIterator<Item = &'a Entry>) -> bool {
    items
        .into_iter()
        .any(|item| !item.hidden && (item.is_dir() || item.kind == Kind::Video))
}

enum FileState {
    Waiting,
    Rewriting,
    /// Rewritten with `fix`; only the issues in the set were written.
    Fixed(Fix, HashSet<usize>),
    Failed(SharedString),
    /// The scan could not read the file, so its names were not checked.
    Unreadable,
}

struct Flagged {
    path: PathBuf,
    /// The file's name.
    name: String,
    /// The folder it is in, below the folder being reviewed; `None` when it
    /// is in that folder itself.
    folder: Option<String>,
    issues: Vec<Issue>,
    /// Which stream each issue is about, told apart from its siblings:
    /// "Audio 2 · AAC · Tamil". One per issue.
    streams: Vec<SharedString>,
    state: FileState,
}

impl Flagged {
    fn fixed(&self) -> bool {
        matches!(self.state, FileState::Fixed(..))
    }
}

#[derive(Clone, Copy)]
enum Row {
    File(usize),
    Issue(usize, usize),
    /// A folder or item the scan could not look into.
    Unread(usize),
}

pub struct Cleanup {
    root: PathBuf,
    /// Files with names to fix, then files that could not be read.
    files: Vec<Flagged>,
    /// Folders (and items) the scan could not look into, so the videos in
    /// them were never checked. Listed after the files.
    unread: Vec<Unread>,
    rows: Vec<Row>,
    /// Issues the person has unticked, as (file, issue).
    skipped: HashSet<(usize, usize)>,
    fix: Fix,
    /// Videos looked at so far, read or not.
    checked: usize,
    total: Option<usize>,
    /// Some of the videos are in subfolders, so the counts are more than
    /// the folder's own list shows.
    deep: bool,
    scanning: bool,
    /// Files done and files to do, while a fix is running.
    applying: Option<(usize, usize)>,
    /// Progress through the file being rewritten, in thousandths.
    progress: Arc<AtomicU32>,
    /// Ends the scan.
    cancel: Arc<AtomicBool>,
    /// Ends the rewrite after the file in progress is put back as it was.
    stop: Arc<AtomicBool>,
    scan: Option<Task<()>>,
    rewrite: Option<Task<()>>,
    /// Keys go to the sheet while it is open: Escape closes, Return confirms.
    focus: FocusHandle,
}

impl EventEmitter<CleanupEvent> for Cleanup {}

impl Focusable for Cleanup {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Cleanup {
    pub fn new(root: PathBuf, cx: &mut Context<Self>) -> Self {
        let mut this = Cleanup {
            root,
            files: Vec::new(),
            unread: Vec::new(),
            rows: Vec::new(),
            skipped: HashSet::new(),
            fix: Fix::Standard,
            checked: 0,
            total: None,
            deep: false,
            scanning: true,
            applying: None,
            progress: Arc::new(AtomicU32::new(0)),
            cancel: Arc::new(AtomicBool::new(false)),
            stop: Arc::new(AtomicBool::new(false)),
            scan: None,
            rewrite: None,
            focus: cx.focus_handle(),
        };
        this.start_scan(cx);
        this
    }

    fn start_scan(&mut self, cx: &mut Context<Self>) {
        let (root, cancel) = (self.root.clone(), self.cancel.clone());
        let cache = Library::global(cx).read(cx).cache();
        self.scan = Some(cx.spawn(async move |this, cx| {
            let walk = (root.clone(), cancel.clone());
            let found = cx
                .background_spawn(async move { titles::video_files(&walk.0, &walk.1) })
                .await;
            let paths = found.files;
            if this
                .update(cx, |this, cx| {
                    this.total = Some(paths.len());
                    this.deep = paths.iter().any(|path| path.parent() != Some(&root))
                        || found.unread.iter().any(|place| place.path != root);
                    this.unread = found.unread;
                    this.rebuild_rows();
                    cx.notify();
                })
                .is_err()
            {
                return;
            }

            for batch in paths.chunks(8) {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                let (batch, cache, root) = (batch.to_vec(), cache.clone(), root.clone());
                let count = batch.len();
                let found = cx
                    .background_spawn(async move {
                        batch
                            .into_iter()
                            .filter_map(|path| {
                                let (name, folder) = place(&path, &root);
                                let Probe::Ready(info) = probe_one(&path, cache.as_deref()) else {
                                    // Counted and named, never passed off as tidy.
                                    return Some(Flagged {
                                        path,
                                        name,
                                        folder,
                                        issues: Vec::new(),
                                        streams: Vec::new(),
                                        state: FileState::Unreadable,
                                    });
                                };
                                let issues = titles::review(&info);
                                let streams = issues
                                    .iter()
                                    .map(|issue| stream_identity(issue, &info).into())
                                    .collect();
                                (!issues.is_empty()).then_some(Flagged {
                                    path,
                                    name,
                                    folder,
                                    issues,
                                    streams,
                                    state: FileState::Waiting,
                                })
                            })
                            .collect::<Vec<_>>()
                    })
                    .await;
                let alive = this.update(cx, |this, cx| {
                    this.checked += count;
                    this.add(found);
                    cx.notify();
                });
                if alive.is_err() {
                    return;
                }
            }
            this.update(cx, |this, cx| {
                this.scanning = false;
                cx.notify();
            })
            .ok();
        }));
    }

    /// Takes in a batch of scan results. Files with names to fix go before
    /// the unreadable ones; indexes of files already listed never change,
    /// because ticks and a running fix refer to them.
    fn add(&mut self, found: Vec<Flagged>) {
        for file in found {
            match file.state {
                FileState::Unreadable => self.files.push(file),
                _ => {
                    let at = self
                        .files
                        .iter()
                        .position(|f| matches!(f.state, FileState::Unreadable))
                        .unwrap_or(self.files.len());
                    self.files.insert(at, file);
                }
            }
        }
        self.rebuild_rows();
    }

    /// Every file and its names, then the places the scan could not read.
    fn rebuild_rows(&mut self) {
        self.rows = self
            .files
            .iter()
            .enumerate()
            .flat_map(|(f, file)| {
                std::iter::once(Row::File(f))
                    .chain((0..file.issues.len()).map(move |i| Row::Issue(f, i)))
            })
            .chain((0..self.unread.len()).map(Row::Unread))
            .collect();
    }

    /// Issues still ticked in files that have not been fixed yet.
    fn chosen(&self) -> Vec<(usize, Vec<usize>)> {
        self.files
            .iter()
            .enumerate()
            .filter(|(_, file)| !file.fixed())
            .filter_map(|(f, file)| {
                let issues: Vec<usize> = (0..file.issues.len())
                    .filter(|i| !self.skipped.contains(&(f, *i)))
                    .collect();
                (!issues.is_empty()).then_some((f, issues))
            })
            .collect()
    }

    fn any_fixed(&self) -> bool {
        self.files.iter().any(Flagged::fixed)
    }

    /// Some video was found but could not be read.
    fn has_unreadable(&self) -> bool {
        self.files
            .iter()
            .any(|file| matches!(file.state, FileState::Unreadable))
    }

    /// Rewrites each chosen file in turn. One at a time: each is a full copy
    /// of the file, and disks do not thank you for doing several at once.
    /// Files found while this runs wait for the next round.
    fn apply(&mut self, cx: &mut Context<Self>) {
        let chosen = self.chosen();
        // The review is approved whole: not while rows can still arrive.
        if chosen.is_empty() || self.applying.is_some() || self.scanning {
            return;
        }
        let fix = self.fix;
        let jobs: Vec<(usize, HashSet<usize>, PathBuf, remux::Edits)> = chosen
            .into_iter()
            .map(|(f, issues)| {
                let file = &self.files[f];
                let edits = titles::edits(issues.iter().map(|i| &file.issues[*i]), fix);
                (f, issues.into_iter().collect(), file.path.clone(), edits)
            })
            .collect();
        self.stop.store(false, Ordering::Relaxed);
        self.applying = Some((0, jobs.len()));
        let (stop, progress) = (self.stop.clone(), self.progress.clone());

        self.rewrite = Some(cx.spawn(async move |this, cx| {
            for (done, (index, written, path, edits)) in jobs.into_iter().enumerate() {
                progress.store(0, Ordering::Relaxed);
                let alive = this.update(cx, |this, cx| {
                    this.files[index].state = FileState::Rewriting;
                    this.applying = this.applying.map(|(_, total)| (done, total));
                    cx.notify();
                });
                if alive.is_err() {
                    return;
                }
                let (stop_flag, meter) = (stop.clone(), progress.clone());
                let result = cx
                    .background_spawn(async move {
                        remux::rewrite(&path, &edits, &stop_flag, |fraction| {
                            meter.store((fraction * 1000.) as u32, Ordering::Relaxed)
                        })
                    })
                    .await;
                let stopped = matches!(&result, Err(e) if e.is::<Cancelled>());
                this.update(cx, |this, cx| {
                    this.files[index].state = match result {
                        // Only what was ticked was written; the rest stays as it was.
                        Ok(()) => FileState::Fixed(fix, written),
                        Err(_) if stopped => FileState::Waiting,
                        Err(e) => FileState::Failed(format!("{e:#}").into()),
                    };
                    cx.notify();
                })
                .ok();
                if stopped {
                    break;
                }
            }
            this.update(cx, |this, cx| {
                this.applying = None;
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// Stops the rewrite in progress; the file being written is left as it
    /// was.
    fn stop(&mut self, cx: &mut Context<Self>) {
        self.stop.store(true, Ordering::Relaxed);
        cx.notify();
    }

    /// Ends the review. A rewrite has to be stopped first.
    fn close(&mut self, cx: &mut Context<Self>) {
        if self.applying.is_some() {
            return;
        }
        self.cancel.store(true, Ordering::Relaxed);
        self.stop.store(true, Ordering::Relaxed);
        cx.emit(CleanupEvent::Done);
    }

    /// What Escape does: stop a rewrite, or close.
    fn dismiss(&mut self, cx: &mut Context<Self>) {
        match self.applying {
            Some(_) => self.stop(cx),
            None => self.close(cx),
        }
    }

    /// What Return does when no control has focus: close a result with
    /// nothing left to fix. Rewriting files is never one stray keystroke
    /// away: it takes the Fix button, focused or clicked.
    fn confirm(&mut self, cx: &mut Context<Self>) {
        if self.applying.is_some() || (self.scanning && self.rows.is_empty()) {
            return;
        }
        if self.chosen().is_empty() {
            self.close(cx);
        }
    }

    /// The line under the sheet's title. Counts what was read and what
    /// could not be, videos and folders alike; "tidy" is only said of files
    /// that were read, and "every" only of a complete scan. Says when the
    /// count reaches into subfolders, so it can be squared with the folder's
    /// own list.
    fn status(&self) -> String {
        let folder = folder_name(&self.root);
        let deep = if self.deep {
            ", including subfolders"
        } else {
            ""
        };
        let unreadable = self
            .files
            .iter()
            .filter(|f| matches!(f.state, FileState::Unreadable))
            .count();
        let flagged = self.files.iter().filter(|f| !f.issues.is_empty()).count();
        let issues: usize = self.files.iter().map(|f| f.issues.len()).sum();
        let (fixed_files, fixed_names) = self
            .files
            .iter()
            .filter_map(|f| match &f.state {
                FileState::Fixed(_, written) => Some(written.len()),
                _ => None,
            })
            .fold((0, 0), |(files, names), n| (files + 1, names + n));
        let failed = self
            .files
            .iter()
            .filter(|f| matches!(f.state, FileState::Failed(_)))
            .count();
        let skipped = self.unread.len();
        let unread = unread_phrase(unreadable, &self.unread);
        let found = format!("{} to fix in {}", names(issues), videos(flagged));
        let checked = format!("{} checked{deep}", videos(self.checked));
        let line = |parts: Vec<Option<String>>| {
            parts.into_iter().flatten().collect::<Vec<_>>().join(" · ")
        };
        match (self.scanning, self.total) {
            (true, None) => format!("Looking for videos in “{folder}” and its subfolders…"),
            (true, Some(total)) => line(vec![
                Some(format!(
                    "Checking {} of {}{deep}…",
                    self.checked,
                    videos(total)
                )),
                (issues > 0).then_some(found),
                unread,
            ]),
            (false, _) if self.checked == 0 => match unread {
                None => format!("There are no videos in “{folder}” or its subfolders."),
                Some(unread) => format!("No videos found in “{folder}”, but {unread}."),
            },
            (false, _) if self.checked == unreadable && skipped == 0 => match unreadable {
                1 => format!("The video in “{folder}” couldn’t be read."),
                n => format!("None of the {n} videos in “{folder}”{deep} could be read."),
            },
            (false, _) if fixed_names > 0 => line(vec![
                Some(format!(
                    "Fixed {} in {}",
                    names(fixed_names),
                    videos(fixed_files)
                )),
                (failed > 0).then(|| format!("{} not changed", videos(failed))),
                unread,
            ]),
            // The scope, then the verdict on a line of its own, so the
            // sentence never wraps onto a lone word.
            (false, _) if issues == 0 && unread.is_none() => format!(
                "Checked {} in “{folder}”{deep}.\nEvery stream name is tidy.",
                videos(self.checked)
            ),
            (false, _) if issues == 0 => line(vec![
                Some(checked),
                unread,
                match (self.checked - unreadable, unreadable > 0) {
                    (0, _) => None,
                    (1, true) => Some("the other is tidy".to_owned()),
                    (n, true) => Some(format!("the other {n} are tidy")),
                    (1, false) => Some("its names are tidy".to_owned()),
                    (_, false) => Some("their names are tidy".to_owned()),
                },
            ]),
            (false, _) => line(vec![Some(found), Some(checked), unread]),
        }
    }

    /// Every row of the review. A file is its name, and under it the
    /// subfolder it is in; each name in it is two lines: the tick, which
    /// stream, the old name struck through and the new one, then why it was
    /// flagged, under the old name. Nothing is cut: text that runs out of
    /// room wraps, so every name can be read, by keyboard focus as well.
    /// `identity` is the width of the stream column, shared by every row so
    /// the names line up.
    fn render_rows(&self, identity: Pixels, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let t = Theme::of(cx).clone();
        let busy = self.applying.is_some();
        let entity = cx.entity().downgrade();
        self.rows
            .iter()
            .enumerate()
            .map(|(r, row)| match *row {
                Row::File(f) => {
                    let file = &self.files[f];
                    let state = div().id(("state", f));
                    let state = match &file.state {
                        FileState::Waiting => state,
                        FileState::Rewriting => state.text_color(t.secondary_label).child(format!(
                            "Rewriting… {:.0}%",
                            self.progress.load(Ordering::Relaxed) as f32 / 10.
                        )),
                        FileState::Fixed(..) => state.text_color(t.green).child("Fixed"),
                        FileState::Failed(message) => state
                            .min_w_0()
                            .truncate()
                            .text_color(t.red)
                            .child("Not changed")
                            .tip(message.clone(), None),
                        FileState::Unreadable => state
                            .text_color(t.orange)
                            .child("Couldn’t be read")
                            .tip("Seer couldn’t read this file, so its stream names weren’t checked.", None),
                    };
                    div()
                        .id(("file", f))
                        .min_h(size::ROW)
                        .w_full()
                        .px(size::SHEET_PADDING)
                        .py(size::GAP_TINY)
                        .flex()
                        .items_center()
                        .gap(size::GAP)
                        // A hairline between one file's names and the next.
                        .when(r > 0, |this| {
                            this.border_t(size::HAIRLINE).border_color(t.separator)
                        })
                        // Seer's own glyph, as the file list draws it.
                        .child(ui::kind_glyph(ui::Glyph::Video, false, cx))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .font_weight(FontWeight::MEDIUM)
                                        .child(file.name.clone()),
                                )
                                // Two files may share a name in different
                                // subfolders.
                                .when_some(file.folder.clone(), |this, folder| {
                                    this.child(
                                        div()
                                            .text_size(text::SMALL)
                                            .line_height(text::SMALL_LINE)
                                            .text_color(t.secondary_label)
                                            .child(folder),
                                    )
                                }),
                        )
                        .child(
                            state
                                .flex_none()
                                .text_size(text::SMALL)
                                .font_features(theme::tabular()),
                        )
                        .into_any_element()
                }
                Row::Issue(f, i) => {
                    let file = &self.files[f];
                    let issue = &file.issues[i];
                    // In a fixed file only what was written shows as fixed.
                    let (fix, ticked, locked) = match &file.state {
                        FileState::Fixed(fix, written) => (*fix, written.contains(&i), true),
                        _ => (self.fix, !self.skipped.contains(&(f, i)), busy),
                    };
                    // A ticked name is struck through and its new name
                    // reads as the result; an unticked one stays as is.
                    let (old_ink, new_ink) = match ticked {
                        true => (t.secondary_label, t.label),
                        false => (t.label, t.secondary_label),
                    };
                    let new_name: SharedString = match fix {
                        Fix::Standard => issue.standard.clone(),
                        Fix::Remove => None,
                    }
                    .unwrap_or_else(|| "No name".into())
                    .into();
                    let old_name: SharedString = issue.current.clone().into();
                    // What a screen reader says beside the tick's state:
                    // which stream, the edit, why, and whether it is done.
                    let spoken = spoken_edit(
                        &file.streams[i],
                        &old_name,
                        &new_name,
                        issue.reason.label(),
                        matches!(file.state, FileState::Fixed(..)) && ticked,
                    );
                    let entity = entity.clone();
                    // Names take the room they need; when the row runs out
                    // they wrap onto more lines rather than being cut.
                    let line = div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .items_start()
                        .gap(size::GAP)
                        .child(
                            div()
                                .w(identity)
                                .flex_none()
                                .text_color(t.secondary_label)
                                .child(file.streams[i].clone()),
                        )
                        .child(
                            div()
                                .min_w_0()
                                .text_color(old_ink)
                                .when(ticked, |this| this.line_through())
                                .child(old_name),
                        )
                        .child(div().flex_none().text_color(t.secondary_label).child("→"))
                        .child(div().min_w_0().text_color(new_ink).child(new_name));
                    div()
                        .id(("issue", r))
                        .w_full()
                        .pl(size::SHEET_PADDING + size::ICON + size::GAP)
                        .pr(size::SHEET_PADDING)
                        .py(size::GAP_SMALL)
                        .flex()
                        .flex_col()
                        .gap(size::GAP_TINY)
                        .child(
                            ui::Checkbox::new(("tick", r), ticked)
                                .accessible_label(spoken)
                                .disabled(locked)
                                .label(line)
                                .on_toggle(move |on, _, cx| {
                                    entity
                                        .update(cx, |this, cx| {
                                            match on {
                                                true => this.skipped.remove(&(f, i)),
                                                false => this.skipped.insert((f, i)),
                                            };
                                            cx.notify();
                                        })
                                        .ok();
                                }),
                        )
                        .child(
                            div()
                                .pl(size::CHECKBOX + size::GAP + identity + size::GAP)
                                .text_size(text::SMALL)
                                .line_height(text::SMALL_LINE)
                                .text_color(t.secondary_label)
                                .child(issue.reason.label()),
                        )
                        .into_any_element()
                }
                Row::Unread(u) => {
                    let place = &self.unread[u];
                    let (glyph, why) = match place.folder {
                        true => (
                            ui::Glyph::Folder,
                            "Seer couldn’t open this folder, so the videos in it weren’t checked.",
                        ),
                        false => (
                            ui::Glyph::Other,
                            "Seer couldn’t tell what this item is, so it wasn’t checked.",
                        ),
                    };
                    let (name, folder) = match place.path == self.root {
                        true => (folder_name(&self.root), None),
                        false => self::place(&place.path, &self.root),
                    };
                    div()
                        .id(("unread", u))
                        .min_h(size::ROW)
                        .w_full()
                        .px(size::SHEET_PADDING)
                        .py(size::GAP_TINY)
                        .flex()
                        .items_center()
                        .gap(size::GAP)
                        .when(r > 0, |this| {
                            this.border_t(size::HAIRLINE).border_color(t.separator)
                        })
                        .child(ui::kind_glyph(glyph, false, cx))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(div().font_weight(FontWeight::MEDIUM).child(name))
                                .when_some(folder, |this, folder| {
                                    this.child(
                                        div()
                                            .text_size(text::SMALL)
                                            .line_height(text::SMALL_LINE)
                                            .text_color(t.secondary_label)
                                            .child(folder),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .id(("unread-state", u))
                                .flex_none()
                                .text_size(text::SMALL)
                                .text_color(t.orange)
                                .child("Couldn’t be read")
                                .tip(why, None),
                        )
                        .into_any_element()
                }
            })
            .collect()
    }
}

/// A review row as words, for assistive technology: "Audio 2 · AAC · Tamil:
/// “www.site.io” to “Mono AAC”. Contains a web address." The tick's own
/// state is announced with it.
fn spoken_edit(stream: &str, old: &str, new: &str, reason: &str, fixed: bool) -> String {
    let done = if fixed { " Fixed." } else { "" };
    format!("{stream}: “{old}” to “{new}”. {reason}.{done}")
}

/// What could not be read, for the status line: "1 couldn’t be read" when
/// it is only videos (it follows "7 videos checked"), else each kind named,
/// "1 video and 1 folder couldn’t be read". `None` when everything was read.
fn unread_phrase(videos_unread: usize, places: &[Unread]) -> Option<String> {
    let folders = places.iter().filter(|place| place.folder).count();
    let items = places.len() - folders;
    if folders == 0 && items == 0 {
        return (videos_unread > 0).then(|| format!("{videos_unread} couldn’t be read"));
    }
    let counted = |n: usize, one: &str, many: &str| match n {
        0 => None,
        1 => Some(format!("1 {one}")),
        n => Some(format!("{n} {many}")),
    };
    let parts: Vec<String> = [
        counted(videos_unread, "video", "videos"),
        counted(folders, "folder", "folders"),
        counted(items, "item", "items"),
    ]
    .into_iter()
    .flatten()
    .collect();
    let named = match parts.as_slice() {
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
        [] => return None,
    };
    Some(format!("{named} couldn’t be read"))
}

fn videos(n: usize) -> String {
    match n {
        1 => "1 video".to_owned(),
        n => format!("{n} videos"),
    }
}

/// The Fix button's label, from the names it will rewrite.
fn fix_label(names: usize) -> String {
    match names {
        1 => "Fix 1 Name".to_owned(),
        n => format!("Fix {n} Names"),
    }
}

fn names(n: usize) -> String {
    match n {
        1 => "1 name".to_owned(),
        n => format!("{n} names"),
    }
}

/// A file's name, and the folder it is in below `root` (`None` when it is
/// right in `root`).
fn place(path: &Path, root: &Path) -> (String, Option<String>) {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    let folder = path
        .parent()
        .and_then(|parent| parent.strip_prefix(root).ok())
        .filter(|folder| !folder.as_os_str().is_empty())
        .map(|folder| folder.display().to_string());
    (name, folder)
}

/// Which stream an issue is about, in words a person can match to the
/// file: the kind, its number among streams of that kind when there is more
/// than one, the codec, and the language when the file says.
/// "Audio 2 · AAC · Tamil", "Video · H.264", "File title".
fn stream_identity(issue: &Issue, info: &MediaInfo) -> String {
    let Target::Stream(index) = issue.target else {
        return issue.label.clone();
    };
    let Some(stream) = info.streams.iter().find(|s| s.index == index) else {
        return issue.label.clone();
    };
    let siblings: Vec<usize> = info
        .streams
        .iter()
        .filter(|s| s.kind == stream.kind)
        .map(|s| s.index)
        .collect();
    let kind = match siblings.iter().position(|&s| s == index) {
        Some(n) if siblings.len() > 1 => format!("{} {}", stream.kind.label(), n + 1),
        _ => stream.kind.label().to_owned(),
    };
    let mut parts = vec![kind, format::codec(&stream.codec)];
    parts.extend(
        stream
            .language
            .as_deref()
            .map(str::trim)
            .filter(|code| !code.is_empty() && !code.eq_ignore_ascii_case("und"))
            .map(format::language),
    );
    parts.join(" · ")
}

impl Render for Cleanup {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.applying.is_some() {
            // Keep the percentage moving while a file is rewritten.
            window.request_animation_frame();
        }
        let chosen = self.chosen();
        let names: usize = chosen.iter().map(|(_, issues)| issues.len()).sum();
        let this = cx.entity().downgrade();
        let (dismiss, confirm) = (this.clone(), this.clone());

        let mut sheet = ui::Sheet::new(
            "clean-up",
            platform::label(Command::CleanUpStreamNames).trim_end_matches('…'),
        )
        .status(self.status())
        .focus(&self.focus)
        .on_dismiss(move |_, cx| {
            dismiss.update(cx, |this, cx| this.dismiss(cx)).ok();
        })
        .on_confirm(move |_, cx| {
            confirm.update(cx, |this, cx| this.confirm(cx)).ok();
        });

        let close = |id: &'static str, label: &'static str, cx: &mut Context<Self>| {
            ui::Button::new(id, label).on_click(cx.listener(|this, _, _, cx| this.close(cx)))
        };

        if self.rows.is_empty() {
            // Still looking, or nothing to fix: the compact sheet, with no
            // list, no mode and no warning. It keeps its width from the
            // scan to the result; it widens only when there are rows.
            return match self.scanning {
                true => {
                    let fraction = match self.total {
                        Some(total) if total > 0 => self.checked as f32 / total as f32,
                        _ => 0.,
                    };
                    sheet.note(ui::progress_bar(fraction, cx)).button(close(
                        "cancel-cleanup",
                        "Cancel",
                        cx,
                    ))
                }
                false => sheet.button(close("close-cleanup", "Done", cx).primary()),
            };
        }

        // The stream column is as wide as its widest entry, with a little
        // room for rounding so that entry is never the one that wraps, up to
        // a third of the sheet; every old name starts on one edge. An
        // identity wider than that wraps inside its column.
        let viewport = window.viewport_size();
        let sheet_width = size::SHEET_WIDTH.min(viewport.width - size::SHEET_GUTTER * 2.);
        let identity = self
            .files
            .iter()
            .flat_map(|file| &file.streams)
            .map(|stream| {
                ui::text_width(stream, text::BODY, FontWeight::NORMAL, window) + size::GAP
            })
            .fold(size::ISSUE_LABEL, |widest, width| widest.max(width))
            .min(sheet_width / 3.);
        let has_issues = self.files.iter().any(|file| !file.issues.is_empty());
        if has_issues {
            sheet = sheet
                .accessory_stops(1)
                .accessory(div().w(size::MODE_SWITCH).child(ui::segmented(
                    "cleanup-fix",
                    &[
                        (Fix::Standard, "Standard Names"),
                        (Fix::Remove, "Remove Names"),
                    ],
                    self.fix,
                    move |fix, _, cx| {
                        this.update(cx, |this, cx| {
                            if this.applying.is_none() {
                                this.fix = fix;
                                cx.notify();
                            }
                        })
                        .ok();
                    },
                    cx,
                )));
        }
        // The sheet's body scrolls once the rows outgrow it.
        sheet = sheet.bleed().body(
            div()
                .w_full()
                .flex()
                .flex_col()
                .text_size(text::BODY)
                .line_height(text::BODY_LINE)
                .children(self.render_rows(identity, cx)),
        );

        if let Some((done, total)) = self.applying {
            return sheet
                .note(
                    div()
                        .font_features(theme::tabular())
                        .child(format!("Rewriting file {} of {total}…", done + 1)),
                )
                .button(
                    ui::Button::new("stop-cleanup", "Stop")
                        .on_click(cx.listener(|this, _, _, cx| this.stop(cx))),
                );
        }

        // Rows arrive while the scan runs; the review is approved once it is
        // whole, so Fix waits for the scan.
        if self.scanning {
            let fraction = match self.total {
                Some(total) if total > 0 => self.checked as f32 / total as f32,
                _ => 0.,
            };
            let sheet = sheet.note(ui::progress_bar(fraction, cx)).button(close(
                "cancel-cleanup",
                "Cancel",
                cx,
            ));
            return match names {
                0 => sheet,
                n => sheet.button(
                    ui::Button::new("apply-cleanup", fix_label(n))
                        .primary()
                        .disabled(true),
                ),
            };
        }

        if names == 0 {
            // Everything fixed, everything unticked, or nothing readable.
            if !has_issues {
                sheet = sheet.note(match (self.unread.is_empty(), self.has_unreadable()) {
                    (true, _) => "Names in a file Seer can’t read aren’t checked.",
                    (false, false) => "Videos in a folder Seer can’t read aren’t checked.",
                    (false, true) => "Names in files and folders Seer can’t read aren’t checked.",
                });
            }
            return sheet.button(close("close-cleanup", "Done", cx).primary());
        }

        let files = chosen.len();
        let label = fix_label(names);
        let trash = platform::trash();
        // Short enough for one line beside the buttons.
        let consequence = match files {
            1 => format!("Rewritten without re-encoding. The original goes to the {trash}."),
            _ => format!("Rewritten without re-encoding. Originals go to the {trash}."),
        };
        sheet
            .note(consequence)
            .button(close(
                "close-cleanup",
                if self.any_fixed() { "Done" } else { "Cancel" },
                cx,
            ))
            .button(
                ui::Button::new("apply-cleanup", label)
                    .primary()
                    .on_click(cx.listener(|this, _, _, cx| this.apply(cx))),
            )
    }
}

#[cfg(test)]
mod tests {
    // Not `super::*`: GPUI's own `test` attribute would shadow the standard one.
    use super::{place, spoken_edit, stream_identity, unread_phrase, worth_reviewing};
    use seer_core::fs::{Entry, Kind};
    use seer_core::probe::{MediaInfo, Stream, StreamFlags, StreamKind};
    use seer_core::titles::{self, Unread};
    use std::path::{Path, PathBuf};

    fn entry(name: &str, kind: Kind, hidden: bool) -> Entry {
        Entry {
            name: name.into(),
            path: Path::new("/media").join(name),
            kind,
            size: 0,
            modified: None,
            hidden,
        }
    }

    #[test]
    fn clean_up_is_offered_only_where_videos_can_be() {
        let notes = entry("notes.txt", Kind::from_extension("txt"), false);
        let hidden = entry(".cache", Kind::Folder, true);
        assert!(!worth_reviewing([&notes, &hidden]));
        assert!(!worth_reviewing([]));
        let folder = entry("Clips", Kind::Folder, false);
        assert!(worth_reviewing([&notes, &folder]));
        let video = entry("Trip.MKV", Kind::Video, false);
        assert!(worth_reviewing([&notes, &video]));
    }

    #[test]
    fn a_file_in_a_subfolder_says_which() {
        let root = Path::new("/media");
        assert_eq!(
            place(Path::new("/media/Trip.mkv"), root),
            ("Trip.mkv".to_owned(), None)
        );
        assert_eq!(
            place(Path::new("/media/Holiday 2025/Day 1/Trip.mkv"), root),
            ("Trip.mkv".to_owned(), Some("Holiday 2025/Day 1".to_owned()))
        );
    }

    fn stream(index: usize, kind: StreamKind, codec: &str, language: Option<&str>) -> Stream {
        Stream {
            index,
            kind,
            codec: codec.into(),
            codec_description: None,
            profile: None,
            language: language.map(str::to_owned),
            title: Some("www.rips.example".into()),
            flags: StreamFlags::default(),
            bit_rate: None,
            duration: None,
            tags: Vec::new(),
            width: None,
            height: None,
            frame_rate: None,
            pixel_format: None,
            bit_depth: None,
            color_space: None,
            color_transfer: None,
            color_primaries: None,
            sample_rate: None,
            channels: Some(2),
            channel_layout: None,
        }
    }

    #[test]
    fn streams_are_told_apart_by_number_and_language() {
        let info = MediaInfo {
            container: "matroska".into(),
            container_description: "Matroska".into(),
            duration: 60.,
            bit_rate: 0,
            tags: vec![("title".into(), "www.rips.example".into())],
            streams: vec![
                stream(0, StreamKind::Video, "h264", Some("und")),
                stream(1, StreamKind::Audio, "aac", Some("hin")),
                stream(2, StreamKind::Audio, "aac", Some("tam")),
                stream(3, StreamKind::Subtitle, "subrip", None),
            ],
            chapters: Vec::new(),
        };
        let names: Vec<String> = titles::review(&info)
            .iter()
            .map(|issue| stream_identity(issue, &info))
            .collect();
        assert_eq!(
            names,
            [
                "File title",
                "Video · H.264",
                "Audio 1 · AAC · Hindi",
                "Audio 2 · AAC · Tamil",
                "Subtitle · SRT",
            ]
        );
    }

    #[test]
    fn what_could_not_be_read_is_named_by_kind() {
        let folder = |name: &str| Unread {
            path: PathBuf::from("/media").join(name),
            folder: true,
        };
        let item = Unread {
            path: PathBuf::from("/media/odd"),
            folder: false,
        };
        assert_eq!(unread_phrase(0, &[]), None);
        assert_eq!(unread_phrase(1, &[]).as_deref(), Some("1 couldn’t be read"));
        assert_eq!(
            unread_phrase(0, &[folder("Locked")]).as_deref(),
            Some("1 folder couldn’t be read")
        );
        assert_eq!(
            unread_phrase(1, &[folder("A"), folder("B")]).as_deref(),
            Some("1 video and 2 folders couldn’t be read")
        );
        assert_eq!(
            unread_phrase(2, &[folder("A"), item]).as_deref(),
            Some("2 videos, 1 folder and 1 item couldn’t be read")
        );
    }

    #[test]
    fn a_review_row_reads_as_one_sentence() {
        assert_eq!(
            spoken_edit(
                "Audio 2 · AAC · Tamil",
                "www.site.io",
                "Mono AAC",
                "Contains a web address",
                false
            ),
            "Audio 2 · AAC · Tamil: “www.site.io” to “Mono AAC”. Contains a web address."
        );
    }
}
