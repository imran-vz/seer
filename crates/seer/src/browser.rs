//! The file list: one folder, its rows, the selection and how you move
//! through it. Behaviour follows Finder's list view.

use crate::actions::*;
use crate::library::{Library, Probe};
use crate::theme::{self, Theme, size, text};
use crate::{native, ui};
use gpui_kit::assets::IconName;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::scroll::Scrollbar;
use gpui_kit::prelude::*;
use gpui_kit::*;
use seer_core::format;
use seer_core::fs::{self, Entry, SortKey};
use seer_core::probe::{MediaInfo, StreamKind};
use std::collections::HashSet;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub enum BrowserEvent {
    SelectionChanged,
    Navigated,
}

/// Widths of the panes beside the list, so it can decide which columns fit.
#[derive(Default, Clone, Copy)]
pub struct SidePanes(pub Pixels);
impl Global for SidePanes {}

#[derive(Clone, Copy, PartialEq)]
enum Column {
    Video,
    Audio,
    Duration,
    Size,
    Modified,
}

impl Column {
    /// In the order they are given up as the pane narrows, last first.
    const BY_PRIORITY: [Column; 5] = [
        Column::Size,
        Column::Duration,
        Column::Video,
        Column::Audio,
        Column::Modified,
    ];
    const IN_ORDER: [Column; 5] = [
        Column::Video,
        Column::Audio,
        Column::Duration,
        Column::Size,
        Column::Modified,
    ];

    fn title(self) -> &'static str {
        match self {
            Column::Video => "Video",
            Column::Audio => "Audio",
            Column::Duration => "Duration",
            Column::Size => "Size",
            Column::Modified => "Date Modified",
        }
    }

    fn width(self) -> Pixels {
        px(match self {
            Column::Video => 112.,
            Column::Audio => 104.,
            Column::Duration => 68.,
            Column::Size => 76.,
            Column::Modified => 156.,
        })
    }

    fn numeric(self) -> bool {
        matches!(self, Column::Duration | Column::Size)
    }

    fn sort_key(self) -> Option<SortKey> {
        match self {
            Column::Size => Some(SortKey::Size),
            Column::Modified => Some(SortKey::Modified),
            _ => None,
        }
    }
}

const NAME_MIN: Pixels = px(220.);
const COLUMN_GAP: Pixels = px(12.);
/// How long typed characters keep accumulating for type-to-select.
const TYPEAHEAD: Duration = Duration::from_secs(1);

pub struct Browser {
    dir: PathBuf,
    all: Vec<Entry>,
    rows: Vec<Entry>,
    selected: HashSet<usize>,
    /// Where a shift-extension started.
    anchor: Option<usize>,
    /// The row the keyboard last moved to.
    lead: Option<usize>,
    sort: (SortKey, bool),
    show_hidden: bool,
    /// Lowercased text the names must contain; empty shows everything.
    query: String,
    back: Vec<PathBuf>,
    forward: Vec<PathBuf>,
    error: Option<SharedString>,
    /// Why the last command did nothing. Shown in the status bar until the next one.
    notice: Option<SharedString>,
    loaded: bool,
    /// Selected once the next listing arrives: the folder we came up out of.
    reselect: Option<PathBuf>,
    typed: (String, Instant),
    /// The row whose name is being edited, and its text field.
    renaming: Option<(usize, Entity<InputState>, Subscription)>,
    /// A new item to select and start renaming once it is listed.
    rename_when_listed: Option<PathBuf>,
    scroll: UniformListScrollHandle,
    focus: FocusHandle,
    library: Entity<Library>,
    load: Option<Task<()>>,
    watch: Option<(notify::RecommendedWatcher, Task<()>)>,
}

impl EventEmitter<BrowserEvent> for Browser {}

