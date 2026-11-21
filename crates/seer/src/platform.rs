//! What differs between operating systems, in one place. Views never branch
//! on the OS and never spell a modifier key or a file manager's name; they
//! ask here. See DESIGN.md, "Platforms".
//!
//! Only macOS is implemented for real. The Windows and GNOME values are data,
//! so their layouts can be judged before a port exists: in debug builds
//! `SEER_PLATFORM=windows|gnome` presents that OS's insets, control side,
//! wording and shortcut hints on the Mac, while the key bindings stay the
//! Mac's (they follow [`host`], the OS actually running).

use crate::theme::size;
use gpui_kit::component::scroll::ScrollbarMode;
use gpui_kit::{Pixels, SharedString, px};
use std::sync::OnceLock;

/// An operating system, as far as Seer's design cares.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Os {
    Mac,
    Windows,
    /// GNOME and other Linux desktops. KDE differs in places (server-side
    /// decorations, an in-window menu bar); not modelled yet.
    Linux,
}

/// The OS Seer is running on. Key bindings and system services follow this.
pub fn host() -> Os {
    if cfg!(target_os = "macos") {
        Os::Mac
    } else if cfg!(target_os = "windows") {
        Os::Windows
    } else {
        Os::Linux
    }
}

/// The OS whose layout and wording Seer presents: [`host`], unless a debug
/// build sets `SEER_PLATFORM=mac|windows|gnome`.
pub fn os() -> Os {
    static OS: OnceLock<Os> = OnceLock::new();
    *OS.get_or_init(|| {
        if !cfg!(debug_assertions) {
            return host();
        }
        match std::env::var("SEER_PLATFORM").as_deref() {
            Ok("windows") => Os::Windows,
            Ok("gnome" | "linux") => Os::Linux,
            Ok("mac" | "macos") => Os::Mac,
            _ => host(),
        }
    })
}

/// True when the presented OS is not the host, so Seer hides the system's
/// window controls and draws stand-ins (see `ui::window_controls`).
pub fn simulated() -> bool {
    os() != host()
}

/// Which side of the titlebar holds the window controls.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Leading,
    Trailing,
}

/// Where the commands live when they are not all on screen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuPlacement {
    /// The system's menu bar (macOS). No menu button in the window.
    MenuBar,
    /// A "Main Menu" icon button at the leading end of the titlebar
    /// (Windows, as Claude and Devin draw it).
    TitlebarLeading,
    /// A "Main Menu" icon button at the trailing end of the titlebar, before
    /// the window controls (GNOME's primary menu).
    TitlebarTrailing,
}

/// The window frame Seer draws its titlebar into.
#[derive(Clone, Copy, Debug)]
pub struct Chrome {
    /// Side of the window controls (traffic lights, caption buttons).
    pub controls: Side,
    /// Space from the window's leading edge to Seer's first titlebar control.
    /// Includes the window controls when they are on that side.
    pub leading: Pixels,
    /// Space from Seer's last titlebar control to the window's trailing edge.
    /// Includes the window controls when they are on that side.
    pub trailing: Pixels,
    /// Width of the window controls themselves, inside `leading` or
    /// `trailing`. `ui::window_controls` draws stand-ins this wide.
    pub controls_width: Pixels,
    /// Height of the titlebar strip.
    pub titlebar: Pixels,
    /// The sidebar may let the window's blur show through (macOS only).
    pub translucent_sidebar: bool,
    /// Where the command menu is: the menu bar, or a "Main Menu" button in
    /// the titlebar built with `ui::command_menu`.
    pub menu: MenuPlacement,
    /// Bare titlebar always kept free of controls, so the window can be
    /// dragged by it. The breadcrumb gives up room before this does.
    pub drag_min: Pixels,
    /// Seer's last titlebar control is separated from window controls on
    /// the same side by a hairline with `size::GAP` on each side of it.
    /// That space is not in `leading` or `trailing`; add it when true.
    pub controls_separator: bool,
    /// The host's UI font families, preferred first. `theme::font` uses the
    /// first one installed, else the system font.
    pub fonts: &'static [&'static str],
    /// The host's text sizes and list row height.
    pub scale: TypeScale,
}

/// Text sizes and the list row height of an OS, in points (plain numbers,
/// so `theme` can make constants of them). Each OS's
/// default UI text: 13pt on the Mac, 14 on Windows (Segoe UI Variable's
/// Body) and GNOME (11pt Adwaita Sans at 96 dpi), with rows tall enough
/// for it. Title keeps a clear step (two points or more) above Body on
/// every OS: 15/20 on the Mac, 16/22 where Body is 14.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TypeScale {
    /// Folder name in the titlebar, file name in the inspector, sheet titles.
    pub title: f32,
    pub title_line: f32,
    pub body: f32,
    pub body_line: f32,
    pub small: f32,
    pub small_line: f32,
    pub caption: f32,
    pub caption_line: f32,
    /// A list row.
    pub row: f32,
    /// A sidebar row: a step above a list row.
    pub sidebar_row: f32,
    /// A Small button, around `small` text.
    pub control_small: f32,
}

