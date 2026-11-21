//! The app's shared controls. One of each; see DESIGN.md principle 7. Every
//! size and colour comes from `theme`; call sites add layout, never style.
//!
//! - Text roles: [`title`], [`display`], [`section_header`].
//! - Controls: [`Button`], [`IconButton`], [`Checkbox`], [`segmented`] (and
//!   [`tabs`]), [`PopoverTrigger`], [`text_field`].
//! - File glyphs: [`kind_glyph`] (and [`kind_glyph_in`], [`Glyph`]).
//! - Rows and data: [`field_with_rail`] (and [`field_rail`]),
//!   [`progress_bar`], [`share_bar`].
//! - Navigation: [`Breadcrumb`].
//! - Menus: [`MenuButton`] (and [`open_menu`]), [`menu_item`],
//!   [`command_menu_with`].
//! - Surfaces: [`Sheet`], [`overlay_card`], [`EmptyState`] (and
//!   [`trace_glyph`]), [`Tooltip`] via the [`Tip`] trait,
//!   [`window_controls`].

use crate::actions;
use crate::platform::{self, Command, Side};
use crate::theme::{self, Theme, motion, size, text};
use gpui_kit::assets::IconName;
use gpui_kit::base::FocusTrapElement as _;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{DropdownMenu, PopupMenu, PopupMenuItem};
use gpui_kit::component::{Icon, Selectable, Sizable as _};
use gpui_kit::prelude::*;
use gpui_kit::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::thread::LocalKey;

/// Opacity of a disabled control's fill (never of its text or glyph, which
/// use `tertiary_label` at full strength).
pub const DISABLED: f32 = 0.4;
/// Opacity of titlebar controls while the window is not key.
pub const INACTIVE: f32 = 0.5;

type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;
type Handler = Rc<dyn Fn(&mut Window, &mut App) + 'static>;
type ToggleHandler = Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>;

pub fn icon(name: IconName, color: Hsla) -> Icon {
    Icon::new(name).with_size(size::ICON).text_color(color)
}

// ---------------------------------------------------------------------------
// Text roles

/// Folder name in the titlebar, file name in the inspector header, sheet
/// titles: 15/20 semibold in `label`, truncated at the tail.
pub fn title(content: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .min_w_0()
        .truncate()
        .text_size(text::TITLE)
        .line_height(text::TITLE_LINE)
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(Theme::of(cx).label)
        .child(content.into())
}

/// Empty-state titles in the main pane only: 20/24 semibold.
pub fn display(content: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_size(text::DISPLAY)
        .line_height(text::DISPLAY_LINE)
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(Theme::of(cx).label)
        .child(content.into())
}

/// Small semibold heading above a group of rows, in `label` so it outranks
/// the field labels beside it.
pub fn section_header(title: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_size(text::SMALL)
        .line_height(text::SMALL_LINE)
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(Theme::of(cx).label)
        .child(title.into())
}

// ---------------------------------------------------------------------------
// Tooltips

/// A tooltip: the command's name, then its shortcut in secondary. Built by
/// [`Tip::tip`]; shown after [`motion::TOOLTIP_DELAY`].
pub struct Tooltip {
    label: SharedString,
    shortcut: Option<SharedString>,
    /// Where the control it names sits, when known, and where the pointer
    /// was when the tip was shown (GPUI places the tip from there): the tip
    /// then opens below the control instead of at the pointer.
    anchor: Option<(Bounds<Pixels>, Point<Pixels>)>,
}

impl Render for Tooltip {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::of(cx);
        // GPUI places a tooltip one point below and right of the pointer.
        // Below an anchor in the upper half of the window, a transparent
        // band pushes the card down to `GAP_SMALL` under the anchor, so it
        // never covers the control or its neighbours in the same row, and
        // the card moves sideways to sit centred under the anchor, kept
        // `GAP_SMALL` inside the window. Lower down, GPUI's own placement
        // (flipping above) is kept.
        let viewport = window.viewport_size();
        let (drop, shift) = self
            .anchor
            .filter(|(anchor, _)| anchor.bottom() < viewport.height / 2.)
            .map(|(anchor, pointer)| {
                let top = pointer.y + px(1.);
                let drop = (anchor.bottom() + size::GAP_SMALL - top).max(px(0.));
                let width = self.card_width(window);
                // Where GPUI puts the card's leading edge: right of the
                // pointer, or left of it when that would run off the window.
                let mut origin = pointer.x + px(1.);
                if origin + width > viewport.width && pointer.x - width - px(1.) >= px(0.) {
                    origin = pointer.x - width - px(1.);
                }
                let centred = (anchor.center().x - width / 2.)
                    .min(viewport.width - width - size::GAP_SMALL)
                    .max(size::GAP_SMALL);
                (drop, centred - origin)
            })
            .unwrap_or_default();
        let card = div()
            .relative()
            .left(shift)
            .font_family(t.font.clone())
            .flex()
            .items_center()
            .gap(size::GAP)
            .max_w(size::TOOLTIP_MAX_WIDTH)
            .py(size::GAP_SMALL)
            .px(size::GAP)
            .rounded(size::RADIUS_CONTROL)
            .bg(t.surface_overlay)
            .border_1()
            .border_color(t.border_overlay)
            .shadow(t.overlay_shadow())
            .text_size(text::SMALL)
            .line_height(text::SMALL_LINE)
            .text_color(t.label)
            .child(self.label.clone())
            .when_some(self.shortcut.clone(), |this, keys| {
                this.child(
                    div()
                        .flex_none()
                        .text_color(t.secondary_label)
                        .font_features(theme::tabular())
                        .child(keys),
                )
            });
        div().pt(drop).child(card)
    }
}

impl Tooltip {
    /// How wide the card is drawn: its padding and border, the label, and
    /// the shortcut after a gap, up to `TOOLTIP_MAX_WIDTH`.
    fn card_width(&self, window: &Window) -> Pixels {
        let measure =
            |text: &SharedString| text_width(text, text::SMALL, FontWeight::NORMAL, window);
        let shortcut = self
            .shortcut
            .as_ref()
            .map_or(px(0.), |keys| size::GAP + measure(keys));
        (size::GAP * 2. + size::HAIRLINE * 2. + measure(&self.label) + shortcut)
            .min(size::TOOLTIP_MAX_WIDTH)
    }
}

thread_local! {
    /// The last painted bounds of each control that opens its tooltip
    /// below itself (`anchored_tip`), by element id.
    static ANCHORS: RefCell<HashMap<ElementId, Bounds<Pixels>>> = RefCell::default();
}

/// An invisible child, the size of its (relative) parent, that records the
/// parent's bounds under `key` each frame.
fn anchor_probe(key: ElementId) -> impl IntoElement {
    canvas(
        move |bounds, _, _| {
            ANCHORS.with_borrow_mut(|anchors| anchors.insert(key, bounds));
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// [`Tip::tip`] for a control that also renders [`anchor_probe`] under
/// `key`: the tooltip opens below the control rather than at the pointer.
fn anchored_tip<E: StatefulInteractiveElement>(
    element: E,
    key: ElementId,
    label: impl Into<SharedString>,
    command: Option<Command>,
) -> E {
    let label = label.into();
    let shortcut = command.and_then(platform::shortcut);
    element
        .tooltip(move |window, cx| {
            let (label, shortcut) = (label.clone(), shortcut.clone());
            let pointer = window.mouse_position();
            let anchor = ANCHORS
                .with_borrow(|anchors| anchors.get(&key).copied())
                .map(|bounds| (bounds, pointer));
            cx.new(|_| Tooltip {
                label,
                shortcut,
                anchor,
            })
            .into()
        })
        .tooltip_show_delay(motion::TOOLTIP_DELAY)
}

/// Adds Seer's delayed tooltip to any element with an id. Every icon-only
/// control has one: its name, and its shortcut when `command` has one.
pub trait Tip: StatefulInteractiveElement + Sized {
    fn tip(self, label: impl Into<SharedString>, command: Option<Command>) -> Self {
        let label = label.into();
        let shortcut = command.and_then(platform::shortcut);
        self.tooltip(move |_, cx| {
            let (label, shortcut) = (label.clone(), shortcut.clone());
            cx.new(|_| Tooltip {
                label,
                shortcut,
                anchor: None,
            })
            .into()
        })
        .tooltip_show_delay(motion::TOOLTIP_DELAY)
    }
}

impl<E: StatefulInteractiveElement> Tip for E {}

// ---------------------------------------------------------------------------
// Buttons

/// Height and type size of a [`Button`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ButtonSize {
    /// 22pt, 11pt text. Inspector, popovers, empty states, rows in a sheet.
    Small,
    /// 28pt, 13pt text. Titlebar and sheet footers.
    #[default]
    Regular,
}

/// What a [`Button`] means. Exactly one `Primary` per surface.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Variant {
    /// The default action: accent fill (inverted label when the accent is
    /// grey). Return triggers it in a sheet.
    Primary,
    /// Raised fill and a 1px border.
    #[default]
    Secondary,
    /// No fill until hovered. For low-emphasis actions in dense places.
    Ghost,
    /// Rewrites or deletes something: red text on the Secondary fill
    /// (`Theme::destructive`, which reads at 4.5:1 on it).
    Destructive,
}

/// The one push button. Hover and pressed fills, a focus ring when focused
/// from the keyboard, and Return or Space press it when focused. A disabled
/// button keeps its place, draws as a Secondary button with its fill at 40%
/// and its label in `tertiary_label`, and has no handler.
///
/// ```ignore
/// ui::Button::new("analyze", "Analyse Bit Rate")
///     .small()
///     .primary()
///     .on_click(cx.listener(|this, _, _, cx| this.analyze(cx)))
/// ```
#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: SharedString,
    icon: Option<IconName>,
    size: ButtonSize,
    variant: Variant,
    disabled: bool,
    on: bool,
    tip: Option<(SharedString, Option<Command>)>,
    on_click: Option<ClickHandler>,
}

