//! Every command in the app, and the table that puts each one in the menu
//! bar (and, on other desktops, in the "Main Menu" button).

use gpui_kit::actions;

actions!(
    seer,
    [
        // App
        About,
        Quit,
        Hide,
        HideOthers,
        ShowAll,
        Minimize,
        Zoom,
        // File
        OpenFolder,
        OpenSelection,
        NewFolder,
        Rename,
        RevealInFinder,
        CleanUpStreamNames,
        MoveToTrash,
        // Edit
        Copy,
        Paste,
        MoveItemHere,
        SelectAll,
        ClearSelection,
        Find,
        // View
        ToggleSidebar,
        ToggleInspector,
        ToggleHiddenFiles,
        SortByName,
        SortByVideo,
        SortByAudio,
        SortByDuration,
        SortBySize,
        SortByModified,
        ShowTransfers,
        // Go
        GoBack,
        GoForward,
        GoToEnclosingFolder,
        GoHome,
        GoDesktop,
        GoDocuments,
        GoDownloads,
        GoMovies,
        GoMusic,
        GoPictures,
        // List navigation
        MoveUp,
        MoveDown,
        ExtendUp,
        ExtendDown,
        MoveToFirst,
        MoveToLast,
        // Inspector
        AnalyzeBitrate,
        // Windows and GNOME: open the "Main Menu" button's menu (F10, Alt).
        ShowMainMenu,
        // The focused row's context menu, from the keyboard (Shift+F10, the
        // Menu key, ⌃↩ on the Mac).
        ShowContextMenu,
    ]
);

use crate::platform::{self, Command as C, Os};
use gpui_kit::{Action, Menu, MenuItem, SystemMenuType};
use seer_core::fs::SortKey;

/// What "About Seer" says, on every OS.
pub struct AppInfo {
    pub name: &'static str,
    pub version: &'static str,
    /// One line: what Seer is for.
    pub description: &'static str,
    /// The name in Tamil and what it means.
    pub meaning: &'static str,
}

pub const APP: AppInfo = AppInfo {
    name: "Seer",
    version: env!("CARGO_PKG_VERSION"),
    description: "Look at, prune and tidy your media files.",
    meaning: "சீர் · order, neatness",
};

/// What a command needs before it can run. Each entry of the command table
/// names one; [`Availability::allows`] answers it for the current window.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Needs {
    /// Always available.
    Nothing,
    /// The folder lists at least one item (Find, Select All).
    Items,
    /// At least one item is selected.
    Selection,
    /// Exactly one item is selected.
    Single,
    /// Exactly one media file on this computer is selected.
    SingleMedia,
    /// The folder is on this computer, not on a device.
    Local,
    /// Something has been copied.
    Pasteboard,
    /// Clean Up has videos to check here.
    CleanUp,
    Back,
    Forward,
    /// The folder has an enclosing folder.
    Parent,
    /// A transfer is running or has a result to show.
    Transfers,
}

/// A View-menu sort that can carry a checkmark: every column the file list
/// can sort by, in column order. The menu lists them all, so a sort whose
/// header has folded at a narrow width can still be seen and chosen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sort {
    Name,
    Video,
    Audio,
    Duration,
    Size,
    Modified,
}

impl Sort {
    /// The View-menu entry for the list's sort key, if it has one.
    pub fn of(key: SortKey) -> Option<Sort> {
        match key {
            SortKey::Name => Some(Sort::Name),
            SortKey::Video => Some(Sort::Video),
            SortKey::Audio => Some(Sort::Audio),
            SortKey::Duration => Some(Sort::Duration),
            SortKey::Size => Some(Sort::Size),
            SortKey::Modified => Some(Sort::Modified),
            SortKey::Kind => None,
        }
    }
}

/// A command that shows a checkmark when its state is on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Toggle {
    Sidebar,
    Inspector,
    HiddenFiles,
    Sort(Sort),
}

