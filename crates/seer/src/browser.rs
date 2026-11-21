//! The file list: one folder, its rows, the selection and how you move
//! through it. Behaviour follows Finder's list view.

use crate::actions::*;
use crate::devices::{Devices, DevicesEvent, Dragged, Ending, Unresolved};
use crate::library::{Library, Probe};
use crate::platform::{self, Command};
use crate::theme::{self, Theme, size, text};
use crate::ui;
use crate::ui::Tip as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::menu::{ContextMenuExt as _, PopupMenu};
use gpui_kit::component::popover::{Popover, PopoverState};
use gpui_kit::component::scroll::Scrollbar;
use gpui_kit::prelude::*;
use gpui_kit::*;
use seer_core::format;
use seer_core::fs::{self, Entry, Kind, SortKey};
use seer_core::mtp;
use seer_core::probe::{MediaInfo, StreamKind};
use seer_core::transfer::{Detail, ItemStatus, Mode, Progress, Stage};
use std::cell::Cell;
use std::collections::HashSet;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

pub enum BrowserEvent {
    SelectionChanged,
    Navigated,
    /// The folder's listing arrived or changed.
    Listed,
}

/// Widths of the panes beside the list, so it can decide which columns fit.
#[derive(Default, Clone, Copy)]
pub struct SidePanes(pub Pixels);
impl Global for SidePanes {}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Column {
    Video,
    Audio,
    Duration,
    Size,
    Modified,
}

impl Column {
    /// Left to right.
    const IN_ORDER: [Column; 5] = [
        Column::Video,
        Column::Audio,
        Column::Duration,
        Column::Size,
        Column::Modified,
    ];

    /// The order columns are kept in as the list narrows: the first is
    /// kept longest, the last given up first, and only once Name is down to
    /// `NAME_MIN`. Size goes last, so a narrow window still compares files;
    /// then what the media is and how long it runs. In a folder of audio,
    /// Audio takes Video's place and Video is never shown, at any width: it
    /// could only ever be empty.
    fn by_priority(audio_folder: bool) -> &'static [Column] {
        use Column::*;
        match audio_folder {
            false => &[Size, Video, Duration, Audio, Modified],
            true => &[Size, Audio, Duration, Modified],
        }
    }

    /// How many of [`Column::by_priority`] are essential: Size, what the
    /// media is and Duration. To keep them Name may go below `NAME_MIN`, as
    /// far as [`name_floor`]; the rest only join beside a `NAME_MIN` Name.
    const ESSENTIAL: usize = 3;

    /// The column that shows what `key` sorts by, if one does.
    fn for_sort(key: SortKey) -> Option<Column> {
        Column::IN_ORDER.into_iter().find(|c| c.sort_key() == key)
    }

    /// Whether it says something about media, so it is blank for anything
    /// that is not media.
    fn media(self) -> bool {
        matches!(self, Column::Video | Column::Audio | Column::Duration)
    }

    fn title(self) -> &'static str {
        match self {
            Column::Video => "Video",
            Column::Audio => "Audio",
            Column::Duration => "Duration",
            Column::Size => "Size",
            Column::Modified => "Date Modified",
        }
    }

    /// Fits the widest value the column shows ("1440p ProRes", "5.1 E-AC-3
    /// +2", "12 Nov 2026 at 14:32").
    fn width(self) -> Pixels {
        match self {
            Column::Video => size::COLUMN_VIDEO,
            Column::Audio => size::COLUMN_AUDIO,
            Column::Duration => size::COLUMN_DURATION,
            Column::Size => size::COLUMN_SIZE,
            Column::Modified => size::COLUMN_MODIFIED,
        }
    }

    fn numeric(self) -> bool {
        matches!(self, Column::Duration | Column::Size)
    }

    /// Every column sorts. Video orders by height then codec, Audio by codec
    /// then channels.
    fn sort_key(self) -> SortKey {
        match self {
            Column::Video => SortKey::Video,
            Column::Audio => SortKey::Audio,
            Column::Duration => SortKey::Duration,
            Column::Size => SortKey::Size,
            Column::Modified => SortKey::Modified,
        }
    }

    /// Space before this column: wider when a left-aligned column follows
    /// a right-aligned one, so "459 KB" and "Today at 17:22" don't read as
    /// one phrase.
    fn gap_after(self, previous: Option<Column>) -> Pixels {
        match previous {
            Some(previous) if previous.numeric() && !self.numeric() => size::COLUMN_GAP_WIDE,
            _ => size::COLUMN_GAP,
        }
    }
}

/// Width `columns` take beside Name, gaps included.
fn columns_width(columns: &[Column]) -> Pixels {
    let mut previous = None;
    let mut width = px(0.);
    for &column in columns {
        width += column.gap_after(previous) + column.width();
        previous = Some(column);
    }
    width
}

/// Narrowest Name gets so that the essential columns stay: what the
/// narrowest list the workspace gives (the smallest window beside the
/// narrowest inspector) leaves beside Video, Duration and Size. 171pt, so a
/// 760pt window with the inspector open still compares sizes.
fn name_floor() -> Pixels {
    let list = size::WINDOW_MIN.0 - size::INSPECTOR.0 - size::HAIRLINE;
    list - size::ROW_INSET * 4. - columns_width(&[Column::Video, Column::Duration, Column::Size])
}

/// The narrowest list that shows the folder's five columns: Name at
/// `NAME_MIN` beside its four leading columns. In a mixed folder that is
/// Video, Audio, Duration and Size, 648pt; in a folder of audio, where
/// Video never shows, Audio, Duration, Size and Date Modified, 692pt. The
/// workspace narrows the panes toward their minimums to give the list this
/// before it settles for four columns (decision 58, F3-2, F4-3).
pub fn list_comfort(audio_folder: bool) -> Pixels {
    let five = &Column::by_priority(audio_folder)[..4];
    let columns: Vec<Column> = Column::IN_ORDER
        .into_iter()
        .filter(|c| five.contains(c))
        .collect();
    size::ROW_INSET * 4. + size::NAME_MIN + columns_width(&columns)
}

/// The narrowest list that still shows the four essentials (Name at its
/// floor beside Video, Duration and Size), 499pt: what the smallest window
/// leaves beside the narrowest inspector. The workspace keeps the sidebar
/// docked, at its narrowest beside the narrowest inspector, as long as the
/// list keeps this (S4-4).
pub fn list_essential() -> Pixels {
    use Column::*;
    size::ROW_INSET * 4. + name_floor() + columns_width(&[Video, Duration, Size])
}

/// Name's width and the columns beside it in a list `width` wide, left to
/// right. Name gives way first, down to `NAME_MIN`; only then do columns go.
/// The essential ones (Size, the primary media column, Duration) come
/// first under every sort and stay while Name keeps [`name_floor`]. Each
/// other column then joins, the one the list is `sorted` by first, if it
/// fits beside a `NAME_MIN` Name; one that does not is passed over, not
/// waited for, so a narrower one behind it can still join. A sorted column
/// that cannot fit folds its header, and the status bar names the sort
/// instead (decision 58). Name is never wider than it `wanted`: slack goes
/// after the last column.
fn fit_columns(
    width: Pixels,
    wanted: Pixels,
    audio_folder: bool,
    sorted: Option<Column>,
) -> (Pixels, Vec<Column>) {
    // The row's outer and inner insets on both sides.
    let room = width - size::ROW_INSET * 4.;
    let by_priority = Column::by_priority(audio_folder);
    let (essential, rest) = by_priority.split_at(Column::ESSENTIAL);
    // The sorted column leads its group: among the essentials it is the
    // last to go; among the rest it is the first to join.
    let lead = |group: &[Column]| {
        let mut group = group.to_vec();
        if let Some(at) = sorted.and_then(|sorted| group.iter().position(|c| *c == sorted)) {
            let column = group.remove(at);
            group.insert(0, column);
        }
        group
    };
    let in_order = |kept: &[Column]| -> Vec<Column> {
        Column::IN_ORDER
            .into_iter()
            .filter(|c| kept.contains(c))
            .collect()
    };
    let essential = lead(essential);
    let mut kept: Vec<Column> = (0..=essential.len())
        .rev()
        .map(|n| essential[..n].to_vec())
        .find(|kept| columns_width(&in_order(kept)) + name_floor() <= room)
        .unwrap_or_default();
    if kept.len() == essential.len() {
        for column in lead(rest) {
            kept.push(column);
            if columns_width(&in_order(&kept)) + size::NAME_MIN > room {
                kept.pop();
            }
        }
    }
    let fit = in_order(&kept);
    let name = wanted.min(room - columns_width(&fit)).max(px(0.));
    (name, fit)
}