impl Button {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Button {
            id: id.into(),
            label: label.into(),
            icon: None,
            size: ButtonSize::Regular,
            variant: Variant::Secondary,
            disabled: false,
            on: false,
            tip: None,
            on_click: None,
        }
    }

    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    /// Shorthand for `.size(ButtonSize::Small)`.
    pub fn small(self) -> Self {
        self.size(ButtonSize::Small)
    }

    pub fn variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self
    }

    pub fn primary(self) -> Self {
        self.variant(Variant::Primary)
    }

    pub fn ghost(self) -> Self {
        self.variant(Variant::Ghost)
    }

    pub fn destructive(self) -> Self {
        self.variant(Variant::Destructive)
    }

    /// A glyph before the label.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Shows that this button's choice is the one in effect (one of a set
    /// of alternatives that are not a segmented control): the `fill_on`
    /// fill, in both appearances.
    pub fn on(mut self, on: bool) -> Self {
        self.on = on;
        self
    }

    /// A tooltip. Use it when the label is shortened or the action has a
    /// shortcut worth teaching.
    pub fn tip(mut self, label: impl Into<SharedString>, command: Option<Command>) -> Self {
        self.tip = Some((label.into(), command));
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Button {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Theme::of(cx).clone();
        let (height, padding, font, line, glyph) = match self.size {
            ButtonSize::Small => (
                size::CONTROL_SMALL,
                size::BUTTON_PADDING_SMALL,
                text::SMALL,
                text::SMALL_LINE,
                size::ICON_SMALL,
            ),
            ButtonSize::Regular => (
                size::CONTROL,
                size::BUTTON_PADDING,
                text::BODY,
                text::BODY_LINE,
                size::ICON,
            ),
        };
        let (fill, hover, pressed, ink, border) = match self.variant {
            Variant::Primary => (
                Some(t.primary),
                t.primary_hover,
                t.primary_pressed,
                t.on_primary,
                None,
            ),
            // On: the on-state fill in both appearances, the usual border,
            // so it reads as chosen and not as focused.
            Variant::Secondary => (
                Some(if self.on { t.fill_on } else { t.control }),
                if self.on {
                    t.fill_pressed
                } else {
                    t.control_hover
                },
                t.control_pressed,
                t.label,
                Some(t.border),
            ),
            Variant::Ghost => (
                self.on.then_some(t.fill_on),
                t.fill_hover,
                t.fill_pressed,
                t.label,
                None,
            ),
            Variant::Destructive => (
                Some(t.control),
                t.control_hover,
                t.control_pressed,
                t.destructive,
                Some(t.border),
            ),
        };
        let enabled = !self.disabled;
        // Disabled: the Secondary shape, a faded fill, tertiary ink. Opacity
        // is for fills only, so the label stays legible.
        let (fill, ink, border) = match (enabled, self.variant) {
            (true, _) => (fill, ink, border),
            (false, Variant::Ghost) => (None, t.tertiary_label, None),
            (false, _) => (
                Some(t.control.opacity(DISABLED)),
                t.tertiary_label,
                Some(t.border),
            ),
        };
        let focus = t.focus_shadow();
        let key = self.id.clone();
        // A disabled button says why when its tooltip does ("No videos in
        // this folder"), else that it is unavailable.
        let unavailable = (!enabled).then(|| match &self.tip {
            Some((tip, _)) if *tip != self.label => tip.clone(),
            _ => SharedString::new_static("Unavailable"),
        });
        let keys = self
            .tip
            .as_ref()
            .and_then(|(_, command)| command.and_then(platform::shortcut));
        div()
            .id(self.id)
            .role(Role::Button)
            .aria_label(self.label.clone())
            .when_some(unavailable, |this, why| this.aria_description(why))
            .when_some(keys, |this, keys| this.aria_keyshortcuts(keys))
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .gap(size::GAP_SMALL)
            .h(height)
            .px(padding)
            .rounded(size::RADIUS_CONTROL)
            .text_size(font)
            .line_height(line)
            .text_color(ink)
            .whitespace_nowrap()
            .when_some(fill, |this, fill| this.bg(fill))
            .when_some(border, |this, border| {
                this.border(size::HAIRLINE).border_color(border)
            })
            .when(enabled, |this| {
                this.focusable()
                    .tab_stop(true)
                    .hover(move |s| s.bg(hover))
                    .active(move |s| s.bg(pressed))
                    .focus_visible(move |s| s.shadow(focus))
            })
            // A press on a control must not start dragging the window or
            // take focus from the list.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .when_some(self.icon, |this, name| {
                this.child(Icon::new(name).with_size(glyph).text_color(ink))
            })
            .child(self.label)
            .when_some(self.tip, |this, (label, command)| {
                anchored_tip(this, key.clone(), label, command).child(anchor_probe(key))
            })
            .when_some(self.on_click.filter(|_| enabled), |this, handler| {
                this.on_click(move |event, window, cx| handler(event, window, cx))
            })
    }
}

/// A borderless icon-only button: titlebar and pane controls. It always has
/// a tooltip (its `label`, plus the `command`'s shortcut), a hover fill,
/// and an on state for toggles whose pane is open.
///
/// Assistive technology reads it as a button named by its `label` (or
/// [`IconButton::accessible_label`]), with its shortcut; a toggle made
/// with [`IconButton::pressed`] also reports whether it is on.
///
/// ```ignore
/// ui::IconButton::new("sidebar", IconName::PanelLeft, "Hide Sidebar")
///     .command(Command::ToggleSidebar)
///     .accessible_label("Sidebar")
///     .pressed(sidebar_visible)
///     .on_click(cx.listener(|this, _, window, cx| this.toggle(Pane::Sidebar, window, cx)))
/// ```
#[derive(IntoElement)]
pub struct IconButton {
    id: ElementId,
    icon: IconName,
    label: SharedString,
    accessible_label: Option<SharedString>,
    command: Option<Command>,
    on: bool,
    toggle: bool,
    disabled: bool,
    small: bool,
    tab_stop: bool,
    on_click: Option<ClickHandler>,
}

impl IconButton {
    pub fn new(id: impl Into<ElementId>, icon: IconName, label: impl Into<SharedString>) -> Self {
        IconButton {
            id: id.into(),
            icon,
            label: label.into(),
            accessible_label: None,
            command: None,
            on: false,
            toggle: false,
            disabled: false,
            small: false,
            tab_stop: true,
            on_click: None,
        }
    }

    /// The name a screen reader says, when it should not change with the
    /// state as the tooltip does ("Sidebar" for "Show Sidebar" and "Hide
    /// Sidebar").
    pub fn accessible_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessible_label = Some(label.into());
        self
    }

    /// A toggle: drawn filled while `on` (the thing it toggles is showing),
    /// and says whether it is on to assistive technology.
    pub fn pressed(mut self, on: bool) -> Self {
        self.on = on;
        self.toggle = true;
        self
    }

    /// Whether Tab stops on it (default true). Off for a button inside a
    /// group with one roving stop, which it joins while its row has focus.
    pub fn tab_stop(mut self, tab_stop: bool) -> Self {
        self.tab_stop = tab_stop;
        self
    }

    /// The command this button runs, for the shortcut in its tooltip.
    pub fn command(mut self, command: Command) -> Self {
        self.command = Some(command);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// 22pt square with a 12pt glyph, for rows and popovers.
    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for IconButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Theme::of(cx).clone();
        let enabled = !self.disabled;
        let (side, glyph) = match self.small {
            true => (size::CONTROL_SMALL, size::ICON_SMALL),
            false => (size::ICON_BUTTON, size::ICON),
        };
        let (hover, pressed, focus) = (t.fill_hover, t.fill_pressed, t.focus_shadow());
        let key = self.id.clone();
        let name = self.accessible_label.unwrap_or_else(|| self.label.clone());
        let keys = self.command.and_then(platform::shortcut);
        let this = div()
            .id(self.id)
            .role(Role::Button)
            .aria_label(name)
            .when(self.toggle, |this| {
                this.aria_toggled(if self.on {
                    Toggled::True
                } else {
                    Toggled::False
                })
            })
            .when(!enabled, |this| this.aria_description("Unavailable"))
            .when_some(keys, |this, keys| this.aria_keyshortcuts(keys))
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .w(side)
            .h(if self.small { side } else { size::CONTROL })
            .rounded(size::RADIUS_CONTROL)
            .when(self.on, |this| this.bg(t.fill_on))
            .when(enabled, |this| {
                this.focusable()
                    .tab_stop(self.tab_stop)
                    .hover(move |s| s.bg(hover))
                    .active(move |s| s.bg(pressed))
                    .focus_visible(move |s| s.shadow(focus))
            })
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                Icon::new(self.icon)
                    .with_size(glyph)
                    .text_color(match (enabled, self.on) {
                        (false, _) => t.tertiary_label,
                        (true, true) => t.label,
                        (true, false) => t.secondary_label,
                    }),
            )
            .child(anchor_probe(key.clone()));
        anchored_tip(this, key, self.label, self.command)
            .when_some(self.on_click.filter(|_| enabled), |this, handler| {
                this.on_click(move |event, window, cx| handler(event, window, cx))
            })
    }
}

/// Borderless text that opens a popover when clicked. It stays filled while
/// the popover is open.
#[derive(IntoElement)]
pub struct PopoverTrigger {
    content: Stateful<Div>,
    open: bool,
}

impl PopoverTrigger {
    pub fn new(content: Stateful<Div>) -> Self {
        PopoverTrigger {
            content,
            open: false,
        }
    }
}

impl Selectable for PopoverTrigger {
    fn selected(mut self, selected: bool) -> Self {
        self.open = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.open
    }
}

impl RenderOnce for PopoverTrigger {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Theme::of(cx);
        let (hover, on) = (t.fill_hover, t.fill_on);
        self.content
            .px(size::TRIGGER_PADDING)
            .rounded(size::RADIUS_INLINE)
            .hover(move |s| s.bg(hover))
            .when(self.open, |this| this.bg(on))
    }
}

