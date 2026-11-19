//! The details pane for the current selection.

use crate::browser::display_path;
use crate::chart::{PlotBounds, bitrate_chart};
use crate::library::{Bitrate, Library, Probe, Prune};
use crate::theme::{self, Theme, size, text};
use crate::{ai, native, ui};
use gpui_kit::component::Sizable as _;
use gpui_kit::component::input::{Input, InputState};
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
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

/// Half a point: one device pixel on a Retina display.
const HAIRLINE: Pixels = px(0.5);

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

pub struct Inspector {
    items: Vec<Entry>,
    tab: Tab,
    /// Indexes of the streams marked for removal in the Streams tab.
    unwanted: HashSet<usize>,
    library: Entity<Library>,
    /// Horizontal pointer position while it is over the chart.
    pointer: Option<Pixels>,
    plot: PlotBounds,
    asking: Asking,
    /// The Name section has been opened although the name is already tidy.
    name_open: bool,
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
        Inspector {
            items: Vec::new(),
            tab: Tab::Info,
            unwanted: HashSet::new(),
            library,
            pointer: None,
            plot: PlotBounds::default(),
            asking: Asking::Idle,
            name_open: false,
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

    fn render_setup(&self, cx: &mut Context<Self>) -> Div {
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
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .pt(px(6.))
            .child(ui::segmented(
                "ai-provider",
                &services,
                provider,
                move |provider, window, cx| {
                    this.update(cx, |this, cx| this.choose_provider(provider, window, cx)).ok();
                },
                cx,
            ))
            .child(Input::new(&self.model_field).small())
            .child(Input::new(&self.key_field).small())
            .child(note(
                format!(
                    "{} The key is kept in your Keychain. Only the file name, its title tag, duration and audio languages are sent to {}, never the file.",
                    match has_key {
                        true => "A key is saved; leave the field empty to keep it.",
                        false => "Paste your API key.",
                    },
                    provider.label()
                ),
                cx,
            ))
            .when_some(missing, |this, missing| this.child(note(missing, cx)))
            .child(
                div()
                    .flex()
                    .gap(px(6.))
                    .child(ui::small_button("save-ai", "Save and Ask", false, cx).on_click(
                        cx.listener(|this, _, window, cx| this.save_setup_and_ask(window, cx)),
                    ))
                    .child(ui::small_button("cancel-ai", "Cancel", false, cx).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.asking = Asking::Idle;
                            cx.notify();
                        },
                    ))),
            )
    }

    /// Name suggestions for a video: one worked out here, one from an AI
    /// model on request.
    fn render_naming(&self, entry: &Entry, cx: &mut Context<Self>) -> Option<Div> {
        if entry.kind != Kind::Video {
            return None;
        }
        let t = Theme::of(cx).clone();
        let service = self.ai.provider.label();
        let proposal =
            |id: &'static str, source: String, name: String, cx: &mut Context<Self>| {
                let (path, new_name) = (entry.path.clone(), name.clone());
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .py(px(4.))
                    .text_size(text::SMALL)
                    .line_height(text::SMALL_LINE)
                    .child(div().text_color(t.secondary_label).child(source))
                    .child(div().text_color(t.label).child(name))
                    .child(
                        div()
                            .flex()
                            .child(ui::small_button(id, "Rename", false, cx).on_click(
                                cx.listener(move |this, _, _, cx| {
                                    this.asking = Asking::Idle;
                                    cx.emit(InspectorEvent::Rename {
                                        path: path.clone(),
                                        name: new_name.clone(),
                                    });
                                    cx.notify();
                                }),
                            )),
                    )
            };

        let offline = naming::suggest(&entry.name);
        // Nothing to suggest and nothing asked: one quiet row, opened on request.
        if offline.is_none() && matches!(self.asking, Asking::Idle) && !self.name_open {
            return Some(
                section("Name", cx).child(
                    div()
                        .id("open-name")
                        .flex()
                        .justify_between()
                        .text_size(text::SMALL)
                        .line_height(text::SMALL_LINE)
                        .text_color(t.secondary_label)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.name_open = true;
                            cx.notify();
                        }))
                        .child("Already tidy")
                        .child(div().text_color(t.tertiary_label).child("Ask AI…")),
                ),
            );
        }
        let mut body = section("Name", cx);
        match &offline {
            Some(name) => {
                body = body.child(proposal(
                    "rename-offline",
                    "Suggested".into(),
                    name.clone(),
                    cx,
                ))
            }
            None => {
                body = body.child(note(
                    "This name is already tidy, or has no title and year to work from.",
                    cx,
                ))
            }
        }

        // Ask, and a way back to the form to change service, model or key.
        let actions = |label: String, cx: &mut Context<Self>| {
            div()
                .flex()
                .gap(px(6.))
                .pt(px(4.))
                .child(
                    ui::small_button("ask-ai", label, false, cx)
                        .on_click(cx.listener(|this, _, window, cx| this.ask_ai(window, cx))),
                )
                .child(
                    ui::small_button("ai-setup", "AI Settings…", false, cx)
                        .on_click(cx.listener(|this, _, window, cx| this.open_setup(window, cx))),
                )
        };
        body = match &self.asking {
            Asking::Idle => body.child(actions("Ask AI".into(), cx)),
            Asking::Setup => body.child(self.render_setup(cx)),
            Asking::Waiting => body.child(
                div()
                    .pt(px(4.))
                    .child(note(format!("Asking {service}…"), cx)),
            ),
            Asking::Answered(name) if *name == entry.name => body.child(
                div()
                    .pt(px(4.))
                    .child(note(format!("{service} would keep the current name."), cx)),
            ),
            Asking::Answered(name) if offline.as_ref() == Some(name) => body.child(
                div()
                    .pt(px(4.))
                    .child(note(format!("{service} suggests the same name."), cx)),
            ),
            Asking::Answered(name) => {
                let source = format!("From {service} ({})", self.ai.model(self.ai.provider));
                body.child(proposal("rename-ai", source, name.clone(), cx))
            }
            Asking::Failed(message) => body
                .child(
                    div()
                        .pt(px(4.))
                        .child(note(message.clone(), cx).text_color(t.red)),
                )
                .child(actions("Try Again".into(), cx)),
        };
        Some(body)
    }

    pub fn show(&mut self, items: Vec<Entry>, cx: &mut Context<Self>) {
        if let [only] = items.as_slice()
            && only.kind.is_probeable()
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
            self.name_open = false;
            self.request = None;
        }
        self.items = items;
        self.pointer = None;
        cx.notify();
    }

    /// Starts analysing the selected file and shows the result.
    pub fn analyze(&mut self, cx: &mut Context<Self>) {
        let [only] = self.items.as_slice() else {
            return;
        };
        if !only.kind.is_probeable() {
            return;
        }
        let path = only.path.clone();
        self.tab = Tab::Bitrate;
        self.library
            .update(cx, |library, cx| library.analyze(&path, cx));
        cx.notify();
    }

    fn render_header(&self, entry: &Entry, cx: &App) -> impl IntoElement {
        let t = Theme::of(cx);
        let kind = entry
            .extension()
            .map(|e| format!("{} {}", e.to_uppercase(), entry.kind.label().to_lowercase()));
        let subtitle = match (entry.is_dir(), kind) {
            (true, _) => "Folder".to_owned(),
            (false, Some(kind)) => format!("{kind}  ·  {}", format::size(entry.size)),
            (false, None) => format::size(entry.size),
        };
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .children(
                native::icon_for(&entry.path, entry.is_dir(), 32)
                    .map(|icon| img(icon).size(px(32.)).flex_none()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(text::BODY)
                            .line_height(text::BODY_LINE)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(t.label)
                            .line_clamp(2)
                            .child(entry.name.clone()),
                    )
                    .child(
                        div()
                            .text_size(text::SMALL)
                            .line_height(text::SMALL_LINE)
                            .text_color(t.secondary_label)
                            .font_features(theme::tabular())
                            .child(subtitle),
                    ),
            )
    }

    fn render_info(&self, entry: &Entry, cx: &App) -> Div {
        let library = self.library.read(cx);
        let mut body = div().flex().flex_col().gap(size::PANE_PADDING).child(
            section("General", cx)
                .child(ui::field("Kind", entry.kind.label(), cx))
                .when(!entry.is_dir(), |this| {
                    this.child(ui::field("Size", exact_size(entry.size), cx))
                })
                .when_some(entry.modified, |this, time| {
                    this.child(ui::field("Modified", format::date(time), cx))
                })
                .when_some(entry.path.parent(), |this, parent| {
                    this.child(ui::field("Where", display_path(parent), cx))
                }),
        );

        match library.probe(&entry.path) {
            Some(Probe::Ready(info)) => {
                body = body.child(
                    section("Container", cx)
                        .child(ui::field("Format", info.container_description.clone(), cx))
                        .when(info.duration > 0., |this| {
                            this.child(ui::field("Duration", format::duration(info.duration), cx))
                        })
                        .when(info.bit_rate > 0, |this| {
                            this.child(ui::field("Bit Rate", format::bitrate(info.bit_rate), cx))
                        })
                        .when(!info.chapters.is_empty(), |this| {
                            this.child(ui::field("Chapters", info.chapters.len().to_string(), cx))
                        }),
                );
                for (kind, title) in [
                    (StreamKind::Video, "Video"),
                    (StreamKind::Audio, "Audio"),
                    (StreamKind::Subtitle, "Subtitles"),
                    (StreamKind::Attachment, "Attachments"),
                    (StreamKind::Data, "Data"),
                ] {
                    let streams: Vec<_> = info.streams_of(kind).collect();
                    if streams.is_empty() {
                        continue;
                    }
                    let title = match streams.len() {
                        1 => title.to_owned(),
                        n => format!("{title}  {n}"),
                    };
                    body = body
                        .child(section(title, cx).children(
                            streams.into_iter().map(|s| stream_row(s, false, false, cx)),
                        ));
                }
                if !info.tags.is_empty() {
                    body = body.child(
                        section("Tags", cx).children(
                            info.tags
                                .iter()
                                .map(|(key, value)| tag_row(key.clone(), value.clone(), cx)),
                        ),
                    );
                }
            }
            Some(Probe::Pending) => body = body.child(note("Reading media information…", cx)),
            Some(Probe::Unreadable) => {
                body = body.child(note(
                    "This file doesn’t contain media that Seer can read.",
                    cx,
                ));
            }
            None => {}
        }
        body
    }

    fn render_streams(&self, entry: &Entry, cx: &mut Context<Self>) -> Div {
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

        let rows = info
            .streams
            .iter()
            .map(|stream| {
                let index = stream.index;
                let removing = self.unwanted.contains(&index);
                div()
                    .id(("stream", index))
                    .flex()
                    .items_start()
                    .gap(size::GAP)
                    .when(!running, |this| {
                        this.on_click(cx.listener(move |this, _, _, cx| {
                            if !this.unwanted.remove(&index) {
                                this.unwanted.insert(index);
                            }
                            cx.notify();
                        }))
                    })
                    .child(div().pt(px(5.)).child(ui::removal_mark(removing, cx)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(stream_row(stream, true, removing, cx)),
                    )
            })
            .collect::<Vec<_>>();

        let suggestions = (!running)
            .then(|| self.render_suggestions(&info, cx))
            .flatten();

        let path = entry.path.clone();
        let action = match prune {
            Some(Prune::Running(progress)) => div()
                .flex()
                .flex_col()
                .gap(size::GAP)
                .child(ui::progress_bar(progress.fraction(), cx))
                .child(
                    div()
                        .text_size(text::SMALL)
                        .text_color(t.secondary_label)
                        .font_features(theme::tabular())
                        .child(format!(
                            "Rewriting the file… {:.0}%",
                            progress.fraction() * 100.
                        )),
                )
                .child(div().flex().child(
                    ui::button("cancel-prune", "Cancel", false, cx).on_click(cx.listener(
                        move |this, _, _, cx| {
                            this.library
                                .update(cx, |library, cx| library.cancel_prune(&path, cx));
                        },
                    )),
                )),
            _ => {
                let label = match unwanted.len() {
                    0 => "Remove Streams".to_owned(),
                    1 => "Remove 1 Stream".to_owned(),
                    n => format!("Remove {n} Streams"),
                };
                let button = if unwanted.is_empty() || !keeps_something {
                    ui::disabled_button(label, cx).into_any_element()
                } else {
                    ui::button("prune", label, true, cx)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let remove: Vec<usize> = this.unwanted.drain().collect();
                            this.library.update(cx, |library, cx| {
                                library.remove_streams(&path, remove, cx)
                            });
                            cx.notify();
                        }))
                        .into_any_element()
                };
                div()
                    .flex()
                    .flex_col()
                    .gap(size::GAP)
                    .when_some(
                        match prune {
                            Some(Prune::Failed(message)) => Some(message),
                            _ => None,
                        },
                        |this, message| {
                            this.child(
                                note(format!("The file was not changed: {message}"), cx)
                                    .text_color(t.red),
                            )
                        },
                    )
                    .when(!keeps_something, |this| {
                        this.child(note("A file must keep at least one stream.", cx))
                    })
                    .child(div().flex().child(button))
            }
        };

        body.child(note(
            "Click the streams you want to remove. The file is rewritten without re-encoding and the original moves to the Trash.",
            cx,
        ))
        .children(suggestions)
        .child(section("Streams", cx).children(rows))
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
                .gap(px(2.))
                .py(px(4.))
                .text_size(text::SMALL)
                .line_height(text::SMALL_LINE)
                .child(div().text_color(t.label).child(title))
                .child(div().text_color(t.secondary_label).child(detail))
        };
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
                ui::small_button(("keep-language", i), format!("Keep {name}"), in_effect, cx)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        // Choosing a language replaces any earlier language choice.
                        this.unwanted.retain(|i| !tagged.contains(i));
                        this.unwanted.extend(others.iter().copied());
                        cx.notify();
                    }))
            });
            rows.push(
                suggestion("Original audio only", detail).child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(6.))
                        .pt(px(4.))
                        .children(choices),
                ),
            );
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
                ui::small_button(id, label, false, cx).on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.library
                            .update(cx, |library, cx| library.rewrite(&path, edits.clone(), cx));
                        cx.notify();
                    },
                ))
            };
            let changes = names.iter().map(|issue| {
                div()
                    .flex()
                    .gap(px(6.))
                    .text_color(t.secondary_label)
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .line_through()
                            .child(issue.current.clone()),
                    )
                    .child(div().flex_none().text_color(t.tertiary_label).child("→"))
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
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(6.))
                            .pt(px(4.))
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
            rows.push(suggestion("Best quality only", detail).child(
                div().flex().pt(px(4.)).child(
                    ui::small_button("keep-best", "Mark Lower Quality", in_effect, cx).on_click(
                        cx.listener(move |this, _, _, cx| {
                            this.unwanted.extend(duplicates.iter().copied());
                            cx.notify();
                        }),
                    ),
                ),
            ));
        }

        (!rows.is_empty()).then(|| section("Suggestions", cx).children(rows))
    }

    fn render_bitrate(&self, entry: &Entry, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let t = Theme::of(cx).clone();
        let path = entry.path.clone();
        let body = div().flex().flex_col().gap(size::PANE_PADDING);
        match self.library.read(cx).bitrate(&entry.path) {
            Bitrate::Idle => body
                .child(note(
                    "See how the bit rate changes over the length of the file, and how much each stream contributes.",
                    cx,
                ))
                .child(div().flex().child(
                    ui::button("analyze", "Analyse Bit Rate", true, cx)
                        .on_click(cx.listener(|this, _, _, cx| this.analyze(cx))),
                )),
            Bitrate::Running(progress) => {
                // Keep repainting while the worker advances.
                window.request_animation_frame();
                let fraction = progress.fraction();
                body.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(ui::progress_bar(fraction, cx))
                        .child(
                            div()
                                .text_size(text::SMALL)
                                .text_color(t.secondary_label)
                                .font_features(theme::tabular())
                                .child(format!("Reading packets… {:.0}%", fraction * 100.)),
                        ),
                )
                .child(div().flex().child(ui::button("cancel", "Cancel", false, cx).on_click(
                    cx.listener(move |this, _, _, cx| {
                        this.library.update(cx, |library, cx| library.cancel_analysis(&path, cx));
                    }),
                )))
            }
            Bitrate::Failed(message) => body
                .child(note(format!("The analysis failed: {message}"), cx).text_color(t.red))
                .child(div().flex().child(
                    ui::button("retry", "Try Again", false, cx)
                        .on_click(cx.listener(|this, _, _, cx| this.analyze(cx))),
                )),
            Bitrate::Ready(analysis) => body
                .child(
                    div()
                        .id("chart")
                        .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                            this.pointer = Some(event.position.x);
                            cx.notify();
                        }))
                        .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                            if !hovered {
                                this.pointer = None;
                                cx.notify();
                            }
                        }))
                        .child(bitrate_chart(analysis.clone(), self.pointer, self.plot.clone(), window, cx)),
                )
                .child(
                    section("Overall", cx)
                        .child(ui::field("Average", format::bitrate(analysis.stats.average), cx))
                        .child(ui::field("Median", format::bitrate(analysis.stats.median), cx))
                        .child(ui::field(
                            "Peak",
                            format!(
                                "{} at {}",
                                format::bitrate(analysis.stats.max),
                                format::timestamp(analysis.stats.peak_at, analysis.duration)
                            ),
                            cx,
                        ))
                        .child(ui::field("Lowest", format::bitrate(analysis.stats.min), cx)),
                )
                .child(section("Streams", cx).children(shares(&analysis, cx))),
        }
    }
}

