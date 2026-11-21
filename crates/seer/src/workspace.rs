//! The window: titlebar, sidebar, file list and inspector, and the commands
//! that span them.

use crate::actions::*;
use crate::browser::{self, Browser, BrowserEvent, SidePanes, folder_name};
use crate::cleanup::{self, Cleanup, CleanupEvent};
use crate::devices::{Devices, Dragged};
use crate::inspector::{Inspector, InspectorEvent};
use crate::native;
use crate::platform::{self, Command, MenuPlacement, Side};
use crate::state::Saved;
use crate::theme::{self, Theme, size, text};
use crate::ui::{self, Tip as _};
use gpui_kit::assets::IconName;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::menu::{ContextMenuExt as _, PopupMenu, PopupMenuItem};
use gpui_kit::prelude::*;
use gpui_kit::*;
use seer_core::fs::{Entry, SortKey};
use seer_core::mtp;
use seer_core::transfer::Mode;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

#[derive(Clone)]
struct Place {
    name: String,
    icon: IconName,
    path: PathBuf,
    /// A disk that can be ejected: an eject button shows on hover and focus.
    ejectable: bool,
}

/// Whether Clean Up can run in the folder shown, and if not, why. Worked
/// out from each listing ([`cleanup::worth_reviewing`]), so it follows
/// videos arriving and leaving; the toolbar button, the command and the
/// Main Menu all read it.
#[derive(Clone, Copy, PartialEq, Debug)]
enum CleanUpState {
    Ready,
    /// Neither a video nor a folder here.
    NoVideos,
    /// A device keeps its files to itself.
    OnDevice,
    /// The folder could not be listed.
    Unreadable,
}

impl CleanUpState {
    fn of(dir: &Path, unreadable: bool, listing: Option<&[Entry]>) -> Option<Self> {
        if mtp::is_device_path(dir) {
            return Some(CleanUpState::OnDevice);
        }
        if unreadable {
            return Some(CleanUpState::Unreadable);
        }
        // Still listing: the caller keeps what it had.
        let items = listing?;
        Some(match cleanup::worth_reviewing(items) {
            true => CleanUpState::Ready,
            false => CleanUpState::NoVideos,
        })
    }

    fn available(self) -> bool {
        self == CleanUpState::Ready
    }