/// A text field (search, rename, a key): the kit's `Input` in Seer's
/// control radius, with Seer's focus ring while it has focus: the border
/// in the opaque `focus_ring` (the kit's own translucent ring is off, see
/// `theme::apply`) and a 2pt ring outside it, as on buttons. Chain the
/// kit's own options (`.small()`, `.prefix(…)`) on the result.
///
/// ```ignore
/// ui::text_field(&self.search, window, cx).prefix(ui::icon(IconName::Search, t.secondary_label))
/// ```
pub fn text_field(state: &Entity<InputState>, window: &Window, cx: &App) -> Input {
    let t = Theme::of(cx);
    let (ring, well) = (t.focus_shadow(), t.surface_content);
    let focused = state.read(cx).focus_handle(cx).is_focused(window);
    // An opaque fill while focused: the ring is a spread shadow, which
    // would show through the field's translucent fill as a solid block.
    Input::new(state)
        .rounded(size::RADIUS_CONTROL)
        .when(focused, |this| this.bg(well).shadow(ring))
}

/// Styles a kit `Popover` (or any panel) as a Seer card: the overlay
/// surface, a 1px border, card radius, the overlay shadow and pane padding.
///
/// ```ignore
/// ui::overlay_card(Popover::new("transfer").anchor(Anchor::BottomLeft), cx)
///     .trigger(ui::PopoverTrigger::new(trigger))
///     .content(|_, _, cx| …)
/// ```
pub fn overlay_card<E: Styled>(element: E, cx: &App) -> E {
    let t = Theme::of(cx);
    element
        .bg(t.surface_overlay)
        .text_color(t.label)
        .border_1()
        .border_color(t.border_overlay)
        .rounded(size::RADIUS_CARD)
        .shadow(t.overlay_shadow())
        .p(size::PANE_PADDING)
}

// ---------------------------------------------------------------------------
// Checkboxes and choices

/// The 14pt square checkbox mark of a [`Checkbox`], drawn disabled when `enabled` is false: a faded fill and a
/// `tertiary_label` edge.
fn checkbox_mark(checked: bool, enabled: bool, cx: &App) -> Div {
    let t = Theme::of(cx);
    let fade = if enabled { 1. } else { DISABLED };
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(size::CHECKBOX)
        .rounded(size::RADIUS_INLINE)
        .map(|this| match checked {
            true => this.bg(t.primary.opacity(fade)).child(
                Icon::new(IconName::Check)
                    .with_size(size::ICON_SMALL)
                    .text_color(t.on_primary),
            ),
            false => this
                .bg(t.control.opacity(fade))
                .border(size::HAIRLINE)
                .border_color(if enabled {
                    t.secondary_label
                } else {
                    t.tertiary_label
                }),
        })
}

thread_local! {
    /// Focus handles that outlive one render, for controls that move focus
    /// among their siblings (segments, sheet checkboxes). Keyed by element id.
    static SEGMENT_FOCUS: RefCell<HashMap<ElementId, FocusHandle>> = RefCell::default();
    static CHECKBOX_FOCUS: RefCell<HashMap<ElementId, FocusHandle>> = RefCell::default();
}

/// The lasting focus handle for `key` in `registry`.
fn kept_focus(
    registry: &'static LocalKey<RefCell<HashMap<ElementId, FocusHandle>>>,
    key: ElementId,
    cx: &App,
) -> FocusHandle {
    registry.with_borrow_mut(|handles| {
        handles
            .entry(key)
            .or_insert_with(|| cx.focus_handle())
            .clone()
    })
}

/// An independent on/off choice: the box, then its label. The whole row is
/// the mouse target; the box alone takes keyboard focus and draws the ring,
/// so a long label never sits inside a ring. Space toggles it when focused;
/// Return does not (in a sheet it is for the focused button). Up and Down
/// move to the previous and next checkbox in the same focus scope (a
/// sheet's rows). Text size comes from the parent.
///
/// Assistive technology reads it as a checkbox with its checked state,
/// [`Checkbox::accessible_label`] (required: the row's meaning in words,
/// such as the track and what will happen to it) and, when given,
/// [`Checkbox::accessible_description`] ("Will be removed").
#[derive(IntoElement)]
pub struct Checkbox {
    id: ElementId,
    checked: bool,
    label: Option<AnyElement>,
    accessible_label: Option<SharedString>,
    accessible_description: Option<SharedString>,
    disabled: bool,
    on_toggle: Option<ToggleHandler>,
}

impl Checkbox {
    pub fn new(id: impl Into<ElementId>, checked: bool) -> Self {
        Checkbox {
            id: id.into(),
            checked,
            label: None,
            accessible_label: None,
            accessible_description: None,
            disabled: false,
            on_toggle: None,
        }
    }

    pub fn label(mut self, label: impl IntoElement) -> Self {
        self.label = Some(label.into_any_element());
        self
    }

    /// What a screen reader says for this checkbox, beside its state: for
    /// a Clean Up row, the track, the proposed name and the reason.
    pub fn accessible_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessible_label = Some(label.into());
        self
    }

    /// What a screen reader adds after the name and state: what the checked
    /// state will do ("Will be removed").
    pub fn accessible_description(mut self, description: impl Into<SharedString>) -> Self {
        self.accessible_description = Some(description.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Called with the new state.
    pub fn on_toggle(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }
}

/// Moves focus one tab stop forward or back, and keeps it there only if it
/// landed on another checkbox. True when focus moved.
fn step_checkbox(forward: bool, window: &mut Window, cx: &mut App) -> bool {
    let Some(from) = window.focused(cx) else {
        return false;
    };
    match forward {
        true => window.focus_next(cx),
        false => window.focus_prev(cx),
    }
    let landed = window.focused(cx);
    let on_checkbox = landed.as_ref().is_some_and(|to| {
        *to != from && CHECKBOX_FOCUS.with_borrow(|handles| handles.values().any(|h| h == to))
    });
    if !on_checkbox {
        from.focus(window, cx);
    }
    on_checkbox
}

impl RenderOnce for Checkbox {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Theme::of(cx);
        let enabled = !self.disabled;
        let checked = self.checked;
        let toggle = self.on_toggle.filter(|_| enabled);
        let focus = t.focus_ring;
        let handle = kept_focus(&CHECKBOX_FOCUS, self.id.clone(), cx);
        // Every checkbox says what it is for by itself (S4-1): the visible
        // label is any element, so its words are passed separately.
        debug_assert!(
            self.accessible_label.is_some(),
            "Checkbox {:?} needs an accessible_label",
            self.id
        );
        let label = self.accessible_label;
        let description = self.accessible_description;
        // The ring: a border, not the spread shadow other controls use, on
        // a wrapper around the mark only, with a negative margin so the
        // layout is unchanged.
        let mark = div()
            .id("mark")
            .flex_none()
            .rounded(size::RADIUS_INLINE + size::FOCUS_RING)
            .border(size::FOCUS_RING)
            .border_color(gpui_kit::transparent_black())
            .m(-size::FOCUS_RING)
            .role(Role::CheckBox)
            .aria_toggled(if checked {
                Toggled::True
            } else {
                Toggled::False
            })
            .when_some(label, |this, label| this.aria_label(label))
            .when_some(description, |this, description| {
                this.aria_description(description)
            })
            .when(!enabled, |this| this.aria_description("Unavailable"))
            .when(enabled, |this| {
                this.track_focus(&handle.tab_stop(true))
                    .focus_visible(move |s| s.border_color(focus))
                    .on_key_down(|event, window, cx| {
                        let forward = match event.keystroke.key.as_str() {
                            "down" => true,
                            "up" => false,
                            _ => return,
                        };
                        if event.keystroke.modifiers.modified() {
                            return;
                        }
                        step_checkbox(forward, window, cx);
                        cx.stop_propagation();
                    })
            })
            // A click must not take focus from the list: keep the box from
            // focusing itself (its own focus listener runs after this one).
            .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
            // Space on the focused box. Mouse clicks are the row's.
            .when_some(toggle.clone(), |this, toggle| {
                this.on_click(move |event, window, cx| {
                    if let ClickEvent::Keyboard(KeyboardClickEvent {
                        button: KeyboardButton::Space,
                        ..
                    }) = event
                    {
                        toggle(!checked, window, cx);
                    }
                })
            })
            .child(checkbox_mark(checked, enabled, cx));
        div()
            .id(self.id)
            .flex()
            .items_center()
            .gap(size::GAP)
            .min_w_0()
            .when(!enabled, |this| this.text_color(t.tertiary_label))
            // Clicking must not take focus from the list.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(mark)
            .children(self.label)
            .when_some(toggle, |this, toggle| {
                this.on_click(move |event, window, cx| {
                    if let ClickEvent::Mouse(_) = event {
                        toggle(!checked, window, cx);
                    }
                })
            })
    }
}