/// The longest names, by characters, measured to size the Name column. The
/// widest name is among them, and a large folder stays cheap to measure.
const NAMES_MEASURED: usize = 64;
/// Most items of one transfer drawn at once.
const TRANSFER_ROWS: usize = 50;
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
    /// A right-click landed on a row, not on the space below the rows.
    row_pressed: bool,
    /// A selected row that was pressed. The rest of the selection is let go
    /// on release, unless the press turns into a drag of all of it.
    collapse_on_release: Option<usize>,
    /// The row whose name is being edited, and its text field.
    renaming: Option<(usize, Entity<InputState>, Subscription)>,
    /// A new item to select and start renaming once it is listed.
    rename_when_listed: Option<PathBuf>,
    scroll: UniformListScrollHandle,
    /// The width the list was last laid out at, measured, so the columns
    /// follow the room the list is actually given.
    width: Rc<Cell<Pixels>>,
    /// The columns shown in this frame, shared by the header and the rows.
    visible: Vec<Column>,
    /// Name's width this frame, shared by the header and the rows.
    name_width: Pixels,
    /// What Name would like to be: the widest name with its icon, plus
    /// room. Measured once per listing; `None` until then.
    name_wanted: Option<Pixels>,
    /// The context menu last opened on the list. While it has the keyboard
    /// the rows it acts on keep their focused selection colour.
    menu: Option<WeakEntity<PopupMenu>>,
    /// The same menu opened from the keyboard (Shift+F10, the Menu key,
    /// ⌃↩), under the lead row.
    key_menu: ui::KeyMenu,
    /// Where the lead row and the list were drawn last frame, for placing
    /// [`Browser::key_menu`]. The lead row's is `None` while it is scrolled
    /// out of view.
    lead_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    list_bounds: Rc<Cell<Bounds<Pixels>>>,
    focus: FocusHandle,
    /// The status bar's transfer trigger, so dismissing the last outcome
    /// can tell whether focus is about to go with it.
    activity_focus: FocusHandle,
    library: Entity<Library>,
    devices: Entity<Devices>,
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
        cx.observe(&library, |this: &mut Browser, _, cx| {
            // Probe results may move rows when the list is sorted by them.
            if this.shown_sort().0.needs_media() {
                this.reorder(false, cx);
            }
            cx.notify();
        })
        .detach();
        let devices = Devices::global(cx);
        cx.observe(&devices, |_, _, cx| cx.notify()).detach();
        // The panes beside the list changed width: refit the columns in the
        // same frame, not after the next layout is measured.
        cx.observe_global::<SidePanes>(|_, cx| cx.notify()).detach();
        cx.subscribe(&devices, |this, _, event: &DevicesEvent, cx| match event {
            // The device being shown may have just left, or come back.
            DevicesEvent::Changed => {
                if mtp::is_device_path(&this.dir) {
                    this.reload(cx);
                }
            }
            // A failure stays in the status bar's activity until it is
            // dismissed there, so it is not repeated as a notice.
            DevicesEvent::Finished => this.reload(cx),
        })
        .detach();
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
            row_pressed: false,
            collapse_on_release: None,
            renaming: None,
            rename_when_listed: None,
            scroll: UniformListScrollHandle::new(),
            width: Rc::new(Cell::new(px(0.))),
            visible: Vec::new(),
            name_width: size::NAME_MIN,
            name_wanted: None,
            menu: None,
            key_menu: ui::KeyMenu::default(),
            lead_bounds: Rc::new(Cell::new(None)),
            list_bounds: Rc::new(Cell::new(Bounds::default())),
            focus: cx.focus_handle(),
            activity_focus: cx.focus_handle().tab_stop(true),
            library,
            devices,
            load: None,
            watch: None,
        };
        this.show(dir, cx);
        this
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The folder's items as listed (hidden ones only when shown), ignoring
    /// any search; `None` until the listing arrives, or if the folder could
    /// not be read.
    pub fn listing(&self) -> Option<Vec<Entry>> {
        (self.loaded && self.error.is_none()).then(|| {
            self.all
                .iter()
                .filter(|e| self.show_hidden || !e.hidden)
                .cloned()
                .collect()
        })
    }

    /// Whether the folder could not be read; the list says why.
    pub fn unreadable(&self) -> bool {
        self.loaded && self.error.is_some()
    }

    /// The order the list is in, as its header and the View menu show it:
    /// the one asked for, except that a Video sort in a folder that has no
    /// video (where Video is never a column) shows as Name, A to Z. The
    /// choice itself is kept, see [`Browser::saved_sort`].
    pub fn sort(&self) -> (SortKey, bool) {
        self.shown_sort()
    }

    /// The order the person chose, to remember between launches. It can
    /// differ from [`Browser::sort`] in a folder with no video.
    pub fn saved_sort(&self) -> (SortKey, bool) {
        self.sort
    }

    /// Whether a search is narrowing the rows: the list shows matches, not
    /// the whole folder.
    pub fn filtered(&self) -> bool {
        !self.query.is_empty()
    }

    /// A folder of audio and no video: Audio takes Video's place, and the
    /// workspace fits the panes to its columns ([`list_comfort`]).
    pub fn audio_folder(&self) -> bool {
        let has = |kind: Kind| self.all.iter().any(|e| e.kind == kind);
        has(Kind::Audio) && !has(Kind::Video)
    }

    /// See [`Browser::sort`]: never a sort by a column the folder cannot
    /// show at any width.
    fn shown_sort(&self) -> (SortKey, bool) {
        match self.sort.0 == SortKey::Video && self.audio_folder() {
            true => (SortKey::Name, true),
            false => self.sort,
        }
    }

    pub fn show_hidden(&self) -> bool {
        self.show_hidden
    }

    /// Explains in the status bar why a command did nothing.
    pub fn say(&mut self, notice: &'static str, cx: &mut Context<Self>) {
        self.notice = Some(notice.into());
        cx.notify();
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
            let listing = cx.background_spawn(async move { list(&dir) }).await;
            this.update(cx, |this, cx| {
                this.loaded = true;
                this.name_wanted = None;
                match listing {
                    Ok(entries) => {
                        this.all = entries;
                        this.error = None;
                    }
                    Err(message) => {
                        this.all.clear();
                        this.error = Some(message.into());
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
        let kept = self.kept_selection(true);
        self.rows = self
            .all
            .iter()
            .filter(|e| self.show_hidden || !e.hidden)
            .filter(|e| self.query.is_empty() || e.name.to_lowercase().contains(&self.query))
            .cloned()
            .collect();
        self.sort_rows(cx);
        self.carry_selection(kept, true, cx);
        // The same rows may now describe changed files.
        if !self.selected.is_empty() {
            cx.emit(BrowserEvent::SelectionChanged);
        }

        let rows = self.rows.clone();
        self.library
            .update(cx, |library, cx| library.scan(&rows, cx));
        cx.emit(BrowserEvent::Listed);
        cx.notify();
    }

    /// Sorts the rows again, as when probes arrive under a media sort,
    /// keeping the selection on the same files. `scroll` brings the lead
    /// row into view; a sort the person did not ask for leaves the scroll
    /// where it is.
    fn reorder(&mut self, scroll: bool, cx: &mut Context<Self>) {
        let before: Vec<PathBuf> = self.rows.iter().map(|e| e.path.clone()).collect();
        let kept = self.kept_selection(false);
        self.sort_rows(cx);
        if self.rows.iter().map(|e| &e.path).ne(before.iter()) {
            self.carry_selection(kept, scroll, cx);
        }
    }

    fn sort_rows(&mut self, cx: &App) {
        let library = self.library.read(cx);
        let (key, ascending) = self.shown_sort();
        fs::sort_with(&mut self.rows, key, ascending, |path| {
            library.info(path).map(|info| info.as_ref())
        });
    }

    /// The files selected now, and the one the keyboard is on, to find again
    /// after the rows change. A new listing selects what `reselect` names
    /// instead.
    fn kept_selection(&mut self, listing: bool) -> (HashSet<PathBuf>, Option<PathBuf>) {
        let reselect = match listing {
            true => self.reselect.take(),
            false => None,
        };
        let kept: HashSet<PathBuf> = match reselect {
            Some(path) => HashSet::from([path]),
            None => self.selection().into_iter().map(|e| e.path).collect(),
        };
        let lead = self
            .lead
            .and_then(|i| self.rows.get(i))
            .map(|e| e.path.clone());
        (kept, lead)
    }

    /// Selects `kept` again in the rows as they now stand.
    fn carry_selection(
        &mut self,
        (kept, lead): (HashSet<PathBuf>, Option<PathBuf>),
        scroll: bool,
        cx: &mut Context<Self>,
    ) {
        // Row indexes are about to change under any open name field.
        self.renaming = None;
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
        if let Some(lead) = lead
            && scroll
        {
            self.scroll.scroll_to_item(lead, ScrollStrategy::Nearest);
        }
        self.anchor = lead;
        self.set_selection(selected, lead, cx);
    }

    /// Sorts by `key`; again reverses the order on screen. A new key starts
    /// where a person would look first: names and audio codecs A to Z,
    /// everything else largest, longest or newest first. Video in a folder
    /// with no video says why nothing moved rather than sorting out of sight.
    pub fn set_sort(&mut self, key: SortKey, cx: &mut Context<Self>) {
        if key == SortKey::Video && self.audio_folder() {
            self.say("There’s no video here to sort by.", cx);
            return;
        }
        let (shown, ascending) = self.shown_sort();
        self.sort = if shown == key {
            (key, !ascending)
        } else {
            (key, matches!(key, SortKey::Name | SortKey::Audio))
        };
        self.reorder(true, cx);
        cx.notify();
    }

    /// Clears the search from the list's own Clear Search button. The
    /// search field is the workspace's, and `Navigated` is what empties it
    /// (and resets the folder summary to its loading state); so that is
    /// said first, and the `Listed` that clearing the query sends comes
    /// after it and leaves the summary complete again, as clearing the
    /// field by Escape, its clear button or deleting the text does
    /// (decision 60, A3-1).
    fn clear_search(&mut self, cx: &mut Context<Self>) {
        cx.emit(BrowserEvent::Navigated);
        self.set_query("", cx);
    }

    /// Shows only the items whose names contain `query`. A change sends
    /// `Listed`, so whatever summarises the folder hears of it however the
    /// search was cleared.
    pub fn set_query(&mut self, query: &str, cx: &mut Context<Self>) {
        let query = query.trim().to_lowercase();
        if query != self.query {
            self.query = query;
            self.rebuild(cx);
        }
    }

    pub fn toggle_hidden(&mut self, cx: &mut Context<Self>) {
        self.show_hidden = !self.show_hidden;
        self.name_wanted = None;
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
        } else if self.selected.len() > 1 && self.selected.contains(&index) {
            self.collapse_on_release = Some(index);
        } else {
            self.select_only(index, cx);
        }
    }

    /// Rows dropped on a folder in the list go into it.
    fn drop_on_row(&mut self, index: usize, dragged: &Dragged, cx: &mut Context<Self>) {
        let Some(folder) = self.rows.get(index).map(|e| e.path.clone()) else {
            return;
        };
        if dragged.0.contains(&folder) {
            return;
        }
        self.notice = None;
        let (sources, mode) = (dragged.0.to_vec(), dragged.mode(&folder));
        self.devices.update(cx, |devices, cx| {
            devices.transfer(sources, folder, mode, cx)
        });
        cx.notify();
    }

    /// A right-click acts on the selection if the row is part of it, and on
    /// that row alone if it is not.
    fn secondary_click_row(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_rename(true, window, cx);
        window.focus(&self.focus, cx);
        self.row_pressed = true;
        if !self.selected.contains(&index) {
            self.select_only(index, cx);
        }
    }

    /// The commands for what was right-clicked: the selection, or the folder
    /// itself when nothing is selected. Each is also in the menu bar.
    fn context_menu(&self, menu: PopupMenu, cx: &App) -> PopupMenu {
        let menu = menu.action_context(self.focus.clone());
        let on_device = mtp::is_device_path(&self.dir);
        let copied = self.devices.read(cx).clipboard().len();
        let paste = match copied {
            0 | 1 => "Paste Item".to_owned(),
            n => format!("Paste {n} Items"),
        };
        let move_here = match copied {
            0 | 1 => "Move Item Here".to_owned(),
            n => format!("Move {n} Items Here"),
        };
        let reveal = platform::label(Command::Reveal);
        let selection = self.selection();
        let Some(first) = selection.first() else {
            return menu
                .item(ui::menu_item(
                    platform::label(Command::NewFolder),
                    Box::new(NewFolder),
                    Some(Command::NewFolder),
                    false,
                ))
                .separator()
                .item(ui::menu_item(
                    paste,
                    Box::new(Paste),
                    Some(Command::Paste),
                    copied == 0,
                ))
                .item(ui::menu_item(
                    move_here,
                    Box::new(MoveItemHere),
                    Some(Command::MoveItemHere),
                    copied == 0,
                ))
                .separator()
                .item(ui::menu_item(
                    reveal,
                    Box::new(RevealInFinder),
                    Some(Command::Reveal),
                    on_device,
                ))
                .item(ui::menu_item(
                    platform::label(Command::CleanUpStreamNames),
                    Box::new(CleanUpStreamNames),
                    Some(Command::CleanUpStreamNames),
                    on_device
                        || self.error.is_some()
                        || !crate::cleanup::worth_reviewing(&self.all),
                ))
                .separator()
                .item(
                    ui::menu_item(
                        "Show Hidden Files",
                        Box::new(ToggleHiddenFiles),
                        Some(Command::ToggleHiddenFiles),
                        false,
                    )
                    .checked(self.show_hidden),
                );
        };
        let single = selection.len() == 1;
        let media = single && first.kind.is_probeable() && !on_device;
        let copy = match selection.len() {
            1 => "Copy".to_owned(),
            n => format!("Copy {n} Items"),
        };
        menu.item(ui::menu_item(
            platform::label(Command::OpenSelection),
            Box::new(OpenSelection),
            Some(Command::OpenSelection),
            !single,
        ))
        .separator()
        .item(ui::menu_item(
            copy,
            Box::new(Copy),
            Some(Command::Copy),
            false,
        ))
        .item(ui::menu_item(
            paste,
            Box::new(Paste),
            Some(Command::Paste),
            copied == 0,
        ))
        .item(ui::menu_item(
            move_here,
            Box::new(MoveItemHere),
            Some(Command::MoveItemHere),
            copied == 0,
        ))
        .separator()
        .item(ui::menu_item(
            platform::label(Command::Rename),
            Box::new(Rename),
            Some(Command::Rename),
            !single,
        ))
        .item(ui::menu_item(
            reveal,
            Box::new(RevealInFinder),
            Some(Command::Reveal),
            on_device,
        ))
        .item(ui::menu_item(
            platform::label(Command::AnalyzeBitrate),
            Box::new(AnalyzeBitrate),
            Some(Command::AnalyzeBitrate),
            !media,
        ))
        .separator()
        .item(ui::menu_item(
            match on_device {
                true => "Delete…",
                false => platform::label(Command::MoveToTrash),
            },
            Box::new(MoveToTrash),
            Some(Command::MoveToTrash),
            false,
        ))
    }

    /// Opens the context menu from the keyboard (Shift+F10, the Menu key,
    /// ⌃↩): for the selection, under the lead row's name, or for the folder
    /// at the top of the list when nothing is selected. The same menu a
    /// right-click opens; Escape gives the keyboard back to the list.
    fn show_context_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.renaming.is_some() || !self.loaded || self.error.is_some() {
            return;
        }
        let lead = self.lead.filter(|i| self.selected.contains(i));
        // A lead row scrolled out of view comes back first, and the menu
        // opens under it once it is drawn.
        if let Some(lead) = lead
            && self.lead_bounds.get().is_none()
        {
            self.scroll.scroll_to_item(lead, ScrollStrategy::Center);
            cx.notify();
            cx.on_next_frame(window, |this, window, cx| {
                if this.lead_bounds.get().is_some() {
                    this.open_key_menu(window, cx);
                }
            });
            return;
        }
        self.open_key_menu(window, cx);
    }

    fn open_key_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let list = self.list_bounds.get();
        let lead = self.lead.filter(|i| self.selected.contains(i));
        let at = match (lead, self.lead_bounds.get()) {
            (Some(_), Some(row)) => point(
                row.left() + size::ROW_INSET * 2. + size::ICON + size::GAP,
                row.bottom(),
            ),
            _ => point(
                list.left() + size::ROW_INSET * 2.,
                list.top() + size::HEADER_ROW,
            ),
        };
        let mut key_menu = std::mem::take(&mut self.key_menu);
        key_menu.open(at, window, cx, |menu, _, cx| self.context_menu(menu, cx));
        // Its rows keep the focused colour while it is open, as with a
        // right-click.
        self.menu = key_menu.menu().map(|menu| menu.downgrade());
        self.key_menu = key_menu;
        cx.notify();
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
        } else if mtp::is_device_path(&entry.path) {
            self.notice = Some("Copy it to this computer to open it.".into());
            cx.notify();
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

    fn trash_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        if mtp::is_device_path(&self.dir) {
            // A device has no trash, so this one asks first.
            let what = match paths.as_slice() {
                [only] => format!("“{}”", folder_name(only)),
                many => format!("{} items", many.len()),
            };
            let answer = window.prompt(
                PromptLevel::Warning,
                &format!("Delete {what} from the device?"),
                Some(&format!(
                    "A device has no {}. This can’t be undone.",
                    platform::trash()
                )),
                &["Delete", "Cancel"],
                cx,
            );
            cx.spawn(async move |this, cx| {
                if answer.await != Ok(0) {
                    return;
                }
                let result = cx
                    .background_spawn(async move {
                        mtp::delete(&paths, &std::sync::atomic::AtomicBool::new(false))
                    })
                    .await;
                this.update(cx, |this, cx| {
                    this.notice = result.err().map(|e| format!("{e:#}").into());
                    this.set_selection(HashSet::new(), next, cx);
                    this.reload(cx);
                })
                .ok();
            })
            .detach();
            return;
        }
        cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move { fs::trash(&paths) }).await;
            this.update(cx, |this, cx| {
                this.notice = result
                    .err()
                    .map(|e| format!("Nothing was moved to the {}. {e}", platform::trash()).into());
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
        let on_device = mtp::is_device_path(path);
        let taken = match on_device {
            true => self.all.iter().any(|e| e.name.eq_ignore_ascii_case(name)),
            false => target.exists(),
        };
        if name.contains('/') || name.contains(':') {
            self.notice =
                Some(format!("“{old}” was not renamed. Names can’t contain “/” or “:”.").into());
        } else if taken {
            self.notice = Some(
                format!("“{old}” was not renamed. The name “{name}” is already taken.").into(),
            );
        } else {
            let (from, to, name) = (path.to_owned(), target.clone(), name.to_owned());
            cx.spawn(async move |this, cx| {
                let result = cx
                    .background_spawn(async move {
                        match on_device {
                            true => mtp::rename(&from, &name).map_err(|e| format!("{e:#}")),
                            false => std::fs::rename(&from, &to).map_err(|e| e.to_string()),
                        }
                    })
                    .await;
                this.update(cx, |this, cx| {
                    match result {
                        Ok(()) => {
                            this.notice = None;
                            this.reselect = Some(target);
                            this.reload(cx);
                        }
                        Err(message) => {
                            this.notice = Some(format!("“{old}” was not renamed. {message}").into())
                        }
                    }
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        cx.notify();
    }

    /// Creates "untitled folder" here and starts renaming it.
    fn new_folder(&mut self, cx: &mut Context<Self>) {
        let on_device = mtp::is_device_path(&self.dir);
        let taken = |name: &str| match on_device {
            true => self.all.iter().any(|e| e.name.eq_ignore_ascii_case(name)),
            false => self.dir.join(name).exists(),
        };
        let mut name = "untitled folder".to_owned();
        let mut n = 2;
        while taken(&name) {
            name = format!("untitled folder {n}");
            n += 1;
        }
        let path = self.dir.join(name);
        cx.spawn(async move |this, cx| {
            let made = path.clone();
            let result = cx
                .background_spawn(async move {
                    match on_device {
                        true => mtp::create_dir(&made).map_err(|e| format!("{e:#}")),
                        false => std::fs::create_dir(&made).map_err(|e| explain(&e)),
                    }
                })
                .await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        this.notice = None;
                        this.reselect = Some(path.clone());
                        this.rename_when_listed = Some(path);
                        this.reload(cx);
                    }
                    Err(message) => {
                        this.notice = Some(format!("The folder was not created. {message}").into())
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    // Copying and moving

    /// Remembers the selection for Paste, as Copy does in Finder.
    fn copy_selection(&mut self, cx: &mut Context<Self>) {
        let paths: Vec<PathBuf> = self.selection().into_iter().map(|e| e.path).collect();
        if !paths.is_empty() {
            self.devices.update(cx, |devices, _| devices.copy(paths));
        }
    }

    /// Copies or moves what Copy remembered into this folder.
    fn paste(&mut self, mode: Mode, cx: &mut Context<Self>) {
        let sources = self.devices.read(cx).clipboard().to_vec();
        if sources.is_empty() {
            let how = match platform::shortcut(Command::Copy) {
                Some(keys) => format!("press {keys}"),
                None => "choose Copy".to_owned(),
            };
            self.notice = Some(format!("Nothing to paste. Select items and {how} first.").into());
            cx.notify();
            return;
        }
        self.receive(sources, mode, cx);
    }

    fn receive(&mut self, sources: Vec<PathBuf>, mode: Mode, cx: &mut Context<Self>) {
        self.notice = None;
        let dest = self.dir.clone();
        self.devices
            .update(cx, |devices, cx| devices.transfer(sources, dest, mode, cx));
        cx.notify();
    }

    // Rendering

    /// Name's width and the columns that fit beside it at `width`; see
    /// [`fit_columns`].
    fn fit_columns(&self, width: Pixels) -> (Pixels, Vec<Column>) {
        let wanted = self.name_wanted.unwrap_or(size::NAME_MIN);
        let sorted = Column::for_sort(self.shown_sort().0);
        fit_columns(width, wanted, self.audio_folder(), sorted)
    }

    /// Measures what Name wants for this listing, once: the widest name
    /// with its icon, plus `NAME_PADDING`, within `NAME_MIN`..`NAME_MAX`.
    /// Names hidden by a search count, so the columns hold still while
    /// someone types.
    fn measure_names(&mut self, window: &Window) {
        if self.name_wanted.is_some() || !self.loaded {
            return;
        }
        let mut names: Vec<&str> = self
            .all
            .iter()
            .filter(|e| self.show_hidden || !e.hidden)
            .map(|e| e.name.as_str())
            .collect();
        if names.len() > NAMES_MEASURED {
            names.select_nth_unstable_by_key(NAMES_MEASURED, |name| {
                std::cmp::Reverse(name.chars().count())
            });
            names.truncate(NAMES_MEASURED);
        }
        let widest = names.into_iter().fold(px(0.), |widest, name| {
            let name = SharedString::from(name.to_owned());
            widest.max(ui::text_width(
                &name,
                text::BODY,
                FontWeight::NORMAL,
                window,
            ))
        });
        let wanted = size::ICON + size::GAP + widest + size::NAME_PADDING;
        self.name_wanted = Some(wanted.max(size::NAME_MIN).min(size::NAME_MAX));
    }

    /// The width the list has: measured at the last layout, or, before the
    /// first one, the window less the panes beside the list.
    fn list_width(&self, window: &Window, cx: &App) -> Pixels {
        // What the workspace says is beside the list is exact and current;
        // the measured width is a frame late, so it is only the fallback.
        match cx.try_global::<SidePanes>() {
            Some(side) => window.viewport_size().width - side.0,
            None => match self.width.get() {
                width if width > px(0.) => width,
                _ => window.viewport_size().width,
            },
        }
    }

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::of(cx).clone();
        let (key, ascending) = self.shown_sort();
        // Every title sorts and fills on hover; the fill reaches past the
        // text by its padding, so the text stays aligned with the cells below.
        let title = |id: &'static str, label: &'static str, numeric: bool, sort: SortKey| {
            let sorted = sort == key;
            let arrow = || {
                sorted.then(|| {
                    ui::icon(
                        match ascending {
                            true => IconName::ChevronUp,
                            false => IconName::ChevronDown,
                        },
                        t.secondary_label,
                    )
                    .size(size::ICON_SMALL)
                })
            };
            let group: SharedString = format!("header-{id}").into();
            let hover = t.fill_hover;
            div()
                .id(id)
                .group(group.clone())
                .flex()
                .items_center()
                .h_full()
                .when(numeric, |this| this.justify_end())
                .on_click(cx.listener(move |this, _, _, cx| this.set_sort(sort, cx)))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(size::GAP_TINY)
                        .min_w_0()
                        .px(size::GAP_SMALL)
                        .mx(-size::GAP_SMALL)
                        .py(size::GAP_TINY)
                        .rounded(size::RADIUS_INLINE)
                        .text_color(if sorted { t.label } else { t.secondary_label })
                        .when(sorted, |this| this.font_weight(FontWeight::MEDIUM))
                        .group_hover(group, move |s| s.bg(hover))
                        // Numeric columns put the arrow on their leading
                        // side, so the title stays over its values.
                        .when(numeric, |this| this.children(arrow()))
                        .child(div().min_w_0().truncate().child(label))
                        .when(!numeric, |this| this.children(arrow())),
                )
        };
        let mut previous = None;
        let cells: Vec<_> = self
            .visible
            .iter()
            .map(|&column| {
                let gap = column.gap_after(previous);
                previous = Some(column);
                title(
                    column.title(),
                    column.title(),
                    column.numeric(),
                    column.sort_key(),
                )
                .flex_none()
                .w(column.width())
                .ml(gap)
            })
            .collect();
        div()
            .flex()
            .flex_none()
            .items_center()
            .h(size::HEADER_ROW)
            .px(size::ROW_INSET * 2.)
            .bg(t.surface_raised)
            .border_b(size::HAIRLINE)
            .border_color(t.separator)
            .text_size(text::SMALL)
            .line_height(text::SMALL_LINE)
            .child(
                title("Name", "Name", false, SortKey::Name)
                    .w(self.name_width)
                    .min_w_0(),
            )
            .children(cells)
    }

    fn render_rows(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<impl IntoElement + use<>> {
        let t = Theme::of(cx).clone();
        let columns = self.visible.clone();
        let name_width = self.name_width;
        // The first media column says "Unreadable" for a file FFmpeg could
        // not read; the others stay blank.
        let first_media = columns.iter().copied().find(|c| c.media());
        let (count, lead) = (self.rows.len(), self.lead);
        let lead_bounds = Rc::clone(&self.lead_bounds);
        // A context menu opened on the list acts on its selection, which so
        // keeps the focused colour rather than looking let go.
        let menu_open = self
            .menu
            .as_ref()
            .and_then(WeakEntity::upgrade)
            .is_some_and(|menu| menu.focus_handle(cx).contains_focused(window, cx));
        let focused = (self.focus.is_focused(window) || menu_open) && window.is_window_active();
        let library = self.library.read(cx);
        // Dragging a selected row takes the whole selection along.
        let selection = Rc::new(self.selection().into_iter().map(|e| e.path).collect());
        let browser = cx.entity().downgrade();
        let drop_fill = t.selection_inactive;
        let (fill, strong_on, weak_on) = match focused {
            true => (t.selection, t.on_selection, t.on_selection_secondary),
            false => (
                t.selection_inactive,
                t.on_selection_inactive,
                t.on_selection_inactive_secondary,
            ),
        };

        range
            .filter_map(|index| {
                let entry = self.rows.get(index)?;
                let selected = self.selected.contains(&index);
                // A run of selected rows is one block: only its ends are round.
                let above = index > 0 && self.selected.contains(&(index - 1));
                let below = self.selected.contains(&(index + 1));
                let dragged = Dragged(match selected {
                    true => Rc::clone(&selection),
                    false => Rc::new(vec![entry.path.clone()]),
                });
                let browser = browser.clone();
                let (strong, weak, dash, warning) = match selected {
                    true => (strong_on, weak_on, weak_on, strong_on),
                    false => (t.label, t.secondary_label, t.tertiary_label, t.orange),
                };
                let probe = library.probe(&entry.path);
                // What a screen reader says after the name: each column
                // shown with its value ("Video 1080p H.264, Size 459 MB").
                let spoken: Vec<String> = columns
                    .iter()
                    .filter_map(|&column| {
                        value(column, entry, probe, first_media == Some(column))
                            .spoken()
                            .map(|value| format!("{} {value}", column.title()))
                    })
                    .collect();
                let is_lead = lead == Some(index);

                let mut previous = None;
                let cells: Vec<_> = columns
                    .iter()
                    .map(|&column| {
                        let gap = column.gap_after(previous);
                        previous = Some(column);
                        let cell = div()
                            .flex_none()
                            .w(column.width())
                            .ml(gap)
                            .truncate()
                            .when(column.numeric(), |this| this.text_right())
                            .text_color(weak);
                        match value(column, entry, probe, first_media == Some(column)) {
                            // Said in words, with why in its tooltip.
                            Value::Unreadable => cell
                                .id(("unreadable", index))
                                .text_color(warning)
                                .tip(UNREADABLE, None)
                                .child("Unreadable")
                                .into_any_element(),
                            value => cell.child(value.render(strong, dash)).into_any_element(),
                        }
                    })
                    .collect();

                Some(
                    div()
                        // By path, so a row keeps its identity through a
                        // re-sort or a search.
                        .id(ElementId::Name(
                            entry.path.to_string_lossy().into_owned().into(),
                        ))
                        .relative()
                        // One option of the list: its full name, where it
                        // is, whether it is selected; the lead row is the
                        // one assistive technology reports as focused.
                        .role(Role::ListBoxOption)
                        .aria_label(entry.name.clone())
                        .aria_selected(selected)
                        .aria_position_in_set(index + 1)
                        .aria_size_of_set(count)
                        .when(!spoken.is_empty(), |this| {
                            this.aria_description(spoken.join(", "))
                        })
                        .when(is_lead, |this| {
                            let bounds = Rc::clone(&lead_bounds);
                            this.aria_active_descendant().child(
                                canvas(move |row, _, _| bounds.set(Some(row)), |_, _, _, _| {})
                                    .absolute()
                                    .inset_0(),
                            )
                        })
                        .h(size::ROW)
                        .px(size::ROW_INSET)
                        // A selected row shows no stripe, even in its margins.
                        .when(index % 2 == 1 && !selected, |this| this.bg(t.alternate_row))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                                this.click_row(index, event, window, cx);
                                cx.stop_propagation();
                            }),
                        )
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                if this.collapse_on_release.take() == Some(index) {
                                    this.select_only(index, cx);
                                }
                            }),
                        )
                        .on_drag(dragged, move |dragged, _, _, cx| {
                            browser
                                .update(cx, |this, _| this.collapse_on_release = None)
                                .ok();
                            cx.new(|_| dragged.clone())
                        })
                        .when(entry.is_dir(), |this| {
                            this.drag_over::<Dragged>(move |style, _, _, _| style.bg(drop_fill))
                                .on_drop(cx.listener(move |this, dragged: &Dragged, _, cx| {
                                    this.drop_on_row(index, dragged, cx);
                                }))
                        })
                        // Not stopped: the press goes on to open the menu.
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |this, _, window, cx| {
                                this.secondary_click_row(index, window, cx);
                            }),
                        )
                        .child(
                            div()
                                .size_full()
                                .flex()
                                .items_center()
                                .px(size::ROW_INSET)
                                .when(selected, |this| {
                                    let radius = size::RADIUS_CONTROL;
                                    this.bg(fill)
                                        .when(!above, |this| this.rounded_t(radius))
                                        .when(!below, |this| this.rounded_b(radius))
                                })
                                .font_features(theme::tabular())
                                .child(
                                    div()
                                        .w(name_width)
                                        .min_w_0()
                                        .flex()
                                        .items_center()
                                        .gap(size::GAP)
                                        // Seer's own glyph for the kind, the
                                        // same on every OS (decision 56).
                                        .child(ui::kind_glyph_in(
                                            glyph(entry),
                                            selected.then_some(strong),
                                            cx,
                                        ))
                                        .child(match &self.renaming {
                                            Some((row, field, _)) if *row == index => div()
                                                .flex_1()
                                                .min_w_0()
                                                .text_color(t.label)
                                                // Clicks in the field edit text; they don't reselect the row.
                                                .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                                    cx.stop_propagation()
                                                })
                                                .child(ui::text_field(field, window, cx).small()),
                                            _ => div()
                                                .min_w_0()
                                                // Hidden items are dimmed, as in the file manager.
                                                .text_color(if entry.hidden {
                                                    weak
                                                } else {
                                                    strong
                                                })
                                                .whitespace_nowrap()
                                                .text_ellipsis_middle()
                                                .child(entry.name.clone()),
                                        }),
                                )
                                .children(cells),
                        ),
                )
            })
            .collect()
    }

    /// The count or the selection at the leading edge; why the last command
    /// did nothing after it; running or just-finished work at the trailing
    /// edge. Never the path: the titlebar says where this is.
    fn render_status(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::of(cx).clone();
        let jobs = self.library.read(cx).running_jobs();
        let activity = activity(cx);
        let bytes: u64 = self.selection().iter().map(|e| e.size).sum();
        let summary = status_summary(self.rows.len(), self.selected.len(), bytes, self.filtered());
        // A sort whose column has folded is still named, with its direction.
        let (key, ascending) = self.shown_sort();
        let cue = sort_cue(key, &self.visible)
            .filter(|_| !self.rows.is_empty())
            .map(|label| {
                div()
                    .flex()
                    .items_center()
                    .gap(size::GAP_TINY)
                    .flex_shrink(1.)
                    .min_w_0()
                    .child(div().min_w_0().truncate().child(format!(" · {label}")))
                    .child(
                        ui::icon(
                            match ascending {
                                true => IconName::ChevronUp,
                                false => IconName::ChevronDown,
                            },
                            t.secondary_label,
                        )
                        .size(size::ICON_SMALL)
                        .flex_none(),
                    )
            });
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(size::GAP)
            .h(size::STATUS_BAR)
            .px(size::ROW_INSET * 2.)
            .bg(t.surface_raised)
            .border_t(size::HAIRLINE)
            .border_color(t.separator)
            .text_size(text::SMALL)
            .line_height(text::SMALL_LINE)
            .text_color(t.secondary_label)
            .font_features(theme::tabular())
            // The cue follows the count as one phrase, and gives way
            // before it.
            .child(
                div()
                    .flex()
                    .items_center()
                    .min_w_0()
                    .child(div().flex_none().child(summary))
                    .children(cue),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(t.red)
                    .children(self.notice.clone()),
            )
            .when(jobs > 0, |this| {
                let label = match jobs {
                    1 => "1 job running".to_owned(),
                    n => format!("{n} jobs running"),
                };
                this.child(div().flex_none().child(label))
            })
            .when_some(activity, |this, (activity, unresolved)| {
                this.child(
                    div()
                        // Gives way, rather than pushing past the edge, when
                        // the list is narrow.
                        .flex_shrink(1.)
                        .min_w_0()
                        .max_w(size::TRANSFER_LABEL)
                        // The trigger's hover padding hangs past the edge,
                        // so its text ends where the status bar's text does.
                        .mr(-size::TRIGGER_PADDING)
                        .child(activity_trigger(
                            activity,
                            unresolved,
                            Focus {
                                trigger: self.activity_focus.clone(),
                                list: self.focus.clone(),
                            },
                            cx,
                        )),
                )
            })
    }

    /// What the main pane says when it has no rows to show, or `None` when
    /// it has (or is still reading the folder).
    fn empty_state(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if let Some(error) = &self.error {
            return Some(
                ui::EmptyState::new("This folder can’t be shown")
                    .glyph(IconName::CircleAlert)
                    .detail(error.clone())
                    .into_any_element(),
            );
        }
        if !self.loaded || !self.rows.is_empty() {
            return None;
        }
        let state = if self.filtered() {
            ui::EmptyState::new("No matches")
                .trace()
                .detail(format!(
                    "Nothing in “{}” has that in its name.",
                    folder_name(&self.dir)
                ))
                .action(
                    ui::Button::new("clear-search", "Clear Search").on_click(cx.listener(
                        |this, _, window, cx| {
                            // The button goes with the empty state; the
                            // keyboard lands on the rows it brings back.
                            window.focus(&this.focus, cx);
                            this.clear_search(cx);
                        },
                    )),
                )
        } else if let Some(incoming) = self.devices.read(cx).incoming(&self.dir) {
            // Not empty for long: say what is on its way. The inspector
            // says the same.
            ui::EmptyState::new(incoming.arriving())
                .glyph(IconName::CircleArrowDown)
                .detail("Each one appears in the list as it arrives.")
        } else if !self.all.is_empty() {
            ui::EmptyState::new("Only hidden items")
                .glyph(IconName::EyeOff)
                .detail("Everything in this folder is hidden.")
                .action(
                    ui::Button::new("show-hidden", "Show Hidden Files")
                        .tip("Show Hidden Files", Some(Command::ToggleHiddenFiles))
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_hidden(cx))),
                )
        } else {
            let state = ui::EmptyState::new("Folder is empty").trace();
            match mtp::is_device_path(&self.dir) {
                true => state.detail("Drop files here to copy them to the device."),
                false => state
                    .detail("Drop files here to copy them in, or open another folder.")
                    .action(
                        ui::Button::new("open-folder", "Open Folder…")
                            .tip("Open Folder…", Some(Command::GoToFolder))
                            .on_click(|_, window, cx| {
                                window.dispatch_action(Box::new(OpenFolder), cx)
                            }),
                    ),
            }
        };
        Some(state.into_any_element())
    }
}

