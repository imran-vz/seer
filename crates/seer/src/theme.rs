//! Every size and colour in the app. See DESIGN.md for where the values come
//! from. Views never use literals; they ask here.

use crate::native;
use gpui_kit::{App, FontFeatures, Global, Hsla, Pixels, WindowAppearance, hsla, px, rgb};
use std::sync::Arc;

/// Sizes in points.
pub mod size {
    use super::{Pixels, px};

    pub const TITLEBAR: Pixels = px(40.);
    /// Space kept clear for the traffic lights.
    pub const TRAFFIC_LIGHTS: Pixels = px(78.);
    pub const STATUS_BAR: Pixels = px(24.);

    pub const ROW: Pixels = px(24.);
    pub const HEADER_ROW: Pixels = px(28.);
    pub const SIDEBAR_ROW: Pixels = px(28.);
    pub const INSPECTOR_ROW: Pixels = px(20.);

    pub const CONTROL: Pixels = px(24.);
    pub const ICON: Pixels = px(16.);
    pub const RADIUS: Pixels = px(6.);

    /// Gap between related things, inset of a row's content, padding of a pane.
    pub const GAP: Pixels = px(8.);
    pub const ROW_INSET: Pixels = px(8.);
    pub const PANE_PADDING: Pixels = px(12.);

    pub const SIDEBAR: (Pixels, Pixels, Pixels) = (px(160.), px(200.), px(320.));
    pub const INSPECTOR: (Pixels, Pixels, Pixels) = (px(260.), px(300.), px(420.));
    pub const CONTENT_MIN: Pixels = px(320.);
    pub const WINDOW_MIN: (Pixels, Pixels) = (px(760.), px(440.));
    pub const DIVIDER_HIT: Pixels = px(8.);
}

/// Text sizes in points. Line heights follow the macOS text styles.
pub mod text {
    use super::{Pixels, px};

    pub const BODY: Pixels = px(13.);
    pub const BODY_LINE: Pixels = px(16.);
    pub const SMALL: Pixels = px(11.);
    pub const SMALL_LINE: Pixels = px(14.);
    pub const CAPTION: Pixels = px(10.);
    pub const CAPTION_LINE: Pixels = px(13.);
}

pub const FONT: &str = ".SystemUIFont";

/// Monospaced digits, for every number shown in a column.
pub fn tabular() -> FontFeatures {
    FontFeatures(Arc::new(vec![("tnum".into(), 1)]))
}

#[derive(Clone, Debug)]
pub struct Theme {
    pub dark: bool,
    pub label: Hsla,
    pub secondary_label: Hsla,
    pub tertiary_label: Hsla,
    pub separator: Hsla,
    /// Faint fill for hovered icon buttons, selected sidebar rows, tracks.
    pub fill: Hsla,
    pub content: Hsla,
    pub alternate_row: Hsla,
    /// Tint laid over the window's blur behind the sidebar.
    pub sidebar: Hsla,
    pub selection: Hsla,
    pub selection_inactive: Hsla,
    pub on_selection: Hsla,
    pub on_selection_secondary: Hsla,
    pub accent: Hsla,
    pub red: Hsla,
}

impl Global for Theme {}

impl Theme {
    pub fn of(cx: &App) -> &Theme {
        cx.global::<Theme>()
    }

    pub fn resolve(appearance: WindowAppearance) -> Theme {
        let dark = matches!(
            appearance,
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        );
        let system = native::system_colors(dark);
        let ink = |alpha: f32| {
            if dark {
                hsla(0., 0., 1., alpha)
            } else {
                hsla(0., 0., 0., alpha)
            }
        };
        Theme {
            dark,
            label: ink(0.85),
            secondary_label: ink(if dark { 0.55 } else { 0.50 }),
            tertiary_label: ink(if dark { 0.25 } else { 0.26 }),
            separator: ink(if dark { 0.12 } else { 0.10 }),
            fill: ink(if dark { 0.10 } else { 0.07 }),
            content: rgb(if dark { 0x1E1E1E } else { 0xFFFFFF }).into(),
            alternate_row: if dark {
                hsla(0., 0., 1., 0.05)
            } else {
                rgb(0xF4F5F5).into()
            },
            sidebar: if dark {
                hsla(0., 0., 0.14, 0.9)
            } else {
                hsla(0., 0., 0.94, 0.9)
            },
            selection: system.selection,
            selection_inactive: rgb(if dark { 0x464646 } else { 0xDCDCDC }).into(),
            on_selection: hsla(0., 0., 1., 1.),
            on_selection_secondary: hsla(0., 0., 1., 0.7),
            accent: system.accent,
            red: rgb(if dark { 0xFF4245 } else { 0xFF383C }).into(),
        }
    }
}
