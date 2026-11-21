//! The details pane for the current selection.
//!
//! One row grammar throughout (`ui::field_with_rail`: label, then value,
//! on one left edge, the label column narrowing in a narrow pane). Info is media first: duration, video and audio lead; then the
//! file's details, naming and tags. Streams is a list of independent
//! checkboxes with the consequence beside the action. Bit Rate is the chart.
//!
//! With nothing selected the folder is the subject: its size and what it
//! holds, how long its media runs and how much of it could be read, one bar
//! segment per file in the two data hues, and its files by codec. Several
//! items get the same summary for the selection. Totals always say what
//! they leave out ("0:35 · 1 unreadable", "2 of 3 videos").
//!
//! Text fits at every inspector width: separated facts are packed into
//! lines by measuring them (`dot_list`), so no line starts or ends with
//! "·", and the location is chosen to fit, never cut twice.
//!
//! The tabs are one tab stop, Left and Right move between them
//! (`ui::tabs`). The chart is one tab stop too: arrow keys move its readout
//! (see `readout_step`), and it is announced as a slider named "Bit rate
//! over time" whose value is the readout's sample, or the headline when
//! there is none (`chart::spoken_value`).
//!
//! The file's name wraps onto a second line before it is cut in the
//! middle. While a search narrows the list, the folder summary says its
//! numbers are the whole folder's. During a copy into the folder, its
//! header measures what has arrived; the list already says it is copying.

use crate::browser::{display_path, folder_name};
use crate::chart::{self, PlotBounds, Readout, bitrate_chart};
use crate::devices::Devices;
use crate::library::{Bitrate, Library, Probe, Prune};
use crate::platform::{self, Command};
use crate::theme::{self, Theme, size, text};
use crate::ui::Tip as _;
use crate::{ai, native, ui};
use gpui_kit::assets::IconName;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;
use seer_core::bitrate::Analysis;
use seer_core::format;
use seer_core::fs::Entry;
use seer_core::fs::Kind;
use seer_core::naming;
use seer_core::probe::{MediaInfo, Stream, StreamKind};
use seer_core::recommend;
use seer_core::titles;
use std::cell::Cell;
use std::collections::HashSet;
use std::ops::Range;
use std::path::{MAIN_SEPARATOR, MAIN_SEPARATOR_STR, Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

/// Lines the inspector's title may take before it is cut in the middle.
const TITLE_LINES: usize = 2;

/// Container tags that describe how the file was muxed, not what it holds.
/// They sit behind "Show technical tags".
const TECHNICAL_TAGS: &[&str] = &[
    "major_brand",
    "minor_version",
    "compatible_brands",
    "encoder",
    "handler_name",
    "vendor_id",
];

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Info,
    Streams,
    Bitrate,
}

pub enum InspectorEvent {
    /// The person accepted a suggested name for this file.
    Rename { path: PathBuf, name: String },
}

/// Where the request to an AI service for a name stands.
enum Asking {
    Idle,
    /// The form for choosing a service, model and key is showing.
    Setup,
    Waiting,
    Answered(String),
    Failed(SharedString),
}

/// The folder the list shows, summarised when nothing is selected.
struct Folder {
    dir: PathBuf,
    /// Everything in it as listed, whatever a search shows; `None` while it
    /// is being read.
    entries: Option<Vec<Entry>>,
    /// A search is narrowing the list, so the summary says it covers the
    /// whole folder.
    filtered: bool,
}

pub struct Inspector {
    items: Vec<Entry>,
    folder: Option<Folder>,
    /// The pane's width at the last layout, so text can be fitted to it.
    width: Rc<Cell<Pixels>>,
    tab: Tab,
    /// Indexes of the streams marked for removal in the Streams tab.
    unwanted: HashSet<usize>,
    library: Entity<Library>,
    /// The chart's readout: the sample under the pointer, or the one chosen
    /// from the keyboard while the chart has focus.
    readout: Option<Readout>,
    /// The chart's place in the tab order; arrow keys move its readout.
    chart_focus: FocusHandle,
    plot: PlotBounds,
    asking: Asking,
    /// The technical container tags are showing.
    all_tags: bool,
    ai: ai::Settings,
    model_field: Entity<InputState>,
    key_field: Entity<InputState>,
    request: Option<Task<()>>,
}

impl EventEmitter<InspectorEvent> for Inspector {}

