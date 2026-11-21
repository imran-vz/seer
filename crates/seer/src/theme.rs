//! Every size and colour in the app. See DESIGN.md for where the values come
//! from. Views never use literals; they ask here.

use crate::{native, platform};
use gpui_kit::{
    App, BoxShadow, FontFeatures, Global, Hsla, Pixels, Rgba, SharedString, Window,
    WindowAppearance, hsla, px, rgb,
};
use std::sync::Arc;

/// The type scale of the OS this binary is built for. Text sizes and the
/// list row are constants, so they follow the build target, not
/// `SEER_PLATFORM`: a simulated Windows layout on the Mac keeps the Mac's
/// 13pt type (see DESIGN.md, Platforms).
const SCALE: platform::TypeScale = platform::scale(platform::TARGET);

/// Sizes in points.
pub mod size {
    use super::{Pixels, SCALE, px};

    /// Every separator and border. One point, never half.
    pub const HAIRLINE: Pixels = px(1.);
    pub const STATUS_BAR: Pixels = px(24.);

    /// A list row: 24pt on the Mac, 28 where the UI text is 14pt.
    pub const ROW: Pixels = px(SCALE.row);
    pub const HEADER_ROW: Pixels = px(28.);
    /// A sidebar row: 28pt on the Mac, 32 where a list row is 28.
    pub const SIDEBAR_ROW: Pixels = px(SCALE.sidebar_row);
    pub const INSPECTOR_ROW: Pixels = px(20.);

    /// Small button (inspector, popovers, empty states, sheet rows): 22pt
    /// on the Mac, 24 around 12pt text.
    pub const CONTROL_SMALL: Pixels = px(SCALE.control_small);
    /// Regular button, icon button and search field (titlebar, sheet footer).
    pub const CONTROL: Pixels = px(28.);
    /// Segmented control track.
    pub const SEGMENTED: Pixels = px(24.);
    /// Width of an icon-only button; its height is `CONTROL`.
    pub const ICON_BUTTON: Pixels = px(28.);
    pub const ICON: Pixels = px(16.);
    /// Glyphs inside small controls: sort arrows, chevrons, checkmarks.
    pub const ICON_SMALL: Pixels = px(12.);
    /// The glyph above an empty-state title.
    pub const ICON_EMPTY: Pixels = px(28.);
    /// Width of Seer's trace glyph above an empty-state title (`ui::trace_glyph`).
    pub const TRACE_GLYPH: Pixels = px(40.);
    pub const CHECKBOX: Pixels = px(14.);
    /// Height of a progress bar and of a stream-share bar.
    pub const BAR: Pixels = px(4.);
    pub const BAR_THICK: Pixels = px(6.);
    /// Smallest visible width of a non-zero share bar.
    pub const BAR_MIN: Pixels = px(2.);

    /// Radius scale. Inline marks (checkbox, segment, tag), controls
    /// (buttons, fields, selection), cards (popovers, tooltips' parent
    /// panels, chart), overlays (sheets).
    pub const RADIUS_INLINE: Pixels = px(4.);
    pub const RADIUS_CONTROL: Pixels = px(6.);
    pub const RADIUS_CARD: Pixels = px(10.);
    pub const RADIUS_OVERLAY: Pixels = px(14.);

    /// Gap between related things, inset of a row's content, padding of a pane.
    pub const GAP: Pixels = px(8.);
    /// Gap inside a control, between its glyph and its label.
    pub const GAP_SMALL: Pixels = px(4.);
    /// Between a title and its status line; inset of a segmented track.
    pub const GAP_TINY: Pixels = px(2.);
    /// Vertical padding of an inspector field (20pt row, 14pt line).
    pub const FIELD_PADDING: Pixels = px(3.);
    /// Horizontal padding of borderless text that opens a popover.
    pub const TRIGGER_PADDING: Pixels = px(6.);
    /// Side margin of a centred empty state.
    pub const EMPTY_STATE_MARGIN: Pixels = px(24.);
    /// Side margin of a sheet's header, body and footer.
    pub const SHEET_PADDING: Pixels = px(20.);
    /// Gap between groups (sidebar sections, inspector sections).
    pub const GAP_GROUP: Pixels = px(16.);
    pub const ROW_INSET: Pixels = px(8.);
    pub const PANE_PADDING: Pixels = px(12.);
    /// Horizontal padding inside a Small and a Regular button.
    pub const BUTTON_PADDING_SMALL: Pixels = px(8.);
    pub const BUTTON_PADDING: Pixels = px(12.);
    /// Keyboard focus ring, drawn outside the control.
    pub const FOCUS_RING: Pixels = px(2.);