impl Render for Browser {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::of(cx).clone();
        self.measure_names(window);
        (self.name_width, self.visible) = self.fit_columns(self.list_width(window, cx));

        if let Some(path) = &self.rename_when_listed
            && self.selection().first().is_some_and(|e| &e.path == path)
        {
            self.rename_when_listed = None;
            self.begin_rename(window, cx);
        }

        let empty = self.empty_state(cx);
        let listing = empty.is_none();
        let body = match empty {
            Some(state) => state,
            None => uniform_list(
                "rows",
                self.rows.len(),
                cx.processor(|this, range, window, cx| this.render_rows(range, window, cx)),
            )
            .track_scroll(&self.scroll)
            .size_full()
            .into_any_element(),
        };
        // Measures the room the list is given; a change lays it out again.
        let measured = Rc::clone(&self.width);
        let placed = Rc::clone(&self.list_bounds);
        // Set again by the lead row if it is drawn this frame.
        self.lead_bounds.set(None);
        let this = cx.entity_id();
        let measure = canvas(
            move |bounds, window, cx| {
                placed.set(bounds);
                // Whole points: layout can hand back 691.99 for a 692pt
                // list, which would drop a column that fits exactly.
                let width = bounds.size.width.round();
                if measured.get() != width {
                    measured.set(width);
                    // The list itself must render again to refit its
                    // columns: a refresh alone can reuse its last frame.
                    cx.notify(this);
                    window.refresh();
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0();

        div()
            .id("browser")
            .key_context("Browser")
            .track_focus(&self.focus)
            // The file list, named by its folder; its rows are options.
            .role(Role::ListBox)
            .aria_label(format!("Files in {}", folder_name(&self.dir)))
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(t.surface_content)
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
            .on_action(cx.listener(|this, _: &ShowContextMenu, window, cx| {
                this.show_context_menu(window, cx)
            }))
            .on_action(cx.listener(|this, _: &OpenSelection, _, cx| {
                // Opening several folders at once has no meaning in one pane.
                if let Some(index) = this.lead.filter(|i| this.selected.contains(i)) {
                    this.open(index, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &RevealInFinder, _, cx| {
                if mtp::is_device_path(&this.dir) {
                    this.notice = Some(
                        format!(
                            "{} can’t show what is on a device.",
                            platform::file_manager()
                        )
                        .into(),
                    );
                    cx.notify();
                    return;
                }
                match this.selection().first() {
                    Some(entry) => cx.reveal_path(&entry.path),
                    None => cx.reveal_path(&this.dir),
                }
            }))
            .on_action(
                cx.listener(|this, _: &MoveToTrash, window, cx| this.trash_selection(window, cx)),
            )
            .on_action(cx.listener(|this, _: &Copy, _, cx| this.copy_selection(cx)))
            .on_action(cx.listener(|this, _: &Paste, _, cx| this.paste(Mode::Copy, cx)))
            .on_action(cx.listener(|this, _: &MoveItemHere, _, cx| this.paste(Mode::Move, cx)))
            // Files dragged in from the file manager are copied here.
            .on_drop(cx.listener(|this, dropped: &ExternalPaths, _, cx| {
                this.receive(dropped.paths().to_vec(), Mode::Copy, cx);
            }))
            .on_action(cx.listener(|this, _: &GoBack, _, cx| this.go_back(cx)))
            .on_action(cx.listener(|this, _: &GoForward, _, cx| this.go_forward(cx)))
            .on_action(cx.listener(|this, _: &GoToEnclosingFolder, _, cx| this.go_up(cx)))
            .on_action(cx.listener(|this, _: &ToggleHiddenFiles, _, cx| this.toggle_hidden(cx)))
            .on_action(cx.listener(|this, _: &SortByName, _, cx| this.set_sort(SortKey::Name, cx)))
            .on_action(cx.listener(|this, _: &SortBySize, _, cx| this.set_sort(SortKey::Size, cx)))
            .on_action(
                cx.listener(|this, _: &SortByModified, _, cx| this.set_sort(SortKey::Modified, cx)),
            )
            .on_action(
                cx.listener(|this, _: &SortByVideo, _, cx| this.set_sort(SortKey::Video, cx)),
            )
            .on_action(
                cx.listener(|this, _: &SortByAudio, _, cx| this.set_sort(SortKey::Audio, cx)),
            )
            .on_action(
                cx.listener(|this, _: &SortByDuration, _, cx| this.set_sort(SortKey::Duration, cx)),
            )
            .child(measure)
            // An empty pane has no table, so it has no column titles.
            .when(listing || !self.loaded, |this| {
                this.child(self.render_header(cx))
            })
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
                    // A right-click below the last row is about the folder.
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, _, window, cx| {
                            if !std::mem::take(&mut this.row_pressed) {
                                this.finish_rename(true, window, cx);
                                window.focus(&this.focus, cx);
                                this.set_selection(HashSet::new(), None, cx);
                            }
                        }),
                    )
                    .relative()
                    // Stripes end with the data: below the last row the
                    // pane is plain.
                    .child(body)
                    // The scroller shows as the host's setting says: while
                    // scrolling, or always (`platform::scroll_bars`).
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .right_0()
                            .bottom_0()
                            .w(Scrollbar::width())
                            .child(Scrollbar::vertical(&self.scroll).viewport_from_layout()),
                    )
                    .context_menu({
                        let browser = cx.entity().downgrade();
                        move |menu, _, cx| match browser.upgrade() {
                            Some(browser) => {
                                // Remembered, so the rows it acts on keep
                                // their focused colour while it is open.
                                let opened = cx.entity().downgrade();
                                browser.update(cx, |browser, _| browser.menu = Some(opened));
                                browser.read(cx).context_menu(menu, cx)
                            }
                            None => menu,
                        }
                    }),
            )
            .child(self.render_status(cx))
            .children(self.key_menu.render())
    }
}

/// The status bar's leading text: "11 items", "1 of 11 selected · 459 KB";
/// while searching, "2 matches", "2 matches · 1 selected · 459 KB".
fn status_summary(rows: usize, selected: usize, bytes: u64, searching: bool) -> String {
    let size = match bytes {
        0 => String::new(),
        bytes => format!(" · {}", format::size(bytes)),
    };
    match (searching, selected) {
        (true, 0) => matches(rows),
        (true, n) => format!("{} · {n} selected{size}", matches(rows)),
        (false, 0) => format::count(rows, "item"),
        (false, n) => format!("{n} of {rows} selected{size}"),
    }
}

/// What the status bar says about the order when the column it sorts by is
/// not on screen ("11 items · Sorted by Audio", then the direction's
/// arrow); `None` while the header shows it. Name always has its header.
fn sort_cue(key: SortKey, visible: &[Column]) -> Option<String> {
    let shown = key == SortKey::Name || visible.iter().any(|c| c.sort_key() == key);
    (!shown).then(|| format!("Sorted by {}", key.label()))
}

/// "1 match", "2 matches".
fn matches(n: usize) -> String {
    match n {
        1 => "1 match".to_owned(),
        n => format!("{n} matches"),
    }
}

/// What the status bar's activity trigger shows.
enum Activity {
    /// Transfers are running: what the first is doing, or how many run,
    /// and how far they have got together.
    Running { label: SharedString, fraction: f32 },
    /// The last transfer ended.
    Ended {
        ending: Ending,
        summary: SharedString,
    },
    /// The last transfer's notice has gone; its receipt is kept until it is
    /// dismissed, behind a quiet "Last Transfer".
    Receipt,
}

/// The transfer activity to show in the status bar, if there is any, and
/// the stops and failures behind it that still wait to be dismissed.
fn activity(cx: &App) -> Option<(Activity, Unresolved)> {
    let devices = Devices::global(cx);
    let devices = devices.read(cx);
    let transfers = devices.transfers();
    let activity = match transfers {
        [] => devices
            .finished()
            .last()
            .map(|finished| Activity::Ended {
                ending: finished.ending,
                summary: finished.summary.clone(),
            })
            // Its notice has gone, but the receipt is kept until it is
            // dismissed: a quiet way back to it stays (S4-5).
            .or_else(|| devices.receipt().map(|_| Activity::Receipt)),
        [only] => {
            let fraction = only.progress.fraction();
            let label = match only.progress.cancelled() {
                true => "Stopping…".to_owned(),
                false => format!("{} · {}", only.action, format::percent(fraction)),
            };
            Some(Activity::Running {
                label: label.into(),
                fraction,
            })
        }
        many => {
            let fraction =
                many.iter().map(|t| t.progress.fraction()).sum::<f32>() / many.len() as f32;
            Some(Activity::Running {
                label: format!(
                    "{} · {}",
                    format::count(many.len(), "transfer"),
                    format::percent(fraction)
                )
                .into(),
                fraction,
            })
        }
    };
    activity.map(|activity| (activity, devices.unresolved()))
}

/// The status bar's one trigger for transfers: what is running and how far
/// it has got, or how the last one ended, led by any stop or failure still
/// waiting to be dismissed. Clicking it, or Return or Space once it has
/// focus, opens the activity popover, which says what happened to every
/// item and holds Stop; so does View › Show Transfers. Its chevron says it
/// opens something. The bar yields first when the status bar is tight, then
/// the label.
fn activity_trigger(
    activity: Activity,
    unresolved: Unresolved,
    focus: Focus,
    cx: &App,
) -> impl IntoElement {
    let t = Theme::of(cx).clone();
    let devices = Devices::global(cx);
    let open = devices.read(cx).details_open();
    let chevron = ui::icon(IconName::ChevronUp, t.secondary_label)
        .size(size::ICON_SMALL)
        .flex_none();
    let ring = t.focus_shadow();
    // An earlier stop or failure stays in sight behind newer work.
    let cue = unresolved.any().then(|| {
        let ink = match unresolved.failed {
            0 => t.secondary_label,
            _ => t.red,
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(size::GAP_TINY)
            .text_color(ink)
            .child(ui::icon(IconName::CircleAlert, ink).size(size::ICON_SMALL))
            .child(unresolved.label())
    });
    let body = match activity {
        Activity::Running { label, fraction } => div()
            .flex()
            .items_center()
            .gap(size::GAP)
            .min_w_0()
            .text_color(t.label)
            .child(div().min_w_0().truncate().child(label))
            .child(
                div()
                    .w(size::TRANSFER_BAR)
                    .min_w_0()
                    // Shrinks well ahead of the label beside it.
                    .flex_shrink(TRANSFER_BAR_YIELD)
                    .child(ui::progress_bar(fraction, cx)),
            ),
        Activity::Ended { ending, summary } => {
            let (glyph, ink, mark) = match ending {
                Ending::Done => (Some(IconName::CircleCheck), t.label, t.green),
                Ending::Stopped => (None, t.secondary_label, t.secondary_label),
                Ending::Failed => (Some(IconName::CircleAlert), t.red, t.red),
            };
            let glyph = glyph.map(|glyph| ui::icon(glyph, mark).size(size::ICON_SMALL).flex_none());
            div()
                .flex()
                .items_center()
                .gap(size::GAP_SMALL)
                .min_w_0()
                .text_color(ink)
                .children(glyph)
                .child(div().min_w_0().truncate().child(summary))
        }
        Activity::Receipt => div()
            .min_w_0()
            .truncate()
            .text_color(t.secondary_label)
            .child("Last Transfer"),
    };
    let content = div()
        .id("activity")
        .role(Role::Button)
        .aria_label("Transfer details")
        .track_focus(&focus.trigger)
        .focus_visible(move |s| s.shadow(ring))
        // A click leaves focus on the list, so closing hands it back.
        .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
        .flex()
        .items_center()
        .gap(size::GAP)
        .min_w_0()
        .max_w(size::TRANSFER_LABEL)
        .py(size::GAP_TINY)
        .font_features(theme::tabular())
        .children(cue)
        .child(body)
        .child(chevron);
    ui::overlay_card(Popover::new("transfer-details"), cx)
        .anchor(Anchor::BottomRight)
        // Follows `Devices`, so View › Show Transfers can open it.
        .open(open)
        .on_open_change(move |open, _, cx| {
            let open = *open;
            devices.update(cx, |devices, cx| devices.set_details_open(open, cx));
        })
        .trigger(ui::PopoverTrigger::new(content))
        .content(move |state, window, cx| {
            let popover = OpenDetails {
                state: cx.entity().downgrade(),
                focus: state.focus_handle(cx),
                trigger: focus.trigger.clone(),
                list: focus.list.clone(),
            };
            transfer_details(popover, window, cx)
        })
}

/// Where focus goes around the transfer trigger.
struct Focus {
    trigger: FocusHandle,
    /// The file list: where focus lands when nothing better remains.
    list: FocusHandle,
}

/// The open activity popover, so Dismiss can keep focus somewhere real.
#[derive(Clone)]
struct OpenDetails {
    state: WeakEntity<PopoverState>,
    /// The popover's own surface.
    focus: FocusHandle,
    trigger: FocusHandle,
    list: FocusHandle,
}

impl OpenDetails {
    /// After a Dismiss: if that was the last outcome, the popover and its
    /// trigger go, so close it through the kit first, which returns focus
    /// to where it was before it opened, and if that was the trigger
    /// itself, to the list (decision 64, S3-6). Otherwise the dismissed
    /// block and its button go and focus stays in the popover.
    fn after_dismiss(&self, emptied: bool, window: &mut Window, cx: &mut App) {
        if !emptied {
            self.focus.focus(window, cx);
            return;
        }
        self.state
            .update(cx, |state, cx| state.dismiss(window, cx))
            .ok();
        let focused = window.focused(cx);
        if focused.is_none() || focused.as_ref() == Some(&self.trigger) {
            self.list.focus(window, cx);
        }
    }
}

/// How much faster the transfer bar gives up width than its label.
const TRANSFER_BAR_YIELD: f32 = 8.;

/// The activity popover: one block per transfer, running ones first, then
/// those that just ended. Each has its overall progress and every item that
/// was asked for with its own state, so nothing in a transfer goes unseen.
fn transfer_details(popover: OpenDetails, window: &mut Window, cx: &mut App) -> Div {
    let t = Theme::of(cx).clone();
    let devices = Devices::global(cx);
    // Wide enough for "1.71 GB of 6.38 GB · 4 items, 6 files", never wider
    // than the window allows.
    // The card's own padding is inside its width.
    let width = size::POPOVER_WIDTH.min(window.viewport_size().width - size::SHEET_GUTTER * 2.)
        - size::PANE_PADDING * 2.;
    let small = move |color: Hsla| {
        div()
            .text_size(text::SMALL)
            .line_height(text::SMALL_LINE)
            .text_color(color)
            .font_features(theme::tabular())
    };

    let mut blocks: Vec<AnyElement> = Vec::new();
    {
        let state = devices.read(cx);
        for transfer in state.transfers() {
            let id = transfer.id;
            let stopping = transfer.progress.cancelled();
            let detail = transfer.progress.detail();
            let devices = devices.clone();
            let header = div()
                .flex()
                .items_center()
                .gap(size::GAP)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(match stopping {
                            true => "Stopping…".into(),
                            false => transfer.label.clone(),
                        }),
                )
                .child(
                    ui::Button::new(
                        ("stop", id),
                        match transfer.mode {
                            Mode::Copy => "Stop Copy",
                            Mode::Move => "Stop Move",
                        },
                    )
                    .small()
                    .disabled(stopping)
                    .on_click(move |_, _, cx| {
                        devices.update(cx, |devices, cx| devices.cancel(id, cx))
                    }),
                );
            let body = div()
                .flex()
                .flex_col()
                .gap(size::GAP_SMALL)
                .child(ui::progress_bar(transfer.progress.fraction(), cx))
                .child(small(t.secondary_label).child(transfer_summary(&detail)));
            blocks.push(
                transfer_block(id, header, body, &transfer.progress, &detail, &t, cx)
                    .into_any_element(),
            );
        }
        for finished in state.finished().collect::<Vec<_>>().into_iter().rev() {
            let detail = finished.progress.detail();
            let (glyph, mark) = match finished.ending {
                Ending::Done => (Some(IconName::CircleCheck), t.green),
                Ending::Stopped => (None, t.secondary_label),
                Ending::Failed => (Some(IconName::CircleAlert), t.red),
            };
            let id = finished.id;
            let devices = devices.clone();
            let popover = popover.clone();
            let header = div()
                .flex()
                .items_center()
                .gap(size::GAP_SMALL)
                .children(glyph.map(|glyph| ui::icon(glyph, mark).flex_none()))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(finished.summary.clone()),
                )
                // A stop or a failure stays until it is put away here. A
                // success goes from the status bar by itself, but its
                // receipt stays behind "Last Transfer" until put away too.
                .child({
                    div().flex_none().child(
                        ui::Button::new(("dismiss", id), "Dismiss")
                            .small()
                            .on_click(move |_, window, cx| {
                                let emptied = devices.update(cx, |devices, cx| {
                                    devices.dismiss(id, cx);
                                    !devices.has_details()
                                });
                                popover.after_dismiss(emptied, window, cx);
                            }),
                    )
                });
            let body = small(t.secondary_label).child(transfer_summary(&detail));
            blocks.push(
                transfer_block(
                    finished.id,
                    header,
                    body,
                    &finished.progress,
                    &detail,
                    &t,
                    cx,
                )
                .into_any_element(),
            );
        }
    }

