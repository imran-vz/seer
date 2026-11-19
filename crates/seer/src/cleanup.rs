//! Folder-wide review of stream names: every video under a folder, at any
//! depth, with the names that carry a web address or don't describe their
//! stream, and a way to put them right in one go.

use crate::library::{Library, Probe, probe_one};
use crate::theme::{self, Theme, size, text};
use crate::{native, ui};
use gpui_kit::prelude::*;
use gpui_kit::*;
use seer_core::bitrate::Cancelled;
use seer_core::remux;
use seer_core::titles::{self, Fix, Issue};
use std::collections::HashSet;
use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

pub enum CleanupEvent {
    /// The person is finished with the review.
    Done,
}

enum FileState {
    Waiting,
    Rewriting,
    Fixed,
    Failed(SharedString),
}

struct Flagged {
    path: PathBuf,
    /// Path below the folder being reviewed.
    relative: String,
    issues: Vec<Issue>,
    state: FileState,
}

#[derive(Clone, Copy)]
enum Row {
    File(usize),
    Issue(usize, usize),
}

pub struct Cleanup {
    root: PathBuf,
    files: Vec<Flagged>,
    rows: Vec<Row>,
    /// Issues the person has unticked, as (file, issue).
    skipped: HashSet<(usize, usize)>,
    fix: Fix,
    checked: usize,
    total: Option<usize>,
    scanning: bool,
    /// Files done and files to do, while a fix is running.
    applying: Option<(usize, usize)>,
    /// Progress through the file being rewritten, in thousandths.
    progress: Arc<AtomicU32>,
    cancel: Arc<AtomicBool>,
    task: Option<Task<()>>,
}

impl EventEmitter<CleanupEvent> for Cleanup {}

impl Cleanup {
    pub fn new(root: PathBuf, cx: &mut Context<Self>) -> Self {
        let mut this = Cleanup {
            root,
            files: Vec::new(),
            rows: Vec::new(),
            skipped: HashSet::new(),
            fix: Fix::Standard,
            checked: 0,
            total: None,
            scanning: true,
            applying: None,
            progress: Arc::new(AtomicU32::new(0)),
            cancel: Arc::new(AtomicBool::new(false)),
            task: None,
        };
        this.scan(cx);
        this
    }

