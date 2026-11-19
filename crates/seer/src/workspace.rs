//! The window: sidebar, toolbar, file list and inspector, and the commands
//! that span them.

use crate::actions::*;
use crate::browser::{Browser, BrowserEvent, SidePanes, folder_name};
use crate::cleanup::{Cleanup, CleanupEvent};
use crate::inspector::{Inspector, InspectorEvent};
use crate::state::Saved;
use crate::theme::{Theme, size, text};
use crate::ui;
use gpui_kit::assets::IconName;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::*;
use seer_core::fs::SortKey;
use std::path::PathBuf;
use std::time::Duration;

struct Place {
    name: String,
    icon: IconName,
    path: PathBuf,
}

#[derive(Clone, Copy, PartialEq)]
enum Pane {
    Sidebar,
    Inspector,
}

pub struct Workspace {
    browser: Entity<Browser>,
    inspector: Entity<Inspector>,
    search: Entity<InputState>,
    /// The folder-wide stream name review, while it is open.
    cleanup: Option<Entity<Cleanup>>,
    favourites: Vec<Place>,
    locations: Vec<Place>,
    sidebar_visible: bool,
    inspector_visible: bool,
    sidebar_width: Pixels,
    inspector_width: Pixels,
    /// The divider being dragged.
    resizing: Option<Pane>,
    save: Option<Task<()>>,
}