    /// Table columns: the gap between columns, and the wider gap before a
    /// left-aligned column that follows a right-aligned (numeric) one.
    pub const COLUMN_GAP: Pixels = px(16.);
    pub const COLUMN_GAP_WIDE: Pixels = px(24.);
    /// Widest the Name column grows; slack goes after the last column.
    pub const NAME_MAX: Pixels = px(440.);
    /// Narrowest the Name column gets, and the width pane fitting assumes.
    pub const NAME_MIN: Pixels = px(200.);
    /// Room after the widest visible name: Name is that wide plus this,
    /// clamped to `NAME_MIN`..`NAME_MAX`.
    pub const NAME_PADDING: Pixels = px(24.);
    /// Width of each file-list column beside Name, each fitting the widest
    /// value it shows: "1440p ProRes", "5.1 E-AC-3 +2", "1:23:45",
    /// "1023.9 MB", "12 Nov 2026 at 14:32".
    pub const COLUMN_VIDEO: Pixels = px(112.);
    pub const COLUMN_AUDIO: Pixels = px(104.);
    pub const COLUMN_DURATION: Pixels = px(64.);
    pub const COLUMN_SIZE: Pixels = px(72.);
    pub const COLUMN_MODIFIED: Pixels = px(148.);
    /// Narrowest the file list gets while a side pane is open: its row
    /// insets (32), a `NAME_MIN` Name, then Video (128), Duration (80) and
    /// Size (88). Below this the sidebar closes, then the inspector.
    pub const LIST_MIN: Pixels = px(528.);

    /// The search field in the titlebar, and its width when the titlebar is
    /// tight. Tighter still it becomes an icon button.
    pub const SEARCH_WIDTH: Pixels = px(180.);
    pub const SEARCH_WIDTH_COMPACT: Pixels = px(144.);
    /// Room the breadcrumb keeps before "Clean Up…" gives up its label.
    pub const CRUMB_ROOM: Pixels = px(200.);
    /// Widest one crumb gets; longer names truncate in the middle.
    pub const CRUMB_MAX: Pixels = px(140.);
    /// Bare titlebar always kept for dragging the window, on every platform
    /// (Windows asks for at least 48px of clear strip).
    pub const DRAG_MIN: Pixels = px(48.);

    /// The running copy or move in the status bar: its label and its bar.
    pub const TRANSFER_LABEL: Pixels = px(320.);
    pub const TRANSFER_BAR: Pixels = px(64.);
    /// Tallest the list of a transfer's items gets before it scrolls.
    pub const TRANSFER_LIST: Pixels = px(240.);

    /// Clean Up: the stream identity ("Audio 2 · AAC · Tamil") and the mode
    /// switch in the header.
    pub const ISSUE_LABEL: Pixels = px(108.);
    pub const MODE_SWITCH: Pixels = px(232.);

    /// The bitrate chart: plot height, series stroke, average line's dash.
    pub const CHART_HEIGHT: Pixels = px(168.);
    pub const CHART_STROKE: Pixels = px(1.5);
    pub const CHART_DASH: Pixels = px(3.);
    /// A legend's colour mark (share bars, chart series).
    pub const SWATCH: Pixels = px(8.);

    /// The file icon in the inspector header.
    pub const ICON_HEADER: Pixels = px(32.);

    /// Label column of an inspector field (`ui::field_with_rail`).
    pub const FIELD_LABEL: Pixels = px(72.);
    /// The same label column while the inspector is narrower than
    /// `INSPECTOR_NARROW`.
    pub const FIELD_LABEL_NARROW: Pixels = px(58.);
    pub const INSPECTOR_NARROW: Pixels = px(280.);

    /// Sheets: preferred width, the gutter kept to the window's edges, and
    /// the tallest a sheet grows before its body scrolls.
    pub const SHEET_WIDTH: Pixels = px(640.);
    /// Width of a sheet with no body (a question and its buttons).
    pub const SHEET_WIDTH_COMPACT: Pixels = px(420.);
    pub const SHEET_GUTTER: Pixels = px(24.);
    pub const SHEET_MAX_HEIGHT: Pixels = px(560.);
    /// Popovers and tooltips.
    pub const POPOVER_WIDTH: Pixels = px(340.);
    pub const TOOLTIP_MAX_WIDTH: Pixels = px(280.);
    /// Text measure of an empty state.
    pub const EMPTY_STATE_WIDTH: Pixels = px(280.);

    pub const SIDEBAR: (Pixels, Pixels, Pixels) = (px(160.), px(200.), px(320.));
    pub const INSPECTOR: (Pixels, Pixels, Pixels) = (px(260.), px(300.), px(420.));
    pub const CONTENT_MIN: Pixels = px(320.);
    pub const WINDOW_MIN: (Pixels, Pixels) = (px(760.), px(440.));
    pub const DIVIDER_HIT: Pixels = px(8.);
}

/// Text sizes in points. Line heights follow the macOS text styles; Body,
/// Small and Caption follow the build target's [`platform::TypeScale`]
/// (14 / 12 / 11 on Windows and GNOME).
pub mod text {
    use super::{Pixels, SCALE, px};