impl Focusable for Browser {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Browser {
    pub fn new(
        dir: PathBuf,
        sort: (SortKey, bool),
        show_hidden: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let library = Library::global(cx);
        cx.observe(&library, |_, _, cx| cx.notify()).detach();
        let mut this = Browser {
            dir: PathBuf::new(),
            all: Vec::new(),
            rows: Vec::new(),
            selected: HashSet::new(),
            anchor: None,
            lead: None,
            sort,
            show_hidden,
            query: String::new(),
            back: Vec::new(),
            forward: Vec::new(),
            error: None,
            notice: None,
            loaded: false,
            reselect: None,
            typed: (String::new(), Instant::now()),
            renaming: None,
            rename_when_listed: None,
            scroll: UniformListScrollHandle::new(),
            focus: cx.focus_handle(),
            library,
            load: None,
            watch: None,
        };
        this.show(dir, cx);
        this
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn sort(&self) -> (SortKey, bool) {
        self.sort
    }

    pub fn show_hidden(&self) -> bool {
        self.show_hidden
    }

    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }

    pub fn selection(&self) -> Vec<Entry> {
        let mut indexes: Vec<_> = self.selected.iter().copied().collect();
        indexes.sort_unstable();
        indexes
            .into_iter()
            .filter_map(|i| self.rows.get(i).cloned())
            .collect()
    }

    // Navigation

    /// Goes to a folder, recording where we were.
    pub fn navigate(&mut self, dir: PathBuf, cx: &mut Context<Self>) {
        if dir == self.dir {
            return;
        }
        self.back.push(std::mem::take(&mut self.dir));
        self.forward.clear();
        self.show(dir, cx);
    }

    pub fn go_back(&mut self, cx: &mut Context<Self>) {
        if let Some(dir) = self.back.pop() {
            self.forward.push(std::mem::take(&mut self.dir));
            self.show(dir, cx);
        }
    }

    pub fn go_forward(&mut self, cx: &mut Context<Self>) {
        if let Some(dir) = self.forward.pop() {
            self.back.push(std::mem::take(&mut self.dir));
            self.show(dir, cx);
        }
    }

    pub fn go_up(&mut self, cx: &mut Context<Self>) {
        if let Some(parent) = self.dir.parent().map(Path::to_owned) {
            self.reselect = Some(self.dir.clone());
            self.navigate(parent, cx);
        }
    }

    fn show(&mut self, dir: PathBuf, cx: &mut Context<Self>) {
        self.dir = dir;
        self.all.clear();
        self.rows.clear();
        self.error = None;
        self.notice = None;
        self.query.clear();
        self.renaming = None;
        self.loaded = false;
        self.set_selection(HashSet::new(), None, cx);
        self.watch(cx);
        self.reload(cx);
        cx.emit(BrowserEvent::Navigated);
    }

    /// Reads the folder again, keeping the selection where it can.
    fn reload(&mut self, cx: &mut Context<Self>) {
        let dir = self.dir.clone();
        self.load = Some(cx.spawn(async move |this, cx| {
            let listing = cx.background_spawn(async move { fs::list_dir(&dir) }).await;
            this.update(cx, |this, cx| {
                this.loaded = true;
                match listing {
                    Ok(entries) => {
                        this.all = entries;
                        this.error = None;
                    }
                    Err(e) => {
                        this.all.clear();
                        this.error = Some(explain(&e).into());
                    }
                }
                this.rebuild(cx);
            })
            .ok();
        }));
    }

    /// Reloads when something in the folder changes on disk.
    fn watch(&mut self, cx: &mut Context<Self>) {
        use notify::Watcher as _;
        let (tx, rx) = smol::channel::bounded::<()>(1);
        let watcher = notify::recommended_watcher(move |_| {
            tx.try_send(()).ok();
        })
        .and_then(|mut w| {
            w.watch(&self.dir, notify::RecursiveMode::NonRecursive)
                .map(|_| w)
        });
        self.watch = watcher.ok().map(|watcher| {
            let task = cx.spawn(async move |this, cx| {
                while rx.recv().await.is_ok() {
                    // Let a burst of changes settle into one reload.
                    cx.background_executor()
                        .timer(Duration::from_millis(200))
                        .await;
                    while rx.try_recv().is_ok() {}
                    if this.update(cx, |this, cx| this.reload(cx)).is_err() {
                        break;
                    }
                }
            });
            (watcher, task)
        });
    }