/// The scale of `os`. `theme::text` and `theme::size::ROW` take the
/// scale of the OS Seer is built for ([`TARGET`]), as constants.
pub const fn scale(os: Os) -> TypeScale {
    match os {
        Os::Mac => TypeScale {
            title: 15.,
            title_line: 20.,
            body: 13.,
            body_line: 16.,
            small: 11.,
            small_line: 14.,
            caption: 10.,
            caption_line: 13.,
            row: 24.,
            sidebar_row: 28.,
            control_small: 22.,
        },
        Os::Windows | Os::Linux => TypeScale {
            title: 16.,
            title_line: 22.,
            body: 14.,
            body_line: 20.,
            small: 12.,
            small_line: 16.,
            caption: 11.,
            caption_line: 14.,
            row: 28.,
            sidebar_row: 32.,
            control_small: 24.,
        },
    }
}

/// The OS this binary is built for: [`host`] as a constant, so `theme`'s
/// size tokens can follow it.
pub const TARGET: Os = if cfg!(target_os = "macos") {
    Os::Mac
} else if cfg!(target_os = "windows") {
    Os::Windows
} else {
    Os::Linux
};

/// The UI font families of `os`, preferred first. The Mac's is the system
/// font itself.
pub const fn fonts(os: Os) -> &'static [&'static str] {
    match os {
        Os::Mac => &[".SystemUIFont"],
        Os::Windows => &["Segoe UI Variable Text", "Segoe UI Variable", "Segoe UI"],
        Os::Linux => &["Adwaita Sans", "Cantarell"],
    }
}

/// Windows and GNOME open the "Main Menu" when Alt is pressed and released
/// on its own (F10 is bound as [`Command::MainMenu`]). The window watches
/// modifier changes for this; GPUI has no key binding for a bare modifier.
pub fn alt_opens_menu() -> bool {
    os() != Os::Mac
}

/// How scroll bars show, from the host's setting (`auto_hide`: shown only
/// while scrolling, the Mac's "When scrolling" or "Automatically" with a
/// trackpad; Windows' "Automatically hide scroll bars"). When the person
/// has asked for scroll bars to stay, they stay, on every OS. Otherwise the
/// Mac and GNOME show them while scrolling, and Windows also on hover.
pub fn scroll_bars(auto_hide: bool) -> ScrollbarMode {
    match (auto_hide, os()) {
        (false, _) => ScrollbarMode::Always,
        (true, Os::Windows) => ScrollbarMode::Hover,
        (true, _) => ScrollbarMode::Scrolling,
    }
}

/// The titlebar metrics of the presented OS. The caption-button reserve
/// on Windows and GNOME is a constant until a port can read it from the
/// window (`LeftInset` / `RightInset`, the user's button layout).
pub fn chrome() -> Chrome {
    match os() {
        // 14pt traffic lights at x = 12, 20pt apart: they end at 66; 12pt
        // clear after them.
        Os::Mac => Chrome {
            controls: Side::Leading,
            leading: px(78.),
            trailing: px(8.),
            controls_width: px(66.),
            titlebar: px(40.),
            translucent_sidebar: true,
            menu: MenuPlacement::MenuBar,
            drag_min: size::DRAG_MIN,
            controls_separator: false,
            fonts: fonts(Os::Mac),
            scale: scale(Os::Mac),
        },
        // Three 46px caption buttons; a 48px title bar because it holds
        // interactive content (Microsoft's "tall" title bar).
        Os::Windows => Chrome {
            controls: Side::Trailing,
            leading: px(8.),
            trailing: px(138.),
            controls_width: px(138.),
            titlebar: px(48.),
            translucent_sidebar: false,
            menu: MenuPlacement::TitlebarLeading,
            drag_min: size::DRAG_MIN,
            controls_separator: true,
            fonts: fonts(Os::Windows),
            scale: scale(Os::Windows),
        },
        // GNOME's default button layout is a single close button at the end.
        Os::Linux => Chrome {
            controls: Side::Trailing,
            leading: px(6.),
            trailing: px(46.),
            controls_width: px(40.),
            titlebar: px(46.),
            translucent_sidebar: false,
            menu: MenuPlacement::TitlebarTrailing,
            drag_min: size::DRAG_MIN,
            controls_separator: true,
            fonts: fonts(Os::Linux),
            scale: scale(Os::Linux),
        },
    }
}