    /// Empty-state titles in the main pane. Semibold.
    pub const DISPLAY: Pixels = px(20.);
    pub const DISPLAY_LINE: Pixels = px(24.);
    /// Folder name in the titlebar, file name in the inspector header,
    /// sheet titles. Semibold. 15/20 on the Mac, 16/22 where Body is 14:
    /// always a clear step above Body.
    pub const TITLE: Pixels = px(SCALE.title);
    pub const TITLE_LINE: Pixels = px(SCALE.title_line);
    pub const BODY: Pixels = px(SCALE.body);
    pub const BODY_LINE: Pixels = px(SCALE.body_line);
    pub const SMALL: Pixels = px(SCALE.small);
    pub const SMALL_LINE: Pixels = px(SCALE.small_line);
    pub const CAPTION: Pixels = px(SCALE.caption);
    pub const CAPTION_LINE: Pixels = px(SCALE.caption_line);

    /// gpui-kit sizes its controls and menus in rems and sets body text at
    /// 0.875rem. This rem makes that `BODY` (13pt), its extra-small text
    /// `SMALL` (11pt), a menu item 24pt and a menu's inset 4pt.
    pub const KIT_REM: Pixels = px(SCALE.body / 0.875);
}

/// Timings. Everything else is instant.
pub mod motion {
    use std::time::Duration;

    /// Hover time before a tooltip appears.
    pub const TOOLTIP_DELAY: Duration = Duration::from_millis(600);
    /// How long a finished job's summary stays in the status bar.
    pub const NOTICE: Duration = Duration::from_secs(4);
}

/// The UI font family of the OS this binary is built for: the system font
/// on the Mac, Segoe UI Variable on Windows, Adwaita Sans on GNOME. A family
/// that is not installed falls back to the system's (GPUI's fallback stack,
/// which includes Segoe UI and Cantarell). [`font`] picks among the
/// presented OS's families at run time.
pub const FONT: &str = platform::fonts(platform::TARGET)[0];

/// The UI font family in use: the first of the presented OS's
/// `platform::Chrome::fonts` that is installed, else [`FONT`]. The same as
/// `Theme::font`.
pub fn font(cx: &App) -> SharedString {
    Theme::of(cx).font.clone()
}

thread_local! {
    /// [`Theme::font`], for code that measures text without an `App`.
    static FONT_IN_USE: std::cell::RefCell<SharedString> =
        const { std::cell::RefCell::new(SharedString::new_static(FONT)) };
}

/// The UI font family in use, as [`font`] gives it, without an `App`: for
/// measuring text (`ui::text_width`), so measured and drawn text agree.
pub fn font_in_use() -> SharedString {
    FONT_IN_USE.with_borrow(Clone::clone)
}

/// Resolves the presented OS's UI font against what is installed.
fn resolve_font(cx: &App) -> SharedString {
    let wanted = platform::chrome().fonts;
    // Names starting with "." are the system's own aliases, always there.
    if let Some(alias) = wanted.iter().find(|name| name.starts_with('.')) {
        return SharedString::new_static(alias);
    }
    let installed = cx.text_system().all_font_names();
    wanted
        .iter()
        .find(|name| installed.iter().any(|have| have == *name))
        .map(|name| SharedString::new_static(name))
        .unwrap_or(SharedString::new_static(FONT))
}

/// Monospaced digits, for every number shown in a column.
pub fn tabular() -> FontFeatures {
    FontFeatures(Arc::new(vec![("tnum".into(), 1)]))
}

/// Every colour in the app, resolved for one appearance. Text tokens that
/// carry information meet 4.5:1 on every surface they sit on; see the tests.
#[derive(Clone, Debug)]
pub struct Theme {
    pub dark: bool,
    /// The UI font family in use (see [`font`]).
    pub font: SharedString,

    // Text. `tertiary_label` is for disabled controls and placeholders only:
    // anything a person needs to read uses `label` or `secondary_label`.
    pub label: Hsla,
    pub secondary_label: Hsla,
    pub tertiary_label: Hsla,

    // Surfaces, from back to front.
    /// Sidebar and the titlebar strip above it. Translucent over the
    /// window's blur where the platform has one.
    pub surface_window: Hsla,
    /// The file list and the titlebar over it.
    pub surface_content: Hsla,
    /// Inspector, table header, status bar, segmented track.
    pub surface_raised: Hsla,
    /// Popovers, sheets, tooltips and menus Seer draws. Always with
    /// `border_overlay` and `Theme::overlay_shadow`.
    pub surface_overlay: Hsla,
    /// Laid over the whole window behind a sheet.
    pub scrim: Hsla,
    /// Every other row of a list on `surface_content`. Stripes end with
    /// the data: nothing is striped below the last row.
    pub alternate_row: Hsla,

    // Lines.
    /// Between panes, under the titlebar, between groups.
    pub separator: Hsla,
    /// Around controls (Secondary button, checkbox, field).
    pub border: Hsla,
    /// Around overlays.
    pub border_overlay: Hsla,

    // Fills for borderless things (icon buttons, sidebar rows, tracks).
    pub fill: Hsla,
    pub fill_hover: Hsla,
    pub fill_pressed: Hsla,
    /// An icon button that is on (its pane is open).
    pub fill_on: Hsla,