    fn scan(&mut self, cx: &mut Context<Self>) {
        let (root, cancel) = (self.root.clone(), self.cancel.clone());
        let cache = Library::global(cx).read(cx).cache();
        self.task = Some(cx.spawn(async move |this, cx| {
            let walk = (root.clone(), cancel.clone());
            let paths = cx
                .background_spawn(async move { titles::video_files(&walk.0, &walk.1) })
                .await;
            if this
                .update(cx, |this, cx| {
                    this.total = Some(paths.len());
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
                                let Probe::Ready(info) = probe_one(&path, cache.as_deref()) else {
                                    return None;
                                };
                                let issues = titles::review(&info);
                                let relative = path
                                    .strip_prefix(&root)
                                    .unwrap_or(&path)
                                    .display()
                                    .to_string();
                                (!issues.is_empty()).then_some(Flagged {
                                    path,
                                    relative,
                                    issues,
                                    state: FileState::Waiting,
                                })
                            })
                            .collect::<Vec<_>>()
                    })
                    .await;
                let alive = this.update(cx, |this, cx| {
                    this.checked += count;
                    this.files.extend(found);
                    this.rebuild_rows();
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

    fn rebuild_rows(&mut self) {
        self.rows = self
            .files
            .iter()
            .enumerate()
            .flat_map(|(f, file)| {
                std::iter::once(Row::File(f))
                    .chain((0..file.issues.len()).map(move |i| Row::Issue(f, i)))
            })
            .collect();
    }

    /// Issues still ticked in files that have not been fixed yet.
    fn chosen(&self) -> Vec<(usize, Vec<usize>)> {
        self.files
            .iter()
            .enumerate()
            .filter(|(_, file)| !matches!(file.state, FileState::Fixed))
            .filter_map(|(f, file)| {
                let issues: Vec<usize> = (0..file.issues.len())
                    .filter(|i| !self.skipped.contains(&(f, *i)))
                    .collect();
                (!issues.is_empty()).then_some((f, issues))
            })
            .collect()
    }

    /// Rewrites each chosen file in turn. One at a time: each is a full copy
    /// of the file, and disks do not thank you for doing several at once.
    fn apply(&mut self, cx: &mut Context<Self>) {
        let chosen = self.chosen();
        if chosen.is_empty() || self.applying.is_some() {
            return;
        }
        let jobs: Vec<(usize, PathBuf, remux::Edits)> = chosen
            .iter()
            .map(|(f, issues)| {
                let file = &self.files[*f];
                (
                    *f,
                    file.path.clone(),
                    titles::edits(issues.iter().map(|i| &file.issues[*i]), self.fix),
                )
            })
            .collect();
        self.cancel.store(false, Ordering::Relaxed);
        self.applying = Some((0, jobs.len()));
        let (cancel, progress) = (self.cancel.clone(), self.progress.clone());

        self.task = Some(cx.spawn(async move |this, cx| {
            for (done, (index, path, edits)) in jobs.into_iter().enumerate() {
                progress.store(0, Ordering::Relaxed);
                let alive = this.update(cx, |this, cx| {
                    this.files[index].state = FileState::Rewriting;
                    this.applying = this.applying.map(|(_, total)| (done, total));
                    cx.notify();
                });
                if alive.is_err() {
                    return;
                }
                let (cancel_flag, meter) = (cancel.clone(), progress.clone());
                let result = cx
                    .background_spawn(async move {
                        remux::rewrite(&path, &edits, &cancel_flag, |fraction| {
                            meter.store((fraction * 1000.) as u32, Ordering::Relaxed)
                        })
                    })
                    .await;
                let stop = matches!(&result, Err(e) if e.is::<Cancelled>());
                this.update(cx, |this, cx| {
                    this.files[index].state = match result {
                        Ok(()) => FileState::Fixed,
                        Err(_) if stop => FileState::Waiting,
                        Err(e) => FileState::Failed(format!("{e:#}").into()),
                    };
                    cx.notify();
                })
                .ok();
                if stop {
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

    fn status(&self) -> String {
        let files = |n: usize| {
            if n == 1 {
                "1 video".to_owned()
            } else {
                format!("{n} videos")
            }
        };
        let issues: usize = self.files.iter().map(|f| f.issues.len()).sum();
        let found = match issues {
            0 => "no names to fix".to_owned(),
            1 => "1 name to look at".to_owned(),
            n => format!("{n} names to look at in {}", files(self.files.len())),
        };
        match (self.scanning, self.total) {
            (true, None) => "Looking for videos…".to_owned(),
            (true, Some(total)) => format!("Checked {} of {}, {found}", self.checked, files(total)),
            (false, _) => format!("Checked {}, {found}", files(self.checked)),
        }
    }

    fn render_rows(
        &mut self,
        range: Range<usize>,
        cx: &mut Context<Self>,
    ) -> Vec<impl IntoElement + use<>> {
        let t = Theme::of(cx).clone();
        let busy = self.applying.is_some();
        range
            .filter_map(|r| {
                Some(match *self.rows.get(r)? {
                    Row::File(f) => {
                        let file = &self.files[f];
                        let state = match &file.state {
                            FileState::Waiting => div(),
                            FileState::Rewriting => {
                                div().text_color(t.secondary_label).child(format!(
                                    "Rewriting… {:.0}%",
                                    self.progress.load(Ordering::Relaxed) as f32 / 10.
                                ))
                            }
                            FileState::Fixed => div().text_color(t.secondary_label).child("Fixed"),
                            FileState::Failed(message) => div()
                                .text_color(t.red)
                                .truncate()
                                .child(format!("Not changed: {message}")),
                        };
                        div()
                            .id(("file", f))
                            .h(size::ROW)
                            .w_full()
                            .px(size::ROW_INSET * 2.)
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .bg(t.alternate_row)
                            .children(
                                native::icon_for(&file.path, false, 16)
                                    .map(|icon| img(icon).size(size::ICON).flex_none()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .whitespace_nowrap()
                                    .text_ellipsis_start()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(file.relative.clone()),
                            )
                            .child(
                                state
                                    .flex_none()
                                    .text_size(text::SMALL)
                                    .font_features(theme::tabular()),
                            )
                    }
                    Row::Issue(f, i) => {
                        let file = &self.files[f];
                        let issue = &file.issues[i];
                        let fixed = matches!(file.state, FileState::Fixed);
                        let ticked = !self.skipped.contains(&(f, i));
                        let new_name = match self.fix {
                            Fix::Standard => issue.standard.clone(),
                            Fix::Remove => None,
                        };
                        div()
                            .id(("issue", r))
                            .h(size::ROW)
                            .w_full()
                            .pl(size::ROW_INSET * 2. + px(22.))
                            .pr(size::ROW_INSET * 2.)
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .text_size(text::SMALL)
                            .when(fixed || !ticked, |this| this.opacity(0.5))
                            .when(!busy && !fixed, |this| {
                                this.on_click(cx.listener(move |this, _, _, cx| {
                                    if !this.skipped.remove(&(f, i)) {
                                        this.skipped.insert((f, i));
                                    }
                                    cx.notify();
                                }))
                            })
                            .child(ui::checkbox(ticked && !fixed, cx))
                            .child(
                                div()
                                    .w(px(130.))
                                    .flex_none()
                                    .truncate()
                                    .text_color(t.secondary_label)
                                    .child(issue.label.clone()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_color(t.secondary_label)
                                    .when(ticked, |this| this.line_through())
                                    .child(issue.current.clone()),
                            )
                            .child(div().flex_none().text_color(t.tertiary_label).child("→"))
                            .child(match new_name {
                                Some(name) => div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_color(t.label)
                                    .child(name),
                                None => div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_color(t.tertiary_label)
                                    .child("No name"),
                            })
                            .child(
                                div()
                                    .w(px(170.))
                                    .flex_none()
                                    .truncate()
                                    .text_right()
                                    .text_color(t.tertiary_label)
                                    .child(issue.reason.label()),
                            )
                    }
                })
            })
            .collect()
    }
}

impl Render for Cleanup {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::of(cx).clone();
        if self.applying.is_some() {
            // Keep the percentage moving while a file is rewritten.
            window.request_animation_frame();
        }
        let chosen = self.chosen();
        let names: usize = chosen.iter().map(|(_, issues)| issues.len()).sum();
        let this = cx.entity().downgrade();

        let body = if self.rows.is_empty() && !self.scanning {
            ui::empty_state(
                "Every stream name looks fine",
                format!("Nothing under “{}” carries a web address or a name that doesn’t match its stream.", crate::browser::folder_name(&self.root)),
                cx,
            )
            .into_any_element()
        } else if self.rows.is_empty() {
            ui::empty_state("Checking stream names…", self.status(), cx).into_any_element()
        } else {
            uniform_list(
                "cleanup-rows",
                self.rows.len(),
                cx.processor(|this, range, _, cx| this.render_rows(range, cx)),
            )
            .size_full()
            .into_any_element()
        };

        let action =
            match self.applying {
                Some((done, total)) => div()
                    .flex()
                    .items_center()
                    .gap(size::GAP)
                    .child(
                        div()
                            .text_size(text::SMALL)
                            .text_color(t.secondary_label)
                            .font_features(theme::tabular())
                            .child(format!("Rewriting file {} of {total}", done + 1)),
                    )
                    .child(
                        ui::button("stop-cleanup", "Stop", false, cx).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.cancel.store(true, Ordering::Relaxed);
                                cx.notify();
                            },
                        )),
                    ),
                None => {
                    let label = match (names, chosen.len()) {
                        (0, _) => "Fix Names".to_owned(),
                        (1, _) => "Fix 1 Name".to_owned(),
                        (n, 1) => format!("Fix {n} Names in 1 File"),
                        (n, files) => format!("Fix {n} Names in {files} Files"),
                    };
                    let fix = match names {
                        0 => ui::disabled_button(label, cx).into_any_element(),
                        _ => ui::button("apply-cleanup", label, true, cx)
                            .on_click(cx.listener(|this, _, _, cx| this.apply(cx)))
                            .into_any_element(),
                    };
                    div()
                        .flex()
                        .gap(size::GAP)
                        .child(ui::button("close-cleanup", "Done", false, cx).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.cancel.store(true, Ordering::Relaxed);
                                cx.emit(CleanupEvent::Done);
                            }),
                        ))
                        .child(fix)
                }
            };

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(t.content)
            .text_size(text::BODY)
            .line_height(text::BODY_LINE)
            .text_color(t.label)
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(size::PANE_PADDING)
                    .px(size::ROW_INSET * 2.)
                    .py(px(10.))
                    .border_b(ui::hairline(window))
                    .border_color(t.separator)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(div().font_weight(FontWeight::SEMIBOLD).child("Stream Names"))
                            .child(
                                div()
                                    .text_size(text::SMALL)
                                    .line_height(text::SMALL_LINE)
                                    .text_color(t.secondary_label)
                                    .font_features(theme::tabular())
                                    .truncate()
                                    .child(self.status()),
                            ),
                    )
                    .child(div().w(px(280.)).flex_none().child(ui::segmented(
                        "cleanup-fix",
                        &[(Fix::Standard, "Use Standard Names"), (Fix::Remove, "Remove Names")],
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
                    ))),
            )
            .child(div().flex_1().min_h_0().child(body))
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(size::PANE_PADDING)
                    .px(size::ROW_INSET * 2.)
                    .py(px(8.))
                    .border_t(ui::hairline(window))
                    .border_color(t.separator)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(text::SMALL)
                            .line_height(text::SMALL_LINE)
                            .text_color(t.secondary_label)
                            .child("Each file is rewritten without re-encoding, one at a time. Originals move to the Trash."),
                    )
                    .child(action),
            )
    }
}