/// The state commands depend on, gathered by the window (the workspace
/// from the file list, the clipboard, history and Clean Up). The Main Menu
/// (`ui::command_menu_with`), the Mac menu bar ([`menu_bar`]) and the
/// file list's context menu all read their enabled and checked states
/// from one of these, so they cannot disagree.
///
/// On the Mac, AppKit also asks GPUI whether each menu-bar action has a
/// handler on the focused path, and greys it out when none does. Register a
/// command's handler only when [`Availability::allows`] what its table
/// entry [`Needs`], and the two agree.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Availability {
    /// The folder lists at least one item (hidden ones not shown excluded).
    pub items: bool,
    /// How many items are selected.
    pub selected: usize,
    /// The selection is one media file that can be probed.
    pub single_media: bool,
    /// The folder is on this computer, not on a device.
    pub local: bool,
    /// How many items are on Seer's clipboard.
    pub pasteboard: usize,
    /// Clean Up's eligibility for the folder (decision 45).
    pub clean_up: bool,
    pub back: bool,
    pub forward: bool,
    pub parent: bool,
    /// A transfer is running or a retained receipt can be shown again
    /// (`Devices::has_details`). False once the last receipt is dismissed.
    pub transfers: bool,
    pub sidebar: bool,
    pub inspector: bool,
    pub hidden_files: bool,
    /// The list's sort, when it is one the View menu names.
    pub sort: Option<Sort>,
}

impl Availability {
    /// Everything enabled, nothing checked: for a menu built without
    /// knowing the window's state.
    pub const EVERYTHING: Availability = Availability {
        items: true,
        selected: 1,
        single_media: true,
        local: true,
        pasteboard: 1,
        clean_up: true,
        back: true,
        forward: true,
        parent: true,
        transfers: true,
        sidebar: false,
        inspector: false,
        hidden_files: false,
        sort: None,
    };

    pub fn allows(&self, needs: Needs) -> bool {
        match needs {
            Needs::Nothing => true,
            Needs::Items => self.items,
            Needs::Selection => self.selected > 0,
            Needs::Single => self.selected == 1,
            Needs::SingleMedia => self.selected == 1 && self.single_media && self.local,
            Needs::Local => self.local,
            Needs::Pasteboard => self.pasteboard > 0,
            Needs::CleanUp => self.clean_up && self.local,
            Needs::Back => self.back,
            Needs::Forward => self.forward,
            Needs::Parent => self.parent,
            Needs::Transfers => self.transfers,
        }
    }

    pub fn checked(&self, toggle: Toggle) -> bool {
        match toggle {
            Toggle::Sidebar => self.sidebar,
            Toggle::Inspector => self.inspector,
            Toggle::HiddenFiles => self.hidden_files,
            Toggle::Sort(sort) => self.sort == Some(sort),
        }
    }
}

/// One entry of a [`CommandMenu`].
#[derive(Clone, Copy)]
pub enum Entry {
    /// A command: its label, the action it dispatches, the
    /// [`platform::Command`] it stands for when it has one (for the
    /// shortcut hint), what it needs to be enabled, and the state its
    /// checkmark shows, if it has one.
    Command {
        label: &'static str,
        action: fn() -> Box<dyn Action>,
        command: Option<C>,
        needs: Needs,
        check: Option<Toggle>,
    },
    Separator,
    /// The system's Services submenu (macOS menu bar only).
    Services,
}

/// One menu of the command table: "File", "Edit", "View"…
pub struct CommandMenu {
    pub name: &'static str,
    pub entries: Vec<Entry>,
}

macro_rules! item {
    ($label:expr, $action:expr, $command:expr, $needs:expr) => {
        Entry::Command {
            label: $label,
            action: || Box::new($action),
            command: $command,
            needs: $needs,
            check: None,
        }
    };
    ($label:expr, $action:expr, $command:expr, $needs:expr, $check:expr) => {
        Entry::Command {
            label: $label,
            action: || Box::new($action),
            command: $command,
            needs: $needs,
            check: Some($check),
        }
    };
}