impl Workspace {
    pub fn new(saved: &Saved, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let folder = saved
            .folder
            .clone()
            .filter(|f| f.is_dir())
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("/"));
        let sort = match saved.sort.as_str() {
            "size" => SortKey::Size,
            "modified" => SortKey::Modified,
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
        let search = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Search")
                .clean_on_escape()
        });
        cx.subscribe_in(
            &search,
            window,
            |this, search, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    let query = search.read(cx).value();
                    this.browser
                        .update(cx, |browser, cx| browser.set_query(&query, cx));
                }
            },
        )
        .detach();

        cx.subscribe_in(&browser, window, |this, browser, event, window, cx| {
            match event {
                BrowserEvent::SelectionChanged => {
                    let selection = browser.read(cx).selection();
                    this.inspector
                        .update(cx, |inspector, cx| inspector.show(selection, cx));
                }
                BrowserEvent::Navigated => {
                    window.set_window_title(&folder_name(browser.read(cx).dir()));
                    // A search belongs to the folder it was typed in.
                    this.search
                        .update(cx, |search, cx| search.set_value("", window, cx));
                }
            }
            this.schedule_save(window, cx);
            cx.notify();
        })
        .detach();

        // Selection colours and toolbar dimming depend on whether the window is key.
        cx.observe_window_activation(window, |_, window, _| window.refresh())
            .detach();
        cx.observe_window_appearance(window, |_, window, cx| {
            cx.set_global(Theme::resolve(window.appearance()));
            gpui_kit::component::Theme::sync_system_appearance(Some(window), cx);
            window.refresh();
        })
        .detach();
        cx.observe_window_bounds(window, |this, window, cx| this.schedule_save(window, cx))
            .detach();

        window.set_window_title(&folder_name(browser.read(cx).dir()));
        browser.read(cx).focus_handle(cx).focus(window, cx);

        let this = Workspace {
            browser,
            inspector,
            search,
            cleanup: None,
            favourites: favourites(),
            locations: locations(),
            sidebar_visible: saved.sidebar_visible,
            inspector_visible: saved.inspector_visible,
            sidebar_width: px(saved.sidebar_width).clamp(size::SIDEBAR.0, size::SIDEBAR.2),
            inspector_width: px(saved.inspector_width).clamp(size::INSPECTOR.0, size::INSPECTOR.2),
            resizing: None,
            save: None,
        };
        this.publish_panes(cx);
        this
    }

    fn publish_panes(&self, cx: &mut App) {
        let mut total = px(0.);
        if self.sidebar_visible {
            total += self.sidebar_width;
        }
        if self.inspector_visible {
            total += self.inspector_width;
        }
        cx.set_global(SidePanes(total));
    }

    /// Writes the window's state to disk shortly after it stops changing.
    fn schedule_save(&mut self, window: &Window, cx: &mut Context<Self>) {
        let browser = self.browser.read(cx);
        let (sort, ascending) = browser.sort();
        let frame = match window.window_bounds() {
            WindowBounds::Windowed(b) => Some(b),
            _ => None,
        };
        let saved = Saved {
            folder: Some(browser.dir().to_owned()),
            sidebar_width: self.sidebar_width.into(),
            inspector_width: self.inspector_width.into(),
            sidebar_visible: self.sidebar_visible,
            inspector_visible: self.inspector_visible,
            show_hidden: browser.show_hidden(),
            sort: match sort {
                SortKey::Size => "size",
                SortKey::Modified => "modified",
                _ => "name",
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
        // Going somewhere else ends a review of the folder we were in.
        self.cleanup = None;
        if let Some(path) = path.filter(|p| p.is_dir()) {
            self.browser
                .update(cx, |browser, cx| browser.navigate(path, cx));
        }
    }

    /// Opens the review of stream names for everything under this folder.
    fn clean_up_names(&mut self, cx: &mut Context<Self>) {
        let root = self.browser.read(cx).dir().to_owned();
        let cleanup = cx.new(|cx| Cleanup::new(root, cx));
        cx.subscribe(&cleanup, |this, _, _: &CleanupEvent, cx| {
            this.cleanup = None;
            cx.notify();
        })
        .detach();
        self.cleanup = Some(cleanup);
        cx.notify();
    }

    fn toggle(&mut self, pane: Pane, window: &mut Window, cx: &mut Context<Self>) {
        match pane {
            Pane::Sidebar => self.sidebar_visible = !self.sidebar_visible,
            Pane::Inspector => self.inspector_visible = !self.inspector_visible,
        }
        self.publish_panes(cx);
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
        let other = match pane {
            Pane::Sidebar if self.inspector_visible => self.inspector_width,
            Pane::Inspector if self.sidebar_visible => self.sidebar_width,
            _ => px(0.),
        };
        // Never squeeze the file list below its minimum.
        let room = viewport - other - size::CONTENT_MIN;
        match pane {
            Pane::Sidebar => {
                self.sidebar_width = event
                    .position
                    .x
                    .min(room)
                    .clamp(size::SIDEBAR.0, size::SIDEBAR.2);
            }
            Pane::Inspector => {
                self.inspector_width = (viewport - event.position.x)
                    .min(room)
                    .clamp(size::INSPECTOR.0, size::INSPECTOR.2);
            }
        }
        self.publish_panes(cx);
        cx.notify();
    }

    fn open_folder(&mut self, _: &OpenFolder, _: &mut Window, cx: &mut Context<Self>) {
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

    fn render_sidebar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::of(cx).clone();
        let current = self.browser.read(cx).dir().to_owned();
        let active = window.is_window_active();

        let group = |title: &'static str, places: &[Place], cx: &mut Context<Self>| {
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .px(size::ROW_INSET)
                        .pb(px(4.))
                        .child(ui::section_header(title, cx)),
                )
                .children(places.iter().map(|place| {
                    let selected = place.path == current;
                    let path = place.path.clone();
                    div()
                        .id(SharedString::from(
                            place.path.to_string_lossy().into_owned(),
                        ))
                        .flex()
                        .flex_none()
                        .items_center()
                        .gap(px(6.))
                        .h(size::SIDEBAR_ROW)
                        .px(size::ROW_INSET)
                        .rounded(size::RADIUS)
                        .when(selected, |this| this.bg(t.fill))
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.go(Some(path.clone()), cx)),
                        )
                        .child(ui::icon(
                            place.icon,
                            if active { t.accent } else { t.secondary_label },
                        ))
                        .child(div().min_w_0().truncate().child(place.name.clone()))
                }))
        };

        div()
            .flex()
            .flex_col()
            .flex_none()
            .w(self.sidebar_width)
            .h_full()
            .bg(t.sidebar)
            .text_size(text::BODY)
            .line_height(text::BODY_LINE)
            .text_color(t.label)
            .child(drag_region().h(size::TITLEBAR).flex_none())
            .child(
                div()
                    .id("places")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .px(size::ROW_INSET)
                    .pb(size::PANE_PADDING)
                    .child(group("Favourites", &self.favourites, cx))
                    .child(group("Locations", &self.locations, cx)),
            )
    }

    fn render_toolbar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::of(cx).clone();
        let browser = self.browser.read(cx);
        let (back, forward) = (browser.can_go_back(), browser.can_go_forward());
        let title = folder_name(browser.dir());

        drag_region()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(4.))
            .h(size::TITLEBAR)
            .pl(if self.sidebar_visible {
                size::ROW_INSET
            } else {
                size::TRAFFIC_LIGHTS
            })
            .pr(size::ROW_INSET)
            .bg(t.content)
            .border_b(ui::hairline(window))
            .border_color(t.separator)
            // Controls in a window that is not key are drawn subdued.
            .when(!window.is_window_active(), |this| this.opacity(0.5))
            .child(
                ui::icon_button("sidebar", IconName::PanelLeft, true, cx).on_click(
                    cx.listener(|this, _, window, cx| this.toggle(Pane::Sidebar, window, cx)),
                ),
            )
            .child(
                ui::icon_button("back", IconName::ChevronLeft, back, cx).on_click(
                    cx.listener(|this, _, _, cx| this.browser.update(cx, |b, cx| b.go_back(cx))),
                ),
            )
            .child(
                ui::icon_button("forward", IconName::ChevronRight, forward, cx).on_click(
                    cx.listener(|this, _, _, cx| this.browser.update(cx, |b, cx| b.go_forward(cx))),
                ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .pl(px(6.))
                    .truncate()
                    .text_size(text::BODY)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(t.label)
                    .child(title),
            )
            .child(
                ui::icon_button("clean-up", IconName::ScanSearch, true, cx)
                    .on_click(cx.listener(|this, _, _, cx| this.clean_up_names(cx))),
            )
            .child(
                div()
                    .flex_none()
                    .w(px(180.))
                    // Clicks in the field edit text; they don't drag the window.
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        Input::new(&self.search)
                            .small()
                            .cleanable(true)
                            .prefix(ui::icon(IconName::Search, t.secondary_label).size(px(13.))),
                    ),
            )
            .child(
                ui::icon_button("inspector", IconName::PanelRight, true, cx).on_click(
                    cx.listener(|this, _, window, cx| this.toggle(Pane::Inspector, window, cx)),
                ),
            )
    }

    /// A hairline with a wider invisible strip for grabbing.
    fn divider(&self, pane: Pane, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::of(cx);
        let reset = match pane {
            Pane::Sidebar => size::SIDEBAR.1,
            Pane::Inspector => size::INSPECTOR.1,
        };
        div()
            .relative()
            .flex_none()
            .w(ui::hairline(window))
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
                                this.publish_panes(cx);
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
        div()
            .size_full()
            .flex()
            .font_family(crate::theme::FONT)
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
            .on_action(cx.listener(Self::open_folder))
            .on_action(cx.listener(|this, _: &CleanUpStreamNames, _, cx| this.clean_up_names(cx)))
            .on_action(cx.listener(|this, _: &Find, window, cx| {
                this.search
                    .update(cx, |search, cx| search.focus(window, cx));
            }))
            .on_action(cx.listener(|this, _: &ToggleSidebar, window, cx| {
                this.toggle(Pane::Sidebar, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ToggleInspector, window, cx| {
                this.toggle(Pane::Inspector, window, cx)
            }))
            .on_action(cx.listener(|this, _: &AnalyzeBitrate, window, cx| {
                if !this.inspector_visible {
                    this.toggle(Pane::Inspector, window, cx);
                }
                this.inspector
                    .update(cx, |inspector, cx| inspector.analyze(cx));
            }))
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
            .when(self.sidebar_visible, |this| {
                this.child(self.render_sidebar(window, cx))
                    .child(self.divider(Pane::Sidebar, window, cx))
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    // Opaque, so only the sidebar shows the window's blur.
                    .bg(t.content)
                    .child(self.render_toolbar(window, cx))
                    .child(match &self.cleanup {
                        // A review takes over the content area until it is closed.
                        Some(cleanup) => div().flex_1().min_h_0().child(cleanup.clone()),
                        None => div()
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .h_full()
                                    .child(self.browser.clone()),
                            )
                            .when(self.inspector_visible, |this| {
                                this.child(self.divider(Pane::Inspector, window, cx)).child(
                                    div()
                                        .flex_none()
                                        .w(self.inspector_width)
                                        .h_full()
                                        .child(self.inspector.clone()),
                                )
                            }),
                    }),
            )
    }
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
            })
        })
        .collect()
}

fn locations() -> Vec<Place> {
    let mut places = vec![Place {
        name: "Macintosh HD".into(),
        icon: IconName::HardDrive,
        path: PathBuf::from("/"),
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
                icon: IconName::HardDrive,
                path: volume.path(),
            });
        }
    }
    places
}