impl Inspector {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let library = Library::global(cx);
        cx.observe(&library, |_, _, cx| cx.notify()).detach();
        // A copy into the folder shown changes what its summary says.
        cx.observe(&Devices::global(cx), |this, _, cx| {
            if this.items.is_empty() {
                cx.notify();
            }
        })
        .detach();
        let chart_focus = cx.focus_handle().tab_stop(true);
        // A readout chosen from the keyboard belongs to the chart's focus.
        cx.on_blur(&chart_focus, window, |this, _, cx| {
            if matches!(this.readout, Some(Readout::Sample(_))) {
                this.readout = None;
                cx.notify();
            }
        })
        .detach();
        Inspector {
            items: Vec::new(),
            folder: None,
            width: Rc::new(Cell::new(px(0.))),
            tab: Tab::Info,
            unwanted: HashSet::new(),
            library,
            readout: None,
            chart_focus,
            plot: PlotBounds::default(),
            asking: Asking::Idle,
            all_tags: false,
            ai: ai::Settings::load(),
            model_field: cx.new(|cx| InputState::new(window, cx).placeholder("Model")),
            key_field: cx.new(|cx| {
                InputState::new(window, cx)
                    .masked(true)
                    .placeholder("API key")
            }),
            request: None,
        }
    }

    /// Asks the chosen service for a name, opening the setup form first if
    /// it has no key or no model yet.
    fn ask_ai(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let [entry] = self.items.as_slice() else {
            return;
        };
        let provider = self.ai.provider;
        let model = self.ai.model(provider);
        let (Some(key), false) = (provider.api_key(), model.is_empty()) else {
            self.open_setup(window, cx);
            return;
        };
        let info = self.library.read(cx).info(&entry.path).cloned();
        let query = ai::Query {
            file_name: entry.name.clone(),
            title_tag: info.as_ref().and_then(|i| {
                i.tags
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("title"))
                    .map(|(_, v)| v.clone())
            }),
            duration: info
                .as_ref()
                .filter(|i| i.duration > 0.)
                .map(|i| format::duration(i.duration)),
            audio_languages: info
                .as_ref()
                .map(|i| recommend::audio_languages(i))
                .unwrap_or_default(),
        };
        let path = entry.path.clone();
        self.asking = Asking::Waiting;
        self.request = Some(cx.spawn(async move |this, cx| {
            let answer = cx
                .background_spawn(async move { ai::suggest_name(provider, &model, &key, &query) })
                .await;
            this.update(cx, |this, cx| {
                // The selection may have moved on while we waited.
                if matches!(this.items.as_slice(), [e] if e.path == path) {
                    this.asking = match answer {
                        Ok(name) => Asking::Answered(name),
                        Err(e) => Asking::Failed(format!("{e:#}").into()),
                    };
                    cx.notify();
                }
            })
            .ok();
        }));
        cx.notify();
    }

    /// Shows the form for choosing a service, its model and its key.
    fn open_setup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let model = self.ai.model(self.ai.provider);
        let hint = self.ai.provider.model_hint();
        self.model_field.update(cx, |field, cx| {
            field.set_placeholder(hint, window, cx);
            field.set_value(model, window, cx);
        });
        self.key_field
            .update(cx, |field, cx| field.set_value("", window, cx));
        self.asking = Asking::Setup;
        cx.notify();
    }

    fn choose_provider(
        &mut self,
        provider: ai::Provider,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Keep what was typed for the service being left.
        let typed = self.model_field.read(cx).value();
        self.ai.set_model(self.ai.provider, &typed);
        self.ai.provider = provider;
        self.open_setup(window, cx);
    }

    fn save_setup_and_ask(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let provider = self.ai.provider;
        let model = self.model_field.read(cx).value();
        self.ai.set_model(provider, &model);
        self.ai.save();

        let key = self.key_field.read(cx).value().trim().to_owned();
        if !key.is_empty()
            && let Err(e) = provider.save_api_key(&key)
        {
            self.asking = Asking::Failed(format!("{e:#}").into());
            cx.notify();
            return;
        }
        self.key_field
            .update(cx, |field, cx| field.set_value("", window, cx));
        // Still missing something: stay on the form.
        if provider.api_key().is_none() || self.ai.model(provider).is_empty() {
            cx.notify();
            return;
        }
        self.ask_ai(window, cx);
    }

    /// The service, model and key, in the inspector's row grammar.
    fn render_setup(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let provider = self.ai.provider;
        let this = cx.entity().downgrade();
        let services: Vec<_> = ai::Provider::ALL.iter().map(|p| (*p, p.label())).collect();
        let has_key = provider.api_key().is_some();
        let missing = match (
            has_key,
            self.ai.model(provider).is_empty()
                && self.model_field.read(cx).value().trim().is_empty(),
        ) {
            (false, true) => Some("Enter a model and an API key."),
            (false, false) => Some("Enter an API key."),
            (true, true) => Some("Enter a model."),
            (true, false) => None,
        };
        let detail = format!(
            "{} Only the file name, its title tag, duration and audio languages are sent to {}, never the file. The key is kept in the {}.",
            match has_key {
                true => "A key is saved; leave the field empty to keep it.",
                false => "Paste your API key.",
            },
            provider.label(),
            platform::key_store()
        );
        div()
            .flex()
            .flex_col()
            .gap(size::GAP_SMALL)
            // Full width: three service names do not fit the value column
            // of the narrowest inspector.
            .child(ui::segmented(
                "ai-provider",
                &services,
                provider,
                move |provider, window, cx| {
                    this.update(cx, |this, cx| this.choose_provider(provider, window, cx))
                        .ok();
                },
                cx,
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(ui::field_with_rail(
                        "Model",
                        ui::text_field(&self.model_field, window, cx).small(),
                        self.rail(),
                        cx,
                    ))
                    .child(ui::field_with_rail(
                        "API key",
                        ui::text_field(&self.key_field, window, cx).small(),
                        self.rail(),
                        cx,
                    ))
                    .child(ui::field_with_rail(
                        "",
                        div()
                            .flex()
                            .flex_col()
                            .gap(size::GAP_SMALL)
                            .child(note(detail, cx))
                            .when_some(missing, |this, missing| {
                                this.child(note(missing, cx).text_color(Theme::of(cx).orange))
                            })
                            .child(
                                div()
                                    .flex()
                                    .gap(size::GAP)
                                    .pt(size::GAP_SMALL)
                                    .child(ui::Button::new("cancel-ai", "Cancel").small().on_click(
                                        cx.listener(|this, _, _, cx| {
                                            this.asking = Asking::Idle;
                                            cx.notify();
                                        }),
                                    ))
                                    .child(
                                        ui::Button::new("save-ai", "Save and Ask")
                                            .small()
                                            .primary()
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.save_setup_and_ask(window, cx)
                                            })),
                                    ),
                            ),
                        self.rail(),
                        cx,
                    )),
            )
    }

    /// Whether naming has something to act on, which moves it up to sit
    /// right under the media.
    fn naming_is_active(&self, entry: &Entry) -> bool {
        entry.kind == Kind::Video
            && (naming::suggest(&entry.name).is_some() || !matches!(self.asking, Asking::Idle))
    }

    /// Name suggestions for a video: one worked out here, one from an AI
    /// model on request. Every row is a field: its label says where the
    /// name comes from.
    fn render_naming(
        &self,
        entry: &Entry,
        first: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        if entry.kind != Kind::Video {
            return None;
        }
        let t = Theme::of(cx).clone();
        let service = self.ai.provider.label();
        let rename = |id: &'static str, name: String, cx: &mut Context<Self>| {
            let (path, new_name) = (entry.path.clone(), name);
            ui::Button::new(id, "Rename")
                .small()
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.asking = Asking::Idle;
                    cx.emit(InspectorEvent::Rename {
                        path: path.clone(),
                        name: new_name.clone(),
                    });
                    cx.notify();
                }))
        };
        let ask = |label: &'static str, cx: &mut Context<Self>| {
            ui::Button::new("ask-ai", label)
                .small()
                .on_click(cx.listener(|this, _, window, cx| this.ask_ai(window, cx)))
        };
        let settings = |cx: &mut Context<Self>| {
            ui::Button::new("ai-setup", "AI Settings…")
                .small()
                .ghost()
                .on_click(cx.listener(|this, _, window, cx| this.open_setup(window, cx)))
        };
        let proposal = |name: String, buttons: Div| {
            div()
                .flex()
                .flex_col()
                .gap(size::GAP_SMALL)
                .child(div().text_color(t.label).child(name))
                .child(buttons.flex().gap(size::GAP))
        };

        let offline = naming::suggest(&entry.name);
        let idle = matches!(self.asking, Asking::Idle);
        let mut body = section("Naming", first, cx);
        body = match &offline {
            Some(name) => body.child(ui::field_with_rail(
                "Suggested",
                proposal(
                    name.clone(),
                    div()
                        .child(rename("rename-offline", name.clone(), cx))
                        .when(idle, |this| this.child(ask("Ask AI", cx))),
                ),
                self.rail(),
                cx,
            )),
            // Nothing to suggest: one row, with the way to ask at its end.
            None if idle => body.child(ui::field_with_rail(
                "Name",
                with_controls("Already tidy", [ask("Ask AI", cx).into_any_element()]),
                self.rail(),
                cx,
            )),
            None => body.child(ui::field_with_rail("Name", "Already tidy", self.rail(), cx)),
        };

        body = match &self.asking {
            Asking::Idle => body,
            Asking::Setup => body.child(
                div()
                    .pt(size::GAP_SMALL)
                    .child(self.render_setup(window, cx)),
            ),
            Asking::Waiting => body.child(ui::field_with_rail(
                service,
                note("Asking…", cx),
                self.rail(),
                cx,
            )),
            Asking::Answered(name) if *name == entry.name => body.child(ui::field_with_rail(
                service,
                with_controls(
                    note("Would keep the current name", cx),
                    [settings(cx).into_any_element()],
                ),
                self.rail(),
                cx,
            )),
            Asking::Answered(name) if offline.as_ref() == Some(name) => {
                body.child(ui::field_with_rail(
                    service,
                    with_controls(
                        note("Suggests the same name", cx),
                        [settings(cx).into_any_element()],
                    ),
                    self.rail(),
                    cx,
                ))
            }
            Asking::Answered(name) => body.child(
                ui::field_with_rail(
                    service,
                    proposal(
                        name.clone(),
                        div()
                            .child(rename("rename-ai", name.clone(), cx))
                            .child(settings(cx)),
                    ),
                    self.rail(),
                    cx,
                )
                .id("ai-answer")
                .tip(
                    format!("Suggested by {}", self.ai.model(self.ai.provider)),
                    None,
                ),
            ),
            Asking::Failed(message) => body.child(ui::field_with_rail(
                service,
                div()
                    .flex()
                    .flex_col()
                    .gap(size::GAP_SMALL)
                    .child(note(message.clone(), cx).text_color(t.red))
                    .child(
                        div()
                            .flex()
                            .gap(size::GAP)
                            .child(ask("Try Again", cx))
                            .child(settings(cx)),
                    ),
                self.rail(),
                cx,
            )),
        };
        Some(body)
    }

    pub fn show(&mut self, items: Vec<Entry>, cx: &mut Context<Self>) {
        if let [only] = items.as_slice()
            && only.kind.is_probeable()
            && !seer_core::mtp::is_device_path(&only.path)
        {
            let path = only.path.clone();
            self.library
                .update(cx, |library, cx| library.restore_bitrate(&path, cx));
        }
        let same_file =
            matches!((self.items.as_slice(), items.as_slice()), ([a], [b]) if a.path == b.path);
        if !same_file {
            self.unwanted.clear();
            self.asking = Asking::Idle;
            self.all_tags = false;
            self.request = None;
        }
        self.items = items;
        self.readout = None;
        cx.notify();
    }

    /// The folder the list shows and its items as listed (`None` while it is
    /// still being read). Summarised whenever nothing is selected; probes
    /// fill in as they finish.
    ///
    /// `None` for the folder already summarised keeps its last listing until
    /// the next one arrives: only another folder starts at "Reading…", so a
    /// reset that reads nothing (clearing a search) never leaves the summary
    /// waiting for a listing that is not coming.
    pub fn show_folder(
        &mut self,
        dir: PathBuf,
        entries: Option<Vec<Entry>>,
        cx: &mut Context<Self>,
    ) {
        let same = self.folder.as_ref().filter(|folder| folder.dir == dir);
        let filtered = same.is_some_and(|folder| folder.filtered);
        let entries = match (entries, same) {
            (None, Some(folder)) => folder.entries.clone(),
            (entries, _) => entries,
        };
        self.folder = Some(Folder {
            dir,
            entries,
            filtered,
        });
        if self.items.is_empty() {
            cx.notify();
        }
    }

    /// Whether a search is narrowing the list. The summary then says that
    /// its numbers are the whole folder's ("Whole folder · 11 items").
    /// Another folder starts unfiltered.
    pub fn set_filtered(&mut self, filtered: bool, cx: &mut Context<Self>) {
        let Some(folder) = self.folder.as_mut() else {
            return;
        };
        if folder.filtered != filtered {
            folder.filtered = filtered;
            if self.items.is_empty() {
                cx.notify();
            }
        }
    }

    /// No folder to summarise (it could not be read; the list says why):
    /// with nothing selected the pane shows its neutral prompt.
    pub fn forget_folder(&mut self, cx: &mut Context<Self>) {
        self.folder = None;
        if self.items.is_empty() {
            cx.notify();
        }
    }

    /// The pane's width: measured at the last layout, or its default before
    /// the first one.
    fn width(&self) -> Pixels {
        match self.width.get() {
            width if width > px(0.) => width,
            _ => size::INSPECTOR.1,
        }
    }

    /// The label column of every field at the pane's width: it narrows to
    /// `size::FIELD_LABEL_NARROW` below `size::INSPECTOR_NARROW`.
    fn rail(&self) -> Pixels {
        ui::field_rail(self.width())
    }

    /// Room for a field's value: the pane less its padding, the label
    /// column and the gap after it.
    fn value_width(&self) -> Pixels {
        self.width() - size::PANE_PADDING * 2. - self.rail() - size::GAP
    }

    /// Starts analysing the selected file and shows the result.
    pub fn analyze(&mut self, cx: &mut Context<Self>) {
        let [only] = self.items.as_slice() else {
            return;
        };
        if !only.kind.is_probeable() || seer_core::mtp::is_device_path(&only.path) {
            return;
        }
        let path = only.path.clone();
        self.tab = Tab::Bitrate;
        self.library
            .update(cx, |library, cx| library.analyze(&path, cx));
        cx.notify();
    }

    /// The file's icon, its name as a title (middle-truncated, the full name
    /// on hover) and one line of kind and size. The kind is said only here;
    /// the duration leads the Info tab right below.
    fn render_header(&self, entry: &Entry, window: &Window, cx: &App) -> Div {
        let mut subtitle = vec![kind_name(entry)];
        if !entry.is_dir() {
            subtitle.push(format::size(entry.size));
        }
        header(
            header_icon(&entry.path, entry.is_dir(), cx),
            entry.name.clone(),
            subtitle,
            self.width(),
            window,
            cx,
        )
    }

    fn render_info(
        &self,
        entry: &Entry,
        probe: Option<Probe>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let on_device = seer_core::mtp::is_device_path(&entry.path);
        let info = match &probe {
            Some(Probe::Ready(info)) => Some(info.clone()),
            _ => None,
        };

        let mut groups: Vec<Div> = Vec::new();
        let media = match (&probe, on_device && entry.kind.is_probeable()) {
            (_, true) => Some(note(
                "Copy this file to the computer to read its media.",
                cx,
            )),
            (Some(Probe::Ready(info)), _) => Some(media_fields(
                info,
                self.rail(),
                self.value_width(),
                window,
                cx,
            )),
            (Some(Probe::Pending), _) => Some(note("Reading media information…", cx)),
            (Some(Probe::Unreadable), _) => Some(note(
                "This file doesn’t contain media that Seer can read.",
                cx,
            )),
            (None, _) => None,
        };
        if let Some(media) = media {
            groups.push(section("Media", true, cx).child(media));
        }

        // Naming moves up when it has something to act on.
        let naming_early = self.naming_is_active(entry);
        if naming_early {
            let first = groups.is_empty();
            groups.extend(self.render_naming(entry, first, window, cx));
        }

        groups.push(self.render_file(entry, info.as_deref(), groups.is_empty(), window, cx));

        if !naming_early {
            groups.extend(self.render_naming(entry, false, window, cx));
        }

        if let Some(info) = info.as_ref().filter(|i| !i.tags.is_empty()) {
            groups.push(self.render_tags(info, cx));
        }

        div()
            .flex()
            .flex_col()
            .gap(size::PANE_PADDING)
            .children(groups)
    }

    /// Container, exact size, modified, and the folder on one line with
    /// ways to reveal it and copy the path.
    fn render_file(
        &self,
        entry: &Entry,
        info: Option<&MediaInfo>,
        first: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let on_device = seer_core::mtp::is_device_path(&entry.path);
        let where_row = entry.path.parent().map(|parent| {
            let full = display_path(parent);
            let reveal = {
                let path = entry.path.clone();
                ui::IconButton::new(
                    "reveal",
                    IconName::FolderSearch,
                    platform::label(Command::Reveal),
                )
                .command(Command::Reveal)
                .small()
                .disabled(on_device)
                .on_click(move |_, _, cx| cx.reveal_path(&path))
            };
            let copy = {
                let path = entry.path.display().to_string();
                ui::IconButton::new("copy-path", IconName::Copy, "Copy Path")
                    .small()
                    .on_click(move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(path.clone()))
                    })
            };
            // The two small buttons and the gaps around them.
            let controls = size::CONTROL_SMALL * 2. + size::GAP_TINY + size::GAP;
            let room = self.value_width() - controls;
            let fits = |text: &str| {
                ui::text_width(
                    &text.to_owned().into(),
                    text::SMALL,
                    FontWeight::NORMAL,
                    window,
                ) <= room
            };
            ui::field_with_rail(
                "Where",
                with_controls(
                    div()
                        .id("where")
                        .truncate()
                        .child(fit_location(&full, fits))
                        .tip(full, None),
                    [reveal.into_any_element(), copy.into_any_element()],
                ),
                self.rail(),
                cx,
            )
        });
        section("File", first, cx)
            .when_some(info, |this, info| {
                this.child(ui::field_with_rail(
                    "Container",
                    container_name(&info.container_description),
                    self.rail(),
                    cx,
                ))
            })
            .when(!entry.is_dir(), |this| {
                this.child(ui::field_with_rail(
                    "Size",
                    exact_size(entry.size),
                    self.rail(),
                    cx,
                ))
            })
            .when_some(entry.modified, |this, time| {
                this.child(ui::field_with_rail(
                    "Modified",
                    format::date(time),
                    self.rail(),
                    cx,
                ))
            })
            .children(where_row)
    }

    /// Tags with readable names. Muxing details wait behind a disclosure.
    fn render_tags(&self, info: &MediaInfo, cx: &mut Context<Self>) -> Div {
        let (technical, shown): (Vec<_>, Vec<_>) =
            info.tags.iter().partition(|(key, _)| is_technical(key));
        let tag = |(key, value): &(String, String), cx: &App| {
            ui::field_with_rail(
                tag_label(key),
                div().line_clamp(4).child(value.clone()),
                self.rail(),
                cx,
            )
        };
        let disclosure = (!technical.is_empty()).then(|| {
            let label = match (self.all_tags, technical.len()) {
                (true, _) => "Hide technical tags".to_owned(),
                (false, 1) => "Show 1 technical tag".to_owned(),
                (false, n) => format!("Show {n} technical tags"),
            };
            // The chevron sits on the pane's edge, like a disclosure triangle.
            div().flex().ml(-size::BUTTON_PADDING_SMALL).child(
                ui::Button::new("all-tags", label)
                    .small()
                    .ghost()
                    .icon(match self.all_tags {
                        true => IconName::ChevronDown,
                        false => IconName::ChevronRight,
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.all_tags = !this.all_tags;
                        cx.notify();
                    })),
            )
        });
        section("Tags", false, cx)
            .children(shown.into_iter().map(|t| tag(t, cx)))
            .when(self.all_tags, |this| {
                this.children(technical.into_iter().map(|t| tag(t, cx)))
            })
            .children(disclosure)
    }

    fn render_streams(&self, entry: &Entry, window: &Window, cx: &mut Context<Self>) -> Div {
        let t = Theme::of(cx).clone();
        let library = self.library.read(cx);
        let body = div().flex().flex_col().gap(size::PANE_PADDING);
        let Some(info) = library.info(&entry.path).cloned() else {
            return body.child(note("Reading media information…", cx));
        };
        let prune = library.prune(&entry.path);
        let running = matches!(prune, Some(Prune::Running(_)));
        // Indexes can outlive a rewrite of the file; ignore any that no longer exist.
        let unwanted: Vec<usize> = info
            .streams
            .iter()
            .map(|s| s.index)
            .filter(|i| self.unwanted.contains(i))
            .collect();
        let keeps_something = unwanted.len() < info.streams.len();

        let suggestions = (!running)
            .then(|| self.render_suggestions(&info, cx))
            .flatten();
        let has_suggestions = suggestions.is_some();

        // Details sit under the label, past the checkbox.
        let room = self.width() - size::PANE_PADDING * 2. - size::CHECKBOX - size::GAP;
        // One independent checkbox per stream. A marked stream is struck
        // out and says what will happen to it.
        let rows = info
            .streams
            .iter()
            .map(|stream| {
                let index = stream.index;
                let removing = self.unwanted.contains(&index);
                let toggle = cx.listener(move |this, remove: &bool, _, cx| {
                    match *remove {
                        true => this.unwanted.insert(index),
                        false => this.unwanted.remove(&index),
                    };
                    cx.notify();
                });
                let line = div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap(size::GAP)
                    .child(kind_mark(stream.kind, &t))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(if removing { t.secondary_label } else { t.label })
                            .when(removing, |this| this.line_through())
                            .child(format!(
                                "{} · {}",
                                stream.kind.label(),
                                format::codec(&stream.codec)
                            )),
                    )
                    .child(match removing {
                        true => div().flex_none().text_color(t.red).child("Will be removed"),
                        false => div()
                            .flex_none()
                            .text_color(t.secondary_label)
                            .child(stream.bit_rate.map(format::bitrate).unwrap_or_default()),
                    });
                let alone = info.streams_of(stream.kind).count() == 1;
                let details = stream_details(stream, alone);
                // Which track this is, among those of its kind, when there
                // is more than one: "Audio 2".
                let nth = (!alone)
                    .then(|| {
                        info.streams_of(stream.kind)
                            .position(|s| s.index == index)
                            .map(|at| at + 1)
                    })
                    .flatten();
                div()
                    .flex()
                    .flex_col()
                    .gap(size::GAP_TINY)
                    .py(size::GAP_SMALL)
                    .child(
                        ui::Checkbox::new(("stream", index), removing)
                            .disabled(running)
                            .label(line)
                            // The track it removes, by itself: two AAC
                            // tracks differ by number and language (A4-1).
                            .accessible_label(removal_label(stream, nth))
                            .when(removing, |this| {
                                this.accessible_description("Will be removed")
                            })
                            .on_toggle(move |remove, window, cx| toggle(&remove, window, cx)),
                    )
                    .when(!details.is_empty(), |this| {
                        this.child(div().pl(size::CHECKBOX + size::GAP).child(dot_list(
                            plain(details, t.secondary_label),
                            room,
                            window,
                            &t,
                        )))
                    })
            })
            .collect::<Vec<_>>();

        let path = entry.path.clone();
        let action = match prune {
            Some(Prune::Running(progress)) => div()
                .flex()
                .flex_col()
                .gap(size::GAP)
                .child(ui::progress_bar(progress.fraction(), cx))
                .child(note(
                    format!("Rewriting the file… {:.0}%", progress.fraction() * 100.),
                    cx,
                ))
                .child(
                    div()
                        .flex()
                        .child(ui::Button::new("cancel-prune", "Cancel").small().on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.library
                                    .update(cx, |library, cx| library.cancel_prune(&path, cx));
                            }),
                        )),
                ),
            _ => {
                let label = match unwanted.len() {
                    0 => "Remove Streams".to_owned(),
                    1 => "Remove 1 Stream".to_owned(),
                    n => format!("Remove {n} Streams"),
                };
                let enabled = !unwanted.is_empty() && keeps_something;
                let failed = match prune {
                    Some(Prune::Failed(message)) => Some(message),
                    _ => None,
                };
                // The consequence sits directly above the button it belongs to.
                let consequence = match keeps_something {
                    true => note(
                        "The file is rewritten without the marked streams, with no re-encoding. The original moves to the Trash.",
                        cx,
                    ),
                    false => note("A file must keep at least one stream.", cx).text_color(t.orange),
                };
                div()
                    .flex()
                    .flex_col()
                    .gap(size::GAP)
                    .pt(size::GAP)
                    .border_t(size::HAIRLINE)
                    .border_color(t.separator)
                    .when_some(failed, |this, message| {
                        this.child(
                            note(format!("The file was not changed: {message}"), cx)
                                .text_color(t.red),
                        )
                    })
                    .child(consequence)
                    .child(
                        div().flex().child(
                            ui::Button::new("prune", label)
                                .small()
                                .destructive()
                                .disabled(!enabled)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    let remove: Vec<usize> = this.unwanted.drain().collect();
                                    this.library.update(cx, |library, cx| {
                                        library.remove_streams(&path, remove, cx)
                                    });
                                    cx.notify();
                                })),
                        ),
                    )
            }
        };

        body.children(suggestions)
            .child(
                section("Remove streams", !has_suggestions, cx).child(
                    div()
                        .flex()
                        .flex_col()
                        .text_size(text::SMALL)
                        .line_height(text::SMALL_LINE)
                        .font_features(theme::tabular())
                        .children(rows),
                ),
            )
            .child(action)
    }

    /// Proposals for what to remove. Each one only marks streams; the person
    /// still reviews the marks and presses Remove.
    fn render_suggestions(&self, info: &MediaInfo, cx: &mut Context<Self>) -> Option<Div> {
        let t = Theme::of(cx).clone();
        let suggestion = |title: &'static str, detail: String| {
            div()
                .flex()
                .flex_col()
                .gap(size::GAP_TINY)
                .py(size::GAP_SMALL)
                .text_size(text::SMALL)
                .line_height(text::SMALL_LINE)
                .child(
                    div()
                        .text_color(t.label)
                        .font_weight(FontWeight::MEDIUM)
                        .child(title),
                )
                .child(div().text_color(t.secondary_label).child(detail))
        };
        let buttons = || div().flex().flex_wrap().gap(size::GAP).pt(size::GAP_SMALL);
        let mut rows: Vec<Div> = Vec::new();

        if let Some(original) = recommend::original_language(info) {
            let language = &original.language;
            let detail = match original.evidence {
                recommend::Evidence::MarkedOriginal => {
                    format!(
                        "This file marks {language} as its original language. The other languages are dubs."
                    )
                }
                recommend::Evidence::OthersAreDubs => {
                    format!("This file marks every language except {language} as a dub.")
                }
                recommend::Evidence::DefaultTrack => format!(
                    "The file doesn’t say which language is original. {language} is the default track, so it is the likeliest."
                ),
                recommend::Evidence::FirstTrack => format!(
                    "The file doesn’t say which language is original. {language} comes first, which is only a hint."
                ),
            };
            // The guess leads; the rest follow in stream order.
            let mut languages = recommend::audio_languages(info);
            languages.sort_by_key(|name| name != language);
            let choices = languages.into_iter().enumerate().map(|(i, name)| {
                let others = recommend::other_languages(info, &name);
                let tagged: Vec<usize> = info
                    .streams_of(StreamKind::Audio)
                    .filter(|s| s.language.is_some())
                    .map(|s| s.index)
                    .collect();
                // In effect when every other language is marked and this one survives.
                let in_effect = others.iter().all(|i| self.unwanted.contains(i))
                    && tagged
                        .iter()
                        .any(|i| !others.contains(i) && !self.unwanted.contains(i));
                ui::Button::new(("keep-language", i), format!("Keep {name}"))
                    .small()
                    .on(in_effect)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        // Choosing a language replaces any earlier language choice.
                        this.unwanted.retain(|i| !tagged.contains(i));
                        this.unwanted.extend(others.iter().copied());
                        cx.notify();
                    }))
            });
            rows.push(suggestion("Original audio only", detail).child(buttons().children(choices)));
        }

        let names = titles::review(info);
        if !names.is_empty() {
            let detail = match names.len() {
                1 => "1 name contains a web address or doesn’t describe its stream.".to_owned(),
                n => format!("{n} names contain a web address or don’t describe their stream."),
            };
            let [entry] = self.items.as_slice() else {
                return None;
            };
            let fix_button = |id: &'static str,
                              label: &'static str,
                              fix: titles::Fix,
                              cx: &mut Context<Self>| {
                let (path, edits) = (entry.path.clone(), titles::edits(&names, fix));
                ui::Button::new(id, label)
                    .small()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.library
                            .update(cx, |library, cx| library.rewrite(&path, edits.clone(), cx));
                        cx.notify();
                    }))
            };
            let changes = names.iter().map(|issue| {
                div()
                    .flex()
                    .gap(size::GAP_SMALL)
                    .text_color(t.secondary_label)
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .line_through()
                            .child(issue.current.clone()),
                    )
                    .child(div().flex_none().child("→"))
                    .child(
                        div().flex_none().text_color(t.label).child(
                            issue
                                .standard
                                .clone()
                                .unwrap_or_else(|| "No name".to_owned()),
                        ),
                    )
            });
            rows.push(
                suggestion("Tidy stream names", detail)
                    .children(changes)
                    .child(
                        buttons()
                            .child(fix_button(
                                "names-standard",
                                "Use Standard Names",
                                titles::Fix::Standard,
                                cx,
                            ))
                            .child(fix_button(
                                "names-remove",
                                "Remove Names",
                                titles::Fix::Remove,
                                cx,
                            )),
                    ),
            );
        }

        let duplicates = recommend::lower_quality(info);
        if !duplicates.is_empty() {
            let detail = format!(
                "Keeps the best video, and the best audio track in each language. Marks {} for removal.",
                match duplicates.len() {
                    1 => "1 stream".to_owned(),
                    n => format!("{n} streams"),
                }
            );
            let in_effect = duplicates.iter().all(|i| self.unwanted.contains(i));
            rows.push(
                suggestion("Best quality only", detail).child(
                    buttons().child(
                        ui::Button::new("keep-best", "Mark Lower Quality")
                            .small()
                            .on(in_effect)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.unwanted.extend(duplicates.iter().copied());
                                cx.notify();
                            })),
                    ),
                ),
            );
        }

        (!rows.is_empty()).then(|| section("Suggestions", true, cx).children(rows))
    }

    /// Moves the chart's readout from the keyboard. Unhandled keys pass on.
    fn step_readout(&mut self, analysis: &Analysis, key: &Keystroke, cx: &mut Context<Self>) {
        // Command keys (Alt-Left goes back on Windows) are not the chart's.
        let held = &key.modifiers;
        if held.control || held.alt || held.platform {
            return;
        }
        if self.move_readout(analysis, &key.key, held.shift, cx) {
            cx.stop_propagation();
        }
    }

    /// Moves the chart's readout as `key` says (see [`readout_step`]), for
    /// the keyboard and for assistive technology's increment and decrement.
    /// Whether the key was the chart's.
    fn move_readout(
        &mut self,
        analysis: &Analysis,
        key: &str,
        shift: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        let count = chart::samples(analysis, &self.plot);
        // Where the readout is now: the keyboard's sample, or the one under
        // the pointer.
        let current = match self.readout {
            Some(Readout::Sample(index)) => Some(index),
            Some(Readout::Pointer(x)) => chart::sample_at(analysis, &self.plot, x),
            None => None,
        };
        let Some(next) = readout_step(current, count, key, shift) else {
            return false;
        };
        self.readout = next.map(Readout::Sample);
        cx.notify();
        true
    }

    fn render_bitrate(&self, entry: &Entry, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let t = Theme::of(cx).clone();
        let path = entry.path.clone();
        let body = div().flex().flex_col().gap(size::PANE_PADDING);
        let analyse = platform::label(Command::AnalyzeBitrate);
        match self.library.read(cx).bitrate(&entry.path) {
            // Seer's trace, top-aligned over the prompt: what the analysis
            // is about to draw.
            Bitrate::Idle => body
                .child(div().flex().child(ui::trace_glyph(cx)))
                .child(note(
                    "See how the bit rate changes over the length of the file, and how much each stream takes up.",
                    cx,
                ))
                .child(
                    div().flex().child(
                        ui::Button::new("analyze", analyse)
                            .small()
                            .primary()
                            .tip(analyse, Some(Command::AnalyzeBitrate))
                            .on_click(cx.listener(|this, _, _, cx| this.analyze(cx))),
                    ),
                ),
            Bitrate::Running(progress) => {
                // Keep repainting while the worker advances.
                window.request_animation_frame();
                let fraction = progress.fraction();
                body.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(size::GAP)
                        .child(ui::progress_bar(fraction, cx))
                        .child(note(
                            format!("Reading packets… {:.0}%", fraction * 100.),
                            cx,
                        )),
                )
                .child(
                    div().flex().child(
                        ui::Button::new("cancel", "Cancel")
                            .small()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.library
                                    .update(cx, |library, cx| library.cancel_analysis(&path, cx));
                            })),
                    ),
                )
            }
            Bitrate::Failed(message) => body
                .child(note(format!("The analysis failed: {message}"), cx).text_color(t.red))
                .child(
                    div().flex().child(
                        ui::Button::new("retry", "Try Again")
                            .small()
                            .on_click(cx.listener(|this, _, _, cx| this.analyze(cx))),
                    ),
                ),
            Bitrate::Ready(analysis) => body
                .child({
                    let focus = t.focus_shadow();
                    let keys = analysis.clone();
                    // Assistive technology's increment and decrement step
                    // the readout as Right and Left do.
                    let step = |key: &'static str| {
                        let (this, analysis) = (cx.entity().downgrade(), analysis.clone());
                        move |_: Option<&accesskit::ActionData>, _: &mut Window, cx: &mut App| {
                            this.update(cx, |this, cx| {
                                this.move_readout(&analysis, key, false, cx);
                            })
                            .ok();
                        }
                    };
                    div()
                        .id("chart")
                        // One tab stop: Left and Right step the readout
                        // through the plotted samples, Shift for a tenth of
                        // the plot, Home and End to either end, Escape to
                        // clear it. The ring says the keys are live.
                        .track_focus(&self.chart_focus)
                        // Announced as a slider over time: its name, the
                        // sample the readout shows (or the headline), and
                        // the keys.
                        .role(Role::Slider)
                        .aria_label("Bit rate over time")
                        .aria_value(chart::spoken_value(&analysis, self.readout, &self.plot))
                        .aria_description(
                            "Left and Right move through the samples, Shift moves further, Home and End go to either end.",
                        )
                        .on_a11y_action(AccessibleAction::Increment, step("right"))
                        .on_a11y_action(AccessibleAction::Decrement, step("left"))
                        .rounded(size::RADIUS_CONTROL)
                        .focus_visible(move |s| s.shadow(focus))
                        .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                            this.step_readout(&keys, &event.keystroke, cx)
                        }))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, window, cx| this.chart_focus.focus(window, cx)),
                        )
                        .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                            this.readout = Some(Readout::Pointer(event.position.x));
                            cx.notify();
                        }))
                        .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                            if !hovered && matches!(this.readout, Some(Readout::Pointer(_))) {
                                this.readout = None;
                                cx.notify();
                            }
                        }))
                        .child(bitrate_chart(
                            analysis.clone(),
                            self.readout,
                            self.plot.clone(),
                            window,
                            cx,
                        ))
                })
                // Average and peak lead the chart; the rest of the spread here.
                .child(
                    section("Overall", false, cx)
                        .child(ui::field_with_rail(
                            "Median",
                            format::bitrate(analysis.stats.median),
                            self.rail(), cx,
                        ))
                        .child(ui::field_with_rail(
                            "Lowest",
                            format::bitrate(analysis.stats.min),
                            self.rail(), cx,
                        )),
                )
                .child(section("Streams", false, cx).children(shares(&analysis, cx))),
        }
    }

    /// Several items: a summary where a single file's header would be.
    fn render_summary(&self, items: &[Entry], window: &Window, cx: &App) -> Stateful<Div> {
        let library = self.library.read(cx);
        let totals = MediaTotals::of(items, library);
        let bytes = file_bytes(items);
        let selection = section("Selection", false, cx)
            .child(ui::field_with_rail(
                "Size",
                format::size(bytes),
                self.rail(),
                cx,
            ))
            .when_some(
                totals.render_duration(self.value_width(), window, cx),
                |this, duration| {
                    this.child(ui::field_with_rail("Duration", duration, self.rail(), cx))
                },
            );
        summary_pane("selection-summary")
            .child(header(
                Some(
                    ui::icon(IconName::Files, Theme::of(cx).secondary_label)
                        .with_size(size::ICON_HEADER)
                        .into_any_element(),
                ),
                format!("{} items", items.len()),
                kind_counts(items),
                self.width(),
                window,
                cx,
            ))
            .child(selection)
            .children(totals.render_codecs(self.rail(), self.value_width(), window, cx))
    }

    /// Nothing selected: the folder itself. Its size and what it holds, how
    /// long its media runs (and how much of it could be read), a bar with
    /// one segment per file in the data hues, and the files by codec. Probes
    /// fill the numbers in as they finish. While a search narrows the list,
    /// the "Media" header says the numbers are the whole folder's. A copy
    /// on its way in is measured in the header ("1.20 GB of 6.38 GB
    /// arrived · 2 of 6 files"), the one place an empty folder's prompt and
    /// a full folder's summary share: the list and status bar already say
    /// that it is copying.
    fn render_folder(&self, folder: &Folder, window: &Window, cx: &App) -> AnyElement {
        let t = Theme::of(cx);
        let name = folder_name(&folder.dir);
        let icon = || header_icon(&folder.dir, true, cx);
        let pane = self.width();
        let incoming = Devices::global(cx)
            .read(cx)
            .transfers()
            .iter()
            .find(|transfer| transfer.dest == folder.dir)
            .map(|transfer| {
                let verb = match transfer.mode {
                    seer_core::transfer::Mode::Copy => "copied",
                    seer_core::transfer::Mode::Move => "moved",
                };
                arrival(&transfer.progress.detail(), &transfer.arriving(), verb)
            });
        let Some(entries) = &folder.entries else {
            return summary_pane("folder-summary")
                .child(header(
                    icon(),
                    name,
                    vec!["Reading…".to_owned()],
                    pane,
                    window,
                    cx,
                ))
                .into_any_element();
        };
        if entries.is_empty() {
            let prompt = incoming.unwrap_or_else(|| vec!["This folder is empty.".to_owned()]);
            return summary_pane("folder-summary")
                .child(header(icon(), name, prompt, pane, window, cx))
                .into_any_element();
        }

        let library = self.library.read(cx);
        let totals = MediaTotals::of(entries, library);
        let bytes = file_bytes(entries);
        // The size first: it is what a person pruning wants, and the counts
        // wrap after it.
        let mut subtitle = Vec::new();
        if bytes > 0 {
            subtitle.push(format::size(bytes));
        }
        subtitle.extend(folder_counts(entries));
        subtitle.extend(incoming.into_iter().flatten());
        // Beside a narrowed list, the numbers below are still the folder's.
        let scope = folder.filtered.then(|| whole_folder(entries.len()));

        let bytes_of = |kinds: &[Kind]| -> u64 {
            entries
                .iter()
                .filter(|e| !e.is_dir() && kinds.contains(&e.kind))
                .map(|e| e.size)
                .sum()
        };
        let shares = [
            ("Video files", bytes_of(&[Kind::Video]), t.data_video),
            ("Audio files", bytes_of(&[Kind::Audio]), t.data_audio),
            (
                "Other files",
                bytes_of(&[Kind::Image, Kind::Subtitle, Kind::Other]),
                t.secondary_label,
            ),
        ];
        let has_media = entries
            .iter()
            .any(|e| matches!(e.kind, Kind::Video | Kind::Audio));

        let media = has_media.then(|| {
            let legend =
                div()
                    .flex()
                    .flex_wrap()
                    .gap_x(size::PANE_PADDING)
                    .gap_y(size::GAP_TINY)
                    .font_features(theme::tabular())
                    .children(shares.iter().filter(|(_, b, _)| *b > 0).map(
                        |(label, bytes, color)| {
                            div()
                                .flex()
                                .items_center()
                                .gap(size::GAP_SMALL)
                                .whitespace_nowrap()
                                .child(
                                    div()
                                        .flex_none()
                                        .size(size::SWATCH)
                                        .rounded_full()
                                        .bg(*color),
                                )
                                .child(div().text_color(t.secondary_label).child(*label))
                                .child(div().text_color(t.label).child(format::size(*bytes)))
                        },
                    ));
            scoped_section("Media", scope.clone(), cx)
                .when_some(
                    totals.render_duration(self.value_width(), window, cx),
                    |this, duration| {
                        this.child(ui::field_with_rail("Duration", duration, self.rail(), cx))
                    },
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(size::GAP)
                        .pt(size::GAP_SMALL)
                        .child(file_bar(entries, pane - size::PANE_PADDING * 2., t))
                        .child(legend),
                )
        });

        summary_pane("folder-summary")
            .child(header(icon(), name, subtitle, pane, window, cx))
            .children(media)
            .children(totals.render_codecs(self.rail(), self.value_width(), window, cx))
            .when(!has_media, |this| {
                let prompt = note("No media here. Select a file to see its details.", cx);
                match scope {
                    Some(scope) => {
                        this.child(scoped_section("Media", Some(scope), cx).child(prompt))
                    }
                    None => this.child(prompt),
                }
            })
            .into_any_element()
    }
}