impl Render for Inspector {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::of(cx).clone();
        let pane = div().size_full().bg(t.content).text_color(t.label);

        let entry = match self.items.as_slice() {
            [] => {
                return pane.child(ui::empty_state(
                    "No Selection",
                    "Select a file to see its details.",
                    cx,
                ));
            }
            [entry] => entry.clone(),
            many => {
                let bytes: u64 = many.iter().map(|e| e.size).sum();
                return pane.child(ui::empty_state(
                    format!("{} items", many.len()),
                    format!("{} in total", format::size(bytes)),
                    cx,
                ));
            }
        };

        let media = entry.kind.is_probeable();
        let tab = if media { self.tab } else { Tab::Info };
        if matches!(
            self.library.read(cx).prune(&entry.path),
            Some(Prune::Running(_))
        ) {
            // Keep repainting while the worker advances.
            window.request_animation_frame();
        }
        let this = cx.entity().downgrade();
        let tabs = ui::segmented(
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

        pane.flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .gap(size::PANE_PADDING)
                    .p(size::PANE_PADDING)
                    .child(self.render_header(&entry, cx))
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
                    .child(match tab {
                        Tab::Info => div()
                            .flex()
                            .flex_col()
                            .gap(size::PANE_PADDING)
                            .children(self.render_naming(&entry, cx))
                            .child(self.render_info(&entry, cx)),
                        Tab::Streams => self.render_streams(&entry, cx),
                        Tab::Bitrate => self.render_bitrate(&entry, window, cx),
                    }),
            )
    }
}