    /// The button's tooltip when it is off.
    fn tip(self) -> Option<&'static str> {
        match self {
            CleanUpState::Ready => None,
            CleanUpState::NoVideos => Some("No videos in this folder"),
            CleanUpState::OnDevice => Some("Stream names can’t be cleaned up on a device"),
            CleanUpState::Unreadable => Some("This folder can’t be read"),
        }
    }

    /// What the status bar says when the command is chosen anyway.
    fn notice(self) -> Option<&'static str> {
        match self {
            CleanUpState::Ready => None,
            CleanUpState::NoVideos => Some("There are no videos in this folder to clean up."),
            CleanUpState::OnDevice => Some(
                "Stream names can’t be cleaned up on a device. Copy the files to this computer first.",
            ),
            CleanUpState::Unreadable => {
                Some("This folder can’t be read, so it can’t be cleaned up.")
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Pane {
    Sidebar,
    Inspector,
}

/// The side panes as drawn this frame: `None` when closed or when the
/// window is too narrow for it. Computed by [`fit`] only.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Panes {
    sidebar: Option<Pixels>,
    inspector: Option<Pixels>,
}

impl Panes {
    fn shows(&self, pane: Pane) -> bool {
        match pane {
            Pane::Sidebar => self.sidebar.is_some(),
            Pane::Inspector => self.inspector.is_some(),
        }
    }

    /// Width beside the file list, dividers included.
    fn beside_list(&self) -> Pixels {
        [self.sidebar, self.inspector]
            .into_iter()
            .flatten()
            .fold(px(0.), |sum, width| sum + width + size::HAIRLINE)
    }
}

/// Which panes fit in a window `viewport` wide. Each wanted pane comes as
/// (preferred, narrowest) width; `comfort` is the width that shows all five
/// columns of the folder shown ([`browser::list_comfort`]: 648pt for a
/// mixed folder, 692pt for a folder of audio, whose fifth is Date
/// Modified).
///
/// First the list is given `comfort`: the inspector gives up width toward
/// its narrowest, then the sidebar (decision 58, F4-3). When even their
/// narrowest leave the list short of that, the list settles for
/// `size::LIST_MIN`, its four essential columns, and the panes go back to
/// their preferred widths as far as that allows, the inspector again giving
/// way first. Narrower still, both stay docked at their narrowest as long
/// as the list keeps [`browser::list_essential`] (Name at its floor beside
/// the essentials), so Places stays beside the list at 940pt (S4-4). Only
/// then does the sidebar close (it can still be opened over the list, see
/// [`Workspace::open_places`]). A pane drawn alone gives way in the same
/// order of targets, and below `LIST_MIN` keeps its narrowest while the
/// list keeps `size::CONTENT_MIN`, so either pane can always be opened in
/// the smallest window. The preferences themselves never change here.
fn fit(
    viewport: Pixels,
    comfort: Pixels,
    sidebar: Option<(Pixels, Pixels)>,
    inspector: Option<(Pixels, Pixels)>,
) -> Panes {
    // Both panes beside a list of at least `list`, the inspector yielding
    // first; `None` when their narrowest leave less.
    let both = |list: Pixels, (s, i): ((Pixels, Pixels), (Pixels, Pixels))| {
        let room = viewport - list - size::HAIRLINE * 2.;
        let inspector = i.0.min(room - s.0).max(i.1);
        let sidebar = s.0.min(room - inspector).max(s.1);
        (sidebar + inspector <= room).then_some((sidebar, inspector))
    };
    // Both at their narrowest, the rest to a list that keeps its essentials.
    let compact = |(s, i): ((Pixels, Pixels), (Pixels, Pixels))| {
        let room = viewport - browser::list_essential() - size::HAIRLINE * 2.;
        (s.1 + i.1 <= room).then_some((s.1, i.1))
    };
    let alone = |(preferred, narrowest): (Pixels, Pixels)| {
        let beside = |list: Pixels| preferred.min(viewport - list - size::HAIRLINE);
        let width = Some(beside(comfort))
            .filter(|&width| width >= narrowest)
            .unwrap_or_else(|| beside(size::LIST_MIN).max(narrowest));
        (viewport - width - size::HAIRLINE >= size::CONTENT_MIN).then_some(width)
    };
    let (sidebar, inspector) = match (sidebar, inspector) {
        (Some(s), Some(i)) => {
            match both(comfort, (s, i))
                .or_else(|| both(size::LIST_MIN, (s, i)))
                .or_else(|| compact((s, i)))
            {
                Some((sidebar, inspector)) => (Some(sidebar), Some(inspector)),
                None => match alone(i) {
                    Some(w) => (None, Some(w)),
                    None => (alone(s), None),
                },
            }
        }
        (Some(s), None) => (alone(s), None),
        (None, Some(i)) => (None, alone(i)),
        (None, None) => (None, None),
    };
    Panes { sidebar, inspector }
}

/// Width of the fixed leading cluster: the main menu where the OS puts it
/// first, then sidebar toggle, back, forward.
fn cluster_width() -> Pixels {
    let buttons = match platform::chrome().menu {
        MenuPlacement::TitlebarLeading => 4.,
        _ => 3.,
    };
    size::ICON_BUTTON * buttons + size::GAP_TINY * (buttons - 1.)
}

/// Narrowest the sidebar is drawn: never so narrow that its divider would
/// cut through the leading cluster above it.
fn sidebar_floor() -> Pixels {
    let cluster_end = platform::chrome().leading + cluster_width() + size::ROW_INSET;
    cluster_end.max(size::SIDEBAR.0).min(size::SIDEBAR.2)
}

pub struct Workspace {
    browser: Entity<Browser>,
    inspector: Entity<Inspector>,
    search: Entity<InputState>,
    /// The folder-wide stream name review, while its sheet is open.
    cleanup: Option<Entity<Cleanup>>,
    devices: Entity<Devices>,
    /// Keeps Locations in step with disks being connected and ejected.
    _volumes: Option<(notify::RecommendedWatcher, Task<()>)>,
    /// The last folder shown that is on this computer.
    last_local: PathBuf,
    favourites: Vec<Place>,
    locations: Vec<Place>,
    /// The person's choices, saved. What is drawn comes from [`Workspace::panes`].
    sidebar_visible: bool,
    inspector_visible: bool,
    sidebar_width: Pixels,
    inspector_width: Pixels,
    /// The sidebar is open over the list because the window is too narrow
    /// to dock it beside the inspector. Escape, a click elsewhere or
    /// choosing a place closes it.
    places_overlay: bool,
    /// One per place, kept while it is listed, so focus stays on a place
    /// when the panel docks or the list changes, and its eject button shows
    /// while its row has keyboard focus.
    place_focus: HashMap<PathBuf, FocusHandle>,
    /// The place the arrow keys last moved to: Places' one tab stop, until
    /// a place is chosen (then the current place is).
    places_cursor: Option<PathBuf>,
    /// Where each place's row was drawn last, for a menu opened on it from
    /// the keyboard.
    place_bounds: Rc<RefCell<HashMap<PathBuf, Bounds<Pixels>>>>,
    /// A place's context menu opened from the keyboard.
    places_menu: ui::KeyMenu,
    /// Disks being ejected.
    ejecting: Vec<PathBuf>,
    /// The About sheet is open.
    about: bool,
    about_focus: FocusHandle,
    /// Where keyboard focus was when a sheet or the Places overlay opened;
    /// it goes back there when it closes.
    return_focus: Option<WeakFocusHandle>,
    /// The divider being dragged.
    resizing: Option<Pane>,
    save: Option<Task<()>>,
    /// The search field is open although the titlebar is tight: it was
    /// clicked, Find was chosen, or it has focus. When the titlebar is
    /// tight and this is off, search is a button.
    search_open: bool,
    /// Whether Clean Up can run here.
    clean_up: CleanUpState,
    /// The folder listed with nothing in it, or could not be read: there is
    /// nothing to search. Kept from the last listing while the next loads.
    nothing_listed: bool,
    /// How many items are selected, and whether the one selected is media
    /// that can be probed; kept from each selection change for
    /// [`Workspace::availability`].
    selected: usize,
    single_media: bool,
    /// The "Main Menu" button, so F10 or a bare Alt can open it.
    main_menu_focus: FocusHandle,
    /// Alt is down and nothing else has been pressed with it.
    alt_alone: bool,
    /// The checkmarks last given to the Mac menu bar.
    menu_checks: Option<Availability>,
}

impl Workspace {
    pub fn new(saved: &Saved, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let folder = saved
            .folder
            .clone()
            .filter(|f| f.is_dir() || mtp::is_device_path(f))
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("/"));
        let sort = match saved.sort.as_str() {
            "size" => SortKey::Size,
            "modified" => SortKey::Modified,
            "duration" => SortKey::Duration,
            "video" => SortKey::Video,
            "audio" => SortKey::Audio,
            _ => SortKey::Name,
        };
        let browser =
            cx.new(|cx| Browser::new(folder, (sort, saved.ascending), saved.show_hidden, cx));
        let inspector = cx.new(|cx| Inspector::new(window, cx));
        cx.subscribe(
            &inspector,
            |this, _, event: &InspectorEvent, cx| match event {
                InspectorEvent::Rename { path, name } => {
                    this.browser
                        .update(cx, |browser, cx| browser.rename_path(path, name, cx));
                }
            },
        )
        .detach();
        let placeholder = search_placeholder(browser.read(cx).dir());
        let search = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(placeholder)
                .clean_on_escape()
        });
        cx.subscribe_in(
            &search,
            window,
            |this, search, event: &InputEvent, _, cx| match event {
                InputEvent::Change => {
                    let query = search.read(cx).value();
                    this.browser
                        .update(cx, |browser, cx| browser.set_query(&query, cx));
                }
                InputEvent::Focus => this.search_open = true,
                // An empty field that loses focus folds back into its button
                // when the titlebar is tight.
                InputEvent::Blur if search.read(cx).value().is_empty() => {
                    this.search_open = false;
                    cx.notify();
                }
                _ => {}
            },
        )
        .detach();

        cx.subscribe_in(&browser, window, |this, browser, event, window, cx| {
            match event {
                BrowserEvent::SelectionChanged => {
                    let selection = browser.read(cx).selection();
                    this.selected = selection.len();
                    this.single_media = match selection.as_slice() {
                        [one] => one.kind.is_probeable(),
                        _ => false,
                    };
                    this.inspector
                        .update(cx, |inspector, cx| inspector.show(selection, cx));
                }
                BrowserEvent::Navigated => {
                    let dir = browser.read(cx).dir().to_owned();
                    window.set_window_title(&folder_name(&dir));
                    this.inspector.update(cx, |inspector, cx| {
                        inspector.show_folder(dir.clone(), None, cx)
                    });
                    // A search belongs to the folder it was typed in, and
                    // says which one.
                    this.search.update(cx, |search, cx| {
                        search.set_value("", window, cx);
                        search.set_placeholder(search_placeholder(&dir), window, cx);
                    });
                    // Until the listing arrives Clean Up keeps its state,
                    // except on a device, which is known at once.
                    if let Some(state) = CleanUpState::of(&dir, false, None) {
                        this.clean_up = state;
                    }
                }
                BrowserEvent::Listed => {
                    let (dir, listing, unreadable, filtered) = {
                        let browser = browser.read(cx);
                        let unreadable = browser.unreadable();
                        (
                            browser.dir().to_owned(),
                            browser.listing(),
                            unreadable,
                            browser.filtered(),
                        )
                    };
                    // Every listing, including one that changed because
                    // files arrived or left, decides Clean Up again.
                    if let Some(state) = CleanUpState::of(&dir, unreadable, listing.as_deref()) {
                        this.clean_up = state;
                    }
                    if let Some(nothing) = nothing_listed(unreadable, listing.as_deref()) {
                        this.nothing_listed = nothing;
                    }
                    this.inspector.update(cx, |inspector, cx| match unreadable {
                        // The list shows the error; the inspector doesn't
                        // call the folder empty beside it.
                        true => inspector.forget_folder(cx),
                        // Every change to the search lists again, so the
                        // summary always knows whether it sits beside the
                        // whole folder or a search's matches.
                        false => {
                            inspector.show_folder(dir, listing, cx);
                            inspector.set_filtered(filtered, cx);
                        }
                    });
                }
            }
            this.schedule_save(window, cx);
            cx.notify();
        })
        .detach();

        // The sidebar lists devices as they come and go.
        let devices = Devices::global(cx);
        cx.observe(&devices, |_, _, cx| cx.notify()).detach();

        // Selection colours and titlebar dimming depend on whether the window
        // is key.
        // A changed scroll-bar setting is picked up then too.
        cx.observe_window_activation(window, |_, window, cx| {
            theme::sync_scroll_bars(cx);
            window.refresh();
        })
        .detach();
        cx.observe_window_appearance(window, |_, window, cx| {
            theme::apply(window.appearance(), Some(window), cx);
            window.refresh();
        })
        .detach();
        cx.observe_window_bounds(window, |this, window, cx| this.schedule_save(window, cx))
            .detach();

        window.set_window_title(&folder_name(browser.read(cx).dir()));
        browser.read(cx).focus_handle(cx).focus(window, cx);

        let (clean_up, nothing_listed) = {
            let browser = browser.read(cx);
            let listing = browser.listing();
            (
                CleanUpState::of(browser.dir(), browser.unreadable(), listing.as_deref())
                    .unwrap_or(CleanUpState::Ready),
                nothing_listed(browser.unreadable(), listing.as_deref()).unwrap_or(false),
            )
        };
        Workspace {
            browser,
            inspector,
            search,
            cleanup: None,
            devices,
            _volumes: watch_volumes(cx),
            last_local: dirs::home_dir().unwrap_or_else(|| PathBuf::from("/")),
            favourites: favourites(),
            locations: locations(),
            sidebar_visible: saved.sidebar_visible,
            inspector_visible: saved.inspector_visible,
            sidebar_width: px(saved.sidebar_width).clamp(size::SIDEBAR.0, size::SIDEBAR.2),
            inspector_width: px(saved.inspector_width).clamp(size::INSPECTOR.0, size::INSPECTOR.2),
            places_overlay: false,
            place_focus: HashMap::new(),
            places_cursor: None,
            place_bounds: Rc::default(),
            places_menu: ui::KeyMenu::default(),
            ejecting: Vec::new(),
            about: false,
            about_focus: cx.focus_handle(),
            return_focus: None,
            resizing: None,
            save: None,
            search_open: false,
            clean_up,
            nothing_listed,
            selected: 0,
            single_media: false,
            main_menu_focus: cx.focus_handle(),
            alt_alone: false,
            menu_checks: None,
        }
    }

    /// Whether a modal sheet is open; the window underneath stays put.
    fn modal(&self) -> bool {
        self.cleanup.is_some() || self.about
    }

    /// Notes where keyboard focus is, to return it there when the sheet or
    /// overlay opening now closes.
    fn remember_focus(&mut self, window: &Window, cx: &App) {
        self.return_focus = window.focused(cx).map(|handle| handle.downgrade());
    }

    /// Puts keyboard focus back where it was before the sheet or overlay
    /// opened; the list when that is gone (a menu that has closed).
    fn restore_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let handle = self
            .return_focus
            .take()
            .and_then(|weak| weak.upgrade())
            .unwrap_or_else(|| self.browser.read(cx).focus_handle(cx));
        handle.focus(window, cx);
    }

    /// Opens the search field and puts the cursor in it.
    fn find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Nothing to search in an empty folder.
        if self.modal() || !self.can_search(cx) {
            return;
        }
        self.close_places(false, window, cx);
        self.search_open = true;
        self.search
            .update(cx, |search, cx| search.focus(window, cx));
        cx.notify();
    }

    /// Whether the folder has anything to search: not when it is empty or
    /// could not be read, unless a search is already typed.
    fn can_search(&self, cx: &App) -> bool {
        !self.nothing_listed || !self.search.read(cx).value().is_empty()
    }

    /// What the commands can act on now: the one source for the Main
    /// Menu's enabled and checked items, which handlers this window
    /// registers (so the Mac menu bar greys out the same items), and the
    /// Mac menu bar's checkmarks.
    fn availability(&self, window: &Window, cx: &App) -> Availability {
        let browser = self.browser.read(cx);
        let devices = self.devices.read(cx);
        let dir = browser.dir();
        let panes = self.panes(window.viewport_size().width, cx);
        Availability {
            items: !self.nothing_listed,
            selected: self.selected,
            single_media: self.single_media,
            local: !mtp::is_device_path(dir),
            pasteboard: devices.clipboard().len(),
            clean_up: self.clean_up.available(),
            back: browser.can_go_back(),
            forward: browser.can_go_forward(),
            parent: dir.parent().is_some(),
            // Exactly what the activity popover can show: a running
            // transfer or a receipt nobody has dismissed.
            transfers: devices.has_details(),
            sidebar: panes.sidebar.is_some() || self.places_overlay,
            inspector: panes.inspector.is_some(),
            hidden_files: browser.show_hidden(),
            // The order on screen, which the View menu checks.
            sort: Sort::of(browser.sort().0),
        }
    }

    /// Gives the Mac menu bar its checkmarks again when one changes. Its
    /// enabled states follow the handlers registered in `render`.
    fn update_menu_bar(&mut self, availability: Availability, cx: &mut Context<Self>) {
        if platform::chrome().menu != MenuPlacement::MenuBar {
            return;
        }
        let checks = Availability {
            sidebar: availability.sidebar,
            inspector: availability.inspector,
            hidden_files: availability.hidden_files,
            sort: availability.sort,
            ..Availability::EVERYTHING
        };
        if self.menu_checks != Some(checks) {
            self.menu_checks = Some(checks);
            cx.defer(move |cx| cx.set_menus(menu_bar(&checks)));
        }
    }

    /// Opens the "Main Menu" from the keyboard (F10, a bare Alt), where the
    /// OS has no menu bar.
    fn show_main_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if platform::chrome().menu == MenuPlacement::MenuBar || self.modal() {
            return;
        }
        // Focus goes back where it was before the panel opened, so the menu
        // returns it there when it closes.
        self.close_places(true, window, cx);
        ui::open_menu(&self.main_menu_focus, window, cx);
    }

    /// Watches for Alt pressed and released on its own, which opens the
    /// Main Menu on Windows and GNOME.
    fn modifiers_changed(
        &mut self,
        event: &ModifiersChangedEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !platform::alt_opens_menu() {
            return;
        }
        let keys = event.modifiers;
        let alt_only = keys.alt && !keys.control && !keys.shift && !keys.platform && !keys.function;
        if alt_only {
            self.alt_alone = true;
        } else if self.alt_alone && !keys.modified() {
            self.alt_alone = false;
            self.show_main_menu(window, cx);
        } else {
            self.alt_alone = false;
        }
    }

    /// The side panes as they are drawn in a window `viewport` wide, fitted
    /// to the columns of the folder shown. The one place that decides; the
    /// saved choices are not touched.
    fn panes(&self, viewport: Pixels, cx: &App) -> Panes {
        fit(
            viewport,
            self.comfort(cx),
            self.sidebar_visible.then_some(self.sidebar_wanted()),
            self.inspector_visible
                .then_some((self.inspector_width, size::INSPECTOR.0)),
        )
    }

    /// The list width that shows the folder's five columns.
    fn comfort(&self, cx: &App) -> Pixels {
        browser::list_comfort(self.browser.read(cx).audio_folder())
    }

    /// The sidebar's (preferred, narrowest) width.
    fn sidebar_wanted(&self) -> (Pixels, Pixels) {
        let floor = sidebar_floor();
        (self.sidebar_width.max(floor), floor)
    }

    /// Whether the sidebar would be docked beside whatever else is open, if
    /// it were asked for.
    fn sidebar_docks(&self, viewport: Pixels, cx: &App) -> bool {
        fit(
            viewport,
            self.comfort(cx),
            Some(self.sidebar_wanted()),
            self.inspector_visible
                .then_some((self.inspector_width, size::INSPECTOR.0)),
        )
        .sidebar
        .is_some()
    }

    /// Opens the sidebar over the list, for a window too narrow to dock it
    /// beside the inspector: Places stay reachable and the inspector stays.
    /// The keyboard lands on the current place (S4-2), Up and Down move
    /// between places, Return opens one and Escape goes back.
    fn open_places(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.places_overlay || self.modal() {
            return;
        }
        self.remember_focus(window, cx);
        self.places_overlay = true;
        self.places_cursor = None;
        let places = self.places(cx);
        self.keep_place_focus(&places, cx);
        if let Some(handle) = self
            .places_stop(&places, cx)
            .and_then(|stop| self.place_focus.get(&stop))
        {
            handle.focus(window, cx);
        }
        cx.notify();
    }

    /// Every place, top to bottom: Favourites, Locations, then Devices.
    fn places(&self, cx: &App) -> Vec<Place> {
        let devices = self
            .devices
            .read(cx)
            .connected()
            .iter()
            .map(|device| Place {
                name: device.name.clone(),
                icon: IconName::Smartphone,
                path: device.path.clone(),
                ejectable: false,
            });
        self.favourites
            .iter()
            .chain(&self.locations)
            .cloned()
            .chain(devices)
            .collect()
    }

    /// The place nearest above the folder shown, which stays lit so the
    /// sidebar always says where you are.
    fn lit_place(&self, places: &[Place], cx: &App) -> Option<PathBuf> {
        let current = self.browser.read(cx).dir();
        places
            .iter()
            .map(|place| &place.path)
            .filter(|path| current.starts_with(path))
            .max_by_key(|path| path.components().count())
            .cloned()
    }

    /// Places' one tab stop: where the arrows last moved, else the current
    /// place, else the first.
    fn places_stop(&self, places: &[Place], cx: &App) -> Option<PathBuf> {
        self.places_cursor
            .clone()
            .filter(|cursor| places.iter().any(|place| &place.path == cursor))
            .or_else(|| self.lit_place(places, cx))
            .or_else(|| places.first().map(|place| place.path.clone()))
    }

    /// A focus handle per place, kept while the place is listed.
    fn keep_place_focus(&mut self, places: &[Place], cx: &mut Context<Self>) {
        self.place_focus
            .retain(|path, _| places.iter().any(|place| &place.path == path));
        for place in places {
            self.place_focus
                .entry(place.path.clone())
                .or_insert_with(|| cx.focus_handle());
        }
    }

    /// Whether keyboard focus is on a place or inside one (its eject
    /// button).
    fn place_has_focus(&self, window: &Window, cx: &App) -> bool {
        self.place_focus
            .values()
            .any(|handle| handle.contains_focused(window, cx))
    }

    /// Up and Down (Home and End) between places, in the docked sidebar and
    /// the panel alike.
    fn step_place(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let places = self.places(cx);
        let paths: Vec<&PathBuf> = places.iter().map(|place| &place.path).collect();
        let has_focus = |path: &PathBuf| {
            self.place_focus
                .get(path)
                .is_some_and(|handle| handle.contains_focused(window, cx))
        };
        let Some(from) = paths.iter().position(|path| has_focus(path)) else {
            return false;
        };
        let to = match key {
            "up" => from.saturating_sub(1),
            "down" => (from + 1).min(paths.len() - 1),
            "home" => 0,
            "end" => paths.len() - 1,
            _ => return false,
        };
        if let Some(handle) = self.place_focus.get(paths[to]) {
            handle.focus(window, cx);
        }
        self.places_cursor = Some(paths[to].clone());
        cx.notify();
        true
    }

    /// Opens a place's context menu from the keyboard, under its name.
    fn place_menu_from_key(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let Some(row) = self.place_bounds.borrow().get(&path).copied() else {
            return;
        };
        let Some(place) = self.places(cx).into_iter().find(|place| place.path == path) else {
            return;
        };
        let at = point(
            row.left() + size::ROW_INSET + size::ICON + size::GAP,
            row.bottom(),
        );
        let Some(back) = self.place_focus.get(&path).cloned() else {
            return;
        };
        let workspace = cx.entity().downgrade();
        self.places_menu.open(at, window, cx, move |menu, _, _| {
            // Escape goes back to the place.
            place_menu(menu.action_context(back), workspace, &place)
        });
        cx.notify();
    }

    /// Closes the Places overlay, if open. `refocus` puts keyboard focus
    /// back where it was; a caller moving focus itself passes false.
    fn close_places(&mut self, refocus: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !self.places_overlay {
            return;
        }
        self.places_overlay = false;
        match refocus {
            true => self.restore_focus(window, cx),
            false => self.return_focus = None,
        }
        cx.notify();
    }

    /// Writes the window's state to disk shortly after it stops changing.
    fn schedule_save(&mut self, window: &Window, cx: &mut Context<Self>) {
        let browser = self.browser.read(cx);
        // The order chosen, which a folder with no video may show as Name.
        let (sort, ascending) = browser.saved_sort();
        let frame = match window.window_bounds() {
            WindowBounds::Windowed(b) => Some(b),
            _ => None,
        };
        // A device may not be there next time; reopen where the computer was left.
        if !mtp::is_device_path(browser.dir()) {
            self.last_local = browser.dir().to_owned();
        }
        let saved = Saved {
            folder: Some(self.last_local.clone()),
            sidebar_width: self.sidebar_width.into(),
            inspector_width: self.inspector_width.into(),
            sidebar_visible: self.sidebar_visible,
            inspector_visible: self.inspector_visible,
            show_hidden: browser.show_hidden(),
            sort: match sort {
                SortKey::Size => "size",
                SortKey::Modified => "modified",
                SortKey::Duration => "duration",
                SortKey::Video => "video",
                SortKey::Audio => "audio",
                SortKey::Name | SortKey::Kind => "name",
            }
            .into(),
            ascending,
            window: frame.map(|b| {
                [
                    b.origin.x.into(),
                    b.origin.y.into(),
                    b.size.width.into(),
                    b.size.height.into(),
                ]
            }),
        };
        self.save = Some(cx.spawn(async move |_, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(400))
                .await;
            cx.background_spawn(async move { saved.save() }).await;
        }));
    }

    fn go(&mut self, path: Option<PathBuf>, cx: &mut Context<Self>) {
        // A sheet is modal: the folder stays put under it.
        if self.modal() {
            return;
        }
        if let Some(path) = path.filter(|p| p.is_dir() || mtp::is_device_path(p)) {
            self.browser
                .update(cx, |browser, cx| browser.navigate(path, cx));
        }
    }

    /// Copies or moves dropped items into a sidebar place.
    fn receive(
        &mut self,
        sources: Vec<PathBuf>,
        dest: PathBuf,
        mode: Mode,
        cx: &mut Context<Self>,
    ) {
        self.devices
            .update(cx, |devices, cx| devices.transfer(sources, dest, mode, cx));
    }

    /// Opens the sheet reviewing stream names for everything under this
    /// folder. Keys go to the sheet until it closes; then back to the list.
    fn clean_up_names(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.modal() {
            return;
        }
        if let Some(notice) = self.clean_up.notice() {
            self.browser
                .update(cx, |browser, cx| browser.say(notice, cx));
            return;
        }
        let root = self.browser.read(cx).dir().to_owned();
        self.close_places(false, window, cx);
        self.remember_focus(window, cx);
        let cleanup = cx.new(|cx| Cleanup::new(root, cx));
        cx.subscribe_in(&cleanup, window, |this, _, _: &CleanupEvent, window, cx| {
            this.cleanup = None;
            this.restore_focus(window, cx);
            cx.notify();
        })
        .detach();
        cleanup.read(cx).focus_handle(cx).focus(window, cx);
        self.cleanup = Some(cleanup);
        cx.notify();
    }

    /// Opens the About sheet: the name, version and what Seer is for.
    fn show_about(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.modal() {
            return;
        }
        self.close_places(false, window, cx);
        self.remember_focus(window, cx);
        self.about = true;
        self.about_focus.focus(window, cx);
        cx.notify();
    }

    fn close_about(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.about {
            self.about = false;
            self.restore_focus(window, cx);
            cx.notify();
        }
    }

    fn render_about(&self, cx: &mut Context<Self>) -> AnyElement {
        let this = cx.entity().downgrade();
        let (dismiss, confirm) = (this.clone(), this);
        ui::Sheet::new("about", APP.name)
            .status(APP.description)
            // Seer's mark, the bit-rate trace, beside its name.
            .accessory(ui::trace_glyph(cx))
            .note(
                div()
                    .font_features(theme::tabular())
                    .child(format!("Version {} · {}", APP.version, APP.meaning)),
            )
            .focus(&self.about_focus)
            .on_dismiss(move |window, cx| {
                dismiss
                    .update(cx, |this, cx| this.close_about(window, cx))
                    .ok();
            })
            .on_confirm(move |window, cx| {
                confirm
                    .update(cx, |this, cx| this.close_about(window, cx))
                    .ok();
            })
            .button(
                ui::Button::new("close-about", "Done")
                    .primary()
                    .on_click(cx.listener(|this, _, window, cx| this.close_about(window, cx))),
            )
            .into_any_element()
    }

    /// Opens the transfer details from the keyboard: running copies and
    /// moves, or the last one's receipt.
    fn show_transfers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.modal() {
            return;
        }
        self.close_places(false, window, cx);
        let shown = self
            .devices
            .update(cx, |devices, cx| devices.show_details(cx));
        if !shown {
            self.browser.update(cx, |browser, cx| {
                browser.say("There are no copies or moves to show.", cx)
            });
        }
    }

    /// Hides a pane that is showing and shows one that is not. A sidebar
    /// that cannot be docked beside the inspector opens over the list
    /// instead, so the inspector stays; asked for again, it closes.
    fn toggle(&mut self, pane: Pane, window: &mut Window, cx: &mut Context<Self>) {
        let viewport = window.viewport_size().width;
        if pane == Pane::Sidebar && self.places_overlay {
            self.close_places(true, window, cx);
            return;
        }
        let showing = self.panes(viewport, cx).shows(pane);
        if pane == Pane::Sidebar && !showing && !self.sidebar_docks(viewport, cx) {
            self.open_places(window, cx);
            return;
        }
        let visible = match pane {
            Pane::Sidebar => &mut self.sidebar_visible,
            Pane::Inspector => &mut self.inspector_visible,
        };
        *visible = !showing;
        self.schedule_save(window, cx);
        cx.notify();
    }

    fn resize(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pane) = self.resizing else { return };
        if !event.dragging() {
            self.resizing = None;
            self.schedule_save(window, cx);
            return;
        }
        let viewport = window.viewport_size().width;
        let panes = self.panes(viewport, cx);
        let other = match pane {
            Pane::Sidebar => panes.inspector,
            Pane::Inspector => panes.sidebar,
        }
        .map_or(px(0.), |width| width + size::HAIRLINE);
        // Never squeeze the file list below what it needs.
        let room = viewport - other - size::HAIRLINE - size::LIST_MIN;
        match pane {
            Pane::Sidebar => {
                self.sidebar_width = event
                    .position
                    .x
                    .min(room)
                    .clamp(sidebar_floor(), size::SIDEBAR.2);
            }
            Pane::Inspector => {
                self.inspector_width = (viewport - event.position.x)
                    .min(room)
                    .clamp(size::INSPECTOR.0, size::INSPECTOR.2);
            }
        }
        // The panes yield to keep the list's five columns, so a drag past
        // that point would move nothing on screen; the divider stays with
        // what is drawn.
        let drawn = self.panes(viewport, cx);
        match pane {
            Pane::Sidebar => {
                if let Some(width) = drawn.sidebar {
                    self.sidebar_width = self.sidebar_width.min(width.max(sidebar_floor()));
                }
            }
            Pane::Inspector => {
                if let Some(width) = drawn.inspector {
                    self.inspector_width = self.inspector_width.min(width);
                }
            }
        }
        cx.notify();
    }

    fn open_folder(&mut self, _: &OpenFolder, window: &mut Window, cx: &mut Context<Self>) {
        // A sheet is modal.
        if self.modal() {
            return;
        }
        self.close_places(true, window, cx);
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Open".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = chosen.await {
                this.update(cx, |this, cx| this.go(paths.into_iter().next(), cx))
                    .ok();
            }
        })
        .detach();
    }

    /// The places: Favourites, Locations and Devices. Docked beside the list,
    /// or, with `overlay`, as a panel over it (see [`Workspace::open_places`]).
    /// One tab stop for all of them (the current place, or where the arrows
    /// last moved); Up and Down, Home and End move between places; Return or
    /// a click opens one; the context-menu key (Shift+F10, the Menu key, ⌃↩)
    /// opens its menu. A disk other than the startup disk has an eject
    /// button that shows while the row is hovered or has focus, and Tab
    /// reaches it from its row. Each group is read as a list of places, the
    /// current one selected.
    fn render_sidebar(
        &mut self,
        width: Pixels,
        overlay: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = Theme::of(cx).clone();
        let active = window.is_window_active();
        let places = self.places(cx);
        self.keep_place_focus(&places, cx);
        let lit = self.lit_place(&places, cx);
        let stop = self.places_stop(&places, cx);
        for (path, handle) in &self.place_focus {
            handle.clone().tab_stop(Some(path) == stop.as_ref());
        }
        let place_focus = self.place_focus.clone();
        let place_bounds = Rc::clone(&self.place_bounds);
        let ejecting = self.ejecting.clone();
        let devices: Vec<Place> = places
            .iter()
            .filter(|place| mtp::is_device_path(&place.path))
            .map(Place::clone)
            .collect();

        let group = |title: &'static str, places: &[Place], cx: &mut Context<Self>| {
            let count = places.len();
            div()
                .id(title)
                .role(Role::ListBox)
                .aria_label(title)
                .flex()
                .flex_col()
                .child(
                    div()
                        .px(size::ROW_INSET)
                        .pb(size::GAP_SMALL)
                        .child(ui::section_header(title, cx)),
                )
                .children(places.iter().enumerate().map(|(i, place)| {
                    let selected = lit.as_ref() == Some(&place.path);
                    let is_stop = stop.as_ref() == Some(&place.path);
                    let path = place.path.clone();
                    let (drop_rows, drop_files) = (place.path.clone(), place.path.clone());
                    let (keyed, probed) = (place.path.clone(), place.path.clone());
                    let workspace = cx.entity().downgrade();
                    let (hover, target, focus) = (t.fill_hover, t.fill_on, t.focus_shadow());
                    let id = SharedString::from(place.path.to_string_lossy().into_owned());
                    let handle = place_focus[&place.path].clone();
                    let bounds = Rc::clone(&place_bounds);
                    let eject = place.ejectable.then(|| {
                        let busy = ejecting.contains(&place.path);
                        // Shown while the row is hovered or focused, and
                        // while the disk is being ejected.
                        let shown = busy || handle.contains_focused(window, cx);
                        let disk = place.path.clone();
                        div()
                            .flex_none()
                            .when(!shown, |this| {
                                this.invisible()
                                    .group_hover(id.clone(), |style| style.visible())
                            })
                            .child(
                                ui::IconButton::new(
                                    "eject",
                                    IconName::Eject,
                                    format!("Eject “{}”", place.name),
                                )
                                .small()
                                // Reached by Tab from its row, the group's
                                // one stop, while that row has the keyboard.
                                .tab_stop(is_stop || shown)
                                .disabled(busy)
                                .on_click(cx.listener(
                                    move |this, _, window, cx| this.eject(disk.clone(), window, cx),
                                )),
                            )
                    });
                    let menu_place = place.clone();
                    div()
                        .id(id.clone())
                        .group(id.clone())
                        .role(Role::ListBoxOption)
                        .aria_label(place.name.clone())
                        .aria_selected(selected)
                        .aria_position_in_set(i + 1)
                        .aria_size_of_set(count)
                        .relative()
                        .flex()
                        .flex_none()
                        .items_center()
                        .gap(size::GAP)
                        .h(size::SIDEBAR_ROW)
                        .px(size::ROW_INSET)
                        .rounded(size::RADIUS_CONTROL)
                        .track_focus(&handle)
                        .focus_visible(move |s| s.shadow(focus.clone()))
                        .map(|this| match selected {
                            // Lit in an active window; quieter in one that is not.
                            true => this.bg(if active { t.fill_on } else { t.fill }),
                            false => this.hover(move |s| s.bg(hover)),
                        })
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.choose_place(path.clone(), window, cx)
                        }))
                        .on_action(cx.listener(move |this, _: &ShowContextMenu, window, cx| {
                            this.place_menu_from_key(keyed.clone(), window, cx)
                        }))
                        // Where the row is, for a menu opened on it by key.
                        .child(
                            canvas(
                                move |row, _, _| {
                                    bounds.borrow_mut().insert(probed.clone(), row);
                                },
                                |_, _, _, _| {},
                            )
                            .absolute()
                            .inset_0(),
                        )
                        .child(ui::icon(
                            place.icon,
                            if selected { t.label } else { t.secondary_label },
                        ))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child(place.name.clone()),
                        )
                        .children(eject)
                        // Rows dragged from the list, or files from the
                        // file manager, go into the place they are dropped on.
                        .drag_over::<Dragged>(move |style, _, _, _| style.bg(target))
                        .drag_over::<ExternalPaths>(move |style, _, _, _| style.bg(target))
                        .on_drop(cx.listener(move |this, dragged: &Dragged, _, cx| {
                            let mode = dragged.mode(&drop_rows);
                            this.receive(dragged.0.to_vec(), drop_rows.clone(), mode, cx);
                        }))
                        .on_drop(cx.listener(move |this, dropped: &ExternalPaths, _, cx| {
                            let sources = dropped.paths().to_vec();
                            this.receive(sources, drop_files.clone(), Mode::Copy, cx);
                        }))
                        .context_menu(move |menu, _, _| {
                            place_menu(menu, workspace.clone(), &menu_place)
                        })
                }))
        };

        let favourites = self.favourites.len();
        let locations = self.locations.len();
        div()
            .id(if overlay {
                "places-overlay-list"
            } else {
                "places"
            })
            .flex()
            .flex_col()
            .flex_none()
            .w(width)
            // Docked, the full height of the window; over the list, as tall
            // as its rows, shrinking to scroll inside the panel.
            .map(|this| match overlay {
                true => this.flex_shrink(1.).min_h_0(),
                false => this.h_full(),
            })
            .overflow_y_scroll()
            .gap(size::GAP_GROUP)
            .px(size::ROW_INSET)
            .pt(size::GAP)
            .pb(size::PANE_PADDING)
            .when(!overlay, |this| this.bg(t.surface_window))
            .text_size(text::BODY)
            .line_height(text::BODY_LINE)
            .text_color(t.label)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.modifiers.modified() {
                    return;
                }
                if this.step_place(event.keystroke.key.as_str(), window, cx) {
                    cx.stop_propagation();
                }
            }))
            .child(group("Favourites", &places[..favourites], cx))
            .child(group(
                "Locations",
                &places[favourites..favourites + locations],
                cx,
            ))
            .when(!devices.is_empty(), |this| {
                this.child(group("Devices", &devices, cx))
            })
            .into_any_element()
    }

    /// Opens a place chosen in the sidebar, closes the sidebar when it was
    /// open over the list, and gives the keyboard back to the list.
    fn choose_place(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if self.modal() {
            return;
        }
        // The new current place becomes Places' tab stop.
        self.places_cursor = None;
        self.close_places(false, window, cx);
        self.go(Some(path), cx);
        self.browser.read(cx).focus_handle(cx).focus(window, cx);
    }

    /// The sidebar as a panel over the list, for a window too narrow to dock
    /// it beside the inspector: the overlay surface and shadow, under the
    /// titlebar at the leading edge. Escape or a press outside closes it.
    fn render_places_overlay(&mut self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let width = self.sidebar_wanted().0;
        let top = platform::chrome().titlebar;
        // As tall as its places, so the list below them stays in view; in a
        // window too short for them it stops above the bottom and scrolls.
        let tallest = window.viewport_size().height - top - size::GAP_SMALL * 2.;
        let sidebar = self.render_sidebar(width, true, window, cx);
        let card = ui::overlay_card(
            div()
                .id("places-overlay")
                // A panel that takes the keyboard and goes with Escape.
                .role(Role::Dialog)
                .aria_label("Places")
                .absolute()
                .top(top + size::GAP_SMALL)
                .left(size::GAP_SMALL)
                .max_h(tallest)
                .flex()
                .flex_col()
                .w(width)
                .occlude()
                .overflow_hidden()
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    if event.keystroke.key == "escape" && !event.keystroke.modifiers.modified() {
                        cx.stop_propagation();
                        this.close_places(true, window, cx);
                    }
                })),
            cx,
        )
        .p_0()
        .child(sidebar);
        div()
            .absolute()
            .inset_0()
            // A press anywhere else, the sidebar button included, closes it
            // and goes no further.
            .child(
                div()
                    .id("places-click-away")
                    .absolute()
                    .inset_0()
                    .occlude()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            cx.stop_propagation();
                            this.close_places(true, window, cx);
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, _, window, cx| {
                            cx.stop_propagation();
                            this.close_places(true, window, cx);
                        }),
                    ),
            )
            .child(card)
            .into_any_element()
    }

    /// Unmounts and ejects a disk off the main thread, leaving it first if
    /// it is the one shown. The sidebar drops it when it has gone.
    fn eject(&mut self, disk: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if self.ejecting.contains(&disk) || self.modal() {
            return;
        }
        if self.browser.read(cx).dir().starts_with(&disk) {
            self.choose_place(
                dirs::home_dir().unwrap_or_else(|| PathBuf::from("/")),
                window,
                cx,
            );
        }
        self.ejecting.push(disk.clone());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let target = disk.clone();
            let ejected = cx
                .background_spawn(async move { native::eject(&target) })
                .await;
            this.update(cx, |this, cx| {
                this.ejecting.retain(|path| path != &disk);
                match ejected {
                    true => this.locations.retain(|place| place.path != disk),
                    false => this.browser.update(cx, |browser, cx| {
                        browser.say(
                            "The disk couldn’t be ejected. An app may still be using it.",
                            cx,
                        )
                    }),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// One strip across the window. Leading, right after the window
    /// controls and fixed there whether the sidebar is open or not: the
    /// main menu where the OS has no menu bar and puts it first, then
    /// sidebar toggle, back, forward. Then the breadcrumb, starting over
    /// the list, then search and Clean Up. Trailing: the inspector toggle,
    /// the main menu where the OS puts it last, and a hairline before
    /// window controls on that side.
    ///
    /// Each pane owns the strip above it: over the sidebar it is the
    /// sidebar's surface, over the inspector the inspector's, each with its
    /// divider running to the top; over the list it is the list's, ruled
    /// off below. With the inspector open its strip holds only the trailing
    /// controls, and the list's controls stop at its divider.
    ///
    /// Room is given up in this order as the strip tightens: Clean Up drops
    /// its label, search narrows, then search becomes a button that opens
    /// the field; the breadcrumb takes what is left, and `drag_min` of bare
    /// strip is always kept for moving the window.
    fn render_titlebar(
        &self,
        panes: Panes,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let t = Theme::of(cx).clone();
        let chrome = platform::chrome();
        let viewport = window.viewport_size().width;
        let browser = self.browser.read(cx);
        let (back, forward) = (browser.can_go_back(), browser.can_go_forward());
        let dir = browser.dir().to_owned();
        let menu_leading = chrome.menu == MenuPlacement::TitlebarLeading;
        let menu_trailing = chrome.menu == MenuPlacement::TitlebarTrailing;
        let separator = chrome.controls_separator && chrome.controls == Side::Trailing;

        // The trailing group, and where the list's controls must stop.
        let trailing = size::ICON_BUTTON
            + if menu_trailing {
                size::GAP_TINY + size::ICON_BUTTON
            } else {
                px(0.)
            }
            + if separator {
                size::GAP * 2. + size::HAIRLINE
            } else {
                px(0.)
            }
            + chrome.trailing;
        let trailing_box = match panes.inspector {
            Some(width) => (width + size::HAIRLINE).max(trailing),
            None => trailing,
        };

        // Where the breadcrumb starts: over the list's leading edge, or
        // right after the cluster when the sidebar is closed.
        let cluster_end = chrome.leading + cluster_width();
        let crumbs_start = match panes.sidebar {
            Some(width) => (width + size::HAIRLINE).max(cluster_end) + size::ROW_INSET,
            None => cluster_end + size::GAP,
        };
        // The list's controls sit between GAP on either side.
        let room = viewport - trailing_box - crumbs_start - size::GAP * 2. - chrome.drag_min;
        let clean_up_label = "Clean Up…";
        let labelled = size::BUTTON_PADDING * 2.
            + size::ICON
            + size::GAP_SMALL
            + ui::text_width(
                &clean_up_label.into(),
                text::BODY,
                FontWeight::NORMAL,
                window,
            )
            + size::HAIRLINE * 2.;
        // A search in use, or asked for, stays a field.
        let searching = self.search_open || !self.search.read(cx).value().is_empty();
        let fields = [
            (true, Some(size::SEARCH_WIDTH)),
            (false, Some(size::SEARCH_WIDTH)),
            (false, Some(size::SEARCH_WIDTH_COMPACT)),
            (false, None),
        ];
        let used = |(label, search): (bool, Option<Pixels>)| {
            (if label { labelled } else { size::ICON_BUTTON })
                + size::GAP
                + search.unwrap_or(size::ICON_BUTTON)
        };
        let (show_label, search_width) = fields
            .into_iter()
            .find(|&tier| room - used(tier) >= size::CRUMB_ROOM)
            .unwrap_or((false, None));
        let folder = folder_name(&dir);
        let trail = trail(&dir);
        // Searching in a titlebar too tight for the field: the field takes
        // what the current folder's name and the "…" (Parent folders) menu
        // leave, between half its compact width and its compact width. The
        // trail folds to "…" and the current folder, so where the search
        // runs stays legible and the folders above it stay reachable.
        let tight_search = search_width.is_none() && searching;
        let search_width = match tight_search {
            true => {
                let leaf = (ui::text_width(
                    &folder.clone().into(),
                    text::TITLE,
                    FontWeight::SEMIBOLD,
                    window,
                ) + size::GAP_SMALL * 2.)
                    .min(size::CRUMB_MAX);
                let parents = match trail.len() > 1 {
                    true => crumb_overflow(),
                    false => px(0.),
                };
                let left = room - used((false, Some(px(0.)))) - leaf - parents;
                Some(left.clamp(size::SEARCH_WIDTH_COMPACT / 2., size::SEARCH_WIDTH_COMPACT))
            }
            false => search_width,
        };
        let crumb_room = (room - used((show_label, search_width))).max(px(0.));

        let devices = self.devices.read(cx).connected();
        let crumbs: Vec<ui::Crumb> = trail
            .into_iter()
            .map(|(label, path)| {
                // A device shows by the name it gives itself.
                let label = devices
                    .iter()
                    .find(|device| device.path == path)
                    .map_or(label, |device| device.name.clone());
                ui::Crumb::new(label)
                    .on_click(cx.listener(move |this, _, _, cx| this.go(Some(path.clone()), cx)))
            })
            .collect();

        // Nothing to search in an empty folder.
        let can_search = self.can_search(cx);
        let search_tip = match can_search {
            true => format!("Search “{folder}”"),
            false => format!("“{folder}” has nothing to search"),
        };
        let search = match search_width {
            Some(width) => div()
                .id("search")
                .flex_none()
                .w(width)
                // Clicks in the field edit text; they don't drag the window.
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .tip(search_tip, can_search.then_some(Command::Find))
                .child(
                    ui::text_field(&self.search, window, cx)
                        .small()
                        .h(size::CONTROL)
                        .cleanable(true)
                        .disabled(!can_search)
                        .prefix(ui::icon(
                            IconName::Search,
                            if can_search {
                                t.secondary_label
                            } else {
                                t.tertiary_label
                            },
                        )),
                )
                .into_any_element(),
            None => ui::IconButton::new("search", IconName::Search, search_tip)
                .command(Command::Find)
                .disabled(!can_search)
                .on_click(cx.listener(|this, _, window, cx| this.find(window, cx)))
                .into_any_element(),
        };

        // Off on a device, where there are no files to read, in a folder
        // with neither a video nor a folder in it, and in one that can't be
        // read; on again as soon as a listing says otherwise.
        let clean_up_off = !self.clean_up.available();
        let clean_up_tip = self
            .clean_up
            .tip()
            .unwrap_or(platform::label(Command::CleanUpStreamNames));
        let clean_up = cx.listener(|this, _, window, cx| this.clean_up_names(window, cx));
        let clean_up = match show_label {
            true => ui::Button::new("clean-up", clean_up_label)
                .icon(IconName::BrushCleaning)
                .tip(clean_up_tip, Some(Command::CleanUpStreamNames))
                .disabled(clean_up_off)
                .on_click(clean_up)
                .into_any_element(),
            false => ui::IconButton::new("clean-up", IconName::BrushCleaning, clean_up_tip)
                .command(Command::CleanUpStreamNames)
                .disabled(clean_up_off)
                .on_click(clean_up)
                .into_any_element(),
        };

        let sidebar_shown = panes.sidebar.is_some() || self.places_overlay;
        let (sidebar_tip, inspector_tip) = (
            if sidebar_shown {
                "Hide Sidebar"
            } else {
                "Show Sidebar"
            },
            if panes.inspector.is_some() {
                "Hide Inspector"
            } else {
                "Show Inspector"
            },
        );

        let controls = drag_region()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .pl(chrome.leading)
            // Controls in a window that is not key are drawn subdued.
            .when(!window.is_window_active(), |this| {
                this.opacity(ui::INACTIVE)
            })
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(size::GAP_TINY)
                    .when(menu_leading, |this| this.child(self.main_menu(false, cx)))
                    .child(
                        ui::IconButton::new("sidebar", IconName::PanelLeft, sidebar_tip)
                            .command(Command::ToggleSidebar)
                            .accessible_label("Sidebar")
                            .pressed(sidebar_shown)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.toggle(Pane::Sidebar, window, cx)
                            })),
                    )
                    .child(
                        ui::IconButton::new(
                            "back",
                            IconName::ChevronLeft,
                            platform::label(Command::GoBack),
                        )
                        .command(Command::GoBack)
                        .disabled(!back)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.browser.update(cx, |b, cx| b.go_back(cx))
                        })),
                    )
                    .child(
                        ui::IconButton::new(
                            "forward",
                            IconName::ChevronRight,
                            platform::label(Command::GoForward),
                        )
                        .command(Command::GoForward)
                        .disabled(!forward)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.browser.update(cx, |b, cx| b.go_forward(cx))
                        })),
                    ),
            )
            .child(div().flex_none().w(crumbs_start - cluster_end))
            // The breadcrumb, then bare strip to drag the window by.
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .child(ui::Breadcrumb::new("location", crumbs).max_width(crumb_room)),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(size::GAP)
                    .px(size::GAP)
                    .child(search)
                    .child(clean_up),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .justify_end()
                    .items_center()
                    .gap(size::GAP_TINY)
                    .w(trailing_box)
                    .pr(chrome.trailing)
                    .child(
                        ui::IconButton::new("inspector", IconName::PanelRight, inspector_tip)
                            .command(Command::ToggleInspector)
                            .accessible_label("Inspector")
                            .pressed(panes.inspector.is_some())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.toggle(Pane::Inspector, window, cx)
                            })),
                    )
                    .when(menu_trailing, |this| this.child(self.main_menu(true, cx)))
                    // Seer's controls end; the window's begin.
                    .when(separator, |this| {
                        this.child(
                            div()
                                .flex_none()
                                .ml(size::GAP - size::GAP_TINY)
                                .mr(size::GAP)
                                .w(size::HAIRLINE)
                                .h(size::ICON)
                                .bg(t.separator),
                        )
                    }),
            );

        div()
            .relative()
            .flex_none()
            .w_full()
            .h(chrome.titlebar)
            // Surfaces: each side pane's strip and its divider run up
            // through the titlebar; the rest is the list's, ruled off below.
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .when_some(panes.sidebar, |this, width| {
                        this.child(div().flex_none().w(width).h_full().bg(t.surface_window))
                            .child(div().flex_none().w(size::HAIRLINE).h_full().bg(t.separator))
                    })
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .bg(t.surface_content)
                            .border_b(size::HAIRLINE)
                            .border_color(t.separator),
                    )
                    .when_some(panes.inspector, |this, width| {
                        this.child(div().flex_none().w(size::HAIRLINE).h_full().bg(t.separator))
                            .child(div().flex_none().w(width).h_full().bg(t.surface_raised))
                    }),
            )
            .child(controls)
            .children(ui::window_controls(window, cx))
    }

    /// The "Main Menu" button on desktops with no menu bar: every command,
    /// from the same table as the Mac's menu bar, sent to the file list as
    /// if it had focus. `at_end` opens the menu from the button's trailing
    /// edge, for a button at the trailing end of the titlebar.
    fn main_menu(&self, at_end: bool, cx: &mut Context<Self>) -> AnyElement {
        let list = self.browser.read(cx).focus_handle(cx);
        let workspace = cx.entity().downgrade();
        let button = ui::MenuButton::new("main-menu", IconName::Menu, "Main Menu")
            .track_focus(&self.main_menu_focus);
        // Enabled and checked from the window's state when it opens.
        let build = move |menu, window: &mut Window, cx: &mut Context<PopupMenu>| {
            let availability = workspace
                .upgrade()
                .map_or(Availability::EVERYTHING, |workspace| {
                    workspace.read(cx).availability(window, cx)
                });
            ui::command_menu_with(menu, Some(list.clone()), availability, window, cx)
        };
        match at_end {
            true => button.menu_from_end(build).into_any_element(),
            false => button.menu(build).into_any_element(),
        }
    }

    /// A hairline with a wider invisible strip for grabbing.
    fn divider(&self, pane: Pane, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::of(cx);
        let reset = match pane {
            Pane::Sidebar => size::SIDEBAR.1,
            Pane::Inspector => size::INSPECTOR.1,
        };
        div()
            .relative()
            .flex_none()
            .w(size::HAIRLINE)
            .h_full()
            .bg(t.separator)
            .child(
                div()
                    .id(match pane {
                        Pane::Sidebar => "sidebar-divider",
                        Pane::Inspector => "inspector-divider",
                    })
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(-size::DIVIDER_HIT / 2.)
                    .w(size::DIVIDER_HIT)
                    .cursor_col_resize()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                            if event.click_count >= 2 {
                                match pane {
                                    Pane::Sidebar => this.sidebar_width = reset,
                                    Pane::Inspector => this.inspector_width = reset,
                                }
                                this.schedule_save(window, cx);
                            } else {
                                this.resizing = Some(pane);
                            }
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::of(cx).clone();
        let viewport = window.viewport_size().width;
        // The overlay was for a window too narrow to dock the sidebar; once
        // it can be docked (wider, or the inspector closed) it settles there.
        // Focus on a place stays on that place: its handle moves with it
        // from the panel to the docked sidebar.
        if self.places_overlay && self.sidebar_docks(viewport, cx) {
            self.places_overlay = false;
            self.sidebar_visible = true;
            self.return_focus = None;
            self.schedule_save(window, cx);
        }
        let panes = self.panes(viewport, cx);
        // A sidebar that undocks (the window narrowed, or it was hidden)
        // takes no focus with it: the keyboard goes to the list.
        if panes.sidebar.is_none() && !self.places_overlay && self.place_has_focus(window, cx) {
            let list = self.browser.read(cx).focus_handle(cx);
            window.defer(cx, move |window, cx| list.focus(window, cx));
        }
        // The list picks its columns from the width left to it.
        let beside = panes.beside_list();
        if cx.try_global::<SidePanes>().map(|s| s.0) != Some(beside) {
            cx.set_global(SidePanes(beside));
        }
        // A command's handler is registered only while it can run, so the
        // Mac menu bar greys out the same items the Main Menu does.
        let can = self.availability(window, cx);
        self.update_menu_bar(can, cx);
        let modal = self.modal();
        let places_overlay =
            (self.places_overlay && !modal).then(|| self.render_places_overlay(window, cx));
        let about = self.about.then(|| self.render_about(cx));

        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .font_family(theme::font(cx))
            .text_color(t.label)
            .on_mouse_move(cx.listener(Self::resize))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    if this.resizing.take().is_some() {
                        this.schedule_save(window, cx);
                    }
                }),
            )
            .on_modifiers_changed(cx.listener(Self::modifiers_changed))
            // Any key pressed with Alt makes it a shortcut, not the menu key.
            .capture_key_down(cx.listener(|this, _: &KeyDownEvent, _, _| {
                this.alt_alone = false;
            }))
            .on_action(cx.listener(Self::open_folder))
            .on_action(cx.listener(|this, _: &About, window, cx| this.show_about(window, cx)))
            .on_action(
                cx.listener(|this, _: &ShowMainMenu, window, cx| this.show_main_menu(window, cx)),
            )
            .when(can.allows(Needs::CleanUp), |this| {
                this.on_action(cx.listener(|this, _: &CleanUpStreamNames, window, cx| {
                    this.clean_up_names(window, cx)
                }))
            })
            .when(can.allows(Needs::Items), |this| {
                this.on_action(cx.listener(|this, _: &Find, window, cx| this.find(window, cx)))
            })
            .when(can.allows(Needs::Transfers), |this| {
                this.on_action(cx.listener(|this, _: &ShowTransfers, window, cx| {
                    this.show_transfers(window, cx)
                }))
            })
            .on_action(cx.listener(|this, _: &ToggleSidebar, window, cx| {
                this.toggle(Pane::Sidebar, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ToggleInspector, window, cx| {
                this.toggle(Pane::Inspector, window, cx)
            }))
            .when(can.allows(Needs::SingleMedia), |this| {
                this.on_action(cx.listener(|this, _: &AnalyzeBitrate, window, cx| {
                    if this.modal() {
                        return;
                    }
                    if !this
                        .panes(window.viewport_size().width, cx)
                        .shows(Pane::Inspector)
                    {
                        this.toggle(Pane::Inspector, window, cx);
                    }
                    this.inspector
                        .update(cx, |inspector, cx| inspector.analyze(cx));
                }))
            })
            .on_action(cx.listener(|this, _: &GoHome, _, cx| this.go(dirs::home_dir(), cx)))
            .on_action(cx.listener(|this, _: &GoDesktop, _, cx| this.go(dirs::desktop_dir(), cx)))
            .on_action(
                cx.listener(|this, _: &GoDocuments, _, cx| this.go(dirs::document_dir(), cx)),
            )
            .on_action(
                cx.listener(|this, _: &GoDownloads, _, cx| this.go(dirs::download_dir(), cx)),
            )
            .on_action(cx.listener(|this, _: &GoMovies, _, cx| this.go(dirs::video_dir(), cx)))
            .on_action(cx.listener(|this, _: &GoMusic, _, cx| this.go(dirs::audio_dir(), cx)))
            .on_action(cx.listener(|this, _: &GoPictures, _, cx| this.go(dirs::picture_dir(), cx)))
            .on_action(|_: &Minimize, window, _| window.minimize_window())
            .on_action(|_: &Zoom, window, _| window.zoom_window())
            .child(self.render_titlebar(panes, window, cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .when_some(panes.sidebar, |this, width| {
                        this.child(self.render_sidebar(width, false, window, cx))
                            .child(self.divider(Pane::Sidebar, cx))
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            // Opaque, so only the sidebar shows the window's blur.
                            .bg(t.surface_content)
                            .child(self.browser.clone()),
                    )
                    .when_some(panes.inspector, |this, width| {
                        this.child(self.divider(Pane::Inspector, cx)).child(
                            div()
                                .flex_none()
                                .w(width)
                                .h_full()
                                .bg(t.surface_raised)
                                .child(self.inspector.clone()),
                        )
                    }),
            )
            .children(places_overlay)
            .children(self.places_menu.render())
            // Last, so their scrim covers the whole window, titlebar included.
            .children(self.cleanup.clone())
            .children(about)
    }
}

/// A place's context menu: Open, Reveal, and Eject for a disk that can be
/// ejected. The same for a right-click and for the context-menu key.
fn place_menu(menu: PopupMenu, workspace: WeakEntity<Workspace>, place: &Place) -> PopupMenu {
    let (open, reveal) = (place.path.clone(), place.path.clone());
    let on_device = mtp::is_device_path(&reveal);
    let open_in = workspace.clone();
    let menu = menu
        .item(PopupMenuItem::new("Open").on_click(move |_, window, cx| {
            open_in
                .update(cx, |this, cx| this.choose_place(open.clone(), window, cx))
                .ok();
        }))
        .item(
            PopupMenuItem::new(platform::label(Command::Reveal))
                .disabled(on_device)
                .on_click(move |_, _, cx| cx.reveal_path(&reveal)),
        );
    match place.ejectable {
        true => {
            let disk = place.path.clone();
            menu.separator()
                .item(PopupMenuItem::new("Eject").on_click(move |_, window, cx| {
                    workspace
                        .update(cx, |this, cx| this.eject(disk.clone(), window, cx))
                        .ok();
                }))
        }
        false => menu,
    }
}

/// What the breadcrumb's "…" (Parent folders) button takes with its
/// chevron, as `ui::Breadcrumb` lays it out.
fn crumb_overflow() -> Pixels {
    size::ICON_BUTTON + size::GAP_SMALL * 2. + size::ICON_SMALL
}

/// An area that moves the window when dragged, like a native title bar.
fn drag_region() -> Stateful<Div> {
    div()
        .id("drag-region")
        .on_mouse_down(MouseButton::Left, |event, window, _| {
            if event.click_count >= 2 {
                window.titlebar_double_click();
            } else {
                window.start_window_move();
            }
        })
}

/// Whether a listing has nothing to search: empty, or unreadable. `None`
/// while it is still loading.
fn nothing_listed(unreadable: bool, listing: Option<&[Entry]>) -> Option<bool> {
    match (unreadable, listing) {
        (true, _) => Some(true),
        (false, Some(items)) => Some(items.is_empty()),
        (false, None) => None,
    }
}

/// The search field's placeholder, which says what it searches.
fn search_placeholder(dir: &Path) -> String {
    format!("Search {}", folder_name(dir))
}

/// The folders from the top of the place `dir` is in down to `dir`, each
/// with the name it shows in the breadcrumb: from Home when it is under
/// Home, from the disk when it is on another one, from the device on a
/// device, else from the root volume.
fn trail(dir: &Path) -> Vec<(String, PathBuf)> {
    let volume = {
        let mut parts = dir.components();
        match (parts.next(), parts.next(), parts.next()) {
            (
                Some(Component::RootDir),
                Some(Component::Normal(v)),
                Some(Component::Normal(name)),
            ) if v == "Volumes" => Some(Path::new("/Volumes").join(name)),
            _ => None,
        }
    };
    let (mut path, mut out) = if mtp::is_device_path(dir) {
        (PathBuf::from(mtp::ROOT), Vec::new())
    } else if let Some(home) = dirs::home_dir().filter(|home| dir.starts_with(home)) {
        (home.clone(), vec![(folder_name(&home), home)])
    } else if let Some(volume) = volume {
        (volume.clone(), vec![(folder_name(&volume), volume)])
    } else {
        let root = PathBuf::from("/");
        (root.clone(), vec![(platform::root_name().to_owned(), root)])
    };
    let rest = dir
        .strip_prefix(&path)
        .map(Path::to_owned)
        .unwrap_or_default();
    for part in rest.components() {
        path.push(part);
        out.push((
            part.as_os_str().to_string_lossy().into_owned(),
            path.clone(),
        ));
    }
    if out.is_empty() {
        out.push((folder_name(dir), dir.to_owned()));
    }
    out
}

fn favourites() -> Vec<Place> {
    let candidates = [
        (dirs::home_dir(), IconName::House),
        (dirs::desktop_dir(), IconName::Monitor),
        (dirs::document_dir(), IconName::FileText),
        (dirs::download_dir(), IconName::CircleArrowDown),
        (dirs::video_dir(), IconName::Film),
        (dirs::audio_dir(), IconName::Music),
        (dirs::picture_dir(), IconName::Image),
    ];
    candidates
        .into_iter()
        .filter_map(|(path, icon)| {
            let path = path.filter(|p| p.is_dir())?;
            Some(Place {
                name: folder_name(&path),
                icon,
                path,
                ejectable: false,
            })
        })
        .collect()
}

/// Lists the disks again whenever one is mounted or ejected.
fn watch_volumes(cx: &mut Context<Workspace>) -> Option<(notify::RecommendedWatcher, Task<()>)> {
    use notify::Watcher as _;
    let (tx, rx) = smol::channel::bounded::<()>(1);
    let mut watcher = notify::recommended_watcher(move |_| {
        tx.try_send(()).ok();
    })
    .ok()?;
    watcher
        .watch(
            std::path::Path::new("/Volumes"),
            notify::RecursiveMode::NonRecursive,
        )
        .ok()?;
    let task = cx.spawn(async move |this, cx| {
        while rx.recv().await.is_ok() {
            // A disk appears in /Volumes a moment before it is readable.
            cx.background_executor()
                .timer(Duration::from_millis(500))
                .await;
            while rx.try_recv().is_ok() {}
            let found = cx.background_spawn(async { locations() }).await;
            let alive = this.update(cx, |this, cx| {
                this.locations = found;
                cx.notify();
            });
            if alive.is_err() {
                break;
            }
        }
    });
    Some((watcher, task))
}

/// The startup disk, then every other mounted disk. Only the others can be
/// ejected, and they have their own icon.
fn locations() -> Vec<Place> {
    let mut places = vec![Place {
        name: platform::root_name().into(),
        icon: IconName::HardDrive,
        path: PathBuf::from("/"),
        ejectable: false,
    }];
    let volumes = std::fs::read_dir("/Volumes")
        .into_iter()
        .flatten()
        .flatten();
    for volume in volumes {
        // The boot volume also appears here, as a link back to "/".
        let is_link = volume.file_type().is_ok_and(|t| t.is_symlink());
        if !is_link && volume.path().is_dir() {
            places.push(Place {
                name: volume.file_name().to_string_lossy().into_owned(),
                // Any other disk, image or share: a disc, never the
                // startup disk's drive or a USB plug it may not be.
                icon: IconName::Disc,
                path: volume.path(),
                ejectable: true,
            });
        }
    }
    places
}

#[cfg(test)]
mod tests {
    // Not `super::*`: GPUI's own `test` attribute would shadow the standard one.
    use super::{fit, folder_name, platform, trail};
    use crate::browser::{list_comfort, list_essential};
    use crate::theme::size;
    use gpui_kit::{Pixels, px};
    use std::path::Path;

    const SIDEBAR: (Pixels, Pixels) = (px(200.), px(174.));
    const INSPECTOR: (Pixels, Pixels) = (px(300.), px(260.));

    /// What a mixed folder's five columns need.
    fn mixed() -> Pixels {
        list_comfort(false)
    }

    #[test]
    fn both_panes_fit_a_default_window() {
        let panes = fit(px(1180.), mixed(), Some(SIDEBAR), Some(INSPECTOR));
        assert_eq!(panes.sidebar, Some(px(200.)));
        assert_eq!(panes.inspector, Some(px(300.)));
    }

    #[test]
    fn the_five_column_list_is_648pt() {
        assert_eq!(mixed(), px(648.));
    }

    #[test]
    fn panes_yield_to_keep_five_columns() {
        // F3-2: at 1100 with both panes the inspector gives up 40pt to its
        // narrowest, then the sidebar 10pt, and Audio stays.
        let panes = fit(px(1100.), mixed(), Some(SIDEBAR), Some(INSPECTOR));
        assert_eq!(panes.inspector, Some(px(260.)));
        assert_eq!(panes.sidebar, Some(px(190.)));
        assert_eq!(px(1100.) - panes.beside_list(), mixed());
        // At 940 with the inspector alone it gives up 9pt.
        let panes = fit(px(940.), mixed(), None, Some(INSPECTOR));
        assert_eq!(panes.inspector, Some(px(291.)));
        assert!(px(940.) - panes.beside_list() >= mixed());
        // A pane widened past that point is drawn where the list keeps
        // its five columns; the preference itself is kept.
        let panes = fit(
            px(1180.),
            mixed(),
            Some(SIDEBAR),
            Some((px(400.), px(260.))),
        );
        assert_eq!(panes.inspector, Some(px(330.)));
        assert_eq!(px(1180.) - panes.beside_list(), mixed());
    }

    #[test]
    fn panes_go_back_to_their_widths_when_five_columns_cannot_fit() {
        // At 1083 even the narrowest panes leave the list short of five
        // columns, so it settles for four and the panes are drawn as wide
        // as that allows.
        let narrowest = SIDEBAR.1 + INSPECTOR.1 + px(2.);
        let panes = fit(px(1084.), mixed(), Some(SIDEBAR), Some(INSPECTOR));
        assert_eq!(panes.beside_list(), narrowest);
        let panes = fit(px(1083.), mixed(), Some(SIDEBAR), Some(INSPECTOR));
        assert_eq!(panes.sidebar, Some(px(200.)));
        assert_eq!(panes.inspector, Some(px(300.)));
    }

    #[test]
    fn the_inspector_narrows_before_the_sidebar() {
        // Five columns are out of reach below 1084. 1000 leaves 470 for
        // both beside a 528pt list: the inspector gives up 30pt. At 970
        // (440) it is at its narrowest and the sidebar gives up 20pt.
        let panes = fit(px(1000.), mixed(), Some(SIDEBAR), Some(INSPECTOR));
        assert_eq!(panes.sidebar, Some(px(200.)));
        assert_eq!(panes.inspector, Some(px(270.)));
        let panes = fit(px(970.), mixed(), Some(SIDEBAR), Some(INSPECTOR));
        assert_eq!(panes.inspector, Some(px(260.)));
        assert_eq!(panes.sidebar, Some(px(180.)));
    }

    #[test]
    fn places_stay_docked_while_the_essentials_fit() {
        // S4-4: at 940 both panes stay, at their narrowest, beside a list
        // that keeps Name, Video, Duration and Size (504pt).
        let panes = fit(px(940.), mixed(), Some(SIDEBAR), Some(INSPECTOR));
        assert_eq!(panes.sidebar, Some(SIDEBAR.1));
        assert_eq!(panes.inspector, Some(INSPECTOR.1));
        assert_eq!(px(940.) - panes.beside_list(), px(504.));
        // Down to the essential list, 935pt here.
        let edge = SIDEBAR.1 + INSPECTOR.1 + px(2.) + list_essential();
        assert_eq!(edge, px(935.));
        let panes = fit(edge, mixed(), Some(SIDEBAR), Some(INSPECTOR));
        assert_eq!(panes.sidebar, Some(SIDEBAR.1));
        assert_eq!(px(935.) - panes.beside_list(), list_essential());
        // The same in a folder of audio: docking does not follow the folder.
        let panes = fit(edge, list_comfort(true), Some(SIDEBAR), Some(INSPECTOR));
        assert_eq!(panes.sidebar, Some(SIDEBAR.1));
    }

    #[test]
    fn the_sidebar_closes_first() {
        // Below that both no longer fit; the inspector stays, as wide as
        // five columns allow.
        let panes = fit(px(934.), mixed(), Some(SIDEBAR), Some(INSPECTOR));
        assert_eq!(panes.sidebar, None);
        assert_eq!(panes.inspector, Some(px(285.)));
        assert!(px(934.) - panes.beside_list() >= mixed());
    }

    #[test]
    fn panes_yield_for_an_audio_folders_date_modified() {
        // F4-3, A4-3: in a folder of audio the default window gives Date
        // Modified its room: the inspector yields 14pt, the sidebar none.
        let audio = list_comfort(true);
        let panes = fit(px(1180.), audio, Some(SIDEBAR), Some(INSPECTOR));
        assert_eq!(panes.sidebar, Some(px(200.)));
        assert_eq!(panes.inspector, Some(px(286.)));
        assert_eq!(px(1180.) - panes.beside_list(), audio);
        // A mixed folder in the same window keeps both at their widths.
        let panes = fit(px(1180.), mixed(), Some(SIDEBAR), Some(INSPECTOR));
        assert_eq!(panes.inspector, Some(px(300.)));
        // Too narrow for the audio folder's five: the panes go back to
        // their widths, as for a mixed folder.
        let narrowest = SIDEBAR.1 + INSPECTOR.1 + px(2.);
        let panes = fit(
            narrowest + audio - px(1.),
            audio,
            Some(SIDEBAR),
            Some(INSPECTOR),
        );
        assert_eq!(panes.inspector, Some(px(300.)));
    }

    #[test]
    fn the_minimum_window_keeps_the_inspector() {
        // Below the list's preferred minimum, a lone pane keeps its
        // narrowest width while the list keeps the content minimum.
        let panes = fit(px(760.), mixed(), Some(SIDEBAR), Some(INSPECTOR));
        assert_eq!(panes.sidebar, None);
        assert_eq!(panes.inspector, Some(px(260.)));
        assert!(px(760.) - panes.beside_list() >= size::CONTENT_MIN);
    }

    #[test]
    fn the_sidebar_alone_docks_in_the_minimum_window() {
        // With the inspector closed, the sidebar always docks; beside the
        // inspector it opens over the list instead (`Workspace::toggle`).
        let panes = fit(px(760.), mixed(), Some(SIDEBAR), None);
        assert_eq!(panes.sidebar.map(|w| w >= SIDEBAR.1), Some(true));
        assert!(px(760.) - panes.beside_list() >= size::CONTENT_MIN);
    }

    #[test]
    fn closed_panes_stay_closed() {
        let panes = fit(px(1600.), mixed(), None, Some(INSPECTOR));
        assert_eq!(panes.sidebar, None);
        assert_eq!(panes.beside_list(), px(301.));
    }

    #[test]
    fn the_trail_starts_at_home() {
        let Some(home) = dirs::home_dir() else { return };
        let trail = trail(&home.join("Movies/Concert"));
        let names: Vec<&str> = trail.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(names, [folder_name(&home).as_str(), "Movies", "Concert"]);
        assert_eq!(trail[2].1, home.join("Movies/Concert"));
    }

    #[test]
    fn the_trail_starts_at_the_disk() {
        let trail = trail(Path::new("/Volumes/Archive/Clips"));
        let names: Vec<&str> = trail.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(names, ["Archive", "Clips"]);
        let trail = super::trail(Path::new("/private/tmp"));
        assert_eq!(trail[0].0, platform::root_name());
        assert_eq!(trail.len(), 3);
    }
}