/// The folder's files as one 6pt bar, a segment per file sized by its
/// bytes, largest first, in its kind's hue (`data_video`, `data_audio`,
/// neutral for the rest), a hairline apart. Files too small for a 2pt
/// segment of their own share one neutral segment at the end. Hovering a
/// segment names its file and size.
fn file_bar(entries: &[Entry], width: Pixels, t: &Theme) -> Div {
    let mut files: Vec<&Entry> = entries
        .iter()
        .filter(|e| !e.is_dir() && e.size > 0)
        .collect();
    files.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.name.cmp(&b.name)));
    let sizes: Vec<u64> = files.iter().map(|e| e.size).collect();
    let total = sizes.iter().sum::<u64>().max(1) as f32;
    let own = own_segments(
        &sizes,
        f32::from(width),
        f32::from(size::BAR_MIN),
        f32::from(size::HAIRLINE),
    );
    let (shown, rest) = files.split_at(own);
    let segment = |id: usize, bytes: u64, color: Hsla, tip: String| {
        div()
            .id(("file-share", id))
            .h_full()
            .rounded_full()
            .bg(color)
            .w(relative(bytes as f32 / total))
            .min_w(size::BAR_MIN)
            .tip(tip, None)
    };
    let others = (!rest.is_empty()).then(|| {
        let bytes: u64 = rest.iter().map(|e| e.size).sum();
        let tip = match rest {
            [only] => format!("{} · {}", only.name, format::size(bytes)),
            _ => format!("{} smaller files · {}", rest.len(), format::size(bytes)),
        };
        segment(own, bytes, t.secondary_label, tip)
    });
    div()
        .flex()
        .w_full()
        .h(size::BAR_THICK)
        .gap(size::HAIRLINE)
        .children(shown.iter().enumerate().map(|(i, entry)| {
            segment(
                i,
                entry.size,
                file_hue(entry.kind, t),
                format!("{} · {}", entry.name, format::size(entry.size)),
            )
        }))
        .children(others)
}