/// Every command, grouped as the Mac menu bar shows them. The menu bar
/// ([`menu_bar`]) and the "Main Menu" button on other desktops
/// (`ui::command_menu_with`) are both built from this table, so they cannot
/// drift apart. Commands that only the Mac has (Hide, Services, the Window
/// menu, which the caption buttons replace) are left out on other OSes;
/// there, About ends the Main Menu instead ([`main_menu_extras`]).
pub fn command_menus() -> Vec<CommandMenu> {
    use Needs as N;
    let mac = platform::os() == Os::Mac;
    let mut menus = Vec::new();
    if mac {
        menus.push(CommandMenu {
            name: "Seer",
            entries: vec![
                item!(platform::label(C::About), About, None, N::Nothing),
                Entry::Separator,
                Entry::Services,
                Entry::Separator,
                item!(platform::label(C::Hide), Hide, Some(C::Hide), N::Nothing),
                item!(
                    platform::label(C::HideOthers),
                    HideOthers,
                    Some(C::HideOthers),
                    N::Nothing
                ),
                item!("Show All", ShowAll, None, N::Nothing),
                Entry::Separator,
                item!(platform::label(C::Quit), Quit, Some(C::Quit), N::Nothing),
            ],
        });
    }
    let mut file = vec![
        item!(
            platform::label(C::GoToFolder),
            OpenFolder,
            Some(C::GoToFolder),
            N::Nothing
        ),
        item!(
            platform::label(C::NewFolder),
            NewFolder,
            Some(C::NewFolder),
            N::Nothing
        ),
        item!(
            platform::label(C::OpenSelection),
            OpenSelection,
            Some(C::OpenSelection),
            N::Single
        ),
        Entry::Separator,
        item!(
            platform::label(C::Rename),
            Rename,
            Some(C::Rename),
            N::Single
        ),
        item!(
            platform::label(C::Reveal),
            RevealInFinder,
            Some(C::Reveal),
            N::Local
        ),
        Entry::Separator,
        item!(
            platform::label(C::AnalyzeBitrate),
            AnalyzeBitrate,
            Some(C::AnalyzeBitrate),
            N::SingleMedia
        ),
        item!(
            platform::label(C::CleanUpStreamNames),
            CleanUpStreamNames,
            Some(C::CleanUpStreamNames),
            N::CleanUp
        ),
        Entry::Separator,
        item!(
            platform::label(C::MoveToTrash),
            MoveToTrash,
            Some(C::MoveToTrash),
            N::Selection
        ),
    ];
    if !mac {
        // Quit lives in the app menu on the Mac; elsewhere it ends File.
        file.push(Entry::Separator);
        file.push(item!(
            platform::label(C::Quit),
            Quit,
            Some(C::Quit),
            N::Nothing
        ));
    }
    menus.push(CommandMenu {
        name: "File",
        entries: file,
    });
    menus.push(CommandMenu {
        name: "Edit",
        entries: vec![
            item!(platform::label(C::Copy), Copy, Some(C::Copy), N::Selection),
            item!(
                platform::label(C::Paste),
                Paste,
                Some(C::Paste),
                N::Pasteboard
            ),
            item!(
                platform::label(C::MoveItemHere),
                MoveItemHere,
                Some(C::MoveItemHere),
                N::Pasteboard
            ),
            Entry::Separator,
            item!(
                platform::label(C::SelectAll),
                SelectAll,
                Some(C::SelectAll),
                N::Items
            ),
            item!(
                platform::label(C::ClearSelection),
                ClearSelection,
                Some(C::ClearSelection),
                N::Selection
            ),
            Entry::Separator,
            item!(platform::label(C::Find), Find, Some(C::Find), N::Items),
            Entry::Separator,
            // What a right-click shows, for the focused row or place. The
            // view that has focus handles it; elsewhere it is greyed out.
            item!(
                platform::label(C::ContextMenu),
                ShowContextMenu,
                Some(C::ContextMenu),
                N::Nothing
            ),
        ],
    });
    menus.push(CommandMenu {
        name: "View",
        entries: vec![
            item!(
                platform::label(C::ToggleSidebar),
                ToggleSidebar,
                Some(C::ToggleSidebar),
                N::Nothing,
                Toggle::Sidebar
            ),
            item!(
                platform::label(C::ToggleInspector),
                ToggleInspector,
                Some(C::ToggleInspector),
                N::Nothing,
                Toggle::Inspector
            ),
            Entry::Separator,
            item!(
                "Sort by Name",
                SortByName,
                None,
                N::Nothing,
                Toggle::Sort(Sort::Name)
            ),
            item!(
                "Sort by Video",
                SortByVideo,
                None,
                N::Nothing,
                Toggle::Sort(Sort::Video)
            ),
            item!(
                "Sort by Audio",
                SortByAudio,
                None,
                N::Nothing,
                Toggle::Sort(Sort::Audio)
            ),
            item!(
                "Sort by Duration",
                SortByDuration,
                None,
                N::Nothing,
                Toggle::Sort(Sort::Duration)
            ),
            item!(
                "Sort by Size",
                SortBySize,
                None,
                N::Nothing,
                Toggle::Sort(Sort::Size)
            ),
            item!(
                "Sort by Date Modified",
                SortByModified,
                None,
                N::Nothing,
                Toggle::Sort(Sort::Modified)
            ),
            Entry::Separator,
            item!(
                platform::label(C::ToggleHiddenFiles),
                ToggleHiddenFiles,
                Some(C::ToggleHiddenFiles),
                N::Nothing,
                Toggle::HiddenFiles
            ),
            Entry::Separator,
            item!(
                platform::label(C::ShowTransfers),
                ShowTransfers,
                Some(C::ShowTransfers),
                N::Transfers
            ),
        ],
    });
    menus.push(CommandMenu {
        name: "Go",
        entries: vec![
            item!(platform::label(C::GoBack), GoBack, Some(C::GoBack), N::Back),
            item!(
                platform::label(C::GoForward),
                GoForward,
                Some(C::GoForward),
                N::Forward
            ),
            item!(
                platform::label(C::GoToEnclosingFolder),
                GoToEnclosingFolder,
                Some(C::GoToEnclosingFolder),
                N::Parent
            ),
            Entry::Separator,
            item!(
                platform::label(C::GoHome),
                GoHome,
                Some(C::GoHome),
                N::Nothing
            ),
            item!(
                platform::label(C::GoDesktop),
                GoDesktop,
                Some(C::GoDesktop),
                N::Nothing
            ),
            item!(
                platform::label(C::GoDocuments),
                GoDocuments,
                Some(C::GoDocuments),
                N::Nothing
            ),
            item!(
                platform::label(C::GoDownloads),
                GoDownloads,
                Some(C::GoDownloads),
                N::Nothing
            ),
            item!("Movies", GoMovies, None, N::Nothing),
            item!("Music", GoMusic, None, N::Nothing),
            item!("Pictures", GoPictures, None, N::Nothing),
        ],
    });
    if mac {
        menus.push(CommandMenu {
            name: "Window",
            entries: vec![
                item!(
                    platform::label(C::Minimize),
                    Minimize,
                    Some(C::Minimize),
                    N::Nothing
                ),
                item!("Zoom", Zoom, None, N::Nothing),
            ],
        });
    }
    menus
}