    /// Applies filter and sort to the listing and carries the selection over.
    fn rebuild(&mut self, cx: &mut Context<Self>) {
        let kept: HashSet<PathBuf> = match self.reselect.take() {
            Some(path) => HashSet::from([path]),
            None => self.selection().into_iter().map(|e| e.path).collect(),
        };
        let lead = self
            .lead
            .and_then(|i| self.rows.get(i))
            .map(|e| e.path.clone());
        // Row indexes are about to change under any open name field.
        self.renaming = None;

        self.rows = self
            .all
            .iter()
            .filter(|e| self.show_hidden || !e.hidden)
            .filter(|e| self.query.is_empty() || e.name.to_lowercase().contains(&self.query))
            .cloned()
            .collect();
        fs::sort(&mut self.rows, self.sort.0, self.sort.1);

        let selected: HashSet<usize> = self
            .rows
            .iter()
            .enumerate()
            .filter(|(_, e)| kept.contains(&e.path))
            .map(|(i, _)| i)
            .collect();
        let lead = lead
            .and_then(|path| self.rows.iter().position(|e| e.path == path))
            .or_else(|| selected.iter().min().copied());
        if let Some(lead) = lead {
            self.scroll.scroll_to_item(lead, ScrollStrategy::Nearest);
        }
        self.anchor = lead;
        self.set_selection(selected, lead, cx);
        // The same rows may now describe changed files.
        if !self.selected.is_empty() {
            cx.emit(BrowserEvent::SelectionChanged);
        }

        let rows = self.rows.clone();
        self.library
            .update(cx, |library, cx| library.scan(&rows, cx));
        cx.notify();
    }

    pub fn set_sort(&mut self, key: SortKey, cx: &mut Context<Self>) {
        self.sort = if self.sort.0 == key {
            (key, !self.sort.1)
        } else {
            (key, key == SortKey::Name)
        };
        self.rebuild(cx);
    }

    /// Shows only the items whose names contain `query`.
    pub fn set_query(&mut self, query: &str, cx: &mut Context<Self>) {
        let query = query.trim().to_lowercase();
        if query != self.query {
            self.query = query;
            self.rebuild(cx);
        }
    }

    pub fn toggle_hidden(&mut self, cx: &mut Context<Self>) {
        self.show_hidden = !self.show_hidden;
        self.rebuild(cx);
    }

    // Selection

    fn set_selection(
        &mut self,
        selected: HashSet<usize>,
        lead: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        let changed = self.selected != selected;
        self.selected = selected;
        self.lead = lead;
        if changed {
            cx.emit(BrowserEvent::SelectionChanged);
        }
        cx.notify();
    }

    fn select_only(&mut self, index: usize, cx: &mut Context<Self>) {
        self.anchor = Some(index);
        self.scroll.scroll_to_item(index, ScrollStrategy::Nearest);
        self.set_selection(HashSet::from([index]), Some(index), cx);
    }

    fn extend_to(&mut self, index: usize, cx: &mut Context<Self>) {
        let anchor = self.anchor.unwrap_or(index);
        let range = anchor.min(index)..=anchor.max(index);
        self.scroll.scroll_to_item(index, ScrollStrategy::Nearest);
        self.set_selection(range.collect(), Some(index), cx);
    }

    fn click_row(
        &mut self,
        index: usize,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.finish_rename(true, window, cx);
        window.focus(&self.focus, cx);
        if event.click_count >= 2 {
            self.open(index, cx);
        } else if event.modifiers.platform {
            let mut selected = self.selected.clone();
            if !selected.remove(&index) {
                selected.insert(index);
            }
            self.anchor = Some(index);
            self.set_selection(selected, Some(index), cx);
        } else if event.modifiers.shift {
            self.extend_to(index, cx);
        } else {
            self.select_only(index, cx);
        }
    }