/// How many files, largest first (`sizes` descending), get a segment of
/// their own in a bar `width` wide with `gap` between segments: as many as
/// keep the smallest of them at least `min` wide. Any left over share one
/// more segment, which is counted among the gaps.
fn own_segments(sizes: &[u64], width: f32, min: f32, gap: f32) -> usize {
    let total: u64 = sizes.iter().sum();
    if total == 0 {
        return 0;
    }
    (1..=sizes.len())
        .rev()
        .find(|&own| {
            let segments = own + usize::from(own < sizes.len());
            let usable = width - gap * segments.saturating_sub(1) as f32;
            usable * sizes[own - 1] as f32 / total as f32 >= min
        })
        .unwrap_or(0)
}

/// The data hue for a file kind: video and audio have their own; anything
/// else is neutral.
fn file_hue(kind: Kind, t: &Theme) -> Hsla {
    match kind {
        Kind::Video => t.data_video,
        Kind::Audio => t.data_audio,
        _ => t.secondary_label,
    }
}

/// The scope of a folder summary beside a search: "Whole folder · 11 items".
fn whole_folder(items: usize) -> String {
    match items {
        1 => "Whole folder · 1 item".to_owned(),
        n => format!("Whole folder · {n} items"),
    }
}

/// How far a copy or move into the folder shown has got, for its header:
/// "1.20 GB of 6.38 GB copied" (`verb`; bytes in flight have not arrived
/// yet), then "2 of 6 files" when there is more than one; by files alone
/// when no data is read (a device moving its own files), and only finished
/// files are said to have arrived. Before anything is sized, `pending`
/// ("Copying 4 items here…").
fn arrival(detail: &seer_core::transfer::Detail, pending: &str, verb: &str) -> Vec<String> {
    let files = (detail.files_total > 1)
        .then(|| format!("{} of {} files", detail.files_done, detail.files_total));
    match (detail.bytes_total, detail.files_total) {
        (0, 0) => vec![pending.to_owned()],
        (0, 1) => vec![format!("{} of 1 file arrived", detail.files_done)],
        (0, total) => vec![format!("{} of {total} files arrived", detail.files_done)],
        (bytes, _) => [
            Some(format!(
                "{} of {} {verb}",
                format::size(detail.bytes_done.min(bytes)),
                format::size(bytes)
            )),
            files,
        ]
        .into_iter()
        .flatten()
        .collect(),
    }
}

