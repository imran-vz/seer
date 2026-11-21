mod actions;
mod ai;
mod browser;
mod chart;
mod cleanup;
mod devices;
mod inspector;
mod library;
mod native;
mod platform;
mod state;
mod theme;
mod ui;
mod workspace;

use actions::*;
use gpui_kit::*;
use library::Library;
use platform::Command as C;
use state::Saved;
use theme::size;
use workspace::Workspace;

fn main() {
    if let Err(e) = seer_core::init() {
        eprintln!("Seer could not start FFmpeg: {e:#}");
        std::process::exit(1);
    }

    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(|cx: &mut App| {
            gpui_kit::init(cx);
            // SEER_APPEARANCE=light|dark overrides the system setting, for checking both.
            match std::env::var("SEER_APPEARANCE").as_deref() {
                Ok("light") => native::set_appearance(Some(false)),
                Ok("dark") => native::set_appearance(Some(true)),
                _ => {}
            }
            // Seer's theme, and the kit controls (fields, popovers) mapped onto it.
            theme::apply(cx.window_appearance(), None, cx);
            Library::init(cx);
            devices::Devices::init(cx);

            bind_keys(cx);
            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.on_action(|_: &Hide, cx| cx.hide());
            cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
            cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
            cx.set_menus(actions::menu_bar(&Availability::EVERYTHING));
            // A single-window app ends when its window closes.
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            let mut saved = Saved::load();
            // `seer <folder>` opens that folder instead of the last one.
            if let Some(folder) = std::env::args_os().nth(1).map(std::path::PathBuf::from) {
                saved.folder = match seer_core::mtp::is_device_path(&folder) {
                    true => Some(folder),
                    false => std::fs::canonicalize(folder).ok(),
                };
            }
            let frame = saved.window.map(|[x, y, w, h]| {
                WindowBounds::Windowed(Bounds::new(
                    point(px(x), px(y)),
                    gpui_kit::size(px(w), px(h)),
                ))
            });
            let options = WindowOptions {
                window_bounds: frame.or_else(|| {
                    Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        gpui_kit::size(px(1180.), px(720.)),
                        cx,
                    )))
                }),
                titlebar: Some(TitlebarOptions {
                    title: None,
                    appears_transparent: true,
                    traffic_light_position: Some(point(
                        platform::TRAFFIC_LIGHTS.0,
                        platform::TRAFFIC_LIGHTS.1,
                    )),
                }),
                app_owns_titlebar_drag: true,
                window_min_size: Some(gpui_kit::size(size::WINDOW_MIN.0, size::WINDOW_MIN.1)),
                window_background: match platform::chrome().translucent_sidebar {
                    true => WindowBackgroundAppearance::Blurred,
                    false => WindowBackgroundAppearance::Opaque,
                },
                ..Default::default()
            };
            gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| Workspace::new(&saved, window, cx))
            })
            .expect("could not open the main window");
            // SEER_PLATFORM presents another OS: Seer draws its controls.
            if platform::simulated() {
                native::hide_window_buttons();
            }
            cx.activate(true);
        });
}

/// Binds every command to the keys `platform::keys` gives for the OS Seer
/// is running on.
fn bind_keys(cx: &mut App) {
    let list = Some("Browser");
    let os = platform::host();
    let mut bindings = Vec::new();
    macro_rules! bind {
        ($command:expr, $action:expr, $context:expr) => {
            for keys in platform::keys($command, os) {
                bindings.push(KeyBinding::new(keys, $action, $context));
            }
        };
    }
    bind!(C::Quit, Quit, None);
    bind!(C::Hide, Hide, None);
    bind!(C::HideOthers, HideOthers, None);
    bind!(C::Minimize, Minimize, None);
    bind!(C::GoToFolder, OpenFolder, None);
    bind!(C::ToggleSidebar, ToggleSidebar, None);
    bind!(C::ToggleInspector, ToggleInspector, None);
    bind!(C::ToggleHiddenFiles, ToggleHiddenFiles, None);
    bind!(C::GoBack, GoBack, None);
    bind!(C::GoForward, GoForward, None);
    bind!(C::GoToEnclosingFolder, GoToEnclosingFolder, None);
    bind!(C::GoHome, GoHome, None);
    bind!(C::GoDesktop, GoDesktop, None);
    bind!(C::GoDocuments, GoDocuments, None);
    bind!(C::GoDownloads, GoDownloads, None);
    bind!(C::Reveal, RevealInFinder, None);
    bind!(C::AnalyzeBitrate, AnalyzeBitrate, None);
    bind!(C::Find, Find, None);
    bind!(C::CleanUpStreamNames, CleanUpStreamNames, None);
    bind!(C::ShowTransfers, ShowTransfers, None);
    bind!(C::ContextMenu, ShowContextMenu, None);
    bind!(C::Copy, Copy, list);
    bind!(C::Paste, Paste, list);
    bind!(C::MoveItemHere, MoveItemHere, list);
    bind!(C::SelectAll, SelectAll, list);
    bind!(C::ClearSelection, ClearSelection, list);
    bind!(C::OpenSelection, OpenSelection, list);
    bind!(C::MoveToTrash, MoveToTrash, list);
    bind!(C::Rename, Rename, list);
    bind!(C::NewFolder, NewFolder, list);
    bind!(C::MoveUp, MoveUp, list);
    bind!(C::MoveDown, MoveDown, list);
    bind!(C::ExtendUp, ExtendUp, list);
    bind!(C::ExtendDown, ExtendDown, list);
    bind!(C::MoveToFirst, MoveToFirst, list);
    bind!(C::MoveToLast, MoveToLast, list);
    // The Main Menu button follows the presented OS, so its key does too:
    // F10 opens it in a simulated Windows or GNOME layout on the Mac.
    for keys in platform::keys(C::MainMenu, platform::os()) {
        bindings.push(KeyBinding::new(keys, ShowMainMenu, None));
    }
    cx.bind_keys(bindings);
}