    // Bezelled controls (Secondary and Destructive buttons, segment thumb).
    pub control: Hsla,
    pub control_hover: Hsla,
    pub control_pressed: Hsla,
    /// The chosen segment of a segmented control.
    pub thumb: Hsla,

    // The default action. With an achromatic accent this is the inverted
    // label colour, so it still outranks a Secondary button.
    pub primary: Hsla,
    pub primary_hover: Hsla,
    pub primary_pressed: Hsla,
    pub on_primary: Hsla,

    // Selection. Foregrounds are opaque and resolved against the fill.
    pub selection: Hsla,
    pub on_selection: Hsla,
    pub on_selection_secondary: Hsla,
    pub selection_inactive: Hsla,
    pub on_selection_inactive: Hsla,
    pub on_selection_inactive_secondary: Hsla,

    // Accent: selection, primary button, focus, on-state toggles.
    pub accent: Hsla,
    pub focus_ring: Hsla,
    /// Selected text inside a field.
    pub text_selection: Hsla,

    // Status. Text-safe on every surface.
    pub red: Hsla,
    /// The label of a Destructive button: `red`, moved in lightness until
    /// it reads at 4.5:1 on the button's own fill.
    pub destructive: Hsla,
    pub orange: Hsla,
    pub green: Hsla,

    // Media data only: the chart series, stream-share bars and stream-kind
    // marks. Never chrome.
    pub data_video: Hsla,
    pub data_video_fill: Hsla,
    pub data_audio: Hsla,
    pub data_audio_fill: Hsla,

    /// Ink of overlay shadows.
    pub shadow: Hsla,
}

impl Global for Theme {}

/// Resolves the theme for `appearance`, installs it, and maps gpui-kit's
/// colours (fields, popovers, menus, scroll bars) from it, so kit controls
/// wear Seer's tokens. Call at launch and on every appearance change.
pub fn apply(appearance: WindowAppearance, window: Option<&mut Window>, cx: &mut App) {
    use gpui_kit::component::{Theme as Kit, ThemeMode};
    let mut t = Theme::resolve(appearance);
    t.font = resolve_font(cx);
    FONT_IN_USE.set(t.font.clone());
    if platform::chrome().scale != SCALE {
        // Text sizes are constants of the build target, so a simulated
        // layout keeps them (DESIGN.md, Platforms).
        log::debug!(
            "presenting {:?} with the build target's type scale",
            platform::os()
        );
    }
    let mode = if t.dark {
        ThemeMode::Dark
    } else {
        ThemeMode::Light
    };
    // The host's scroll-bar setting (always shown, or only while
    // scrolling), read through GPUI and applied to every kit scroll bar.
    let scroll_bars = platform::scroll_bars(cx.should_auto_hide_scrollbars());
    // Loads the kit's own theme for the mode; Seer's colours go over it in a
    // second edit, as the kit asks.
    Kit::change(mode, window, cx);
    Kit::update(cx, |kit| {
        kit.scrollbar_mode = scroll_bars;
        kit.font_family = t.font.clone();
        kit.font_size = text::KIT_REM;
        // A menu's outside corner takes the kit's one radius, so menus
        // share popovers' card corner. The kit caps its item highlights at
        // 8pt; fields take back the control radius in `ui::text_field`.
        kit.radius = size::RADIUS_CARD;
        kit.radius_lg = size::RADIUS_OVERLAY;
        // The kit's own field ring is translucent. Without it a focused
        // field's border takes the opaque ring colour, and `ui::text_field`
        // adds Seer's 2pt ring outside it, as on buttons.
        kit.focus_ring = false;
        let c = &mut kit.colors;
        c.background = t.surface_content;
        c.foreground = t.label;
        c.border = t.border;
        c.input = t.border;
        c.ring = t.focus_ring;
        c.caret = t.accent;
        c.selection = t.text_selection;
        c.muted = t.fill;
        // Placeholders ("Search Movies") say what a field acts on, so they
        // are read, not decoration: secondary, not tertiary.
        c.muted_foreground = t.secondary_label;
        c.popover = t.surface_overlay;
        c.popover_foreground = t.label;
        c.accent = t.fill_hover;
        c.accent_foreground = t.label;
        c.list_hover = t.fill_hover;
        c.list_active = t.selection;
        c.secondary = t.control;
        c.secondary_hover = t.control_hover;
        c.secondary_active = t.control_pressed;
        c.secondary_foreground = t.label;
        c.primary = t.primary;
        c.primary_hover = t.primary_hover;
        c.primary_active = t.primary_pressed;
        c.primary_foreground = t.on_primary;
        c.danger = t.red;
    });
    cx.set_global(t);
}

/// Follows a change to the host's scroll-bar setting (System Settings ›
/// Appearance › Show scroll bars on the Mac) without re-resolving the
/// colours. Cheap; the window calls it when it becomes active.
pub fn sync_scroll_bars(cx: &mut App) {
    use gpui_kit::component::Theme as Kit;
    let mode = platform::scroll_bars(cx.should_auto_hide_scrollbars());
    if Kit::global(cx).scrollbar_mode != mode {
        Kit::set_scrollbar_mode(mode, cx);
    }
}