    div()
        .w(width)
        .flex()
        .flex_col()
        .gap(size::PANE_PADDING)
        .font_family(theme::font(cx))
        .text_size(text::BODY)
        .line_height(text::BODY_LINE)
        .text_color(t.label)
        .when(blocks.is_empty(), |this| {
            this.child(small(t.secondary_label).child("Nothing is being copied or moved."))
        })
        .children(blocks.into_iter().enumerate().map(|(i, block)| {
            // Blocks after the first are set off by a hairline.
            div()
                .when(i > 0, |this| {
                    this.pt(size::PANE_PADDING)
                        .border_t(size::HAIRLINE)
                        .border_color(t.separator)
                })
                .child(block)
        }))
}

/// How much of a transfer is done, with its counts labelled: "1.71 GB of
/// 6.38 GB · 4 items, 6 files"; once all of it is, "6.38 GB · 4 items,
/// 6 files".
fn transfer_summary(detail: &Detail) -> String {
    let files = detail.files_total as usize;
    let items = match files > detail.items {
        true => format!(
            "{}, {}",
            format::count(detail.items, "item"),
            format::count(files, "file")
        ),
        false => format::count(detail.items, "item"),
    };
    match (detail.bytes_total, detail.files_total) {
        (0, 0) => "Preparing…".to_owned(),
        (0, total) if detail.files_done >= total => {
            format!("{} · {items}", format::count(total as usize, "file"))
        }
        (0, total) => format!(
            "{} of {} · {items}",
            detail.files_done,
            format::count(total as usize, "file")
        ),
        (bytes, _) if detail.bytes_done >= bytes => format!("{} · {items}", format::size(bytes)),
        (bytes, _) => format!(
            "{} of {} · {items}",
            format::size(detail.bytes_done.min(bytes)),
            format::size(bytes),
        ),
    }
}