/// Two or more mutually exclusive choices, as in an Xcode inspector. One
/// tab stop: the chosen segment. Left and Right choose the neighbouring
/// segment and move focus with it. Read as a radio group.
pub fn segmented<T: Copy + PartialEq + 'static>(
    id: &'static str,
    items: &[(T, &'static str)],
    selected: T,
    on_select: impl Fn(T, &mut Window, &mut App) + Clone + 'static,
    cx: &App,
) -> Div {
    segments(id, items, selected, on_select, false, cx)
}

/// [`segmented`] for a set of tabs (the inspector's): the same control and
/// keys, read as a tab list whose chosen tab is selected.
pub fn tabs<T: Copy + PartialEq + 'static>(
    id: &'static str,
    items: &[(T, &'static str)],
    selected: T,
    on_select: impl Fn(T, &mut Window, &mut App) + Clone + 'static,
    cx: &App,
) -> Div {
    segments(id, items, selected, on_select, true, cx)
}

fn segments<T: Copy + PartialEq + 'static>(
    id: &'static str,
    items: &[(T, &'static str)],
    selected: T,
    on_select: impl Fn(T, &mut Window, &mut App) + Clone + 'static,
    tabs: bool,
    cx: &App,
) -> Div {
    let t = Theme::of(cx).clone();
    let segment_inset = size::GAP_TINY;
    let values: Rc<Vec<T>> = Rc::new(items.iter().map(|(value, _)| *value).collect());
    let handles: Rc<Vec<FocusHandle>> = Rc::new(
        (0..items.len())
            .map(|i| kept_focus(&SEGMENT_FOCUS, (id, i).into(), cx))
            .collect(),
    );
    let count = items.len();
    // With no chosen segment among the items, the first takes the tab stop.
    let stop = values.iter().position(|v| *v == selected).unwrap_or(0);
    let group = div()
        .id(id)
        .role(if tabs {
            Role::TabList
        } else {
            Role::RadioGroup
        })
        .flex()
        .flex_1()
        .children(items.iter().enumerate().map(|(i, (value, label))| {
            let (value, on_select) = (*value, on_select.clone());
            let active = value == selected;
            let focus = t.focus_shadow();
            let (values, handles) = (values.clone(), handles.clone());
            let step = on_select.clone();
            div()
                .id((id, i))
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .rounded(size::RADIUS_INLINE)
                .track_focus(&handles[i].clone().tab_stop(i == stop))
                .focus_visible(move |s| s.shadow(focus))
                .map(|this| match tabs {
                    true => this.role(Role::Tab).aria_selected(active),
                    false => this.role(Role::RadioButton).aria_toggled(if active {
                        Toggled::True
                    } else {
                        Toggled::False
                    }),
                })
                .aria_label(*label)
                .aria_position_in_set(i + 1)
                .aria_size_of_set(count)
                .text_color(if active { t.label } else { t.secondary_label })
                .when(active, |this| {
                    this.bg(t.thumb)
                        .font_weight(FontWeight::MEDIUM)
                        .shadow(t.thumb_shadow())
                })
                // Half the usual hover, so a hovered segment never reads as
                // a second chosen one.
                .when(!active, |this| {
                    let hover = t.fill_hover.opacity(0.5);
                    this.hover(move |s| s.bg(hover))
                })
                .on_key_down(move |event, window, cx| {
                    if event.keystroke.modifiers.modified() {
                        return;
                    }
                    let to = match event.keystroke.key.as_str() {
                        "left" => i.checked_sub(1),
                        "right" => Some(i + 1).filter(|&j| j < values.len()),
                        _ => return,
                    };
                    cx.stop_propagation();
                    if let Some(to) = to {
                        handles[to].focus(window, cx);
                        step(values[to], window, cx);
                    }
                })
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(move |_, window, cx| on_select(value, window, cx))
                .child(*label)
        }));
    div()
        .flex()
        .p(segment_inset)
        .h(size::SEGMENTED)
        .rounded(size::RADIUS_CONTROL)
        .bg(t.fill)
        .text_size(text::SMALL)
        .child(group)
}

// ---------------------------------------------------------------------------
// Rows and data

/// The label column for an inspector `pane_width` wide: `FIELD_LABEL`, or
/// `FIELD_LABEL_NARROW` below `INSPECTOR_NARROW`.
pub fn field_rail(pane_width: Pixels) -> Pixels {
    match pane_width < size::INSPECTOR_NARROW {
        true => size::FIELD_LABEL_NARROW,
        false => size::FIELD_LABEL,
    }
}

/// Inspector row: a left-aligned label in `secondary_label`, in a column
/// `rail` wide (see [`field_rail`]), then its value in `label`, on one left
/// edge. Every inspector row uses it. The value is any element: text, a
/// value plus a small action, two-tone text.
pub fn field_with_rail(
    label: impl Into<SharedString>,
    value: impl IntoElement,
    rail: Pixels,
    cx: &App,
) -> Div {
    let t = Theme::of(cx);
    div()
        .flex()
        .items_start()
        .gap(size::GAP)
        .min_h(size::INSPECTOR_ROW)
        .py(size::FIELD_PADDING)
        .text_size(text::SMALL)
        .line_height(text::SMALL_LINE)
        .child(
            div()
                .w(rail)
                .flex_none()
                .truncate()
                .text_color(t.secondary_label)
                .child(label.into()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_color(t.label)
                .font_features(theme::tabular())
                .child(value),
        )
}

/// Determinate progress: a 4pt accent bar on a faint track.
pub fn progress_bar(fraction: f32, cx: &App) -> Div {
    let t = Theme::of(cx);
    div().h(size::BAR).w_full().rounded_full().bg(t.fill).child(
        div()
            .h_full()
            .rounded_full()
            .bg(t.primary)
            .w(relative(fraction.clamp(0., 1.))),
    )
}

/// A stream's share of a file: a 6pt bar in one of the data hues
/// (`Theme::data_video`, `Theme::data_audio`). A non-zero share is never
/// thinner than 2pt.
pub fn share_bar(fraction: f32, color: Hsla, cx: &App) -> Div {
    let t = Theme::of(cx);
    let fraction = fraction.clamp(0., 1.);
    div()
        .h(size::BAR_THICK)
        .w_full()
        .rounded_full()
        .bg(t.fill)
        .child(
            div()
                .h_full()
                .rounded_full()
                .bg(color)
                .w(relative(fraction))
                .when(fraction > 0., |this| this.min_w(size::BAR_MIN)),
        )
}

// ---------------------------------------------------------------------------
// Navigation

/// One step of a [`Breadcrumb`].
pub struct Crumb {
    pub label: SharedString,
    pub on_click: Option<ClickHandler>,
}

impl Crumb {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Crumb {
            label: label.into(),
            on_click: None,
        }
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

/// Where you are: the current folder as a [`title`], and before it its
/// parent in `secondary_label`. Earlier ancestors fold into a "…" menu
/// button ("Parent folders") that lists them nearest first; the parent folds
/// too when `max_width` leaves no room for it. Each parent is capped at
/// `size::CRUMB_MAX` and the current folder at the room left, truncated in
/// the middle with the full name in a tooltip.
///
/// ```ignore
/// ui::Breadcrumb::new("location", crumbs).max_width(available)
/// ```
#[derive(IntoElement)]
pub struct Breadcrumb {
    id: ElementId,
    crumbs: Vec<Crumb>,
    max_width: Option<Pixels>,
}

impl Breadcrumb {
    pub fn new(id: impl Into<ElementId>, crumbs: Vec<Crumb>) -> Self {
        Breadcrumb {
            id: id.into(),
            crumbs,
            max_width: None,
        }
    }

    /// The width the trail may take. Without it the parent always shows.
    pub fn max_width(mut self, width: Pixels) -> Self {
        self.max_width = Some(width);
        self
    }
}

/// Width of `content` set at `font_size` and `weight` in the UI font.
pub fn text_width(
    content: &SharedString,
    font_size: Pixels,
    weight: FontWeight,
    window: &Window,
) -> Pixels {
    let mut face = font(theme::font_in_use());
    face.weight = weight;
    let run = TextRun {
        len: content.len(),
        font: face,
        color: Hsla::default(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    window
        .text_system()
        .shape_line(content.clone(), font_size, &[run], None)
        .width
}

/// How many of `parents` (counted from the nearest) a breadcrumb shows, given
/// each parent's capped width, the current folder's width and the room. At
/// most one: earlier ancestors always fold. `step` is what each crumb adds
/// besides its text (padding, chevron, gaps); `overflow` is what the "…"
/// button costs with its chevron.
fn parents_shown(
    parents: &[Pixels],
    current: Pixels,
    max: Option<Pixels>,
    step: Pixels,
    overflow: Pixels,
) -> usize {
    let Some(&nearest) = parents.last() else {
        return 0;
    };
    let Some(max) = max else {
        return 1;
    };
    // The current folder keeps at least a crumb's width before the parent
    // is given any.
    let current = current.min(size::CRUMB_MAX);
    let folded = if parents.len() > 1 { overflow } else { px(0.) };
    let with_parent = current + nearest.min(size::CRUMB_MAX) + step + folded;
    usize::from(with_parent <= max)
}

impl RenderOnce for Breadcrumb {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Theme::of(cx).clone();
        let mut crumbs = self.crumbs;
        let Some(current) = crumbs.pop() else {
            return div().into_any_element();
        };
        // Each crumb's text is padded by GAP_SMALL on both sides and followed
        // by a gap, a chevron and a gap.
        let step = size::GAP_SMALL * 4. + size::ICON_SMALL;
        let overflow = size::ICON_BUTTON + size::GAP_SMALL * 2. + size::ICON_SMALL;
        let widths: Vec<Pixels> = crumbs
            .iter()
            .map(|c| text_width(&c.label, text::BODY, FontWeight::NORMAL, window))
            .collect();
        let current_width = text_width(&current.label, text::TITLE, FontWeight::SEMIBOLD, window)
            + size::GAP_SMALL * 2.;
        let shown = parents_shown(&widths, current_width, self.max_width, step, overflow);
        let hidden: Vec<Crumb> = crumbs.drain(..crumbs.len() - shown).collect();
        let parent_width = widths.last().copied().filter(|_| shown > 0);

        let room_for_current = self.max_width.map(|max| {
            let mut used = px(0.);
            if !hidden.is_empty() {
                used += overflow;
            }
            if let Some(w) = parent_width {
                used += w.min(size::CRUMB_MAX) + step;
            }
            max - used
        });
        let current_cut = room_for_current.is_some_and(|room| current_width > room);

        let chevron = || {
            Icon::new(IconName::ChevronRight)
                .with_size(size::ICON_SMALL)
                .text_color(t.tertiary_label)
        };

        div()
            .id(self.id)
            .role(Role::Navigation)
            .aria_label("Location")
            .flex()
            .items_center()
            .min_w_0()
            .gap(size::GAP_SMALL)
            .text_size(text::BODY)
            .line_height(text::BODY_LINE)
            .when(!hidden.is_empty(), |this| {
                // Nearest first, as the Mac's own path menus list them.
                let hidden: Rc<Vec<(SharedString, Option<ClickHandler>)>> = Rc::new(
                    hidden
                        .into_iter()
                        .rev()
                        .map(|crumb| (crumb.label, crumb.on_click))
                        .collect(),
                );
                let menu = MenuButton::glyph("parent-folders", "…", "Parent folders").menu(
                    move |menu, _, _| {
                        hidden.iter().fold(menu, |menu, (label, on_click)| {
                            let on_click = on_click.clone();
                            menu.item(PopupMenuItem::new(label.clone()).on_click(
                                move |event, window, cx| {
                                    if let Some(handler) = &on_click {
                                        handler(event, window, cx);
                                    }
                                },
                            ))
                        })
                    },
                );
                this.child(menu).child(chevron())
            })
            .children(
                crumbs
                    .into_iter()
                    .zip(widths.iter().rev())
                    .map(|(crumb, &width)| {
                        let (hover, ink, focus) = (t.fill_hover, t.label, t.focus_shadow());
                        let cut = width > size::CRUMB_MAX - size::GAP_SMALL * 2.;
                        let label = crumb.label.clone();
                        let name = crumb.label.clone();
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .gap(size::GAP_SMALL)
                            .child(
                                div()
                                    .id("crumb-parent")
                                    .role(Role::Link)
                                    .aria_label(name)
                                    .relative()
                                    .flex_none()
                                    .max_w(size::CRUMB_MAX)
                                    .px(size::GAP_SMALL)
                                    .rounded(size::RADIUS_INLINE)
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis_middle()
                                    .text_color(t.secondary_label)
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                        cx.stop_propagation()
                                    })
                                    .when(cut, |this| {
                                        let key = ElementId::Name("crumb-parent".into());
                                        anchored_tip(this, key.clone(), label, None)
                                            .child(anchor_probe(key))
                                    })
                                    .when_some(crumb.on_click, |this, handler| {
                                        this.focusable()
                                            .tab_stop(true)
                                            .focus_visible(move |s| s.shadow(focus.clone()))
                                            .hover(move |s| s.bg(hover).text_color(ink))
                                            .on_click(move |event, window, cx| {
                                                handler(event, window, cx)
                                            })
                                    })
                                    .child(crumb.label),
                            )
                            .child(chevron())
                    }),
            )
            .child(
                div()
                    .id("crumb-current")
                    .role(Role::Heading)
                    .aria_label(current.label.clone())
                    .relative()
                    .min_w_0()
                    .px(size::GAP_SMALL)
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis_middle()
                    .text_size(text::TITLE)
                    .line_height(text::TITLE_LINE)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(t.label)
                    .when(current_cut, |this| {
                        let key = ElementId::Name("crumb-current".into());
                        anchored_tip(this, key.clone(), current.label.clone(), None)
                            .child(anchor_probe(key))
                    })
                    .child(current.label),
            )
            .into_any_element()
    }
}

// ---------------------------------------------------------------------------
// Menus

/// What a [`MenuButton`] shows.
enum MenuGlyph {
    Icon(IconName),
    Text(SharedString),
}

/// A borderless 28pt button that opens a menu: the "Main Menu" button on
/// Windows and GNOME, the breadcrumb's "Parent folders". Hover, pressed and
/// on fills like an [`IconButton`]; a focus ring; Return or Space opens the
/// menu; its tooltip is its name, shown only while the menu is closed. Its
/// accessible name is its name, open or not.
///
/// To open it from a key (F10, a bare Alt), give it a handle with
/// `.track_focus(&handle)` and call [`open_menu`] with that handle.
///
/// ```ignore
/// ui::MenuButton::new("main-menu", IconName::Menu, "Main Menu")
///     .track_focus(&self.main_menu_focus)
///     .menu(move |menu, window, cx| {
///         ui::command_menu_with(menu, Some(list.clone()), availability, window, cx)
///     })
/// ```
#[derive(IntoElement)]
pub struct MenuButton {
    base: Stateful<Div>,
    glyph: MenuGlyph,
    label: SharedString,
    open: bool,
}

impl MenuButton {
    pub fn new(id: impl Into<ElementId>, icon: IconName, label: impl Into<SharedString>) -> Self {
        MenuButton {
            base: div().id(id),
            glyph: MenuGlyph::Icon(icon),
            label: label.into(),
            open: false,
        }
    }

    /// A button showing one text character instead of an icon ("…").
    pub fn glyph(
        id: impl Into<ElementId>,
        glyph: impl Into<SharedString>,
        label: impl Into<SharedString>,
    ) -> Self {
        MenuButton {
            base: div().id(id),
            glyph: MenuGlyph::Text(glyph.into()),
            label: label.into(),
            open: false,
        }
    }

    /// The menu, opened below the button's leading edge.
    pub fn menu(
        self,
        builder: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
    ) -> impl IntoElement {
        self.dropdown_menu_with_anchor(Anchor::TopLeft, builder)
    }

    /// The menu, opened below the button's trailing edge (a button at the
    /// trailing end of the titlebar).
    pub fn menu_from_end(
        self,
        builder: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
    ) -> impl IntoElement {
        self.dropdown_menu_with_anchor(Anchor::TopRight, builder)
    }
}

impl Styled for MenuButton {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl InteractiveElement for MenuButton {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl Selectable for MenuButton {
    fn selected(mut self, selected: bool) -> Self {
        self.open = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.open
    }
}

impl DropdownMenu for MenuButton {}

impl RenderOnce for MenuButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Theme::of(cx).clone();
        let (hover, pressed, focus) = (t.fill_hover, t.fill_pressed, t.focus_shadow());
        let ink = if self.open {
            t.label
        } else {
            t.secondary_label
        };
        // The tooltip's anchor is keyed by the label: the base's id is not
        // reachable once set.
        let key = ElementId::Name(format!("menu-button:{}", self.label).into());
        // No stop_propagation on mouse down: the menu opens from the press,
        // and it stops the press from dragging the window itself.
        self.base
            .relative()
            .child(anchor_probe(key.clone()))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .w(size::ICON_BUTTON)
            .h(size::CONTROL)
            .rounded(size::RADIUS_CONTROL)
            .focusable()
            .tab_stop(true)
            .hover(move |s| s.bg(hover))
            .active(move |s| s.bg(pressed))
            .focus_visible(move |s| s.shadow(focus))
            .when(self.open, |this| this.bg(t.fill_on))
            .text_color(ink)
            .map(|this| match self.glyph {
                MenuGlyph::Icon(name) => {
                    this.child(Icon::new(name).with_size(size::ICON).text_color(ink))
                }
                MenuGlyph::Text(text) => this
                    .text_size(text::BODY)
                    .line_height(text::BODY_LINE)
                    .child(text),
            })
            .role(Role::Button)
            .aria_label(self.label.clone())
            // A button that opens a menu says whether the menu is open.
            .aria_expanded(self.open)
            // No tooltip over its own open menu.
            .when(!self.open, |this| anchored_tip(this, key, self.label, None))
    }
}

/// Opens (or, when open, closes) the menu of the [`MenuButton`] whose
/// focus handle is `trigger`, as Return on the focused button would: for
/// F10 and a bare Alt on Windows and GNOME. The press goes to the button
/// without focusing it, so the menu remembers where focus really was (the
/// search field, the list) and Escape returns it there, not to the button.
pub fn open_menu(trigger: &FocusHandle, window: &mut Window, cx: &mut App) {
    trigger.dispatch_action(
        &gpui_kit::base::actions::Confirm { secondary: false },
        window,
        cx,
    );
}

/// A [`KeyMenu`]'s open menu and where its top-left corner is.
type OpenMenu = (Entity<PopupMenu>, Point<Pixels>);

/// A context menu opened from the keyboard: Shift+F10 or the Menu key on
/// Windows and GNOME, ⌃↩ on the Mac (`Command::ContextMenu`). The kit's
/// context menus open from the pointer only; this opens the same menu,
/// built by the same function, where the view says (under the focused row).
/// Keep one in the view, call [`KeyMenu::open`] from the command, and add
/// [`KeyMenu::render`] as a child every frame. Escape or an item closes it;
/// focus then goes back to the menu's action context.
#[derive(Default)]
pub struct KeyMenu {
    open: Rc<RefCell<Option<OpenMenu>>>,
    _dismissed: Option<Subscription>,
}

impl KeyMenu {
    /// Builds the menu and shows it with its top-left corner at `at`, kept
    /// inside the window, with keyboard focus in it.
    pub fn open(
        &mut self,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
        build: impl FnOnce(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu,
    ) {
        let menu = PopupMenu::build(window, cx, build);
        let open = Rc::downgrade(&self.open);
        self._dismissed =
            Some(
                window.subscribe(&menu, cx, move |_, _: &DismissEvent, window, _| {
                    if let Some(open) = open.upgrade() {
                        open.borrow_mut().take();
                    }
                    window.refresh();
                }),
            );
        menu.focus_handle(cx).focus(window, cx);
        *self.open.borrow_mut() = Some((menu, at));
        window.refresh();
    }

    /// The menu, while it is open.
    pub fn menu(&self) -> Option<Entity<PopupMenu>> {
        self.open.borrow().as_ref().map(|(menu, _)| menu.clone())
    }

    /// The open menu, drawn above everything else; `None` when closed.
    pub fn render(&self) -> Option<AnyElement> {
        let (menu, at) = self.open.borrow().clone()?;
        Some(
            deferred(
                anchored()
                    .position(at)
                    .snap_to_window_with_margin(size::GAP)
                    .child(menu),
            )
            .with_priority(gpui_kit::base::POPUP_PRIORITY)
            .into_any_element(),
        )
    }
}

/// A context-menu item in Seer's type: the label in `label`, the command's
/// shortcut at the trailing edge in `secondary_label` with tabular
/// figures, both in `tertiary_label` when disabled. Clicking dispatches
/// `action` to the menu's action context. Chain `.checked(on)` for a
/// toggle.
///
/// ```ignore
/// menu.item(ui::menu_item(platform::label(Command::Rename), Box::new(Rename), Some(Command::Rename), !single))
/// ```
pub fn menu_item(
    label: impl Into<SharedString>,
    action: Box<dyn Action>,
    command: Option<Command>,
    disabled: bool,
) -> PopupMenuItem {
    let label = label.into();
    let shortcut = command.and_then(platform::shortcut);
    PopupMenuItem::element(move |_, cx| {
        let t = Theme::of(cx);
        let (ink, hint) = match disabled {
            true => (t.tertiary_label, t.tertiary_label),
            false => (t.label, t.secondary_label),
        };
        div()
            .flex()
            .flex_1()
            .items_center()
            .justify_between()
            .gap(size::GAP_GROUP)
            .text_size(text::BODY)
            .line_height(text::BODY_LINE)
            .whitespace_nowrap()
            .text_color(ink)
            .child(label.clone())
            .when_some(shortcut.clone(), |this, keys| {
                this.child(
                    div()
                        .flex_none()
                        .text_color(hint)
                        .font_features(theme::tabular())
                        .child(keys),
                )
            })
    })
    .action(action)
    .disabled(disabled)
}

/// Fills `menu` with every command, one submenu per group of the command
/// table (`actions::command_menus`), then About Seer
/// (`actions::main_menu_extras`), for the "Main Menu" button on desktops
/// without a menu bar. Each item is enabled and checked from
/// `availability`, the same state the context menu and the Mac menu bar
/// follow; build it when the menu opens. Commands go to `target` (pass the
/// file list's focus handle so its commands reach it), else to wherever
/// focus was.
pub fn command_menu_with(
    menu: PopupMenu,
    target: Option<FocusHandle>,
    availability: actions::Availability,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let menu = match target {
        Some(handle) => menu.action_context(handle),
        None => menu,
    };
    let item = move |entry: &actions::Entry| match *entry {
        actions::Entry::Command {
            label,
            action,
            command,
            needs,
            check,
        } => Some(
            menu_item(label, action(), command, !availability.allows(needs))
                .checked(check.is_some_and(|toggle| availability.checked(toggle))),
        ),
        actions::Entry::Separator | actions::Entry::Services => None,
    };
    let menu = actions::command_menus()
        .into_iter()
        .fold(menu, |menu, group| {
            let entries = Rc::new(group.entries);
            menu.submenu(group.name, window, cx, move |sub, _, _| {
                entries
                    .iter()
                    .fold(sub, |sub, entry| match (entry, item(entry)) {
                        (_, Some(command)) => sub.item(command),
                        (actions::Entry::Separator, None) => sub.separator(),
                        _ => sub,
                    })
            })
        });
    let extras = actions::main_menu_extras();
    match extras.is_empty() {
        true => menu,
        false => extras
            .iter()
            .filter_map(item)
            .fold(menu.separator(), |menu, command| menu.item(command)),
    }
}

// ---------------------------------------------------------------------------
// Surfaces

/// The glyph above an [`EmptyState`]'s title.
enum EmptyGlyph {
    Icon(IconName),
    /// Seer's own mark: a small bit-rate trace ([`trace_glyph`]).
    Trace,
}

/// What a pane shows when it has nothing to show: an optional glyph, a
/// title, an optional detail line and an optional action (a [`Button`]:
/// Regular under the main pane's [`display`] title, Small in a side pane).
/// The main pane centres it; a side pane uses [`EmptyState::compact`], a
/// short prompt at the top. An empty folder and "No matches" use Seer's
/// trace glyph ([`EmptyState::trace`]); states about something else (an
/// error, a copy arriving) keep an icon, in `secondary_label`.
#[derive(IntoElement)]
pub struct EmptyState {
    glyph: Option<EmptyGlyph>,
    title: SharedString,
    detail: Option<SharedString>,
    action: Option<AnyElement>,
    compact: bool,
}

impl EmptyState {
    pub fn new(title: impl Into<SharedString>) -> Self {
        EmptyState {
            glyph: None,
            title: title.into(),
            detail: None,
            action: None,
            compact: false,
        }
    }

    pub fn glyph(mut self, glyph: IconName) -> Self {
        self.glyph = Some(EmptyGlyph::Icon(glyph));
        self
    }

    /// Seer's own glyph, a small bit-rate trace, in place of an icon: for
    /// an empty folder and for no matches.
    pub fn trace(mut self) -> Self {
        self.glyph = Some(EmptyGlyph::Trace);
        self
    }

    /// One or two sentences. An empty string shows nothing.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        let detail = detail.into();
        self.detail = (!detail.is_empty()).then_some(detail);
        self
    }

    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.action = Some(action.into_any_element());
        self
    }

    /// Top-aligned and small, for the inspector and other side panes.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }
}

impl RenderOnce for EmptyState {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Theme::of(cx).clone();
        if self.compact {
            return div()
                .w_full()
                .flex()
                .flex_col()
                .gap(size::GAP_SMALL)
                .p(size::PANE_PADDING)
                .child(
                    div()
                        .text_size(text::BODY)
                        .line_height(text::BODY_LINE)
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(t.label)
                        .child(self.title),
                )
                .when_some(self.detail, |this, detail| {
                    this.child(
                        div()
                            .text_size(text::SMALL)
                            .line_height(text::SMALL_LINE)
                            .text_color(t.secondary_label)
                            .child(detail),
                    )
                })
                .when_some(self.action, |this, action| {
                    this.child(div().flex().pt(size::GAP_SMALL).child(action))
                });
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .px(size::EMPTY_STATE_MARGIN)
            .text_center()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .max_w(size::EMPTY_STATE_WIDTH)
                    .when_some(self.glyph, |this, glyph| {
                        this.child(match glyph {
                            // A mark to be seen, 3:1 or more like the
                            // trace beside it: never the disabled ink.
                            EmptyGlyph::Icon(name) => Icon::new(name)
                                .with_size(size::ICON_EMPTY)
                                .text_color(t.secondary_label)
                                .into_any_element(),
                            EmptyGlyph::Trace => trace_glyph(cx).into_any_element(),
                        })
                        .child(div().h(size::PANE_PADDING))
                    })
                    .child(display(self.title, cx))
                    .when_some(self.detail, |this, detail| {
                        this.child(
                            div()
                                .pt(size::GAP_SMALL)
                                .text_size(text::BODY)
                                .line_height(text::BODY_LINE)
                                .text_color(t.secondary_label)
                                .child(detail),
                        )
                    })
                    .when_some(self.action, |this, action| {
                        this.child(div().pt(size::GAP_GROUP).child(action))
                    }),
            )
    }
}

/// The trace's points, as fractions of the glyph's width and height from
/// its top-left: a quiet start, three peaks of different heights, a tail.
const TRACE: [(f32, f32); 11] = [
    (0.00, 0.70),
    (0.12, 0.62),
    (0.22, 0.68),
    (0.32, 0.18),
    (0.42, 0.56),
    (0.52, 0.46),
    (0.62, 0.64),
    (0.72, 0.32),
    (0.82, 0.58),
    (0.91, 0.50),
    (1.00, 0.60),
];

/// Seer's mark: a small bit-rate trace over a baseline, drawn with the
/// chart's stroke. The trace is `secondary_label` (at least 3:1 on every
/// surface, so it reads as a mark, not as something disabled); the baseline
/// is a `separator` rule that recedes. `size::TRACE_GLYPH` wide and
/// `size::ICON_EMPTY` tall. Used above an empty folder and "No matches", on
/// the Bit Rate tab before analysis, and in the About sheet.
pub fn trace_glyph(cx: &App) -> impl IntoElement {
    let t = Theme::of(cx);
    let (ink, rule) = (t.secondary_label, t.separator);
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let at = |(x, y): (f32, f32)| {
                point(
                    bounds.left() + bounds.size.width * x,
                    bounds.top() + bounds.size.height * y,
                )
            };
            let mut line = PathBuilder::stroke(size::CHART_STROKE);
            line.move_to(at(TRACE[0]));
            for &p in &TRACE[1..] {
                line.line_to(at(p));
            }
            if let Ok(path) = line.build() {
                window.paint_path(path, ink);
            }
            let baseline = bounds.bottom() - size::HAIRLINE;
            window.paint_quad(fill(
                Bounds::new(
                    point(bounds.left(), baseline),
                    gpui_kit::size(bounds.size.width, size::HAIRLINE),
                ),
                rule,
            ));
        },
    )
    .w(size::TRACE_GLYPH)
    .h(size::ICON_EMPTY)
}