/// What the probes have found so far among some items: total running time,
/// how many files could not be read, and each codec with how many files
/// carry it.
struct MediaTotals {
    /// Media files that can be read here (not on a device).
    media: usize,
    /// What those files are, for "2 of 3 videos".
    noun: (&'static str, &'static str),
    /// Of those, the ones read.
    read: usize,
    /// Of those, the ones that could not be read.
    unreadable: usize,
    seconds: f64,
    /// Each video codec and how many files carry it (cover art aside).
    video: Vec<(String, usize)>,
    /// Each audio codec and how many files carry it: a file with an AC-3
    /// and an AAC track counts once under each.
    audio: Vec<(String, usize)>,
}

impl MediaTotals {
    fn of(items: &[Entry], library: &Library) -> MediaTotals {
        let (mut media, mut read, mut unreadable, mut seconds) = (0, 0, 0, 0.);
        let (mut video, mut audio) = (Vec::new(), Vec::new());
        let (mut videos, mut audios) = (0, 0);
        let local = items
            .iter()
            .filter(|e| e.kind.is_probeable() && !seer_core::mtp::is_device_path(&e.path));
        for entry in local {
            media += 1;
            match entry.kind {
                Kind::Video => videos += 1,
                Kind::Audio => audios += 1,
                _ => {}
            }
            match library.probe(&entry.path) {
                Some(Probe::Ready(info)) => {
                    read += 1;
                    seconds += info.duration.max(0.);
                    // Every codec a file carries, once per file. Cover art
                    // is a picture, not the file's video.
                    let codecs = |kind: StreamKind, into: &mut Vec<String>| {
                        let mut seen: Vec<String> = Vec::new();
                        for s in info.streams_of(kind).filter(|s| !s.flags.attached_picture) {
                            let codec = format::codec(&s.codec);
                            if !seen.contains(&codec) {
                                seen.push(codec);
                            }
                        }
                        into.extend(seen);
                    };
                    codecs(StreamKind::Video, &mut video);
                    codecs(StreamKind::Audio, &mut audio);
                }
                Some(Probe::Unreadable) => unreadable += 1,
                Some(Probe::Pending) | None => {}
            }
        }
        let noun = match (videos, audios) {
            (_, 0) => ("video", "videos"),
            (0, _) => ("audio file", "audio files"),
            _ => ("media file", "media files"),
        };
        MediaTotals {
            media,
            noun,
            read,
            unreadable,
            seconds,
            video: codec_counts(video),
            audio: codec_counts(audio),
        }
    }

    /// The running time and what it leaves out: "1:23:45"; "12:30 so far",
    /// "3 of 7 read" while probes run; "0:35", "1 unreadable" when a file
    /// could not be read; "Unavailable", "2 unreadable" when none could.
    /// The second part says what the total does not count.
    ///
    /// "Read" means read successfully, everywhere: the count here is the
    /// one [`MediaTotals::coverage`] gives the codecs ("2 of 7 read" beside
    /// "2 of 7 videos"), and files that failed are the separate
    /// "1 unreadable".
    fn duration(&self) -> Option<(String, Vec<String>)> {
        let settled = self.read + self.unreadable;
        let unreadable = (self.unreadable > 0).then(|| format!("{} unreadable", self.unreadable));
        match (self.media, settled, self.read) {
            (0, _, _) => None,
            // Nothing read yet: no time to give, only what failed.
            (all, settled, 0) if settled < all => {
                Some(("Reading…".to_owned(), unreadable.into_iter().collect()))
            }
            (all, settled, read) if settled < all => Some((
                format!("{} so far", format::duration(self.seconds)),
                [Some(format!("{read} of {all} read")), unreadable]
                    .into_iter()
                    .flatten()
                    .collect(),
            )),
            (_, _, 0) => Some(("Unavailable".to_owned(), unreadable.into_iter().collect())),
            _ if self.seconds > 0. => Some((
                format::duration(self.seconds),
                unreadable.into_iter().collect(),
            )),
            _ => None,
        }
    }

    /// [`MediaTotals::duration`] as a field value: the time in `label`, then
    /// what it leaves out; an unreadable count in `orange`, as the list
    /// marks those files.
    /// Its parts pack into `room` like any separated list.
    fn render_duration(&self, room: Pixels, window: &Window, cx: &App) -> Option<Div> {
        let t = Theme::of(cx);
        let (time, qualifiers) = self.duration()?;
        let mut parts: Vec<Part> = vec![vec![(time, t.label)]];
        parts.extend(qualifiers.into_iter().map(|qualifier| {
            let ink = match qualifier.ends_with("unreadable") {
                true => t.orange,
                false => t.secondary_label,
            };
            vec![(qualifier, ink)]
        }));
        Some(dot_list(parts, room, window, t))
    }

    /// "2 of 3 videos" while some files are not (or cannot be) read, so
    /// the codec counts say what they count.
    fn coverage(&self) -> Option<String> {
        (self.read < self.media).then(|| {
            let noun = match self.media {
                1 => self.noun.0,
                _ => self.noun.1,
            };
            format!("{} of {} {noun}", self.read, self.media)
        })
    }

    /// "Video: H.264 3 · HEVC 1", "Audio: AAC 4" under "Files by codec":
    /// codec in `label`, the number of files carrying it in secondary.
    /// Labels take a `rail` wide column; the values pack into `room`.
    fn render_codecs(&self, rail: Pixels, room: Pixels, window: &Window, cx: &App) -> Option<Div> {
        if self.video.is_empty() && self.audio.is_empty() {
            return None;
        }
        let t = Theme::of(cx);
        let row = |label: &'static str, counts: &[(String, usize)]| {
            (!counts.is_empty()).then(|| {
                ui::field_with_rail(
                    label,
                    dot_list(
                        counts
                            .iter()
                            .map(|(codec, n)| {
                                vec![(codec.clone(), t.label), (n.to_string(), t.secondary_label)]
                            })
                            .collect(),
                        room,
                        window,
                        t,
                    ),
                    rail,
                    cx,
                )
            })
        };
        Some(
            scoped_section("Files by codec", self.coverage(), cx)
                .children(row("Video", &self.video))
                .children(row("Audio", &self.audio)),
        )
    }
}