/// One transfer in the activity popover: its header, its progress, then its
/// items in a list that scrolls.
fn transfer_block(
    id: usize,
    header: Div,
    body: Div,
    progress: &Progress,
    detail: &Detail,
    t: &Theme,
    cx: &App,
) -> Div {
    // A long list is shown as a stretch around the item in hand.
    let start = match detail.items > TRANSFER_ROWS {
        true => detail
            .active
            .unwrap_or(0)
            .saturating_sub(2)
            .min(detail.items - TRANSFER_ROWS),
        false => 0,
    };
    let items = progress.items(start, TRANSFER_ROWS);
    let after = detail.items.saturating_sub(start + items.len());
    let note = |content: String| {
        div()
            .text_size(text::SMALL)
            .line_height(text::SMALL_LINE)
            .text_color(t.secondary_label)
            .child(content)
    };
    let rows: Vec<_> = items
        .into_iter()
        .enumerate()
        .map(|(offset, item)| transfer_row(start + offset, item, &detail.name, t, cx))
        .collect();
    div()
        .flex()
        .flex_col()
        .gap(size::GAP)
        .child(header)
        .child(body)
        .child(
            div()
                .id(("transfer-items", id))
                .max_h(size::TRANSFER_LIST)
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap(size::GAP_SMALL)
                .when(start > 0, |this| {
                    this.child(note(format!(
                        "{} before these",
                        format::count(start, "item")
                    )))
                })
                .children(rows)
                .when(after > 0, |this| {
                    this.child(note(format!(
                        "{} after these",
                        format::count(after, "item")
                    )))
                }),
        )
}