/// The kind of item a file-list row shows, for [`kind_glyph`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    Folder,
    Video,
    Audio,
    Image,
    Text,
    Subtitle,
    Other,
}

/// The inks of a [`kind_glyph`]: its outline and its inner mark. Unselected,
/// the outline is `secondary_label`, a video's mark `data_video`, an audio
/// file's `data_audio` and every other mark neutral. On a selected row the
/// whole glyph takes the selection's ink (`selection_ink`).
pub fn glyph_inks(kind: Glyph, selection_ink: Option<Hsla>, t: &Theme) -> (Hsla, Hsla) {
    if let Some(ink) = selection_ink {
        return (ink, ink);
    }
    let mark = match kind {
        Glyph::Video => t.data_video,
        Glyph::Audio => t.data_audio,
        _ => t.secondary_label,
    };
    (t.secondary_label, mark)
}

/// Seer's file glyph for a list row: a stroked document (a folder for
/// folders) `size::ICON` square, with a small inner mark for its kind: a
/// play triangle (video), a wave (audio), lines (text), caption lines
/// (subtitles), a frame (images), nothing (others). `selected` draws it in
/// `on_selection`; a row whose selection is inactive uses
/// [`kind_glyph_in`] with `on_selection_inactive`.
pub fn kind_glyph(kind: Glyph, selected: bool, cx: &App) -> impl IntoElement {
    let ink = selected.then(|| Theme::of(cx).on_selection);
    kind_glyph_in(kind, ink, cx)
}