/// Top-level entries that end the "Main Menu" after its submenus, on
/// desktops without a menu bar: About Seer, where GNOME's primary menu and
/// Windows apps put it. Empty on the Mac, whose app menu has About.
pub fn main_menu_extras() -> Vec<Entry> {
    match platform::os() {
        Os::Mac => Vec::new(),
        _ => vec![item!(
            platform::label(C::About),
            About,
            None,
            Needs::Nothing
        )],
    }
}

/// The Mac menu bar, from the command table, with checkmarks and enabled
/// states from `availability`. `main.rs` sets it at launch with
/// [`Availability::EVERYTHING`]; call `cx.set_menus(actions::menu_bar(&a))`
/// again when a checkmark changes. (AppKit greys out an item whose action
/// has no handler on the focused path whatever this says; see
/// [`Availability`].)
pub fn menu_bar(availability: &Availability) -> Vec<Menu> {
    command_menus()
        .into_iter()
        .map(|menu| {
            Menu::new(menu.name).items(menu.entries.into_iter().map(|entry| match entry {
                Entry::Command {
                    label,
                    action,
                    needs,
                    check,
                    ..
                } => MenuItem::Action {
                    name: label.into(),
                    action: action(),
                    os_action: None,
                    checked: check.is_some_and(|toggle| availability.checked(toggle)),
                    disabled: !availability.allows(needs),
                },
                Entry::Separator => MenuItem::separator(),
                Entry::Services => MenuItem::os_submenu("Services", SystemMenuType::Services),
            }))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_table_has_no_empty_menus_or_stray_separators() {
        for menu in command_menus() {
            assert!(!menu.entries.is_empty(), "{}", menu.name);
            assert!(!matches!(menu.entries.first(), Some(Entry::Separator)));
            assert!(!matches!(menu.entries.last(), Some(Entry::Separator)));
            for pair in menu.entries.windows(2) {
                assert!(
                    !matches!(pair, [Entry::Separator, Entry::Separator]),
                    "{}",
                    menu.name
                );
            }
        }
    }

    /// What `action` needs, from the table.
    fn needs_of(action: &dyn Action) -> Needs {
        command_menus()
            .iter()
            .flat_map(|menu| &menu.entries)
            .chain(&main_menu_extras())
            .find_map(|entry| match entry {
                Entry::Command {
                    action: a, needs, ..
                } if a().partial_eq(action) => Some(*needs),
                _ => None,
            })
            .expect("in the command table")
    }

    trait Allows {
        fn allows_action(&self, action: &dyn Action) -> bool;
    }

    impl Allows for Availability {
        fn allows_action(&self, action: &dyn Action) -> bool {
            self.allows(needs_of(action))
        }
    }

    #[test]
    fn availability_follows_the_selection_and_clipboard() {
        let nothing = Availability {
            items: true,
            local: true,
            ..Availability::default()
        };
        assert!(!nothing.allows_action(&Rename));
        assert!(!nothing.allows_action(&MoveToTrash));
        assert!(!nothing.allows_action(&Paste));
        assert!(!nothing.allows_action(&AnalyzeBitrate));
        assert!(nothing.allows_action(&SelectAll));
        assert!(nothing.allows_action(&OpenFolder));
        let one_video = Availability {
            selected: 1,
            single_media: true,
            ..nothing
        };
        assert!(one_video.allows_action(&Rename));
        assert!(one_video.allows_action(&AnalyzeBitrate));
        let on_device = Availability {
            local: false,
            ..one_video
        };
        assert!(!on_device.allows_action(&AnalyzeBitrate));
        assert!(!on_device.allows_action(&RevealInFinder));
        let two = Availability {
            selected: 2,
            ..one_video
        };
        assert!(!two.allows_action(&Rename));
        assert!(two.allows_action(&Copy));
        // An empty folder cannot be searched.
        assert!(!Availability::default().allows_action(&Find));
    }

    #[test]
    fn show_transfers_follows_the_retained_receipt() {
        let idle = Availability::default();
        assert!(!idle.allows_action(&ShowTransfers));
        let receipt = Availability {
            transfers: true,
            ..idle
        };
        assert!(receipt.allows_action(&ShowTransfers));
        // Sorting never depends on what is listed or selected.
        for sort in [SortByVideo.boxed_clone(), SortByDuration.boxed_clone()] {
            assert!(idle.allows_action(sort.as_ref()));
        }
    }

    #[test]
    fn view_menu_offers_every_sort_with_one_checkmark() {
        let sorts = [
            Sort::Name,
            Sort::Video,
            Sort::Audio,
            Sort::Duration,
            Sort::Size,
            Sort::Modified,
        ];
        let view = command_menus()
            .into_iter()
            .find(|menu| menu.name == "View")
            .expect("a View menu");
        let listed: Vec<Sort> = view
            .entries
            .iter()
            .filter_map(|entry| match entry {
                Entry::Command {
                    check: Some(Toggle::Sort(sort)),
                    ..
                } => Some(*sort),
                _ => None,
            })
            .collect();
        assert_eq!(listed, sorts);
        for sort in sorts {
            let a = Availability {
                sort: Some(sort),
                ..Availability::default()
            };
            let checked = sorts.iter().filter(|s| a.checked(Toggle::Sort(**s)));
            assert_eq!(checked.collect::<Vec<_>>(), [&sort]);
        }
    }

    #[test]
    fn every_table_command_is_enabled_by_everything() {
        for menu in command_menus() {
            for entry in menu.entries {
                if let Entry::Command { needs, label, .. } = entry {
                    assert!(Availability::EVERYTHING.allows(needs), "{label}");
                }
            }
        }
    }
}