/// One item of a transfer: its name, where it stands and a mark at the
/// trailing edge, and a bar of its own while it is the one in hand.
fn transfer_row(
    index: usize,
    item: ItemStatus,
    in_hand: &str,
    t: &Theme,
    cx: &App,
) -> Stateful<Div> {
    let active = item.stage == Stage::Active;
    let (state, color) = match &item.stage {
        Stage::Waiting => ("Waiting".to_owned(), t.secondary_label),
        Stage::Active if item.bytes_total > 0 => (
            format!(
                "{} of {}",
                format::size(item.bytes_done),
                format::size(item.bytes_total)
            ),
            t.secondary_label,
        ),
        Stage::Active => ("Working…".to_owned(), t.secondary_label),
        Stage::Done if item.bytes_total > 0 => (format::size(item.bytes_total), t.secondary_label),
        Stage::Done => ("Done".to_owned(), t.secondary_label),
        Stage::Skipped => ("Already there".to_owned(), t.secondary_label),
        Stage::Failed(_) => ("Failed".to_owned(), t.red),
    };
    let mark = match &item.stage {
        Stage::Done | Stage::Skipped => Some(ui::icon(IconName::Check, t.secondary_label)),
        Stage::Failed(_) => Some(ui::icon(IconName::CircleAlert, t.red)),
        _ => None,
    };
    let fraction = match item.bytes_total {
        0 => 0.,
        total => item.bytes_done as f32 / total as f32,
    };
    let small = |color: Hsla| {
        div()
            .text_size(text::SMALL)
            .line_height(text::SMALL_LINE)
            .text_color(color)
            .font_features(theme::tabular())
    };
    div()
        .id(index)
        .flex()
        .flex_none()
        .flex_col()
        .gap(size::GAP_TINY)
        .child(
            div()
                .flex()
                .items_center()
                .gap(size::GAP)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .whitespace_nowrap()
                        .text_ellipsis_middle()
                        .when(item.stage == Stage::Waiting, |this| {
                            this.text_color(t.secondary_label)
                        })
                        .child(item.name.clone()),
                )
                .child(small(color).flex_none().child(state))
                .children(mark.map(|mark| mark.size(size::ICON_SMALL).flex_none())),
        )
        .when(active && item.bytes_total > 0, |this| {
            this.child(ui::progress_bar(fraction, cx))
        })
        // Inside a folder, say which of its files is in hand.
        .when(active && item.files > 1 && !in_hand.is_empty(), |this| {
            this.child(
                small(t.secondary_label)
                    .whitespace_nowrap()
                    .text_ellipsis_middle()
                    .child(in_hand.to_owned()),
            )
        })
        .when_some(
            match &item.stage {
                Stage::Failed(reason) => Some(reason.clone()),
                _ => None,
            },
            |this, reason| this.child(small(t.red).child(reason)),
        )
}

/// Why a media file shows "Unreadable"; the inspector says the same.
const UNREADABLE: &str = "This file doesn’t contain media that Seer can read.";

