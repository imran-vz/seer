mod actions;
mod ai;
mod browser;
mod chart;
mod cleanup;
mod inspector;
mod library;
mod native;
mod state;
mod theme;
mod ui;
mod workspace;

use actions::*;
use gpui_kit::*;
use library::Library;
use state::Saved;
use theme::{Theme, size};
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
            // Kit controls (the text field) follow the system appearance too.
            gpui_kit::component::Theme::sync_system_appearance(None, cx);
            cx.set_global(Theme::resolve(cx.window_appearance()));
            Library::init(cx);

            bind_keys(cx);
            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.on_action(|_: &Hide, cx| cx.hide());
            cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
            cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
            cx.set_menus(menus());
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
                saved.folder = std::fs::canonicalize(folder).ok();
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
                    // Centres the 14pt traffic lights in the 40pt title bar.
                    traffic_light_position: Some(point(px(12.), px(13.))),
                }),
                app_owns_titlebar_drag: true,
                window_min_size: Some(gpui_kit::size(size::WINDOW_MIN.0, size::WINDOW_MIN.1)),
                window_background: WindowBackgroundAppearance::Blurred,
                ..Default::default()
            };
            gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| Workspace::new(&saved, window, cx))
            })
            .expect("could not open the main window");
            cx.activate(true);
        });
}

fn bind_keys(cx: &mut App) {
    let list = Some("Browser");
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-h", Hide, None),
        KeyBinding::new("alt-cmd-h", HideOthers, None),
        KeyBinding::new("cmd-m", Minimize, None),
        KeyBinding::new("shift-cmd-g", OpenFolder, None),
        KeyBinding::new("alt-cmd-s", ToggleSidebar, None),
        KeyBinding::new("alt-cmd-i", ToggleInspector, None),
        KeyBinding::new("shift-cmd-.", ToggleHiddenFiles, None),
        KeyBinding::new("cmd-[", GoBack, None),
        KeyBinding::new("cmd-]", GoForward, None),
        KeyBinding::new("cmd-up", GoToEnclosingFolder, None),
        KeyBinding::new("shift-cmd-h", GoHome, None),
        KeyBinding::new("shift-cmd-d", GoDesktop, None),
        KeyBinding::new("shift-cmd-o", GoDocuments, None),
        KeyBinding::new("alt-cmd-l", GoDownloads, None),
        KeyBinding::new("cmd-r", RevealInFinder, None),
        KeyBinding::new("cmd-b", AnalyzeBitrate, None),
        KeyBinding::new("cmd-f", Find, None),
        KeyBinding::new("shift-cmd-k", CleanUpStreamNames, None),
        KeyBinding::new("cmd-a", SelectAll, list),
        KeyBinding::new("escape", ClearSelection, list),
        KeyBinding::new("cmd-o", OpenSelection, list),
        KeyBinding::new("cmd-down", OpenSelection, list),
        KeyBinding::new("cmd-backspace", MoveToTrash, list),
        KeyBinding::new("enter", Rename, list),
        KeyBinding::new("shift-cmd-n", NewFolder, list),
        KeyBinding::new("up", MoveUp, list),
        KeyBinding::new("down", MoveDown, list),
        KeyBinding::new("shift-up", ExtendUp, list),
        KeyBinding::new("shift-down", ExtendDown, list),
        KeyBinding::new("home", MoveToFirst, list),
        KeyBinding::new("alt-up", MoveToFirst, list),
        KeyBinding::new("end", MoveToLast, list),
        KeyBinding::new("alt-down", MoveToLast, list),
    ]);
}

fn menus() -> Vec<Menu> {
    vec![
        Menu::new("Seer").items([
            MenuItem::os_submenu("Services", SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action("Hide Seer", Hide),
            MenuItem::action("Hide Others", HideOthers),
            MenuItem::action("Show All", ShowAll),
            MenuItem::separator(),
            MenuItem::action("Quit Seer", Quit),
        ]),
        Menu::new("File").items([
            MenuItem::action("New Folder", NewFolder),
            MenuItem::action("Open", OpenSelection),
            MenuItem::separator(),
            MenuItem::action("Rename", Rename),
            MenuItem::action("Reveal in Finder", RevealInFinder),
            MenuItem::separator(),
            MenuItem::action("Analyse Bit Rate", AnalyzeBitrate),
            MenuItem::action("Clean Up Stream Names…", CleanUpStreamNames),
            MenuItem::separator(),
            MenuItem::action("Move to Trash", MoveToTrash),
        ]),
        Menu::new("Edit").items([
            MenuItem::action("Select All", SelectAll),
            MenuItem::action("Deselect All", ClearSelection),
            MenuItem::separator(),
            MenuItem::action("Find", Find),
        ]),
        Menu::new("View").items([
            MenuItem::action("Toggle Sidebar", ToggleSidebar),
            MenuItem::action("Toggle Inspector", ToggleInspector),
            MenuItem::separator(),
            MenuItem::action("Sort by Name", SortByName),
            MenuItem::action("Sort by Size", SortBySize),
            MenuItem::action("Sort by Date Modified", SortByModified),
            MenuItem::separator(),
            MenuItem::action("Toggle Hidden Files", ToggleHiddenFiles),
        ]),
        Menu::new("Go").items([
            MenuItem::action("Back", GoBack),
            MenuItem::action("Forward", GoForward),
            MenuItem::action("Enclosing Folder", GoToEnclosingFolder),
            MenuItem::separator(),
            MenuItem::action("Home", GoHome),
            MenuItem::action("Desktop", GoDesktop),
            MenuItem::action("Documents", GoDocuments),
            MenuItem::action("Downloads", GoDownloads),
            MenuItem::action("Movies", GoMovies),
            MenuItem::action("Music", GoMusic),
            MenuItem::action("Pictures", GoPictures),
            MenuItem::separator(),
            MenuItem::action("Go to Folder…", OpenFolder),
        ]),
        Menu::new("Window").items([
            MenuItem::action("Minimize", Minimize),
            MenuItem::action("Zoom", Zoom),
        ]),
    ]
}