impl Theme {
    pub fn of(cx: &App) -> &Theme {
        cx.global::<Theme>()
    }

    /// The shadow under every overlay (popover, sheet, tooltip).
    pub fn overlay_shadow(&self) -> Vec<BoxShadow> {
        // GPUI's blur radius is the gaussian's deviation: half CSS's.
        vec![
            BoxShadow::new(px(0.), px(8.), self.shadow)
                .blur_radius(px(12.))
                .spread_radius(px(-2.)),
            BoxShadow::new(px(0.), px(1.), self.shadow.opacity(0.6))
                .blur_radius(px(1.5))
                .spread_radius(px(0.)),
        ]
    }

    /// The small lift under a segmented control's chosen segment.
    pub fn thumb_shadow(&self) -> Vec<BoxShadow> {
        vec![
            BoxShadow::new(px(0.), px(1.), self.shadow.opacity(0.5))
                .blur_radius(px(1.))
                .spread_radius(px(0.)),
        ]
    }

    /// A focus ring outside a control, for `focus_visible`.
    pub fn focus_shadow(&self) -> Vec<BoxShadow> {
        vec![
            BoxShadow::new(px(0.), px(0.), self.focus_ring)
                .blur_radius(px(0.))
                .spread_radius(size::FOCUS_RING),
        ]
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
        let hex = |light: u32, dark_value: u32| -> Hsla {
            rgb(if dark { dark_value } else { light }).into()
        };

        let surface_content = hex(CONTENT.0, CONTENT.1);
        let translucent = platform::chrome().translucent_sidebar;
        let surface_window = hex(0xF2F2F0, 0x161617).opacity(if translucent { 0.94 } else { 1. });

        let selection = legible_fill(system.selection);
        let (on_selection, on_selection_secondary) = ink_on(selection);
        let selection_inactive = hex(0xDCDCDC, 0x464646);
        let (on_selection_inactive, on_selection_inactive_secondary) = ink_on(selection_inactive);

        let accent = system.accent;
        let primary = if accent.s < 0.1 {
            // Graphite and other greys: invert the label colour.
            hex(0x262626, 0xE8E8E8)
        } else {
            legible_fill(accent)
        };
        let on_primary = ink_on(primary).0;
        let shift = |c: Hsla, by: f32| Hsla {
            l: (c.l + by).clamp(0., 1.),
            ..c
        };
        let (hover, pressed) = if dark { (0.05, -0.05) } else { (-0.05, -0.10) };
        // An inverted primary is light in dark mode: hover darkens it.
        let primary_hover = shift(primary, if primary.l > 0.5 { -0.05 } else { hover });
        let primary_pressed = shift(primary, if primary.l > 0.5 { -0.10 } else { pressed });

        let (label, secondary_label, tertiary_label) = text_inks(dark);
        let surface_raised = hex(0xF7F7F5, 0x232326);
        let surface_overlay = hex(0xFFFFFF, 0x2B2B2F);
        let alternate_row = hex(0xF5F5F4, 0x222224);
        let opaque_window = Hsla {
            a: 1.,
            ..surface_window
        };

        let red = hex(0xD70015, 0xFF6961);
        let control = if dark {
            hsla(0., 0., 1., 0.09)
        } else {
            hsla(0., 0., 1., 1.)
        };
        let control_hover = if dark {
            hsla(0., 0., 1., 0.13)
        } else {
            rgb(0xF4F4F4).into()
        };
        // A Destructive button's fill over every surface it can sit on.
        let destructive_fills: Vec<Hsla> = [surface_content, surface_raised, surface_overlay]
            .into_iter()
            .flat_map(|surface| [control, control_hover].map(|fill| over(fill, surface).into()))
            .collect();

        Theme {
            dark,
            font: SharedString::new_static(FONT),
            label,
            secondary_label,
            tertiary_label,

            surface_window,
            surface_content,
            surface_raised,
            surface_overlay,
            scrim: hsla(0., 0., 0., if dark { 0.45 } else { 0.22 }),
            alternate_row,

            separator: ink(if dark { 0.09 } else { 0.08 }),
            border: ink(if dark { 0.14 } else { 0.13 }),
            border_overlay: ink(0.10),

            fill: ink(if dark { 0.08 } else { 0.05 }),
            fill_hover: ink(if dark { 0.10 } else { 0.06 }),
            fill_pressed: ink(if dark { 0.16 } else { 0.10 }),
            fill_on: ink(if dark { 0.14 } else { 0.09 }),

            control,
            control_hover,
            control_pressed: if dark {
                hsla(0., 0., 1., 0.05)
            } else {
                rgb(0xEAEAEA).into()
            },

            thumb: if dark {
                hsla(0., 0., 1., 0.18)
            } else {
                hsla(0., 0., 1., 1.)
            },

            primary,
            primary_hover,
            primary_pressed,
            on_primary,

            selection,
            on_selection,
            on_selection_secondary,
            selection_inactive,
            on_selection_inactive,
            on_selection_inactive_secondary,

            accent,
            focus_ring: ring_on(
                accent,
                &[
                    surface_content,
                    surface_raised,
                    surface_overlay,
                    opaque_window,
                ],
                dark,
            ),
            text_selection: accent.opacity(if dark { 0.40 } else { 0.28 }),

            red,
            destructive: readable_on(red, &destructive_fills, dark),
            orange: hex(0xB25000, 0xFFA142),
            green: hex(0x1E7F37, 0x4CD964),

            data_video: hex(0x0E756E, 0x3FB8AF),
            data_video_fill: hex(0x0E756E, 0x3FB8AF).opacity(if dark { 0.20 } else { 0.14 }),
            data_audio: hex(0xA86B0B, 0xE0A33E),
            data_audio_fill: hex(0xA86B0B, 0xE0A33E).opacity(if dark { 0.20 } else { 0.14 }),

            shadow: hsla(0., 0., 0., if dark { 0.40 } else { 0.14 }),
        }
    }
}