/// What a cell of the list shows.
#[derive(Debug, PartialEq)]
enum Value {
    /// Nothing: the column does not apply to this kind of item (a folder's
    /// size, a text file's codec), or the file has not been read yet (or
    /// is never read here, as on a device).
    Blank,
    /// A media file that was read and has no such stream: the one
    /// placeholder that means something.
    Dash,
    /// Plain text in the row's secondary ink.
    Text(String),
    /// Media facts in two tones: each part is shown strong (in `label`)
    /// or weak (secondary), joined by spaces. "**1080p** H.264".
    Media(Vec<(String, bool)>),
    /// A media file FFmpeg could not read, said once per row, in its
    /// first media column.
    Unreadable,
}

impl Value {
    /// The value in words, for assistive technology; `None` when blank.
    fn spoken(&self) -> Option<String> {
        match self {
            Value::Blank => None,
            Value::Dash => Some("none".to_owned()),
            Value::Text(text) => Some(text.clone()),
            Value::Media(parts) => Some(
                parts
                    .iter()
                    .map(|(part, _)| part.as_str())
                    .collect::<Vec<_>>()
                    .join(" "),
            ),
            Value::Unreadable => Some("Unreadable".to_owned()),
        }
    }

    fn render(self, strong: Hsla, dash: Hsla) -> AnyElement {
        match self {
            Value::Blank | Value::Unreadable => div().into_any_element(),
            Value::Dash => div().text_color(dash).child("—").into_any_element(),
            Value::Text(text) => SharedString::from(text).into_any_element(),
            Value::Media(parts) => {
                let (text, highlights) = media_runs(parts, strong);
                StyledText::new(text)
                    .with_highlights(highlights)
                    .into_any_element()
            }
        }
    }
}

/// The text of a two-tone media cell, and the runs drawn in `strong`; the
/// rest takes the cell's secondary ink.
fn media_runs(
    parts: Vec<(String, bool)>,
    strong: Hsla,
) -> (String, Vec<(Range<usize>, HighlightStyle)>) {
    let mut text = String::new();
    let mut highlights = Vec::new();
    for (part, is_strong) in parts {
        if !text.is_empty() {
            text.push(' ');
        }
        let start = text.len();
        text.push_str(&part);
        if is_strong {
            let style = HighlightStyle {
                color: Some(strong),
                ..Default::default()
            };
            highlights.push((start..text.len(), style));
        }
    }
    (text, highlights)
}

/// The value of `column` for `entry`, given what probing it found.
/// `first_media` is true for the first media column shown, where an
/// unreadable file says so.
fn value(column: Column, entry: &Entry, probe: Option<&Probe>, first_media: bool) -> Value {
    match column {
        Column::Size if entry.is_dir() => Value::Blank,
        Column::Size => Value::Text(format::size(entry.size)),
        Column::Modified => match entry.modified {
            Some(time) => Value::Text(format::date(time)),
            None => Value::Blank,
        },
        // Folders, text, images and subtitles have nothing to say here.
        _ if !entry.kind.is_probeable() => Value::Blank,
        _ => match probe {
            Some(Probe::Ready(info)) => media_value(column, info),
            Some(Probe::Unreadable) if first_media => Value::Unreadable,
            _ => Value::Blank,
        },
    }
}

/// A media column's value for a file that was read.
fn media_value(column: Column, info: &MediaInfo) -> Value {
    match column {
        Column::Duration => match info.duration {
            seconds if seconds > 0. => Value::Text(format::duration(seconds)),
            _ => Value::Dash,
        },
        Column::Video => match info.primary_video() {
            // In the order `format::video_name` writes it everywhere.
            Some(video) => {
                let size = format::video_size(video.width, video.height);
                let mut parts: Vec<(String, bool)> = size.map(|s| (s, true)).into_iter().collect();
                parts.push((format::codec(&video.codec), false));
                Value::Media(parts)
            }
            None => Value::Dash,
        },
        // The layout leads, as the resolution does in Video, so both media
        // columns have one strong left edge: "**5.1** E-AC-3 +1". The same
        // order as `format::audio_name`, which Clean Up and the inspector
        // use (decision 65).
        Column::Audio => match info.primary_audio() {
            Some(audio) => {
                let mut parts: Vec<(String, bool)> = audio
                    .channels
                    .map(|channels| (format::channels(channels), true))
                    .into_iter()
                    .collect();
                parts.push((format::codec(&audio.codec), false));
                let others = info.streams_of(StreamKind::Audio).count();
                if others > 1 {
                    parts.push((format!("+{}", others - 1), false));
                }
                Value::Media(parts)
            }
            None => Value::Dash,
        },
        Column::Size | Column::Modified => Value::Blank,
    }
}

/// The glyph a list row leads with for `entry` (see [`ui::kind_glyph`]).
pub fn glyph(entry: &Entry) -> ui::Glyph {
    match entry.kind {
        Kind::Folder => ui::Glyph::Folder,
        Kind::Video => ui::Glyph::Video,
        Kind::Audio => ui::Glyph::Audio,
        Kind::Image => ui::Glyph::Image,
        Kind::Subtitle => ui::Glyph::Subtitle,
        Kind::Other if entry.is_text() => ui::Glyph::Text,
        Kind::Other => ui::Glyph::Other,
    }
}