/// [`kind_glyph`] with an explicit selection ink: `None` on an unselected
/// row, else the row's `on_selection` or `on_selection_inactive`.
pub fn kind_glyph_in(kind: Glyph, selection_ink: Option<Hsla>, cx: &App) -> impl IntoElement {
    let (outline, mark) = glyph_inks(kind, selection_ink, Theme::of(cx));
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            paint_kind_glyph(kind, bounds, size::HAIRLINE, outline, mark, window)
        },
    )
    .size(size::ICON)
    .flex_none()
}

/// [`kind_glyph`] at the inspector header's size (`size::ICON_HEADER`),
/// stroked with the chart's line: for a folder in the header, where the
/// system's icon would be a different image on every OS and a pale one in
/// light (F4-1). Files keep their own system icon there.
pub fn kind_glyph_large(kind: Glyph, cx: &App) -> impl IntoElement {
    let (outline, mark) = glyph_inks(kind, None, Theme::of(cx));
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            paint_kind_glyph(kind, bounds, size::CHART_STROKE, outline, mark, window)
        },
    )
    .size(size::ICON_HEADER)
    .flex_none()
}

/// Draws a [`kind_glyph`] on a 16-unit grid scaled to `bounds`, its lines
/// `weight` wide. Outlines sit on half units so a one-point stroke at 16pt
/// lands on whole pixels.
fn paint_kind_glyph(
    kind: Glyph,
    bounds: Bounds<Pixels>,
    weight: Pixels,
    outline: Hsla,
    mark: Hsla,
    window: &mut Window,
) {
    let unit = bounds.size.width / 16.;
    let at = |x: f32, y: f32| point(bounds.left() + unit * x, bounds.top() + unit * y);
    let stroke = |points: &[(f32, f32)], close: bool, ink: Hsla, window: &mut Window| {
        let mut path = PathBuilder::stroke(weight);
        path.move_to(at(points[0].0, points[0].1));
        for &(x, y) in &points[1..] {
            path.line_to(at(x, y));
        }
        if close {
            path.close();
        }
        if let Ok(path) = path.build() {
            window.paint_path(path, ink);
        }
    };
    let filled = |points: &[(f32, f32)], ink: Hsla, window: &mut Window| {
        let mut path = PathBuilder::fill();
        path.move_to(at(points[0].0, points[0].1));
        for &(x, y) in &points[1..] {
            path.line_to(at(x, y));
        }
        path.close();
        if let Ok(path) = path.build() {
            window.paint_path(path, ink);
        }
    };
    let bar = |x: f32, y: f32, w: f32, ink: Hsla, window: &mut Window| {
        window.paint_quad(fill(
            Bounds::new(at(x, y), gpui_kit::size(unit * w, weight)),
            ink,
        ));
    };

    if kind == Glyph::Folder {
        // A tab at the top left, the body below it.
        stroke(
            &[
                (1.5, 3.5),
                (6.0, 3.5),
                (7.5, 5.0),
                (14.5, 5.0),
                (14.5, 13.5),
                (1.5, 13.5),
            ],
            true,
            outline,
            window,
        );
        stroke(&[(1.5, 6.5), (14.5, 6.5)], false, outline, window);
        return;
    }

    // A page with its top-right corner folded.
    stroke(
        &[
            (3.5, 1.5),
            (9.5, 1.5),
            (12.5, 4.5),
            (12.5, 14.5),
            (3.5, 14.5),
        ],
        true,
        outline,
        window,
    );
    stroke(
        &[(9.5, 1.5), (9.5, 4.5), (12.5, 4.5)],
        false,
        outline,
        window,
    );

    match kind {
        Glyph::Video => filled(&[(6.5, 7.0), (6.5, 12.0), (10.5, 9.5)], mark, window),
        Glyph::Audio => stroke(
            &[
                (5.5, 9.5),
                (6.5, 7.5),
                (7.5, 11.5),
                (8.5, 6.5),
                (9.5, 12.0),
                (10.5, 9.5),
            ],
            false,
            mark,
            window,
        ),
        Glyph::Text => {
            bar(5.5, 7.0, 5.0, mark, window);
            bar(5.5, 9.0, 5.0, mark, window);
            bar(5.5, 11.0, 3.0, mark, window);
        }
        Glyph::Subtitle => {
            // Two centred caption lines near the foot of the page.
            bar(5.5, 10.0, 5.0, mark, window);
            bar(6.5, 12.0, 3.0, mark, window);
        }
        Glyph::Image => {
            stroke(
                &[(5.5, 7.5), (10.5, 7.5), (10.5, 12.5), (5.5, 12.5)],
                true,
                mark,
                window,
            );
            filled(&[(6.0, 12.0), (8.0, 9.5), (10.0, 12.0)], mark, window);
        }
        Glyph::Folder | Glyph::Other => {}
    }
}