impl Render for Inspector {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::of(cx).clone();
        // Measures the room the pane is given; a change lays it out again.
        let measured = Rc::clone(&self.width);
        let measure = canvas(
            move |bounds, window, _| {
                if measured.get() != bounds.size.width {
                    measured.set(bounds.size.width);
                    window.refresh();
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0();
        let pane = div()
            .relative()
            .size_full()
            .bg(t.surface_raised)
            .text_color(t.label)
            .text_size(text::SMALL)
            .line_height(text::SMALL_LINE)
            .child(measure);

        let entry = match self.items.as_slice() {
            [] => {
                return match &self.folder {
                    Some(folder) => pane.child(self.render_folder(folder, window, cx)),
                    None => pane.child(
                        ui::EmptyState::new("No Selection")
                            .detail("Select a file to see its media, streams and bit rate.")
                            .compact(),
                    ),
                };
            }
            [entry] => entry.clone(),
            many => return pane.child(self.render_summary(many, window, cx)),
        };

        // Streams and bit rate need the file itself, which a device keeps.
        let media = entry.kind.is_probeable() && !seer_core::mtp::is_device_path(&entry.path);
        let tab = if media { self.tab } else { Tab::Info };
        let probe = self.library.read(cx).probe(&entry.path).cloned();
        if matches!(
            self.library.read(cx).prune(&entry.path),
            Some(Prune::Running(_))
        ) {
            // Keep repainting while the worker advances.
            window.request_animation_frame();
        }
        let this = cx.entity().downgrade();
        // One tab stop; Left and Right move between the tabs (`ui::tabs`).
        let tabs = ui::tabs(
            "inspector-tab",
            &[
                (Tab::Info, "Info"),
                (Tab::Streams, "Streams"),
                (Tab::Bitrate, "Bit Rate"),
            ],
            tab,
            move |tab, _, cx| {
                this.update(cx, |this, cx| {
                    this.tab = tab;
                    cx.notify();
                })
                .ok();
            },
            cx,
        );

        let body = match tab {
            Tab::Info => self.render_info(&entry, probe, window, cx),
            Tab::Streams => self.render_streams(&entry, window, cx),
            Tab::Bitrate => self.render_bitrate(&entry, window, cx),
        };

        pane.flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .gap(size::PANE_PADDING)
                    .p(size::PANE_PADDING)
                    .child(self.render_header(&entry, window, cx))
                    .when(media, |this| this.child(tabs)),
            )
            .child(
                div()
                    .id("inspector-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(size::PANE_PADDING)
                    .pb(size::PANE_PADDING)
                    .child(body),
            )
    }
}

/// The top of the pane: an icon, a title and the facts under it, packed
/// into whole lines for a pane `pane` wide ([`dot_list`]). A long title
/// wraps onto a second line before it is cut in the middle, so the part
/// that tells two versions apart ("(2024).mp4") stays readable; the icon
/// stays level with its first line.
fn header(
    icon: Option<AnyElement>,
    title: impl Into<SharedString>,
    subtitle: Vec<String>,
    pane: Pixels,
    window: &Window,
    cx: &App,
) -> Div {
    let t = Theme::of(cx);
    let title = title.into();
    let room = pane
        - size::PANE_PADDING * 2.
        - match icon {
            Some(_) => size::ICON_HEADER + size::GAP,
            None => px(0.),
        };
    div()
        .flex()
        .items_start()
        .gap(size::GAP)
        .children(icon.map(|icon| div().flex_none().child(icon)))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .child(
                    ui::title(title.clone(), cx)
                        .whitespace_normal()
                        .line_clamp(TITLE_LINES)
                        .id("inspector-title")
                        .role(Role::Heading)
                        .aria_label(title.clone())
                        .tip(title, None),
                )
                .child(
                    div()
                        .text_size(text::SMALL)
                        .line_height(text::SMALL_LINE)
                        .text_color(t.secondary_label)
                        .font_features(theme::tabular())
                        .child(dot_list(
                            plain(subtitle, t.secondary_label),
                            room,
                            window,
                            t,
                        )),
                ),
        )
}

/// The header's mark at `size::ICON_HEADER`. A file shows its own system
/// icon (it is that file's real icon, decision 56). A folder shows Seer's
/// folder glyph, and a disk the drive glyph the sidebar gives it, both in
/// `secondary_label`: the system's folder and volume icons are a different
/// image on every OS, and the volume's body is 1.2:1 on the pane in light
/// (F4-1).
fn header_icon(path: &Path, is_dir: bool, cx: &App) -> Option<AnyElement> {
    if !is_dir {
        return native::icon_for(path, is_dir, f32::from(size::ICON_HEADER) as usize)
            .map(|icon| img(icon).size(size::ICON_HEADER).into_any_element());
    }
    let disk = match path.parent() {
        None => Some(IconName::HardDrive),
        Some(parent) if parent == Path::new("/Volumes") => Some(IconName::Disc),
        _ => None,
    };
    Some(match disk {
        Some(glyph) => ui::icon(glyph, Theme::of(cx).secondary_label)
            .with_size(size::ICON_HEADER)
            .into_any_element(),
        None => ui::kind_glyph_large(ui::Glyph::Folder, cx).into_any_element(),
    })
}

/// The scrolling column a summary (folder or selection) sits in.
fn summary_pane(id: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .size_full()
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap(size::PANE_PADDING)
        .p(size::PANE_PADDING)
}

/// One item of a separated list: runs of text in their inks, a small gap
/// apart ("H.264" in `label`, then "3" in secondary).
type Part = Vec<(String, Hsla)>;

/// Items that are each one run of text in one ink.
fn plain(items: Vec<String>, ink: Hsla) -> Vec<Part> {
    items.into_iter().map(|item| vec![(item, ink)]).collect()
}

/// Items separated by " · " ("1920 × 1080 · 24 fps · 8-bit"), packed into
/// lines `room` wide by measuring them ([`pack`]): a line never starts or
/// ends with "·", and the separator only ever sits between two items on one
/// line. An item too wide for a line of its own is cut at its end.
fn dot_list(parts: Vec<Part>, room: Pixels, window: &Window, t: &Theme) -> Div {
    let measure = |text: &str| {
        ui::text_width(
            &text.to_owned().into(),
            text::SMALL,
            FontWeight::NORMAL,
            window,
        )
        .ceil()
    };
    // Each run gets a hairline of slack: the row is set with tabular
    // figures, which the measurement does not use.
    let widths: Vec<Pixels> = parts
        .iter()
        .map(|runs| {
            runs.iter()
                .map(|(text, _)| measure(text) + size::HAIRLINE)
                .fold(px(0.), |sum, width| sum + width)
                + size::GAP_SMALL * runs.len().saturating_sub(1) as f32
        })
        .collect();
    let separator = measure("·") + size::GAP_SMALL * 2.;
    let mut parts: Vec<Option<Part>> = parts.into_iter().map(Some).collect();
    let lines = pack(&widths, separator, room);
    div()
        .flex()
        .flex_col()
        .children(lines.into_iter().map(|line| {
            let last = line.end - 1;
            let mut row: Vec<AnyElement> = Vec::new();
            for i in line {
                let runs = parts[i].take().unwrap_or_default();
                row.push(
                    div()
                        .flex()
                        .min_w_0()
                        .gap(size::GAP_SMALL)
                        .children(runs.into_iter().enumerate().map(|(n, (text, ink))| {
                            div()
                                .when(n == 0, |this| this.min_w_0().truncate())
                                .when(n > 0, |this| this.flex_none())
                                .text_color(ink)
                                .child(text)
                        }))
                        .into_any_element(),
                );
                if i < last {
                    row.push(
                        div()
                            .flex_none()
                            .text_color(t.secondary_label)
                            .child("·")
                            .into_any_element(),
                    );
                }
            }
            div()
                .flex()
                .min_w_0()
                .gap(size::GAP_SMALL)
                .whitespace_nowrap()
                .children(row)
        }))
}

/// Lines for items `widths` wide, joined by a `separator` that only sits
/// inside a line, each line at most `room` wide: as many items as fit,
/// in order. An item wider than `room` has a line of its own. A last line
/// left with one item (a widow) takes the item before it when that fits
/// and the line above keeps two or more.
fn pack(widths: &[Pixels], separator: Pixels, room: Pixels) -> Vec<Range<usize>> {
    let mut lines = Vec::new();
    let (mut start, mut used) = (0, px(0.));
    for (i, &width) in widths.iter().enumerate() {
        if i == start {
            used = width;
        } else if used + separator + width > room {
            lines.push(start..i);
            start = i;
            used = width;
        } else {
            used += separator + width;
        }
    }
    if start < widths.len() {
        lines.push(start..widths.len());
    }
    if let [.., above, last] = lines.as_mut_slice()
        && last.len() == 1
        && above.len() >= 3
        && widths[above.end - 1] + separator + widths[last.start] <= room
    {
        above.end -= 1;
        last.start -= 1;
    }
    lines
}

/// Where the chart's keyboard readout goes on `key`, with `count` samples
/// plotted: `Some(Some(i))` to sample `i`, `Some(None)` to clear it, `None`
/// when the key is not the chart's. Left and Right step one sample (with
/// Shift, a tenth of the plot), starting from the end they point away
/// from; Home and End jump; Escape clears a readout that is showing.
fn readout_step(
    current: Option<usize>,
    count: usize,
    key: &str,
    shift: bool,
) -> Option<Option<usize>> {
    let last = count.checked_sub(1)?;
    let stride = if shift { (count / 10).max(1) } else { 1 };
    let at = current.map(|i| i.min(last));
    Some(match key {
        "left" => Some(at.map_or(last, |i| i.saturating_sub(stride))),
        "right" => Some(at.map_or(0, |i| (i + stride).min(last))),
        "home" => Some(0),
        "end" => Some(last),
        "escape" if current.is_some() => None,
        _ => return None,
    })
}

/// A titled group of rows. Every group after the first opens with a hairline.
fn section(title: impl Into<SharedString>, first: bool, cx: &App) -> Div {
    let t = Theme::of(cx);
    div()
        .flex()
        .flex_col()
        .when(!first, |this| {
            this.pt(size::GAP)
                .border_t(size::HAIRLINE)
                .border_color(t.separator)
        })
        .child(
            div()
                .pb(size::GAP_SMALL)
                .child(ui::section_header(title, cx)),
        )
}

/// [`section`] (never the first) whose header also says what its numbers
/// cover, in secondary after the title: "Files by codec  2 of 3 videos".
fn scoped_section(title: &'static str, scope: Option<String>, cx: &App) -> Div {
    let t = Theme::of(cx);
    div()
        .flex()
        .flex_col()
        .pt(size::GAP)
        .border_t(size::HAIRLINE)
        .border_color(t.separator)
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_x(size::GAP)
                .pb(size::GAP_SMALL)
                .child(ui::section_header(title, cx))
                .when_some(scope, |this, scope| {
                    this.child(
                        div()
                            .text_size(text::SMALL)
                            .line_height(text::SMALL_LINE)
                            .font_features(theme::tabular())
                            .text_color(t.secondary_label)
                            .child(scope),
                    )
                }),
        )
}

fn note(message: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_size(text::SMALL)
        .line_height(text::SMALL_LINE)
        .text_color(Theme::of(cx).secondary_label)
        .child(message.into())
}

/// A field value with small controls at its trailing edge. The controls are
/// taller than the text line and overhang into the row's padding, so the
/// row keeps the 20pt rhythm and its label stays level with its value.
fn with_controls(value: impl IntoElement, controls: impl IntoIterator<Item = AnyElement>) -> Div {
    div()
        .flex()
        .items_center()
        .gap(size::GAP)
        .h(text::SMALL_LINE)
        .child(div().flex_1().min_w_0().child(value))
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(size::GAP_TINY)
                .children(controls),
        )
}