/// The path with the home folder shortened to "~".
pub fn display_path(path: &Path) -> String {
    if let Ok(on_device) = path.strip_prefix(mtp::ROOT) {
        return match on_device.as_os_str().is_empty() {
            true => "Devices".into(),
            false => on_device.display().to_string(),
        };
    }
    match dirs::home_dir().and_then(|home| path.strip_prefix(home).ok().map(Path::to_owned)) {
        Some(rest) if rest.as_os_str().is_empty() => "~".into(),
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

/// Lists a folder on the Mac or on a device. Blocks.
fn list(dir: &Path) -> Result<Vec<Entry>, String> {
    match mtp::is_device_path(dir) {
        true => mtp::list_dir(dir).map_err(|e| format!("{e:#}")),
        false => fs::list_dir(dir).map_err(|e| explain(&e)),
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
        .unwrap_or_else(|| platform::root_name().into())
}

#[cfg(test)]
mod tests {
    // Not a glob: GPUI's own `test` attribute would shadow the standard one.
    use super::{
        Column, Detail, Entry, Kind, MediaInfo, PathBuf, Probe, SortKey, Value, columns_width,
        fit_columns, format, glyph, list_comfort, list_essential, media_runs, name_floor, size,
        sort_cue, status_summary, transfer_summary, ui, value,
    };
    use gpui_kit::{Hsla, Pixels, px};
    use seer_core::probe::{Stream, StreamKind};
    use std::sync::Arc;

    fn entry(name: &str, kind: Kind) -> Entry {
        Entry {
            name: name.into(),
            path: PathBuf::from("/media").join(name),
            kind,
            size: 459_000,
            modified: None,
            hidden: false,
        }
    }

    fn stream(kind: StreamKind, codec: &str) -> Stream {
        Stream {
            index: 0,
            kind,
            codec: codec.into(),
            codec_description: None,
            profile: None,
            language: None,
            title: None,
            flags: Default::default(),
            bit_rate: None,
            duration: None,
            tags: Vec::new(),
            width: Some(1920),
            height: Some(1080),
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

    fn ready(streams: Vec<Stream>) -> Probe {
        Probe::Ready(Arc::new(MediaInfo {
            container: String::new(),
            container_description: String::new(),
            duration: 187.,
            bit_rate: 0,
            tags: Vec::new(),
            streams,
            chapters: Vec::new(),
        }))
    }

    /// Blank where a column does not apply, a dash only for a stream a
    /// media file lacks, "Unreadable" once.
    #[test]
    fn cells_say_blank_dash_or_unreadable() {
        let folder = entry("Clips", Kind::Folder);
        let notes = entry("notes.txt", Kind::Other);
        let video = entry("a.avi", Kind::Video);
        for column in [Column::Video, Column::Audio, Column::Duration, Column::Size] {
            assert_eq!(value(column, &folder, None, true), Value::Blank);
        }
        for column in [Column::Video, Column::Audio, Column::Duration] {
            assert_eq!(value(column, &notes, None, true), Value::Blank);
            // Not read yet.
            assert_eq!(
                value(column, &video, Some(&Probe::Pending), true),
                Value::Blank
            );
            assert_eq!(value(column, &video, None, true), Value::Blank);
        }
        assert_eq!(
            value(Column::Size, &notes, None, false),
            Value::Text("459 KB".into())
        );

        let silent = ready(vec![stream(StreamKind::Video, "h264")]);
        assert_eq!(
            value(Column::Video, &video, Some(&silent), true),
            Value::Media(vec![("1080p".into(), true), ("H.264".into(), false)])
        );
        assert_eq!(
            value(Column::Audio, &video, Some(&silent), false),
            Value::Dash
        );
        assert_eq!(
            value(Column::Duration, &video, Some(&silent), false),
            Value::Text("3:07".into())
        );

        let unreadable = Some(&Probe::Unreadable);
        assert_eq!(
            value(Column::Video, &video, unreadable, true),
            Value::Unreadable
        );
        assert_eq!(
            value(Column::Audio, &video, unreadable, false),
            Value::Blank
        );
        assert_eq!(
            value(Column::Duration, &video, unreadable, false),
            Value::Blank
        );
    }

    #[test]
    fn audio_folders_never_show_video() {
        assert!(!Column::by_priority(true).contains(&Column::Video));
        for audio in [false, true] {
            let order = Column::by_priority(audio);
            // Size is the last to go.
            assert_eq!(order[0], Column::Size);
            let essential = &order[..Column::ESSENTIAL];
            assert!(essential.contains(&Column::Duration));
        }
        assert!(Column::by_priority(false)[..Column::ESSENTIAL].contains(&Column::Video));
        assert!(Column::by_priority(true)[..Column::ESSENTIAL].contains(&Column::Audio));
    }

    /// The list beside both panes at their defaults in the default window.
    fn default_list() -> Pixels {
        px(1180.) - size::SIDEBAR.1 - size::INSPECTOR.1 - size::HAIRLINE * 2.
    }

    /// The narrowest list: the smallest window beside the narrowest
    /// inspector.
    fn narrowest_list() -> Pixels {
        size::WINDOW_MIN.0 - size::INSPECTOR.0 - size::HAIRLINE
    }

    /// Decision 41: Name yields to `NAME_MIN` before a column drops, then
    /// Date Modified goes, then Audio; Video, Duration and Size stay at
    /// 760pt with the inspector open; Size is the last to go.
    #[test]
    fn name_yields_before_a_column_drops() {
        use Column::*;
        let long = size::NAME_MAX;
        let mixed = |width: Pixels| fit_columns(width, long, false, None);

        // The default window with a long name keeps all four media columns.
        let (name, columns) = mixed(default_list());
        assert_eq!(columns, [Video, Audio, Duration, Size]);
        assert!(name >= size::NAME_MIN);
        // A 680pt list with a 440pt name too (F2-1).
        assert_eq!(mixed(px(680.)).1, [Video, Audio, Duration, Size]);

        // 760pt with the inspector open: Name, Video, Duration, Size, with
        // Name below `NAME_MIN` but at its floor.
        let (name, columns) = mixed(narrowest_list());
        assert_eq!(columns, [Video, Duration, Size]);
        assert_eq!(name, name_floor());
        assert!(name < size::NAME_MIN && name >= px(168.));
        // An audio folder there keeps Audio in Video's place.
        let (_, columns) = fit_columns(narrowest_list(), long, true, None);
        assert_eq!(columns, [Audio, Duration, Size]);

        // The widths at which each column drops, in order: Date Modified,
        // Audio, Duration, Video, and Size last.
        let insets = size::ROW_INSET * 4.;
        let needs = |columns: &[Column], name: Pixels| insets + name + columns_width(columns);
        let steps: [(Pixels, &[Column]); 5] = [
            (
                needs(&[Video, Audio, Duration, Size, Modified], size::NAME_MIN),
                &[Video, Audio, Duration, Size, Modified],
            ),
            (
                needs(&[Video, Audio, Duration, Size], size::NAME_MIN),
                &[Video, Audio, Duration, Size],
            ),
            (
                needs(&[Video, Duration, Size], name_floor()),
                &[Video, Duration, Size],
            ),
            (needs(&[Video, Size], name_floor()), &[Video, Size]),
            (needs(&[Size], name_floor()), &[Size]),
        ];
        assert_eq!(steps[0].0, px(820.));
        assert_eq!(steps[1].0, px(648.));
        assert_eq!(steps[2].0, narrowest_list());
        for (i, (width, columns)) in steps.iter().enumerate() {
            assert_eq!(mixed(*width).1, *columns, "at {width:?}");
            let fewer: &[Column] = match steps.get(i + 1) {
                Some((_, fewer)) => fewer,
                None => &[],
            };
            assert_eq!(mixed(*width - px(1.)).1, fewer, "just below {width:?}");
        }
        // Columns beyond the essential ones join only beside a `NAME_MIN`
        // Name; slack beyond what Name wants goes after the last column.
        let (name, _) = fit_columns(px(2000.), size::NAME_MIN, false, None);
        assert_eq!(name, size::NAME_MIN);
    }

    /// Every sort the View menu offers, and the folders it applies to: a
    /// Video sort never reaches an audio folder (`Browser::shown_sort`).
    fn sorts(audio_folder: bool) -> Vec<SortKey> {
        use SortKey::*;
        [Name, Video, Audio, Duration, Size, Modified]
            .into_iter()
            .filter(|key| !(audio_folder && *key == Video))
            .collect()
    }

    /// Decision 58: at 760pt with the inspector open the four essentials
    /// (Name, the primary media column, Duration, Size) show under every
    /// sort; a sorted column that cannot join folds its header and the
    /// status bar names the sort instead (S3-2, A3-4).
    #[test]
    fn essentials_stay_under_every_sort() {
        use Column::*;
        let long = size::NAME_MAX;
        for audio_folder in [false, true] {
            let essentials = match audio_folder {
                false => [Video, Duration, Size],
                true => [Audio, Duration, Size],
            };
            for key in sorts(audio_folder) {
                let (name, columns) =
                    fit_columns(narrowest_list(), long, audio_folder, Column::for_sort(key));
                assert_eq!(columns, essentials, "sorted by {key:?}");
                assert!(name >= name_floor(), "sorted by {key:?}");
                // The sort is on screen or in the status bar.
                let folded = !columns.iter().any(|c| c.sort_key() == key);
                assert_eq!(
                    sort_cue(key, &columns).is_some(),
                    folded && key != SortKey::Name,
                    "sorted by {key:?}"
                );
            }
        }
        assert_eq!(
            sort_cue(SortKey::Audio, &[Video, Duration, Size]).as_deref(),
            Some("Sorted by Audio")
        );
        assert_eq!(sort_cue(SortKey::Duration, &[Video, Duration, Size]), None);
    }

    /// Decision 58: a list `list_comfort` wide shows the fifth column
    /// (Audio beside Video) under every sort; one point less does not. A
    /// sorted column that does not fit is passed over, so Audio still
    /// joins; one that fits joins ahead of Audio.
    #[test]
    fn the_fifth_column_holds_under_every_sort() {
        use Column::*;
        let long = size::NAME_MAX;
        let comfort = list_comfort(false);
        assert_eq!(comfort, px(648.));
        // The 940pt window with the inspector narrowed to 291pt (F3-2).
        assert_eq!(px(940.) - px(291.) - size::HAIRLINE, comfort);
        for key in sorts(false) {
            let sorted = Column::for_sort(key);
            assert_eq!(
                fit_columns(comfort, long, false, sorted).1,
                [Video, Audio, Duration, Size],
                "sorted by {key:?}"
            );
            assert_eq!(
                fit_columns(comfort - px(1.), long, false, sorted).1,
                [Video, Duration, Size],
                "sorted by {key:?}"
            );
        }
        // The default window: Date Modified cannot join beside Name at
        // `NAME_MIN`, so Audio does and the status bar names the sort.
        let (_, columns) = fit_columns(default_list(), long, false, Some(Modified));
        assert_eq!(columns, [Video, Audio, Duration, Size]);
        assert_eq!(
            sort_cue(SortKey::Modified, &columns).as_deref(),
            Some("Sorted by Date Modified")
        );
        // Where it fits, the sorted column joins ahead of Audio.
        let modified = px(32.) + size::NAME_MIN + columns_width(&[Video, Duration, Size, Modified]);
        assert_eq!(
            fit_columns(modified, long, false, Some(Modified)).1,
            [Video, Duration, Size, Modified]
        );
        assert_eq!(
            fit_columns(modified, long, false, None).1,
            [Video, Audio, Duration, Size]
        );
        // An audio folder brings Date Modified in beside its essentials.
        let audio = px(32.) + size::NAME_MIN + columns_width(&[Audio, Duration, Size, Modified]);
        assert_eq!(
            fit_columns(audio, long, true, Some(Modified)).1,
            [Audio, Duration, Size, Modified]
        );
        assert_eq!(
            fit_columns(audio - px(1.), long, true, Some(Modified)).1,
            [Audio, Duration, Size]
        );
    }

    /// F4-3, A4-3: a folder of audio asks the panes for the width of its
    /// own five columns, 692pt, and gets Date Modified there under every
    /// sort; 14pt more than the 678pt the default panes leave.
    #[test]
    fn audio_folders_ask_for_their_own_five_columns() {
        use Column::*;
        let long = size::NAME_MAX;
        let comfort = list_comfort(true);
        assert_eq!(comfort, px(692.));
        assert_eq!(
            px(1180.) - px(200.) - px(300.) - size::HAIRLINE * 2.,
            comfort - px(14.)
        );
        for key in sorts(true) {
            let sorted = Column::for_sort(key);
            assert_eq!(
                fit_columns(comfort, long, true, sorted).1,
                [Audio, Duration, Size, Modified],
                "sorted by {key:?}"
            );
        }
        assert_eq!(
            fit_columns(comfort - px(1.), long, true, None).1,
            [Audio, Duration, Size]
        );
    }

    /// S4-4: the narrowest list that keeps the essentials, which the
    /// workspace keeps beside both panes before it undocks the sidebar.
    #[test]
    fn the_essential_list_is_the_narrowest_list() {
        use Column::*;
        assert_eq!(list_essential(), narrowest_list());
        assert_eq!(list_essential(), px(499.));
        let (name, columns) = fit_columns(list_essential(), size::NAME_MAX, false, None);
        assert_eq!(columns, [Video, Duration, Size]);
        assert_eq!(name, name_floor());
    }

    #[test]
    fn rows_are_spoken_with_their_values() {
        assert_eq!(Value::Blank.spoken(), None);
        assert_eq!(Value::Dash.spoken().as_deref(), Some("none"));
        let media = Value::Media(vec![("1080p".into(), true), ("H.264".into(), false)]);
        assert_eq!(media.spoken().as_deref(), Some("1080p H.264"));
        assert_eq!(Value::Unreadable.spoken().as_deref(), Some("Unreadable"));
    }

    #[test]
    fn audio_cells_lead_with_the_layout() {
        let video = entry("a.mkv", Kind::Video);
        let mut surround = stream(StreamKind::Audio, "eac3");
        surround.channels = Some(6);
        let two = ready(vec![surround, stream(StreamKind::Audio, "aac")]);
        assert_eq!(
            value(Column::Audio, &video, Some(&two), false),
            Value::Media(vec![
                ("5.1".into(), true),
                ("E-AC-3".into(), false),
                ("+1".into(), false)
            ])
        );
        let mut mono = stream(StreamKind::Audio, "aac");
        mono.channels = Some(1);
        let cell = value(Column::Audio, &video, Some(&ready(vec![mono])), false);
        assert_eq!(
            cell,
            Value::Media(vec![("Mono".into(), true), ("AAC".into(), false)])
        );
        // The list, Clean Up and the inspector write one order (decision 65).
        let Value::Media(parts) = cell else {
            unreachable!()
        };
        let (text, _) = media_runs(parts, Hsla::white());
        assert_eq!(text, format::audio_name("aac", Some(1)));
        let silent = ready(vec![stream(StreamKind::Video, "h264")]);
        let Value::Media(parts) = value(Column::Video, &video, Some(&silent), true) else {
            unreachable!()
        };
        let (text, _) = media_runs(parts, Hsla::white());
        assert_eq!(text, format::video_name("h264", Some(1920), Some(1080)));
    }

    /// Decision 56: every row leads with one of Seer's glyphs, chosen by
    /// kind, with plain-text documents told from other files.
    #[test]
    fn rows_lead_with_seers_glyph_for_their_kind() {
        let cases = [
            ("Clips", Kind::Folder, ui::Glyph::Folder),
            ("a.mkv", Kind::Video, ui::Glyph::Video),
            ("a.flac", Kind::Audio, ui::Glyph::Audio),
            ("Poster.png", Kind::Image, ui::Glyph::Image),
            ("a.en.srt", Kind::Subtitle, ui::Glyph::Subtitle),
            ("Notes.txt", Kind::Other, ui::Glyph::Text),
            ("setup.dmg", Kind::Other, ui::Glyph::Other),
        ];
        for (name, kind, expected) in cases {
            assert_eq!(glyph(&entry(name, kind)), expected, "{name}");
        }
    }

    /// The narrowest list that keeps Name at `NAME_MIN` with Video, Duration
    /// and Size: the pane fitting in the workspace must allow for it.
    #[test]
    fn minimum_list_width() {
        let needed = size::ROW_INSET * 4.
            + size::NAME_MIN
            + columns_width(&[Column::Video, Column::Duration, Column::Size]);
        assert_eq!(needed, size::LIST_MIN);
    }

    #[test]
    fn media_cells_have_two_tones() {
        let strong = Hsla::white();
        let (text, runs) = media_runs(
            vec![("1080p".into(), true), ("H.264".into(), false)],
            strong,
        );
        assert_eq!(text, "1080p H.264");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].0, 0..5);
        assert_eq!(runs[0].1.color, Some(strong));
    }

    #[test]
    fn status_counts_matches_while_searching() {
        assert_eq!(status_summary(11, 0, 0, false), "11 items");
        assert_eq!(
            status_summary(11, 1, 459_000, false),
            "1 of 11 selected · 459 KB"
        );
        assert_eq!(status_summary(2, 0, 0, true), "2 matches");
        assert_eq!(status_summary(1, 0, 0, true), "1 match");
        assert_eq!(status_summary(2, 1, 0, true), "2 matches · 1 selected");
    }

    #[test]
    fn finished_transfers_drop_the_of() {
        let detail = |bytes_done, bytes_total, files_done, files_total| Detail {
            name: String::new(),
            file_done: 0,
            file_size: 0,
            files_done,
            files_total,
            bytes_done,
            bytes_total,
            items: 4,
            active: None,
        };
        assert_eq!(
            transfer_summary(&detail(1_710_000_000, 6_380_000_000, 2, 6)),
            "1.71 GB of 6.38 GB · 4 items, 6 files"
        );
        assert_eq!(
            transfer_summary(&detail(6_380_000_000, 6_380_000_000, 6, 6)),
            "6.38 GB · 4 items, 6 files"
        );
        assert_eq!(
            transfer_summary(&detail(0, 0, 2, 6)),
            "2 of 6 files · 4 items, 6 files"
        );
        assert_eq!(
            transfer_summary(&detail(0, 0, 6, 6)),
            "6 files · 4 items, 6 files"
        );
    }
}