/// Lowest contrast for text a person has to read (WCAG AA, small text).
pub const READABLE: f32 = 4.5;

/// Lowest contrast for a mark that is not text: the focus ring, a
/// control's edge (WCAG 1.4.11).
pub const NON_TEXT: f32 = 3.0;

/// `surface_content` in light and dark.
const CONTENT: (u32, u32) = (0xFFFFFF, 0x1C1C1E);

/// `label`, `secondary_label` and `tertiary_label`: opaque greys, resolved
/// over `surface_content` from black (light) or white (dark) at these
/// alphas. GPUI keeps a text run's base alpha when another run highlights
/// part of it, so translucent inks made two-tone text render as one tone.
/// Selected rows never use them: they take `on_selection*` inks.
pub fn text_inks(dark: bool) -> (Hsla, Hsla, Hsla) {
    let (base, surface, alphas) = if dark {
        (hsla(0., 0., 1., 1.), CONTENT.1, [0.87, 0.62, 0.36])
    } else {
        (hsla(0., 0., 0., 1.), CONTENT.0, [0.85, 0.60, 0.38])
    };
    let surface: Hsla = rgb(surface).into();
    let resolve = |a: f32| -> Hsla { over(base.opacity(a), surface).into() };
    (resolve(alphas[0]), resolve(alphas[1]), resolve(alphas[2]))
}

/// Contrast the primary ink on a fill must reach, so a secondary ink can sit
/// between it and the fill and still be readable.
const FILL_TARGET: f32 = 5.6;

/// `fg` composited over the opaque `bg`.
pub fn over(fg: Hsla, bg: Hsla) -> Rgba {
    let (f, b) = (Rgba::from(fg), Rgba::from(bg));
    let a = fg.a;
    Rgba {
        r: f.r * a + b.r * (1. - a),
        g: f.g * a + b.g * (1. - a),
        b: f.b * a + b.b * (1. - a),
        a: 1.,
    }
}