    /// Moves the lead row by `delta`, optionally growing the selection.
    fn step(&mut self, delta: isize, extend: bool, cx: &mut Context<Self>) {
        if self.rows.is_empty() || self.renaming.is_some() {
            return;
        }
        let last = self.rows.len() - 1;
        let target = match self.lead {
            Some(lead) => lead.saturating_add_signed(delta).min(last),
            None if delta > 0 => 0,
            None => last,
        };
        if extend {
            self.extend_to(target, cx)
        } else {
            self.select_only(target, cx)
        }
    }

    fn open(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(entry) = self.rows.get(index) else {
            return;
        };
        if entry.is_dir() {
            self.navigate(entry.path.clone(), cx);
        } else {
            cx.open_with_system(&entry.path);
        }
    }

    fn type_to_select(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let keys = &event.keystroke;
        if self.renaming.is_some() {
            return;
        }
        if keys.modifiers.platform || keys.modifiers.control || keys.modifiers.alt {
            return;
        }
        let Some(typed) = keys
            .key_char
            .as_deref()
            .filter(|c| !c.chars().any(char::is_control))
        else {
            return;
        };
        if self.typed.1.elapsed() > TYPEAHEAD {
            self.typed.0.clear();
        }
        if self.typed.0.is_empty() && typed == " " {
            return;
        }
        self.typed.0.push_str(&typed.to_lowercase());
        self.typed.1 = Instant::now();

        let prefix = &self.typed.0;
        // First name at or after the typed text, as Finder does.
        let found = self
            .rows
            .iter()
            .position(|e| e.name.to_lowercase().starts_with(prefix.as_str()))
            .or_else(|| {
                self.rows
                    .iter()
                    .position(|e| fs::natural_cmp(&e.name, prefix) != std::cmp::Ordering::Less)
            });
        if let Some(index) = found {
            self.select_only(index, cx);
            cx.stop_propagation();
        }
    }