/// Where macOS draws the traffic lights: centred in the 40pt titlebar.
pub const TRAFFIC_LIGHTS: (Pixels, Pixels) = (px(12.), px(13.));

/// A command whose wording or shortcut depends on the OS. One per action
/// that has either.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Command {
    About,
    Quit,
    Hide,
    HideOthers,
    Minimize,
    GoToFolder,
    OpenSelection,
    NewFolder,
    Rename,
    Reveal,
    CleanUpStreamNames,
    MoveToTrash,
    Copy,
    Paste,
    MoveItemHere,
    SelectAll,
    ClearSelection,
    Find,
    ToggleSidebar,
    ToggleInspector,
    ToggleHiddenFiles,
    GoBack,
    GoForward,
    GoToEnclosingFolder,
    GoHome,
    GoDesktop,
    GoDocuments,
    GoDownloads,
    MoveUp,
    MoveDown,
    ExtendUp,
    ExtendDown,
    MoveToFirst,
    MoveToLast,
    AnalyzeBitrate,
    ShowTransfers,
    /// Opens the "Main Menu" button's menu (Windows and GNOME).
    MainMenu,
    /// Opens the focused row's context menu from the keyboard.
    ContextMenu,
}

/// The command's name as the presented OS words it, for menus, tooltips
/// and buttons.
pub fn label(command: Command) -> &'static str {
    use Command::*;
    let os = os();
    match command {
        About => "About Seer",
        Quit => "Quit Seer",
        Hide => "Hide Seer",
        HideOthers => "Hide Others",
        Minimize => "Minimize",
        // It opens a folder chooser, so it is worded as Open, not Go.
        GoToFolder => "Open Folder…",
        OpenSelection => "Open",
        NewFolder => "New Folder",
        Rename => "Rename",
        Reveal => match os {
            Os::Mac => "Reveal in Finder",
            Os::Windows => "Show in File Explorer",
            Os::Linux => "Show in Files",
        },
        CleanUpStreamNames => "Clean Up Stream Names…",
        MoveToTrash => match os {
            Os::Windows => "Delete",
            _ => "Move to Trash",
        },
        Copy => "Copy",
        Paste => "Paste",
        MoveItemHere => "Move Item Here",
        SelectAll => "Select All",
        ClearSelection => "Deselect All",
        Find => "Find",
        ToggleSidebar => "Toggle Sidebar",
        ToggleInspector => "Toggle Inspector",
        ToggleHiddenFiles => "Toggle Hidden Files",
        GoBack => "Back",
        GoForward => "Forward",
        GoToEnclosingFolder => "Enclosing Folder",
        GoHome => "Home",
        GoDesktop => "Desktop",
        GoDocuments => "Documents",
        GoDownloads => "Downloads",
        MoveUp => "Move Up",
        MoveDown => "Move Down",
        ExtendUp => "Extend Selection Up",
        ExtendDown => "Extend Selection Down",
        MoveToFirst => "First Item",
        MoveToLast => "Last Item",
        AnalyzeBitrate => "Analyse Bit Rate",
        ShowTransfers => "Show Transfers",
        MainMenu => "Main Menu",
        ContextMenu => "Show Context Menu",
    }
}

/// The system file manager's name: "Finder", "File Explorer", "Files".
pub fn file_manager() -> &'static str {
    match os() {
        Os::Mac => "Finder",
        Os::Windows => "File Explorer",
        Os::Linux => "Files",
    }
}

/// Where deleted files go: "Trash" or "Recycle Bin".
pub fn trash() -> &'static str {
    match os() {
        Os::Windows => "Recycle Bin",
        _ => "Trash",
    }
}

/// Where Seer keeps API keys: the system's secret store, by its own name.
pub fn key_store() -> &'static str {
    match os() {
        Os::Mac => "Keychain",
        Os::Windows => "Credential Manager",
        Os::Linux => "keyring",
    }
}

/// What the root of the file system is called when the volume has no name
/// of its own.
pub fn root_name() -> &'static str {
    match os() {
        Os::Mac => "Macintosh HD",
        Os::Windows => "This PC",
        Os::Linux => "Computer",
    }
}