fn luminance(c: Rgba) -> f32 {
    let channel = |v: f32| {
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(c.r) + 0.7152 * channel(c.g) + 0.0722 * channel(c.b)
}

/// WCAG contrast ratio of `fg` (composited) on the opaque `bg`.
pub fn contrast(fg: Hsla, bg: Hsla) -> f32 {
    let (a, b) = (luminance(over(fg, bg)), luminance(over(bg, bg)));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

fn white() -> Hsla {
    hsla(0., 0., 1., 1.)
}

/// Near-black ink, for light fills.
fn black() -> Hsla {
    hsla(0., 0., 0.08, 1.)
}

/// Keeps the system's hue but moves the fill's lightness, as little as
/// needed, until white (preferred, as the Mac draws it) or near-black reads
/// on it at `FILL_TARGET`.
pub fn legible_fill(fill: Hsla) -> Hsla {
    let fill = Hsla { a: 1., ..fill };
    if contrast(white(), fill) >= FILL_TARGET || contrast(black(), fill) >= FILL_TARGET {
        return fill;
    }
    // Darken for white ink, up to 0.2 of lightness; then lighten for black.
    for step in 1..=20 {
        let darker = Hsla {
            l: (fill.l - step as f32 * 0.01).max(0.),
            ..fill
        };
        if contrast(white(), darker) >= FILL_TARGET {
            return darker;
        }
    }
    for step in 1..=40 {
        let lighter = Hsla {
            l: (fill.l + step as f32 * 0.01).min(1.),
            ..fill
        };
        if contrast(black(), lighter) >= FILL_TARGET {
            return lighter;
        }
    }
    fill
}

/// The focus ring: the accent's hue, moved in lightness (darker in light
/// mode, lighter in dark) until it reaches `NON_TEXT` against every surface
/// it can sit next to. Opaque, so a grey (Graphite) accent still shows.
pub fn ring_on(accent: Hsla, surfaces: &[Hsla], dark: bool) -> Hsla {
    let accent = Hsla { a: 1., ..accent };
    let worst = |c: Hsla| {
        surfaces
            .iter()
            .map(|&s| contrast(c, Hsla { a: 1., ..s }))
            .fold(f32::MAX, f32::min)
    };
    let target = NON_TEXT + 0.1;
    for step in 0..=60 {
        let by = step as f32 * 0.01;
        let candidate = Hsla {
            l: if dark {
                (accent.l + by).min(1.)
            } else {
                (accent.l - by).max(0.)
            },
            ..accent
        };
        if worst(candidate) >= target {
            return candidate;
        }
    }
    accent
}

/// `ink`, moved in lightness (lighter in dark mode, darker in light) as
/// little as needed to read at `READABLE` on every opaque fill in `fills`.
/// Keeps the hue, so a red stays red.
pub fn readable_on(ink: Hsla, fills: &[Hsla], dark: bool) -> Hsla {
    let ink = Hsla { a: 1., ..ink };
    let worst = |c: Hsla| {
        fills
            .iter()
            .map(|&f| contrast(c, Hsla { a: 1., ..f }))
            .fold(f32::MAX, f32::min)
    };
    let target = READABLE + 0.05;
    for step in 0..=60 {
        let by = step as f32 * 0.01;
        let candidate = Hsla {
            l: if dark {
                (ink.l + by).min(1.)
            } else {
                (ink.l - by).max(0.)
            },
            ..ink
        };
        if worst(candidate) >= target {
            return candidate;
        }
    }
    ink
}

/// Opaque primary and secondary inks for text on `fill`: white or
/// near-black, whichever reads better, and a secondary that sits as far
/// toward the fill as it can while staying readable.
pub fn ink_on(fill: Hsla) -> (Hsla, Hsla) {
    let fill = Hsla { a: 1., ..fill };
    let primary = if contrast(white(), fill) >= contrast(black(), fill) {
        white()
    } else {
        black()
    };
    let mut secondary = primary;
    for step in 1..=30 {
        let candidate = mix(primary, fill, step as f32 / 100.);
        if contrast(candidate, fill) < READABLE + 0.1 {
            break;
        }
        secondary = candidate;
    }
    (primary, secondary)
}

/// `a` moved `amount` of the way toward `b`, opaque.
fn mix(a: Hsla, b: Hsla, amount: f32) -> Hsla {
    let (a, b) = (Rgba::from(a), Rgba::from(b));
    let t = amount.clamp(0., 1.);
    Rgba {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: 1.,
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grey(v: u32) -> Hsla {
        rgb(v * 0x010101).into()
    }

    #[test]
    fn selection_inks_are_readable_on_graphite_and_blue() {
        // Graphite in light and dark, the system blues, and a pale accent.
        for fill in [
            grey(0x80),
            grey(0x8C),
            grey(0x5A),
            rgb(0x0063E1).into(),
            rgb(0x0059D1).into(),
            rgb(0xFFC0CB).into(),
            rgb(0x7FD1C7).into(),
        ] {
            let fill = legible_fill(fill);
            let (primary, secondary) = ink_on(fill);
            assert!(contrast(primary, fill) >= FILL_TARGET - 0.01, "{fill:?}");
            assert!(contrast(secondary, fill) >= READABLE, "{fill:?}");
            assert_eq!(primary.a, 1.);
            assert_eq!(secondary.a, 1.);
        }
    }

    #[test]
    fn text_inks_are_opaque_and_readable_on_every_surface() {
        for dark in [false, true] {
            let (label, secondary, tertiary) = text_inks(dark);
            for c in [label, secondary, tertiary] {
                assert_eq!(c.a, 1., "dark={dark}");
            }
            // Every surface these inks sit on. The selection greys are not
            // among them: selected rows use the `on_selection*` inks.
            let surfaces: &[u32] = if dark {
                &[0x161617, 0x1C1C1E, 0x232326, 0x2B2B2F, 0x222224]
            } else {
                &[0xF2F2F0, 0xFFFFFF, 0xF7F7F5, 0xF5F5F4]
            };
            for &surface in surfaces {
                let bg: Hsla = rgb(surface).into();
                for (name, ink) in [("label", label), ("secondary", secondary)] {
                    let ratio = contrast(ink, bg);
                    assert!(
                        ratio >= READABLE,
                        "{name} on {surface:06X} dark={dark}: {ratio}"
                    );
                }
            }
        }
    }

    /// A two-tone cell ("1080p" in `label`, " H.264" in `secondary_label`)
    /// must resolve to two different colours however GPUI layers its runs:
    /// with opaque inks, the highlight run's colour is the colour drawn.
    #[test]
    fn two_tone_cells_resolve_to_two_colours() {
        for dark in [false, true] {
            let (label, secondary, tertiary) = text_inks(dark);
            // What GPUI does when one run highlights another: keep the base
            // run's alpha, take the highlight's rgb.
            let layered = |base: Hsla, highlight: Hsla| Hsla {
                a: base.a,
                ..highlight
            };
            for bg in [CONTENT.0, CONTENT.1, 0xF7F7F5, 0x232326] {
                let bg: Hsla = rgb(bg).into();
                let first = over(layered(secondary, label), bg);
                let second = over(layered(label, secondary), bg);
                assert_ne!(first, second, "dark={dark}");
                assert_eq!(first, over(label, bg));
                assert_eq!(second, over(secondary, bg));
            }
            assert!(contrast(label, secondary) > 1.5, "dark={dark}");
            assert!(contrast(secondary, tertiary) > 1.3, "dark={dark}");
        }
    }

    #[test]
    fn destructive_labels_read_on_their_button_fill() {
        // Red; the control fill and its hover over content, raised and
        // overlay surfaces, as `Theme::resolve` builds them.
        let cases: [(bool, u32, [Hsla; 2], [u32; 3]); 2] = [
            (
                false,
                0xD70015,
                [hsla(0., 0., 1., 1.), rgb(0xF4F4F4).into()],
                [0xFFFFFF, 0xF7F7F5, 0xFFFFFF],
            ),
            (
                true,
                0xFF6961,
                [hsla(0., 0., 1., 0.09), hsla(0., 0., 1., 0.13)],
                [0x1C1C1E, 0x232326, 0x2B2B2F],
            ),
        ];
        for (dark, red, controls, surfaces) in cases {
            let fills: Vec<Hsla> = surfaces
                .iter()
                .flat_map(|&s| controls.map(|c| over(c, rgb(s).into()).into()))
                .collect();
            let red: Hsla = rgb(red).into();
            let label = readable_on(red, &fills, dark);
            for &fill in &fills {
                let r = contrast(label, fill);
                assert!(r >= READABLE, "dark={dark}: {r}");
            }
            // Still a red: the hue is kept.
            assert!((label.h - red.h).abs() < 0.01, "dark={dark}");
        }
    }

    #[test]
    fn mac_builds_keep_the_mac_scale() {
        if platform::TARGET == platform::Os::Mac {
            assert_eq!(text::BODY, px(13.));
            assert_eq!(text::SMALL, px(11.));
            assert_eq!(text::CAPTION, px(10.));
            assert_eq!(size::ROW, px(24.));
            assert_eq!(FONT, ".SystemUIFont");
        }
        assert_eq!(text::KIT_REM * 0.875, text::BODY);
    }

    #[test]
    fn title_stays_a_step_above_body_on_every_os() {
        for os in [
            platform::Os::Mac,
            platform::Os::Windows,
            platform::Os::Linux,
        ] {
            let s = platform::scale(os);
            assert!(s.title >= s.body + 2., "{os:?}");
            assert!(s.title_line >= s.body_line, "{os:?}");
            assert!(s.body > s.small && s.small > s.caption, "{os:?}");
            assert!(s.sidebar_row >= s.row, "{os:?}");
            assert!(s.control_small >= s.small_line + 6., "{os:?}");
        }
        if platform::TARGET == platform::Os::Mac {
            assert_eq!(text::TITLE, px(15.));
            assert_eq!(size::SIDEBAR_ROW, px(28.));
            assert_eq!(size::CONTROL_SMALL, px(22.));
        }
    }

    #[test]
    fn focus_ring_shows_next_to_every_surface() {
        let light: Vec<Hsla> = [0xFFFFFF, 0xF7F7F5, 0xF2F2F0]
            .map(|c| rgb(c).into())
            .to_vec();
        let dark: Vec<Hsla> = [0x1C1C1E, 0x232326, 0x2B2B2F, 0x161617]
            .map(|c| rgb(c).into())
            .to_vec();
        // Graphite, the system blues, a pale accent.
        let accents = [0x8C8C8C, 0x808080, 0x5A5A5A, 0x007AFF, 0x0A84FF, 0xFFC0CB];
        for accent in accents {
            for (surfaces, is_dark) in [(&light, false), (&dark, true)] {
                let ring = ring_on(rgb(accent).into(), surfaces, is_dark);
                assert_eq!(ring.a, 1.);
                for &s in surfaces.iter() {
                    let r = contrast(ring, s);
                    assert!(r >= NON_TEXT, "{accent:06X} dark={is_dark}: {r}");
                }
            }
        }
    }

    #[test]
    fn status_and_data_colours_read_on_their_surfaces() {
        let light: [u32; 4] = [0xD70015, 0xB25000, 0x1E7F37, 0x0E756E];
        for c in light {
            for s in [0xFFFFFF, 0xF7F7F5] {
                let r = contrast(rgb(c).into(), rgb(s).into());
                assert!(r >= READABLE, "{c:06X} on {s:06X}: {r}");
            }
        }
        let dark: [u32; 5] = [0xFF6961, 0xFFA142, 0x4CD964, 0x3FB8AF, 0xE0A33E];
        for c in dark {
            for s in [0x1C1C1E, 0x232326, 0x2B2B2F] {
                let r = contrast(rgb(c).into(), rgb(s).into());
                assert!(r >= READABLE, "{c:06X} on {s:06X}: {r}");
            }
        }
        // The audio hue is a mark, not text, in light: 3:1 is its floor.
        assert!(contrast(rgb(0xA86B0B).into(), rgb(0xF7F7F5).into()) >= 3.0);
    }
}