/// Duration, bit rate, then each video and audio stream as a field: the
/// codec on the first line with its rate at the end, then the profile and
/// the rest as a separated list packed to `room`.
fn media_fields(info: &MediaInfo, rail: Pixels, room: Pixels, window: &Window, cx: &App) -> Div {
    let t = Theme::of(cx);
    let mut rows = div().flex().flex_col();
    if info.duration > 0. {
        rows = rows.child(ui::field_with_rail(
            "Duration",
            format::duration(info.duration),
            rail,
            cx,
        ));
    }
    if info.bit_rate > 0 {
        rows = rows.child(ui::field_with_rail(
            "Bit rate",
            format::bitrate(info.bit_rate),
            rail,
            cx,
        ));
    }
    for (kind, title) in [(StreamKind::Video, "Video"), (StreamKind::Audio, "Audio")] {
        let streams: Vec<_> = info.streams_of(kind).collect();
        let numbered = streams.len() > 1;
        for (i, stream) in streams.into_iter().enumerate() {
            let label = match numbered {
                true => format!("{title} {}", i + 1),
                false => title.to_owned(),
            };
            let (name, details) = named_stream(stream, !numbered);
            rows =
                rows.child(ui::field_with_rail(
                    label,
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .flex()
                                .gap(size::GAP)
                                .child(div().flex_1().min_w_0().truncate().child(name))
                                .child(div().flex_none().text_color(t.secondary_label).child(
                                    stream.bit_rate.map(format::bitrate).unwrap_or_default(),
                                )),
                        )
                        .when(!details.is_empty(), |this| {
                            this.child(div().child(dot_list(
                                plain(details, t.secondary_label),
                                room,
                                window,
                                t,
                            )))
                        }),
                    rail,
                    cx,
                ));
        }
    }
    // Subtitles in one line: there can be dozens.
    let subtitles: Vec<String> = info
        .streams_of(StreamKind::Subtitle)
        .map(|s| {
            let name = s
                .language
                .as_deref()
                .map(format::language)
                .unwrap_or_else(|| format::codec(&s.codec));
            match s.flags.forced {
                true => format!("{name} (forced)"),
                false => name,
            }
        })
        .collect();
    if !subtitles.is_empty() {
        rows = rows.child(ui::field_with_rail(
            "Subtitles",
            subtitles.join(", "),
            rail,
            cx,
        ));
    }
    if !info.chapters.is_empty() {
        rows = rows.child(ui::field_with_rail(
            "Chapters",
            info.chapters.len().to_string(),
            rail,
            cx,
        ));
    }
    for (kind, label) in [
        (StreamKind::Attachment, "Attached"),
        (StreamKind::Data, "Data"),
    ] {
        let n = info.streams_of(kind).count();
        if n > 0 {
            let noun = match (kind, n) {
                (StreamKind::Attachment, 1) => "file",
                (StreamKind::Attachment, _) => "files",
                (_, 1) => "stream",
                _ => "streams",
            };
            rows = rows.child(ui::field_with_rail(label, format!("{n} {noun}"), rail, cx));
        }
    }
    rows
}

/// A video or audio stream as the Info tab shows it: its name as the list
/// and Clean Up give it ("1080p H.264", "Stereo AAC"), then the details
/// the name does not already say (the channel layout; the pixel size when
/// it is the name).
fn named_stream(stream: &Stream, alone: bool) -> (String, Vec<String>) {
    let mut details = stream_details(stream, alone);
    let said = match stream.kind {
        StreamKind::Audio => stream.channels.map(format::channels),
        StreamKind::Video => match (stream.width, stream.height) {
            (Some(w), Some(h)) if format::resolution_class(w, h).is_none() => {
                Some(format::resolution(w, h))
            }
            _ => None,
        },
        _ => None,
    };
    if let Some(said) = said
        && let Some(at) = details.iter().position(|part| *part == said)
    {
        details.remove(at);
    }
    let name = match stream.kind {
        StreamKind::Video => format::video_name(&stream.codec, stream.width, stream.height),
        StreamKind::Audio => format::audio_name(&stream.codec, stream.channels),
        _ => format::codec(&stream.codec),
    };
    (name, details)
}

/// Everything about a stream after its codec, profile first, flags last:
/// "Constrained Baseline", "1920 × 1080", "24 fps", "8-bit", "Default".
/// "Default" says something only beside another stream of its kind, so it
/// is left out when the stream is `alone`.
/// What a screen reader says for a stream's removal checkbox: the track
/// ("Audio 2" when the file has more than one of its kind), its codec, its
/// language and its title. "Remove Audio 2, AAC, Tamil".
fn removal_label(stream: &Stream, nth: Option<usize>) -> String {
    let track = match nth {
        Some(n) => format!("Remove {} {n}", stream.kind.label()),
        None => format!("Remove {}", stream.kind.label()),
    };
    let mut parts = vec![track, format::codec(&stream.codec)];
    parts.extend(stream.language.as_deref().map(format::language));
    parts.extend(stream.title.as_ref().map(|title| format!("“{title}”")));
    parts.join(", ")
}

fn stream_details(stream: &Stream, alone: bool) -> Vec<String> {
    let mut parts: Vec<String> = stream.profile.iter().cloned().collect();
    match stream.kind {
        StreamKind::Video => {
            if let (Some(w), Some(h)) = (stream.width, stream.height) {
                parts.push(format::resolution(w, h));
            }
            if let Some(fps) = stream.frame_rate {
                parts.push(format::frame_rate(fps));
            }
            if let Some(depth) = stream.bit_depth {
                parts.push(format!("{depth}-bit"));
            }
            if let Some(range) = stream.dynamic_range() {
                parts.push(range.to_owned());
            }
        }
        StreamKind::Audio => {
            if let Some(channels) = stream.channels {
                parts.push(format::channels(channels));
            }
            if let Some(rate) = stream.sample_rate {
                parts.push(format::sample_rate(rate));
            }
        }
        _ => {}
    }
    if let Some(language) = &stream.language {
        parts.push(format::language(language));
    }
    if let Some(title) = &stream.title {
        parts.push(format!("“{title}”"));
    }
    let flags = &stream.flags;
    for (set, label) in [
        (flags.default && !alone, "Default"),
        (flags.forced, "Forced"),
        (flags.commentary, "Commentary"),
        (flags.hearing_impaired, "SDH"),
        (flags.visual_impaired, "Audio description"),
        (flags.attached_picture, "Cover art"),
    ] {
        if set {
            parts.push(label.to_owned());
        }
    }
    parts
}

/// The data hue for a stream kind: video and audio have their own; anything
/// else is neutral.
fn kind_hue(kind: StreamKind, t: &Theme) -> Hsla {
    match kind {
        StreamKind::Video => t.data_video,
        StreamKind::Audio => t.data_audio,
        _ => t.secondary_label,
    }
}

/// A small dot in the stream kind's hue, as in the chart and share bars.
fn kind_mark(kind: StreamKind, t: &Theme) -> Div {
    div()
        .flex_none()
        .size(size::BAR_THICK)
        .rounded_full()
        .bg(kind_hue(kind, t))
}

/// "MP4 video", "SRT subtitles", "Folder".
fn kind_name(entry: &Entry) -> String {
    if entry.is_dir() {
        return Kind::Folder.label().to_owned();
    }
    match entry.extension() {
        Some(ext) => format!(
            "{} {}",
            ext.to_uppercase(),
            entry.kind.label().to_lowercase()
        ),
        None => entry.kind.label().to_owned(),
    }
}

/// "3 videos", "1 audio file".
fn count_of(kind: Kind, n: usize) -> String {
    let (one, many) = match kind {
        Kind::Folder => ("folder", "folders"),
        Kind::Video => ("video", "videos"),
        Kind::Audio => ("audio file", "audio files"),
        Kind::Image => ("image", "images"),
        Kind::Subtitle => ("subtitle file", "subtitle files"),
        Kind::Other => ("document", "documents"),
    };
    format!("{n} {}", if n == 1 { one } else { many })
}

/// Every kind in the items with its count: "3 videos", "2 folders".
fn kind_counts(items: &[Entry]) -> Vec<String> {
    [
        Kind::Video,
        Kind::Audio,
        Kind::Folder,
        Kind::Image,
        Kind::Subtitle,
        Kind::Other,
    ]
    .into_iter()
    .filter_map(|kind| {
        let n = items.iter().filter(|e| e.kind == kind).count();
        (n > 0).then(|| count_of(kind, n))
    })
    .collect()
}

/// A folder's contents in a few words: media and folders by name, the rest
/// together. "7 videos", "3 folders", "2 other files".
fn folder_counts(items: &[Entry]) -> Vec<String> {
    let mut counts: Vec<String> = [Kind::Video, Kind::Audio, Kind::Folder]
        .into_iter()
        .filter_map(|kind| {
            let n = items.iter().filter(|e| e.kind == kind).count();
            (n > 0).then(|| count_of(kind, n))
        })
        .collect();
    let rest = items
        .iter()
        .filter(|e| matches!(e.kind, Kind::Image | Kind::Subtitle | Kind::Other))
        .count();
    match rest {
        0 => {}
        1 => counts.push("1 other file".to_owned()),
        n => counts.push(format!("{n} other files")),
    }
    counts
}

/// Bytes in the files among the items (a folder's own size says nothing).
fn file_bytes(items: &[Entry]) -> u64 {
    items.iter().filter(|e| !e.is_dir()).map(|e| e.size).sum()
}

/// Each codec with how many files use it, most used first.
fn codec_counts(codecs: Vec<String>) -> Vec<(String, usize)> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for codec in codecs {
        match counts.iter_mut().find(|(c, _)| *c == codec) {
            Some((_, n)) => *n += 1,
            None => counts.push((codec, 1)),
        }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    counts
}

/// The shortest honest form of a folder's path that `fits`: the whole path
/// when it is short, then its last two folders ("…/scratchpad/media"), then
/// its last one ("…/media"). Only one ellipsis, ever: if even the last
/// folder does not fit, that is shown and cut at its end.
fn fit_location(shown: &str, fits: impl Fn(&str) -> bool) -> String {
    let candidates = locations(shown);
    candidates
        .iter()
        .find(|candidate| fits(candidate))
        .or(candidates.last())
        .cloned()
        .unwrap_or_else(|| shown.to_owned())
}

/// The forms [`fit_location`] tries, longest first.
fn locations(shown: &str) -> Vec<String> {
    let parts: Vec<&str> = shown
        .split(MAIN_SEPARATOR)
        .filter(|part| !part.is_empty())
        .collect();
    let mut forms = Vec::new();
    if parts.len() <= 3 {
        forms.push(shown.to_owned());
    }
    for keep in [2, 1] {
        if parts.len() > keep {
            forms.push(format!(
                "…{MAIN_SEPARATOR}{}",
                parts[parts.len() - keep..].join(MAIN_SEPARATOR_STR)
            ));
        }
    }
    if forms.is_empty() {
        forms.push(shown.to_owned());
    }
    forms.dedup();
    forms
}

/// The container as a person would name it: FFmpeg calls a bare stream
/// "raw FLAC"; the file is a FLAC file.
fn container_name(description: &str) -> String {
    description
        .strip_prefix("raw ")
        .unwrap_or(description)
        .to_owned()
}

fn is_technical(key: &str) -> bool {
    TECHNICAL_TAGS.iter().any(|t| t.eq_ignore_ascii_case(key))
}

/// A container tag key as a person would say it: "creation_time" →
/// "Created", "album_artist" → "Album artist".
fn tag_label(key: &str) -> String {
    let lower = key.to_lowercase();
    let known = match lower.as_str() {
        "creation_time" => Some("Created"),
        "date" | "year" => Some("Date"),
        "encoded_by" => Some("Encoded by"),
        "purl" => Some("Link"),
        "comment" => Some("Comment"),
        "description" | "synopsis" => Some("Description"),
        _ => None,
    };
    if let Some(known) = known {
        return known.to_owned();
    }
    let words = lower.replace(['_', '-'], " ");
    let mut chars = words.trim().chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => key.to_owned(),
    }
}

/// Each stream's part of the file: its name, size and share, and a bar in
/// its kind's hue.
fn shares(analysis: &Arc<Analysis>, cx: &App) -> Vec<Div> {
    let t = Theme::of(cx);
    let total: u64 = analysis.streams.iter().map(|s| s.total_bytes).sum();
    analysis
        .streams
        .iter()
        .map(|stream| {
            let share = if total > 0 {
                stream.total_bytes as f32 / total as f32
            } else {
                0.
            };
            let mut name = format!("{} · {}", stream.kind.label(), format::codec(&stream.codec));
            if let Some(language) = &stream.language {
                name = format!("{name} · {}", format::language(language));
            }
            div()
                .flex()
                .flex_col()
                .gap(size::GAP_SMALL)
                .py(size::GAP_SMALL)
                .font_features(theme::tabular())
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .gap(size::GAP)
                        .child(div().min_w_0().truncate().text_color(t.label).child(name))
                        .child(
                            div()
                                .flex_none()
                                .text_color(t.secondary_label)
                                .child(format!(
                                    "{} · {:.1}%",
                                    format::size(stream.total_bytes),
                                    share * 100.
                                )),
                        ),
                )
                .child(ui::share_bar(share, kind_hue(stream.kind, t), cx))
        })
        .collect()
}

