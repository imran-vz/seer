//! The app's few shared controls. One of each; see DESIGN.md principle 7.

use crate::theme::{self, Theme, size, text};
use gpui_kit::assets::IconName;
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::prelude::*;
use gpui_kit::*;

/// One device pixel, so separators stay crisp on Retina displays.
pub fn hairline(window: &Window) -> Pixels {
    px(1.0 / window.scale_factor())
}

pub fn icon(name: IconName, color: Hsla) -> Icon {
    Icon::new(name).with_size(size::ICON).text_color(color)
}

/// Borderless toolbar button. Attach `on_click` at the call site.
pub fn icon_button(id: &'static str, name: IconName, enabled: bool, cx: &App) -> Stateful<Div> {
    let t = Theme::of(cx);
    let (color, fill) = (
        if enabled {
            t.secondary_label
        } else {
            t.tertiary_label
        },
        t.fill,
    );
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .w(px(28.))
        .h(size::CONTROL)
        .rounded(size::RADIUS)
        .when(enabled, |this| {
            this.hover(|s| s.bg(fill)).active(|s| s.opacity(0.7))
        })
        // A press on a control must not start dragging the window.
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(icon(name, color))
}

/// Bezelled push button. `primary` marks the default action.
pub fn button(
    id: &'static str,
    label: impl Into<SharedString>,
    primary: bool,
    cx: &App,
) -> Stateful<Div> {
    let t = Theme::of(cx);
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .h(size::CONTROL)
        .px(px(12.))
        .rounded(size::RADIUS)
        .text_size(text::BODY)
        .map(|this| match primary {
            true => this.bg(t.accent).text_color(t.on_selection),
            false => this.bg(t.fill).text_color(t.label),
        })
        .active(|s| s.opacity(0.75))
        .child(label.into())
}

/// Small bezelled button for secondary actions inside a pane. `active`
/// shows that its choice is the one in effect.
pub fn small_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    active: bool,
    cx: &App,
) -> Stateful<Div> {
    let t = Theme::of(cx);
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .h(px(20.))
        .px(px(8.))
        .rounded(px(5.))
        .text_size(text::SMALL)
        .map(|this| match active {
            true => this.bg(t.accent).text_color(t.on_selection),
            false => this.bg(t.fill).text_color(t.label),
        })
        .active(|s| s.opacity(0.75))
        .child(label.into())
}

/// A button that cannot be pressed right now. It keeps its place and label.
pub fn disabled_button(label: impl Into<SharedString>, cx: &App) -> Div {
    let t = Theme::of(cx);
    div()
        .flex()
        .flex_none()
        .items_center()
        .h(size::CONTROL)
        .px(px(12.))
        .rounded(size::RADIUS)
        .text_size(text::BODY)
        .bg(t.fill)
        .text_color(t.tertiary_label)
        .child(label.into())
}

/// A 14pt checkbox. The row it sits in handles the click.
pub fn checkbox(checked: bool, cx: &App) -> Div {
    let t = Theme::of(cx);
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(14.))
        .rounded(px(3.5))
        .map(|this| match checked {
            true => this
                .bg(t.accent)
                .child(icon(IconName::Check, t.on_selection).size(px(11.))),
            false => this.border_1().border_color(t.tertiary_label),
        })
}

/// Marks a row for removal: an empty ring until chosen, then a red minus.
/// The selected state names the action, so it cannot be read as "keep".
pub fn removal_mark(marked: bool, cx: &App) -> Div {
    let t = Theme::of(cx);
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(14.))
        .rounded_full()
        .map(|this| match marked {
            true => this
                .bg(t.red)
                .child(icon(IconName::Minus, t.on_selection).size(px(10.))),
            false => this.border_1().border_color(t.tertiary_label),
        })
}

/// Small semibold heading above a group of rows.
pub fn section_header(title: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_size(text::SMALL)
        .line_height(text::SMALL_LINE)
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(Theme::of(cx).secondary_label)
        .child(title.into())
}

/// Inspector row: a right-aligned label and its value.
pub fn field(label: &'static str, value: impl Into<SharedString>, cx: &App) -> Div {
    let t = Theme::of(cx);
    div()
        .flex()
        .items_start()
        .gap(size::GAP)
        .min_h(size::INSPECTOR_ROW)
        .py(px(3.))
        .text_size(text::SMALL)
        .line_height(text::SMALL_LINE)
        .child(
            div()
                .w(px(76.))
                .flex_none()
                .text_right()
                .text_color(t.secondary_label)
                .child(label),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_color(t.label)
                .font_features(theme::tabular())
                .child(value.into()),
        )
}

pub fn progress_bar(fraction: f32, cx: &App) -> Div {
    let t = Theme::of(cx);
    div().h(px(4.)).w_full().rounded_full().bg(t.fill).child(
        div()
            .h_full()
            .rounded_full()
            .bg(t.accent)
            .w(relative(fraction.clamp(0., 1.))),
    )
}

/// Centred message for a pane with nothing to show.
pub fn empty_state(
    title: impl Into<SharedString>,
    detail: impl Into<SharedString>,
    cx: &App,
) -> Div {
    let t = Theme::of(cx);
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(4.))
        .px(px(24.))
        .text_center()
        .child(
            div()
                .text_size(text::BODY)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t.secondary_label)
                .child(title.into()),
        )
        .child(
            div()
                .text_size(text::SMALL)
                .line_height(text::SMALL_LINE)
                .text_color(t.tertiary_label)
                .child(detail.into()),
        )
}

/// Two or more mutually exclusive choices, as in an Xcode inspector.
pub fn segmented<T: Copy + PartialEq + 'static>(
    id: &'static str,
    items: &[(T, &'static str)],
    selected: T,
    on_select: impl Fn(T, &mut Window, &mut App) + Clone + 'static,
    cx: &App,
) -> Div {
    let t = Theme::of(cx);
    div()
        .flex()
        .p(px(2.))
        .h(size::CONTROL)
        .rounded(size::RADIUS)
        .bg(t.fill)
        .text_size(text::SMALL)
        .children(items.iter().enumerate().map(|(i, (value, label))| {
            let (value, on_select) = (*value, on_select.clone());
            let active = value == selected;
            div()
                .id((id, i))
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(4.))
                .text_color(if active { t.label } else { t.secondary_label })
                .when(active, |this| {
                    this.bg(if t.dark {
                        hsla(0., 0., 1., 0.16)
                    } else {
                        hsla(0., 0., 1., 1.)
                    })
                    .shadow_sm()
                })
                .on_click(move |_, window, cx| on_select(value, window, cx))
                .child(*label)
        }))
}