    fn trash_selection(&mut self, cx: &mut Context<Self>) {
        let paths: Vec<PathBuf> = self.selection().into_iter().map(|e| e.path).collect();
        if paths.is_empty() || self.renaming.is_some() {
            return;
        }
        // Keep the keyboard somewhere sensible once the rows are gone.
        let next = self
            .selected
            .iter()
            .max()
            .map(|i| i + 1 - self.selected.len());
        cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move { fs::trash(&paths) }).await;
            this.update(cx, |this, cx| {
                this.notice = result
                    .err()
                    .map(|e| format!("Nothing was moved to the Trash. {e}").into());
                this.set_selection(HashSet::new(), next, cx);
                this.reload(cx);
            })
            .ok();
        })
        .detach();
    }

    // Renaming and creating

    /// Puts a text field over the selected row's name, as Return does in Finder.
    fn begin_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut selected = self.selected.iter();
        let (Some(&index), None) = (selected.next(), selected.next()) else {
            return;
        };
        let Some(entry) = self.rows.get(index) else {
            return;
        };
        let name = entry.name.clone();
        // Preselect the name without its extension.
        let stem = match entry.is_dir() {
            true => name.len(),
            false => name.rfind('.').filter(|i| *i > 0).unwrap_or(name.len()),
        };
        let field = cx.new(|cx| InputState::new(window, cx).default_value(name));
        field.update(cx, |field, cx| {
            field.set_selected_range(0..stem, cx);
            field.focus(window, cx);
        });
        let events =
            cx.subscribe_in(
                &field,
                window,
                |this, _, event: &InputEvent, window, cx| match event {
                    InputEvent::PressEnter { .. } | InputEvent::Blur => {
                        this.finish_rename(true, window, cx)
                    }
                    _ => {}
                },
            );
        self.renaming = Some((index, field, events));
        cx.notify();
    }

    fn finish_rename(&mut self, apply: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some((index, field, _)) = self.renaming.take() else {
            return;
        };
        window.focus(&self.focus, cx);
        cx.notify();
        let Some(entry) = self.rows.get(index) else {
            return;
        };
        let name = field.read(cx).value().trim().to_owned();
        if !apply || name.is_empty() || name == entry.name {
            return;
        }
        let path = entry.path.clone();
        self.rename_path(&path, &name, cx);
    }

    /// Renames an item in this folder and keeps it selected.
    pub fn rename_path(&mut self, path: &Path, name: &str, cx: &mut Context<Self>) {
        let old = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let target = path.with_file_name(name);
        let result = if name.contains('/') || name.contains(':') {
            Err("Names can’t contain “/” or “:”.".to_owned())
        } else if target.exists() {
            Err(format!("The name “{name}” is already taken."))
        } else {
            std::fs::rename(path, &target).map_err(|e| e.to_string())
        };
        match result {
            Ok(()) => {
                self.notice = None;
                self.reselect = Some(target);
                self.reload(cx);
            }
            Err(message) => {
                self.notice = Some(format!("“{old}” was not renamed. {message}").into())
            }
        }
        cx.notify();
    }

    /// Creates "untitled folder" here and starts renaming it.
    fn new_folder(&mut self, cx: &mut Context<Self>) {
        let mut path = self.dir.join("untitled folder");
        let mut n = 2;
        while path.exists() {
            path = self.dir.join(format!("untitled folder {n}"));
            n += 1;
        }
        match std::fs::create_dir(&path) {
            Ok(()) => {
                self.notice = None;
                self.reselect = Some(path.clone());
                self.rename_when_listed = Some(path);
                self.reload(cx);
            }
            Err(e) => {
                self.notice = Some(format!("The folder was not created. {}", explain(&e)).into())
            }
        }
        cx.notify();
    }

    // Rendering

    fn columns(&self, window: &Window, cx: &App) -> Vec<Column> {
        let side = cx.try_global::<SidePanes>().map_or(px(0.), |s| s.0);
        let mut room = window.viewport_size().width - side - size::ROW_INSET * 4. - NAME_MIN;
        let mut fit = Vec::new();
        for column in Column::BY_PRIORITY {
            room -= column.width() + COLUMN_GAP;
            if room < px(0.) {
                break;
            }
            fit.push(column);
        }
        Column::IN_ORDER
            .into_iter()
            .filter(|c| fit.contains(c))
            .collect()
    }

    fn render_header(
        &self,
        columns: &[Column],
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let t = Theme::of(cx).clone();
        let (key, ascending) = self.sort;
        let title = |label: &'static str, sorted: bool| {
            div()
                .flex()
                .items_center()
                .gap(px(2.))
                .text_color(if sorted { t.label } else { t.secondary_label })
                .child(label)
                .when(sorted, |this| {
                    let arrow = if ascending {
                        IconName::ChevronUp
                    } else {
                        IconName::ChevronDown
                    };
                    this.child(ui::icon(arrow, t.secondary_label).size(px(10.)))
                })
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(COLUMN_GAP)
            .h(size::HEADER_ROW)
            .px(size::ROW_INSET * 2.)
            .border_b(ui::hairline(window))
            .border_color(t.separator)
            .text_size(text::SMALL)
            .child(
                div()
                    .id("column-name")
                    .flex_1()
                    .min_w_0()
                    .on_click(cx.listener(|this, _, _, cx| this.set_sort(SortKey::Name, cx)))
                    .child(title("Name", key == SortKey::Name)),
            )
            .children(columns.iter().map(|&column| {
                let sorted = column.sort_key() == Some(key);
                div()
                    .id(column.title())
                    .flex_none()
                    .flex()
                    .w(column.width())
                    .when(column.numeric(), |this| this.justify_end())
                    .when_some(column.sort_key(), |this, key| {
                        this.on_click(cx.listener(move |this, _, _, cx| this.set_sort(key, cx)))
                    })
                    .child(title(column.title(), sorted))
            }))
    }

    fn render_rows(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<impl IntoElement + use<>> {
        let t = Theme::of(cx).clone();
        let columns = self.columns(window, cx);
        let focused = self.focus.is_focused(window) && window.is_window_active();
        let library = self.library.read(cx);

        range
            .filter_map(|index| {
                let entry = self.rows.get(index)?;
                let selected = self.selected.contains(&index);
                let (primary, secondary) = match (selected, focused) {
                    (true, true) => (t.on_selection, t.on_selection_secondary),
                    _ => (t.label, t.secondary_label),
                };
                let info = library.info(&entry.path).map(|i| i.as_ref());
                let pending = matches!(library.probe(&entry.path), Some(Probe::Pending));

                let cells = columns.iter().map(|&column| {
                    div()
                        .flex_none()
                        .w(column.width())
                        .truncate()
                        .when(column.numeric(), |this| this.text_right())
                        .child(cell(column, entry, info, pending))
                });

                Some(
                    div()
                        .id(index)
                        .h(size::ROW)
                        .px(size::ROW_INSET)
                        .when(index % 2 == 1, |this| this.bg(t.alternate_row))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                                this.click_row(index, event, window, cx);
                                cx.stop_propagation();
                            }),
                        )
                        .child(
                            div()
                                .size_full()
                                .flex()
                                .items_center()
                                .gap(COLUMN_GAP)
                                .px(size::ROW_INSET)
                                .rounded(size::RADIUS)
                                .when(selected, |this| {
                                    this.bg(if focused {
                                        t.selection
                                    } else {
                                        t.selection_inactive
                                    })
                                })
                                .text_color(secondary)
                                .font_features(theme::tabular())
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .flex()
                                        .items_center()
                                        .gap(px(6.))
                                        .children(
                                            native::icon_for(&entry.path, entry.is_dir(), 16)
                                                .map(|icon| img(icon).size(size::ICON).flex_none()),
                                        )
                                        .child(match &self.renaming {
                                            Some((row, field, _)) if *row == index => div()
                                                .flex_1()
                                                .min_w_0()
                                                .text_color(t.label)
                                                // Clicks in the field edit text; they don't reselect the row.
                                                .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                                    cx.stop_propagation()
                                                })
                                                .child(Input::new(field).xsmall()),
                                            _ => div()
                                                .min_w_0()
                                                .text_color(primary)
                                                .whitespace_nowrap()
                                                .text_ellipsis_middle()
                                                .when(entry.hidden, |this| this.opacity(0.5))
                                                .child(entry.name.clone()),
                                        }),
                                )
                                .children(cells),
                        ),
                )
            })
            .collect()
    }

    fn render_status(&self, window: &Window, cx: &App) -> impl IntoElement {
        let t = Theme::of(cx);
        let jobs = self.library.read(cx).running_jobs();
        let summary = match self.selected.len() {
            0 if !self.query.is_empty() => {
                let total = self
                    .all
                    .iter()
                    .filter(|e| self.show_hidden || !e.hidden)
                    .count();
                format!("{} of {}", self.rows.len(), plural(total, "item"))
            }
            0 => plural(self.rows.len(), "item"),
            n => {
                let bytes: u64 = self.selection().iter().map(|e| e.size).sum();
                let size = if bytes > 0 {
                    format!(", {}", format::size(bytes))
                } else {
                    String::new()
                };
                format!("{n} of {} selected{size}", self.rows.len())
            }
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(size::GAP)
            .h(size::STATUS_BAR)
            .px(size::ROW_INSET * 2.)
            .border_t(ui::hairline(window))
            .border_color(t.separator)
            .text_size(text::SMALL)
            .text_color(t.secondary_label)
            .child(match &self.notice {
                Some(notice) => div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(t.red)
                    .child(notice.clone()),
                None => div()
                    .flex_1()
                    .min_w_0()
                    .whitespace_nowrap()
                    .text_ellipsis_start()
                    .child(display_path(&self.dir)),
            })
            .when(jobs > 0, |this| {
                let label = if jobs == 1 {
                    "1 job running".to_owned()
                } else {
                    format!("{jobs} jobs running")
                };
                this.child(div().flex_none().text_color(t.label).child(label))
            })
            .child(
                div()
                    .flex_none()
                    .font_features(theme::tabular())
                    .child(summary),
            )
    }
}