/// A task on the current window: a panel on the overlay surface, centred
/// over a scrim that covers the whole window. Width `min(640, window − 48)`,
/// height to content up to 560, then the body scrolls. Header: a
/// [`title`] and one status line, with an optional accessory (a mode
/// switch) at the trailing edge. Footer: a note at the leading edge, the
/// buttons at the trailing edge, exactly one Primary.
///
/// A sheet with no [`Sheet::body`] is compact: 420 wide, no separator and
/// no footer band; its buttons sit under the status line. Use it for a
/// result or a question.
///
/// Keyboard: Escape calls `on_dismiss`. Tab and Shift-Tab move between the
/// sheet's controls and never leave it. Focus starts on the sheet's first
/// control: pass the owner's handle to [`Sheet::focus`] and focus it when
/// the sheet opens (or when its content is replaced, as when a scan
/// finishes); the sheet moves focus on to its first control. It does the
/// same when the focused control goes away. A focused button takes Return
/// and Space itself, so Return applies a change only from the button that
/// makes it; a checkbox takes Space only. `on_confirm` runs on Return only
/// in a compact sheet (a result or a question) whose own handle holds
/// focus. When the sheet closes, focus the list again.
///
/// Render it as the last child of the window's root element.
///
/// ```ignore
/// ui::Sheet::new("clean-up", "Clean Up Stream Names")
///     .status("Checked 7 videos. Every stream name is tidy.")
///     .focus(&self.focus)
///     .button(ui::Button::new("done", "Done").primary().on_click(…))
///     .on_dismiss(close.clone())
///     .on_confirm(close)
/// ```
#[derive(IntoElement)]
pub struct Sheet {
    id: ElementId,
    title: SharedString,
    status: Option<SharedString>,
    accessory: Option<AnyElement>,
    accessory_stops: usize,
    body: Option<AnyElement>,
    bleed: bool,
    note: Option<AnyElement>,
    buttons: Vec<AnyElement>,
    focus: Option<FocusHandle>,
    on_dismiss: Option<Handler>,
    on_confirm: Option<Handler>,
}

impl Sheet {
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Sheet {
            id: id.into(),
            title: title.into(),
            status: None,
            accessory: None,
            accessory_stops: 0,
            body: None,
            bleed: false,
            note: None,
            buttons: Vec::new(),
            focus: None,
            on_dismiss: None,
            on_confirm: None,
        }
    }

    /// One line under the title: what was found, what will happen.
    pub fn status(mut self, status: impl Into<SharedString>) -> Self {
        self.status = Some(status.into());
        self
    }

    /// A control at the header's trailing edge, such as a mode switch.
    pub fn accessory(mut self, accessory: impl IntoElement) -> Self {
        self.accessory = Some(accessory.into_any_element());
        self
    }

    /// How many tab stops the accessory holds. The sheet opens with focus
    /// on the first control after them, so a mode switch in the header does
    /// not take the opening focus from the body.
    pub fn accessory_stops(mut self, stops: usize) -> Self {
        self.accessory_stops = stops;
        self
    }

    /// The scrolling content. Padded to the sheet's margins unless
    /// [`Sheet::bleed`] is set.
    pub fn body(mut self, body: impl IntoElement) -> Self {
        self.body = Some(body.into_any_element());
        self
    }

    /// Lets the body run edge to edge (a list with its own row insets).
    pub fn bleed(mut self) -> Self {
        self.bleed = true;
        self
    }

    /// Text or a consequence at the footer's leading edge.
    pub fn note(mut self, note: impl IntoElement) -> Self {
        self.note = Some(note.into_any_element());
        self
    }

    /// A footer button. Add them in reading order; the Primary goes last.
    pub fn button(mut self, button: impl IntoElement) -> Self {
        self.buttons.push(button.into_any_element());
        self
    }

    pub fn focus(mut self, handle: &FocusHandle) -> Self {
        self.focus = Some(handle.clone());
        self
    }

    pub fn on_dismiss(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(handler));
        self
    }

    pub fn on_confirm(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_confirm = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Sheet {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Theme::of(cx).clone();
        let viewport = window.viewport_size();
        let gutter = size::SHEET_GUTTER;
        let compact = self.body.is_none();
        let preferred = match compact {
            true => size::SHEET_WIDTH_COMPACT,
            false => size::SHEET_WIDTH,
        };
        let width = preferred.min(viewport.width - gutter * 2.);
        let height = size::SHEET_MAX_HEIGHT.min(viewport.height - gutter * 2.);
        let margin = size::SHEET_PADDING;
        let (dismiss, confirm) = (self.on_dismiss, self.on_confirm);
        let own_focus = self.focus.clone();
        let trap = self.focus.clone();
        // Just opened (the owner focused the sheet), or the focused control
        // is gone: move focus to the first control once this frame is
        // drawn, so its tab stops are known.
        if let Some(handle) = &self.focus
            && (handle.is_focused(window) || !handle.contains_focused(window, cx))
        {
            let handle = handle.clone();
            let skip = self.accessory_stops;
            window.defer(cx, move |window, cx| {
                focus_first_control(&handle, skip, window, cx)
            });
        }

        // Read as a dialog named by its title, its status line after it.
        let (name, about) = (self.title.clone(), self.status.clone());
        let sheet = div()
            .id(self.id)
            .role(Role::Dialog)
            .aria_label(name)
            .when_some(about, |this, about| this.aria_description(about))
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .bg(t.scrim)
            .font_family(t.font.clone())
            .when_some(self.focus, |this, focus| this.track_focus(&focus))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_key_down(move |event, window, cx| {
                let keys = &event.keystroke;
                if keys.modifiers.modified() {
                    return;
                }
                // A control with focus takes Return and Space itself (GPUI
                // clicks it on key up). The default action is only for a
                // compact sheet whose own handle holds focus: a sheet with a
                // body changes things only from its focused button.
                let sheet_focused = own_focus
                    .as_ref()
                    .is_none_or(|sheet| sheet.is_focused(window));
                let handler = match keys.key.as_str() {
                    "escape" => dismiss.as_ref(),
                    "enter" if compact && sheet_focused => confirm.as_ref(),
                    _ => None,
                };
                if let Some(handler) = handler {
                    cx.stop_propagation();
                    handler(window, cx);
                }
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w(width)
                    .max_h(height)
                    .overflow_hidden()
                    .bg(t.surface_overlay)
                    .border_1()
                    .border_color(t.border_overlay)
                    .rounded(size::RADIUS_OVERLAY)
                    .shadow(t.overlay_shadow())
                    .text_size(text::BODY)
                    .line_height(text::BODY_LINE)
                    .text_color(t.label)
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .items_start()
                            .gap(size::PANE_PADDING)
                            .px(margin)
                            .pt(margin)
                            .pb(size::PANE_PADDING)
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_w_0()
                                    .gap(size::GAP_TINY)
                                    .child(title(self.title, cx))
                                    .when_some(self.status, |this, status| {
                                        this.child(
                                            div()
                                                .text_size(text::SMALL)
                                                .line_height(text::SMALL_LINE)
                                                .text_color(t.secondary_label)
                                                .child(status),
                                        )
                                    }),
                            )
                            .when_some(self.accessory, |this, accessory| {
                                this.child(div().flex_none().child(accessory))
                            }),
                    )
                    .when_some(self.body, |this, body| {
                        this.child(
                            div()
                                .id("sheet-body")
                                .flex_1()
                                .min_h_0()
                                .overflow_y_scroll()
                                .when(!self.bleed, |this| this.px(margin))
                                .pb(size::PANE_PADDING)
                                .child(body),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .gap(size::GAP)
                            .px(margin)
                            .map(|this| match compact {
                                // Under the status line, no band of its own.
                                true => this.pt(size::GAP_SMALL).pb(margin),
                                false => this
                                    .py(size::PANE_PADDING)
                                    .border_t(size::HAIRLINE)
                                    .border_color(t.separator),
                            })
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(text::SMALL)
                                    .line_height(text::SMALL_LINE)
                                    .text_color(t.secondary_label)
                                    .children(self.note),
                            )
                            .children(self.buttons),
                    ),
            );
        // Tab and Shift-Tab cycle inside the sheet: the kit's root moves
        // focus, and a trap keeps it within this container.
        match trap {
            Some(handle) => sheet.focus_trap("sheet-trap", &handle).into_any_element(),
            None => sheet.into_any_element(),
        }
    }
}