fn section(title: impl Into<SharedString>, cx: &App) -> Div {
    let t = Theme::of(cx);
    div()
        .flex()
        .flex_col()
        .pt(size::GAP)
        .border_t(HAIRLINE)
        .border_color(t.separator)
        .child(div().pb(px(4.)).child(ui::section_header(title, cx)))
}

fn note(message: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_size(text::SMALL)
        .line_height(text::SMALL_LINE)
        .text_color(Theme::of(cx).secondary_label)
        .child(message.into())
}

/// One stream: what it is on the first line, the details below. `with_kind`
/// names the stream type, for lists that mix types. `removing` strikes the
/// stream out and labels it as about to be removed.
fn stream_row(stream: &Stream, with_kind: bool, removing: bool, cx: &App) -> Div {
    let t = Theme::of(cx);
    let mut name = format::codec(&stream.codec);
    if with_kind {
        name = format!("{}  ·  {name}", stream.kind.label());
    }
    if let Some(profile) = &stream.profile {
        name = format!("{name} {profile}");
    }

    let mut details: Vec<String> = Vec::new();
    match stream.kind {
        StreamKind::Video => {
            if let (Some(w), Some(h)) = (stream.width, stream.height) {
                details.push(format::resolution(w, h));
            }
            if let Some(fps) = stream.frame_rate {
                details.push(format::frame_rate(fps));
            }
            if let Some(depth) = stream.bit_depth {
                details.push(format!("{depth}-bit"));
            }
            if let Some(range) = stream.dynamic_range() {
                details.push(range.to_owned());
            }
        }
        StreamKind::Audio => {
            if let Some(channels) = stream.channels {
                details.push(format::channels(channels));
            }
            if let Some(rate) = stream.sample_rate {
                details.push(format::sample_rate(rate));
            }
        }
        _ => {}
    }

    let mut about: Vec<String> = Vec::new();
    if let Some(language) = &stream.language {
        about.push(format::language(language));
    }
    if let Some(title) = &stream.title {
        about.push(format!("“{title}”"));
    }
    let flags = &stream.flags;
    for (set, label) in [
        (flags.default, "Default"),
        (flags.forced, "Forced"),
        (flags.commentary, "Commentary"),
        (flags.hearing_impaired, "SDH"),
        (flags.visual_impaired, "Audio description"),
        (flags.attached_picture, "Cover art"),
    ] {
        if set {
            about.push(label.to_owned());
        }
    }

    let rate = stream.bit_rate.map(format::bitrate).unwrap_or_default();

    let line = |parts: Vec<String>| {
        div()
            .text_color(t.secondary_label)
            .when(!parts.is_empty(), |this| this.child(parts.join("  ·  ")))
    };
    div()
        .flex()
        .flex_col()
        .py(px(4.))
        .text_size(text::SMALL)
        .line_height(px(16.))
        .font_features(theme::tabular())
        .child(
            div()
                .flex()
                .justify_between()
                .gap(size::GAP)
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(if removing { t.secondary_label } else { t.label })
                        .when(removing, |this| this.line_through())
                        .child(name),
                )
                .child(match removing {
                    // Say what will happen to this stream, in the destructive colour.
                    true => div().flex_none().text_color(t.red).child("Will be removed"),
                    false => div().flex_none().text_color(t.secondary_label).child(rate),
                }),
        )
        .when(!details.is_empty(), |this| this.child(line(details)))
        .when(!about.is_empty(), |this| this.child(line(about)))
}

fn tag_row(key: String, value: String, cx: &App) -> Div {
    let t = Theme::of(cx);
    div()
        .flex()
        .flex_col()
        .py(px(3.))
        .text_size(text::SMALL)
        .line_height(text::SMALL_LINE)
        .child(div().text_color(t.secondary_label).child(key))
        .child(div().text_color(t.label).line_clamp(4).child(value))
}

/// Each stream's part of the file: a proportional bar, its size and share.
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
            let mut name = format!(
                "{}  ·  {}",
                stream.kind.label(),
                format::codec(&stream.codec)
            );
            if let Some(language) = &stream.language {
                name = format!("{name}  ·  {}", format::language(language));
            }
            div()
                .flex()
                .flex_col()
                .gap(px(3.))
                .py(px(4.))
                .text_size(text::SMALL)
                .line_height(text::SMALL_LINE)
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
                                    "{}  ·  {:.1}%",
                                    format::size(stream.total_bytes),
                                    share * 100.
                                )),
                        ),
                )
                .child(ui::progress_bar(share, cx))
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