impl Render for Browser {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::of(cx).clone();
        let columns = self.columns(window, cx);

        if let Some(path) = &self.rename_when_listed
            && self.selection().first().is_some_and(|e| &e.path == path)
        {
            self.rename_when_listed = None;
            self.begin_rename(window, cx);
        }

        let body = if let Some(error) = &self.error {
            ui::empty_state("This folder can’t be shown", error.clone(), cx).into_any_element()
        } else if self.loaded && self.rows.is_empty() && !self.query.is_empty() {
            ui::empty_state(
                "No matches",
                "Nothing in this folder has that in its name.",
                cx,
            )
            .into_any_element()
        } else if self.loaded && self.rows.is_empty() {
            let detail = if self.all.is_empty() {
                ""
            } else {
                "Hidden files are not shown. Press ⇧⌘. to show them."
            };
            ui::empty_state("Folder is empty", detail, cx).into_any_element()
        } else {
            uniform_list(
                "rows",
                self.rows.len(),
                cx.processor(|this, range, window, cx| this.render_rows(range, window, cx)),
            )
            .track_scroll(&self.scroll)
            .size_full()
            .into_any_element()
        };

        div()
            .id("browser")
            .key_context("Browser")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(t.content)
            .text_size(text::BODY)
            .line_height(text::BODY_LINE)
            .on_key_down(cx.listener(Self::type_to_select))
            .on_action(cx.listener(|this, _: &MoveUp, _, cx| this.step(-1, false, cx)))
            .on_action(cx.listener(|this, _: &MoveDown, _, cx| this.step(1, false, cx)))
            .on_action(cx.listener(|this, _: &ExtendUp, _, cx| this.step(-1, true, cx)))
            .on_action(cx.listener(|this, _: &ExtendDown, _, cx| this.step(1, true, cx)))
            .on_action(cx.listener(|this, _: &MoveToFirst, _, cx| {
                if !this.rows.is_empty() && this.renaming.is_none() {
                    this.select_only(0, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &MoveToLast, _, cx| {
                if !this.rows.is_empty() && this.renaming.is_none() {
                    this.select_only(this.rows.len() - 1, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &SelectAll, _, cx| {
                let all = (0..this.rows.len()).collect();
                this.set_selection(all, this.lead, cx);
            }))
            .on_action(cx.listener(|this, _: &ClearSelection, window, cx| {
                // Escape first backs out of a rename.
                match this.renaming.is_some() {
                    true => this.finish_rename(false, window, cx),
                    false => this.set_selection(HashSet::new(), this.lead, cx),
                }
            }))
            .on_action(cx.listener(|this, _: &Rename, window, cx| {
                // Return starts a rename, and confirms the one in progress:
                // this binding is matched ahead of the text field's own.
                match this.renaming.is_some() {
                    true => this.finish_rename(true, window, cx),
                    false => this.begin_rename(window, cx),
                }
            }))
            .on_action(cx.listener(|this, _: &NewFolder, _, cx| this.new_folder(cx)))
            .on_action(cx.listener(|this, _: &OpenSelection, _, cx| {
                // Opening several folders at once has no meaning in one pane.
                if let Some(index) = this.lead.filter(|i| this.selected.contains(i)) {
                    this.open(index, cx);
                }
            }))
            .on_action(cx.listener(
                |this, _: &RevealInFinder, _, cx| match this.selection().first() {
                    Some(entry) => cx.reveal_path(&entry.path),
                    None => cx.reveal_path(&this.dir),
                },
            ))
            .on_action(cx.listener(|this, _: &MoveToTrash, _, cx| this.trash_selection(cx)))
            .on_action(cx.listener(|this, _: &GoBack, _, cx| this.go_back(cx)))
            .on_action(cx.listener(|this, _: &GoForward, _, cx| this.go_forward(cx)))
            .on_action(cx.listener(|this, _: &GoToEnclosingFolder, _, cx| this.go_up(cx)))
            .on_action(cx.listener(|this, _: &ToggleHiddenFiles, _, cx| this.toggle_hidden(cx)))
            .on_action(cx.listener(|this, _: &SortByName, _, cx| this.set_sort(SortKey::Name, cx)))
            .on_action(cx.listener(|this, _: &SortBySize, _, cx| this.set_sort(SortKey::Size, cx)))
            .on_action(
                cx.listener(|this, _: &SortByModified, _, cx| this.set_sort(SortKey::Modified, cx)),
            )
            .child(self.render_header(&columns, window, cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    // A click below the last row clears the selection.
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            this.finish_rename(true, window, cx);
                            window.focus(&this.focus, cx);
                            this.set_selection(HashSet::new(), None, cx);
                        }),
                    )
                    .relative()
                    .child(body)
                    // Overlay scroller, shown while scrolling as on macOS.
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .right_0()
                            .bottom_0()
                            .w(Scrollbar::width())
                            .child(Scrollbar::vertical(&self.scroll).viewport_from_layout()),
                    ),
            )
            .child(self.render_status(window, cx))
    }
}

fn cell(column: Column, entry: &Entry, info: Option<&MediaInfo>, pending: bool) -> SharedString {
    let none = if pending { "" } else { "--" };
    match column {
        Column::Size if entry.is_dir() => "--".into(),
        Column::Size => format::size(entry.size).into(),
        Column::Modified => entry.modified.map(format::date).unwrap_or_default().into(),
        _ if !entry.kind.is_probeable() => "".into(),
        Column::Duration => match info.map(|i| i.duration).filter(|d| *d > 0.) {
            Some(seconds) => format::duration(seconds).into(),
            None => none.into(),
        },
        Column::Video => match info.and_then(MediaInfo::primary_video) {
            Some(video) => {
                let codec = format::codec(&video.codec);
                match (video.width, video.height) {
                    (Some(w), Some(h)) => match format::resolution_class(w, h) {
                        Some(class) => format!("{class} {codec}"),
                        None => format!("{w}×{h} {codec}"),
                    },
                    _ => codec,
                }
                .into()
            }
            None => none.into(),
        },
        Column::Audio => match info.and_then(MediaInfo::primary_audio) {
            Some(audio) => {
                let mut label = format::codec(&audio.codec);
                if let Some(channels) = audio.channels {
                    label = format!("{label} {}", format::channels(channels));
                }
                let others = info.map_or(0, |i| i.streams_of(StreamKind::Audio).count()) - 1;
                if others > 0 {
                    label = format!("{label} +{others}");
                }
                label.into()
            }
            None => none.into(),
        },
    }
}

fn plural(count: usize, noun: &str) -> String {
    match count {
        1 => format!("1 {noun}"),
        n => format!("{n} {noun}s"),
    }
}

/// The path with the home folder shortened to "~".
pub fn display_path(path: &Path) -> String {
    match dirs::home_dir().and_then(|home| path.strip_prefix(home).ok().map(Path::to_owned)) {
        Some(rest) if rest.as_os_str().is_empty() => "~".into(),
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

fn explain(error: &std::io::Error) -> String {
    match error.kind() {
        std::io::ErrorKind::PermissionDenied => {
            "Seer doesn’t have permission to read it. Allow access in System Settings › Privacy & Security › Files and Folders.".into()
        }
        std::io::ErrorKind::NotFound => "It may have been moved or deleted.".into(),
        _ => error.to_string(),
    }
}

/// Name shown for a folder in the title bar and sidebar.
pub fn folder_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Macintosh HD".into())
}