/// Size as Finder's Get Info shows it: rounded, then exact.
fn exact_size(bytes: u64) -> String {
    if bytes < 1000 {
        return format::size(bytes);
    }
    let digits = bytes.to_string();
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(c);
    }
    format!("{} ({grouped} bytes)", format::size(bytes))
}

#[cfg(test)]
mod tests {
    // Not a glob: the kit exports its own `test` attribute.
    use super::{
        MediaTotals, arrival, codec_counts, container_name, count_of, exact_size, fit_location,
        is_technical, locations, named_stream, own_segments, pack, readout_step, removal_label,
        stream_details, tag_label, whole_folder,
    };
    use gpui_kit::px;
    use seer_core::format;
    use seer_core::fs::Kind;
    use seer_core::probe::{Stream, StreamFlags, StreamKind};
    use seer_core::transfer::Detail;
    use std::path::MAIN_SEPARATOR as S;

    #[test]
    fn separated_lists_break_between_items() {
        // Each item 40 wide, separators 12: two fit in 100, not three.
        let widths = [px(40.); 5];
        assert_eq!(pack(&widths, px(12.), px(100.)), [0..2, 2..4, 4..5]);
        let wide = pack(&widths, px(12.), px(500.));
        assert_eq!((wide.len(), wide[0].clone()), (1, 0..5));
        // An item wider than the line has a line of its own.
        assert_eq!(
            pack(&[px(30.), px(300.), px(30.)], px(12.), px(100.)),
            [0..1, 1..2, 2..3]
        );
        assert!(pack(&[], px(12.), px(100.)).is_empty());
    }

    #[test]
    fn a_wrapped_list_leaves_no_widow() {
        // Three fit on the first line (144 of 150), which would leave the
        // fourth alone; it takes the third with it instead.
        assert_eq!(pack(&[px(40.); 4], px(12.), px(150.)), [0..2, 2..4]);
        // Not when the line above would be left with one, or the two do
        // not fit together.
        assert_eq!(pack(&[px(40.); 3], px(12.), px(100.)), [0..2, 2..3]);
        assert_eq!(
            pack(&[px(40.), px(40.), px(40.), px(120.)], px(12.), px(150.)),
            [0..3, 3..4]
        );
    }

    #[test]
    fn a_folder_bar_has_a_segment_per_file_down_to_two_points() {
        // 100pt with 1pt gaps: 50 + 30 + 18 + 2 tiny files.
        let sizes = [500, 300, 180, 10, 10];
        assert_eq!(own_segments(&sizes, 100., 2., 1.), 3);
        // Wide enough, every file is its own segment.
        assert_eq!(own_segments(&sizes, 1000., 2., 1.), 5);
        // One huge file and a thousand small ones: the small ones share.
        let mut many = vec![1_000_000u64];
        many.extend(std::iter::repeat_n(10, 1000));
        assert_eq!(own_segments(&many, 276., 2., 1.), 1);
        assert_eq!(own_segments(&[], 100., 2., 1.), 0);
    }

    #[test]
    fn the_chart_readout_steps_by_keyboard() {
        assert_eq!(readout_step(None, 100, "right", false), Some(Some(0)));
        assert_eq!(readout_step(None, 100, "left", false), Some(Some(99)));
        assert_eq!(readout_step(Some(5), 100, "right", false), Some(Some(6)));
        assert_eq!(readout_step(Some(5), 100, "right", true), Some(Some(15)));
        assert_eq!(readout_step(Some(5), 100, "left", true), Some(Some(0)));
        assert_eq!(readout_step(Some(98), 100, "right", true), Some(Some(99)));
        assert_eq!(readout_step(Some(5), 100, "home", false), Some(Some(0)));
        assert_eq!(readout_step(Some(5), 100, "end", false), Some(Some(99)));
        assert_eq!(readout_step(Some(5), 100, "escape", false), Some(None));
        // Not the chart's: Escape with nothing to clear, other keys, no plot.
        assert_eq!(readout_step(None, 100, "escape", false), None);
        assert_eq!(readout_step(Some(5), 100, "a", false), None);
        assert_eq!(readout_step(None, 0, "right", false), None);
    }

    fn totals(media: usize, read: usize, unreadable: usize, seconds: f64) -> MediaTotals {
        MediaTotals {
            media,
            noun: ("video", "videos"),
            read,
            unreadable,
            seconds,
            video: Vec::new(),
            audio: Vec::new(),
        }
    }

    #[test]
    fn totals_say_what_they_leave_out() {
        let duration = |t: MediaTotals| t.duration().map(|(time, rest)| (time, rest.join(" · ")));
        let some = |a: &str, b: &str| Some((a.to_owned(), b.to_owned()));
        assert_eq!(duration(totals(3, 3, 0, 35.)), some("0:35", ""));
        assert_eq!(duration(totals(3, 2, 1, 35.)), some("0:35", "1 unreadable"));
        assert_eq!(
            duration(totals(2, 0, 2, 0.)),
            some("Unavailable", "2 unreadable")
        );
        assert_eq!(
            duration(totals(7, 3, 0, 750.)),
            some("12:30 so far", "3 of 7 read")
        );
        // One definition of "read": successful probes, the same count the
        // codecs' coverage gives; a failure is counted apart.
        assert_eq!(
            duration(totals(7, 2, 1, 750.)),
            some("12:30 so far", "2 of 7 read · 1 unreadable")
        );
        assert_eq!(
            totals(7, 2, 1, 750.).coverage().as_deref(),
            Some("2 of 7 videos")
        );
        assert_eq!(
            duration(totals(3, 1, 1, 20.)),
            some("0:20 so far", "1 of 3 read · 1 unreadable")
        );
        assert_eq!(duration(totals(3, 0, 0, 0.)), some("Reading…", ""));
        assert_eq!(
            duration(totals(3, 0, 1, 0.)),
            some("Reading…", "1 unreadable")
        );
        assert_eq!(duration(totals(0, 0, 0, 0.)), None);
        assert_eq!(
            totals(3, 2, 1, 35.).coverage().as_deref(),
            Some("2 of 3 videos")
        );
        assert_eq!(totals(3, 3, 0, 35.).coverage(), None);
    }

    #[test]
    fn default_is_said_only_beside_another_stream() {
        let stream = Stream {
            index: 0,
            kind: StreamKind::Audio,
            codec: "aac".to_owned(),
            codec_description: None,
            profile: Some("LC".to_owned()),
            language: None,
            title: None,
            flags: StreamFlags {
                default: true,
                ..StreamFlags::default()
            },
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
            channels: None,
            channel_layout: None,
        };
        assert_eq!(stream_details(&stream, true), ["LC"]);
        assert_eq!(stream_details(&stream, false), ["LC", "Default"]);

        // The Info tab names it as the list does, and does not say the
        // layout twice.
        let stereo = Stream {
            channels: Some(2),
            sample_rate: Some(48_000),
            ..stream
        };
        let (name, details) = named_stream(&stereo, true);
        assert_eq!(name, format::audio_name("aac", Some(2)));
        assert!(name.starts_with("Stereo"));
        assert_eq!(details, ["LC".to_owned(), format::sample_rate(48_000)]);

        // A4-1: each removal checkbox names its own track, so two AAC
        // tracks in different languages are told apart.
        let tamil = Stream {
            language: Some("tam".to_owned()),
            ..stereo.clone()
        };
        let english = Stream {
            language: Some("eng".to_owned()),
            title: Some("Commentary".to_owned()),
            ..stereo.clone()
        };
        let label = removal_label(&tamil, Some(2));
        assert_eq!(
            label,
            format!("Remove Audio 2, AAC, {}", format::language("tam"))
        );
        assert_ne!(label, removal_label(&english, Some(1)));
        assert!(removal_label(&english, Some(1)).ends_with("“Commentary”"));
        assert_eq!(removal_label(&stereo, None), "Remove Audio, AAC");
    }

    #[test]
    fn locations_drop_whole_folders_and_cut_once() {
        let long = format!("{S}private{S}tmp{S}claude-501{S}scratchpad{S}media");
        assert_eq!(
            locations(&long),
            [format!("…{S}scratchpad{S}media"), format!("…{S}media")]
        );
        let short = format!("~{S}Movies{S}Films");
        assert_eq!(
            locations(&short),
            [
                short.clone(),
                format!("…{S}Movies{S}Films"),
                format!("…{S}Films")
            ]
        );
        // Two folders fit, then one, then only the last (cut by the view).
        assert_eq!(
            fit_location(&long, |text| text.chars().count() <= 20),
            format!("…{S}scratchpad{S}media")
        );
        assert_eq!(
            fit_location(&long, |text| text.chars().count() <= 10),
            format!("…{S}media")
        );
        assert_eq!(fit_location(&long, |_| false), format!("…{S}media"));
        assert_eq!(fit_location("~", |_| true), "~");
    }

    #[test]
    fn codecs_are_counted_most_used_first() {
        let codecs = ["HEVC", "H.264", "VP9", "H.264", "H.264"].map(str::to_owned);
        assert_eq!(
            codec_counts(codecs.to_vec()),
            [
                ("H.264".to_owned(), 3),
                ("HEVC".to_owned(), 1),
                ("VP9".to_owned(), 1)
            ]
        );
    }

    #[test]
    fn containers_are_named_plainly() {
        assert_eq!(container_name("raw FLAC"), "FLAC");
        assert_eq!(container_name("QuickTime / MOV"), "QuickTime / MOV");
    }

    #[test]
    fn tag_keys_read_as_words() {
        assert_eq!(tag_label("creation_time"), "Created");
        assert_eq!(tag_label("album_artist"), "Album artist");
        assert_eq!(tag_label("TITLE"), "Title");
        assert!(is_technical("major_brand"));
        assert!(is_technical("ENCODER"));
        assert!(!is_technical("title"));
    }

    #[test]
    fn a_copy_says_what_has_arrived() {
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
        let pending = "Copying 4 items here…";
        assert_eq!(arrival(&detail(0, 0, 0, 0), pending, "copied"), [pending]);
        // Bytes in flight are copied (or moved), not yet arrived.
        assert_eq!(
            arrival(
                &detail(1_200_000_000, 6_380_000_000, 2, 6),
                pending,
                "copied"
            ),
            ["1.20 GB of 6.38 GB copied", "2 of 6 files"]
        );
        // One file: no file count; by files when no data is read.
        assert_eq!(
            arrival(&detail(5_000_000, 9_000_000, 0, 1), pending, "moved"),
            ["5.0 MB of 9.0 MB moved"]
        );
        assert_eq!(
            arrival(&detail(0, 0, 3, 5), pending, "copied"),
            ["3 of 5 files arrived"]
        );
        assert_eq!(whole_folder(11), "Whole folder · 11 items");
        assert_eq!(whole_folder(1), "Whole folder · 1 item");
    }

    #[test]
    fn counts_and_sizes() {
        assert_eq!(count_of(Kind::Video, 3), "3 videos");
        assert_eq!(count_of(Kind::Audio, 1), "1 audio file");
        assert_eq!(exact_size(36_728_655), "36.7 MB (36,728,655 bytes)");
    }
}