/// GPUI keystrokes bound to `command` on `os`, first one preferred. Empty
/// when the command has no shortcut there.
pub fn keys(command: Command, os: Os) -> &'static [&'static str] {
    use Command::*;
    let mac = os == Os::Mac;
    // Windows and GNOME agree on every binding Seer uses.
    match command {
        About => &[],
        Quit => pick(mac, &["cmd-q"], &["ctrl-q"]),
        Hide => pick(mac, &["cmd-h"], &[]),
        HideOthers => pick(mac, &["alt-cmd-h"], &[]),
        Minimize => pick(mac, &["cmd-m"], &[]),
        GoToFolder => pick(mac, &["shift-cmd-g"], &["ctrl-l"]),
        OpenSelection => pick(mac, &["cmd-down", "cmd-o"], &["enter", "ctrl-o"]),
        NewFolder => pick(mac, &["shift-cmd-n"], &["ctrl-shift-n"]),
        Rename => pick(mac, &["enter"], &["f2"]),
        Reveal => pick(mac, &["cmd-r"], &[]),
        CleanUpStreamNames => pick(mac, &["shift-cmd-k"], &["ctrl-shift-k"]),
        MoveToTrash => pick(mac, &["cmd-backspace"], &["delete"]),
        Copy => pick(mac, &["cmd-c"], &["ctrl-c"]),
        Paste => pick(mac, &["cmd-v"], &["ctrl-v"]),
        MoveItemHere => pick(mac, &["alt-cmd-v"], &["ctrl-shift-v"]),
        SelectAll => pick(mac, &["cmd-a"], &["ctrl-a"]),
        ClearSelection => &["escape"],
        Find => pick(mac, &["cmd-f"], &["ctrl-f"]),
        ToggleSidebar => pick(mac, &["alt-cmd-s"], &["f9"]),
        ToggleInspector => pick(mac, &["alt-cmd-i"], &["alt-enter"]),
        ToggleHiddenFiles => pick(mac, &["shift-cmd-."], &["ctrl-h"]),
        GoBack => pick(mac, &["cmd-["], &["alt-left"]),
        GoForward => pick(mac, &["cmd-]"], &["alt-right"]),
        GoToEnclosingFolder => pick(mac, &["cmd-up"], &["alt-up"]),
        GoHome => pick(mac, &["shift-cmd-h"], &["alt-home"]),
        GoDesktop => pick(mac, &["shift-cmd-d"], &[]),
        GoDocuments => pick(mac, &["shift-cmd-o"], &[]),
        GoDownloads => pick(mac, &["alt-cmd-l"], &[]),
        MoveUp => &["up"],
        MoveDown => &["down"],
        ExtendUp => &["shift-up"],
        ExtendDown => &["shift-down"],
        MoveToFirst => pick(mac, &["home", "alt-up"], &["home"]),
        MoveToLast => pick(mac, &["end", "alt-down"], &["end"]),
        AnalyzeBitrate => pick(mac, &["cmd-b"], &["ctrl-b"]),
        // Browsers' Downloads: Ctrl-J; ⇧⌘J is free in Finder.
        ShowTransfers => pick(mac, &["shift-cmd-j"], &["ctrl-j"]),
        // There is no Main Menu on the Mac: the menu bar has the commands.
        MainMenu => pick(mac, &[], &["f10"]),
        // GNOME and Windows: Shift+F10 or the Menu key on the focused item.
        // The Mac has no convention of its own; ⌃↩ is what Full Keyboard
        // Access users reach for, and Finder leaves it free.
        ContextMenu => pick(mac, &["ctrl-enter"], &["shift-f10", "menu"]),
    }
}

fn pick(
    mac: bool,
    on_mac: &'static [&'static str],
    elsewhere: &'static [&'static str],
) -> &'static [&'static str] {
    if mac { on_mac } else { elsewhere }
}

/// The command's shortcut as hint text on the presented OS ("⌘B",
/// "Ctrl+B"), for tooltips and empty states. `None` when it has none.
pub fn shortcut(command: Command) -> Option<SharedString> {
    let os = os();
    keys(command, os)
        .first()
        .map(|keys| format_keys(keys, os).into())
}