/// Moves focus from the sheet `container` to its first tab stop. The
/// sheet is the last thing drawn, so its controls follow everything else in
/// the tab order; the walk stops at the first stop inside it, or, if a full
/// cycle finds none, leaves focus on the sheet itself.
fn focus_first_control(container: &FocusHandle, skip: usize, window: &mut Window, cx: &mut App) {
    let settled = |window: &Window, cx: &App| {
        container.contains_focused(window, cx) && !container.is_focused(window)
    };
    if settled(window, cx) {
        return;
    }
    container.focus(window, cx);
    let mut visited = Vec::new();
    loop {
        window.focus_next(cx);
        if settled(window, cx) {
            // Past the header's accessory, as long as that stays inside.
            for _ in 0..skip {
                let before = window.focused(cx);
                window.focus_next(cx);
                if !settled(window, cx) {
                    if let Some(before) = before {
                        before.focus(window, cx);
                    }
                    break;
                }
            }
            return;
        }
        let focused = window.focused(cx);
        if visited.contains(&focused) {
            container.focus(window, cx);
            return;
        }
        visited.push(focused);
    }
}

/// Stand-in window controls, drawn only when `SEER_PLATFORM` presents
/// another OS on the Mac (the system draws the real ones everywhere else).
/// Absolutely positioned at the control side of its parent, which must be
/// `relative()` and as tall as `platform::chrome().titlebar`. `None` when
/// the system draws them.
pub fn window_controls(window: &Window, cx: &App) -> Option<AnyElement> {
    if !platform::simulated() {
        return None;
    }
    let chrome = platform::chrome();
    let t = Theme::of(cx).clone();
    let windows = platform::os() == platform::Os::Windows;
    let buttons: &[(&'static str, IconName)] = if windows {
        &[
            ("caption-minimize", IconName::WindowMinimize),
            ("caption-maximize", IconName::WindowMaximize),
            ("caption-close", IconName::WindowClose),
        ]
    } else {
        &[("caption-close", IconName::WindowClose)]
    };
    let count = buttons.len() as f32;
    let width = chrome.controls_width / count;
    let active = window.is_window_active();
    let row = div()
        .absolute()
        .top_0()
        .h(chrome.titlebar)
        .flex()
        .items_center()
        .when(!active, |this| this.opacity(INACTIVE))
        .map(|this| match chrome.controls {
            Side::Leading => this.left_0(),
            Side::Trailing => this.right_0(),
        })
        .children(buttons.iter().map(|&(id, glyph)| {
            let hover = if id == "caption-close" {
                t.red
            } else {
                t.fill_hover
            };
            div()
                .id(id)
                .role(Role::Button)
                .aria_label(match id {
                    "caption-close" => "Close",
                    "caption-maximize" => "Maximize",
                    _ => "Minimize",
                })
                .flex()
                .items_center()
                .justify_center()
                .h_full()
                .w(width)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .hover(move |s| s.bg(hover))
                .on_click(move |_, window, _| match id {
                    "caption-close" => window.remove_window(),
                    "caption-maximize" => window.zoom_window(),
                    _ => window.minimize_window(),
                })
                .child(icon(glyph, t.label))
        }));
    Some(row.into_any_element())
}

#[cfg(test)]
mod tests {
    // Not a glob: gpui's own `test` attribute would shadow the standard one.
    use super::{Pixels, parents_shown, px, size};

    const STEP: Pixels = px(28.);
    const OVERFLOW: Pixels = px(52.);

    #[test]
    fn kind_glyphs_reach_three_to_one_on_every_row() {
        use super::{Glyph, glyph_inks};
        use crate::theme::{NON_TEXT, Theme, contrast, ink_on, legible_fill, over};
        use gpui_kit::{Hsla, WindowAppearance, rgb};
        let kinds = [
            Glyph::Folder,
            Glyph::Video,
            Glyph::Audio,
            Glyph::Image,
            Glyph::Text,
            Glyph::Subtitle,
            Glyph::Other,
        ];
        for appearance in [WindowAppearance::Light, WindowAppearance::Dark] {
            let t = Theme::resolve(appearance);
            let opaque = |c: Hsla| -> Hsla { over(c, t.surface_content).into() };
            // Plain, striped and hovered rows.
            let rows = [
                t.surface_content,
                opaque(t.alternate_row),
                opaque(t.fill_hover),
            ];
            for kind in kinds {
                let (outline, mark) = glyph_inks(kind, None, &t);
                for row in rows {
                    for ink in [outline, mark] {
                        let r = contrast(ink, row);
                        assert!(r >= NON_TEXT, "{kind:?} dark={}: {r}", t.dark);
                    }
                }
                // Selected rows, focused and not, in this Mac's accent.
                for (ink, fill) in [
                    (t.on_selection, t.selection),
                    (t.on_selection_inactive, t.selection_inactive),
                ] {
                    let (outline, mark) = glyph_inks(kind, Some(ink), &t);
                    assert!(contrast(outline, fill) >= NON_TEXT, "{kind:?}");
                    assert!(contrast(mark, fill) >= NON_TEXT, "{kind:?}");
                }
            }
            // Selected rows in other accents: graphite, the blues, a pale one.
            for accent in [0x8C8C8C, 0x5A5A5A, 0x0063E1, 0x0059D1, 0xFFC0CB] {
                let fill = legible_fill(rgb(accent).into());
                let ink = ink_on(fill).0;
                let (outline, mark) = glyph_inks(Glyph::Video, Some(ink), &t);
                assert!(contrast(outline, fill) >= NON_TEXT, "{accent:06X}");
                assert!(contrast(mark, fill) >= NON_TEXT, "{accent:06X}");
            }
        }
    }

    #[test]
    fn earlier_ancestors_always_fold() {
        let parents = [px(60.), px(70.), px(80.)];
        // However much room there is, only the parent shows.
        assert_eq!(parents_shown(&parents, px(90.), None, STEP, OVERFLOW), 1);
        assert_eq!(
            parents_shown(&parents, px(90.), Some(px(2000.)), STEP, OVERFLOW),
            1
        );
        assert_eq!(
            parents_shown(&[], px(90.), Some(px(2000.)), STEP, OVERFLOW),
            0
        );
    }

    #[test]
    fn the_parent_folds_before_the_current_folder_is_squeezed() {
        let parents = [px(60.), px(300.)];
        // A long parent counts as one capped crumb.
        let needed = px(90.) + size::CRUMB_MAX + STEP + OVERFLOW;
        assert_eq!(
            parents_shown(&parents, px(90.), Some(needed), STEP, OVERFLOW),
            1
        );
        let short = needed - px(1.);
        assert_eq!(
            parents_shown(&parents, px(90.), Some(short), STEP, OVERFLOW),
            0
        );
        // With only a parent there is no "…" button to pay for.
        let alone = px(90.) + px(60.) + STEP;
        assert_eq!(
            parents_shown(&[px(60.)], px(90.), Some(alone), STEP, OVERFLOW),
            1
        );
    }
}