/// Formats a GPUI keystroke ("shift-cmd-k") the way `os` prints shortcuts.
pub fn format_keys(keystroke: &str, os: Os) -> String {
    let mut parts: Vec<&str> = keystroke.split('-').collect();
    // A trailing "-" key splits into two empty parts.
    let key = match parts.last() {
        Some(&"") => {
            parts.truncate(parts.len().saturating_sub(2));
            "-"
        }
        _ => parts.pop().unwrap_or_default(),
    };
    let has = |m: &str| parts.contains(&m);
    match os {
        Os::Mac => {
            let mut out = String::new();
            for (name, glyph) in [("ctrl", "⌃"), ("alt", "⌥"), ("shift", "⇧"), ("cmd", "⌘")]
            {
                if has(name) {
                    out.push_str(glyph);
                }
            }
            out.push_str(&match key {
                "up" => "↑".into(),
                "down" => "↓".into(),
                "left" => "←".into(),
                "right" => "→".into(),
                "enter" => "↩".into(),
                "escape" => "⎋".into(),
                "backspace" => "⌫".into(),
                "delete" => "⌦".into(),
                "tab" => "⇥".into(),
                "space" => "Space".into(),
                "home" => "↖".into(),
                "end" => "↘".into(),
                other => upper(other),
            });
            out
        }
        Os::Windows | Os::Linux => {
            let mut out: Vec<String> = Vec::new();
            for (name, word) in [("ctrl", "Ctrl"), ("alt", "Alt"), ("shift", "Shift")] {
                if has(name) {
                    out.push(word.into());
                }
            }
            out.push(match key {
                "escape" => "Esc".into(),
                "menu" => "Menu".into(),
                "enter" | "up" | "down" | "left" | "right" | "backspace" | "delete" | "tab"
                | "space" | "home" | "end" => {
                    let mut chars = key.chars();
                    chars
                        .next()
                        .map(|c| c.to_ascii_uppercase().to_string() + chars.as_str())
                        .unwrap_or_default()
                }
                other => upper(other),
            });
            out.join("+")
        }
    }
}

fn upper(key: &str) -> String {
    key.to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_shortcuts_use_glyphs_in_apple_order() {
        assert_eq!(format_keys("shift-cmd-k", Os::Mac), "⇧⌘K");
        assert_eq!(format_keys("alt-cmd-i", Os::Mac), "⌥⌘I");
        assert_eq!(format_keys("shift-cmd-.", Os::Mac), "⇧⌘.");
        assert_eq!(format_keys("cmd-[", Os::Mac), "⌘[");
        assert_eq!(format_keys("cmd-backspace", Os::Mac), "⌘⌫");
        assert_eq!(format_keys("enter", Os::Mac), "↩");
    }

    #[test]
    fn other_shortcuts_are_spelled_out() {
        assert_eq!(format_keys("ctrl-shift-k", Os::Windows), "Ctrl+Shift+K");
        assert_eq!(format_keys("alt-left", Os::Linux), "Alt+Left");
        assert_eq!(format_keys("f2", Os::Windows), "F2");
        assert_eq!(format_keys("escape", Os::Windows), "Esc");
        assert_eq!(format_keys("ctrl--", Os::Windows), "Ctrl+-");
        assert_eq!(format_keys("f10", Os::Linux), "F10");
        assert_eq!(format_keys("shift-f10", Os::Windows), "Shift+F10");
        assert_eq!(format_keys("menu", Os::Linux), "Menu");
        assert_eq!(format_keys("ctrl-enter", Os::Mac), "⌃↩");
    }

    #[test]
    fn every_os_has_a_keyboard_route_to_the_context_menu() {
        for os in [Os::Mac, Os::Windows, Os::Linux] {
            assert!(!keys(Command::ContextMenu, os).is_empty(), "{os:?}");
        }
        assert!(keys(Command::ContextMenu, Os::Linux).contains(&"shift-f10"));
        assert!(keys(Command::ContextMenu, Os::Windows).contains(&"menu"));
    }

    #[test]
    fn scroll_bars_kept_on_screen_stay_on_screen() {
        assert_eq!(scroll_bars(false), ScrollbarMode::Always);
        assert_ne!(scroll_bars(true), ScrollbarMode::Always);
    }

    #[test]
    fn every_os_has_a_type_scale_that_fits_its_rows() {
        for os in [Os::Mac, Os::Windows, Os::Linux] {
            let s = scale(os);
            assert!(s.body < s.body_line && s.small < s.small_line && s.caption < s.caption_line);
            assert!(s.body_line < s.row, "{os:?}");
            assert!(s.caption <= s.small && s.small < s.body, "{os:?}");
            assert!(!fonts(os).is_empty());
        }
        assert_eq!(scale(Os::Mac).body, 13.);
        assert_eq!(scale(Os::Windows).body, 14.);
    }

    #[test]
    fn every_mac_binding_formats() {
        use Command::*;
        for command in [
            Quit,
            GoToFolder,
            OpenSelection,
            Rename,
            Reveal,
            MoveToTrash,
            Find,
            ToggleSidebar,
            ToggleInspector,
            ToggleHiddenFiles,
            GoBack,
            GoForward,
            AnalyzeBitrate,
            ShowTransfers,
        ] {
            for keys in keys(command, Os::Mac) {
                assert!(!format_keys(keys, Os::Mac).is_empty(), "{command:?}");
            }
        }
    }
}
