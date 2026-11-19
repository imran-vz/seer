# Seer design principles

Seer is a Mac app for looking at, pruning and tidying media files. It is drawn
with GPUI, not AppKit, so nothing is native by default. Everything below exists
so the app reads as a well-made Mac tool anyway. When a decision is not covered
here, copy Finder.

Numbers marked (m) were measured from AppKit on macOS 27.2. The rest come from
Apple's Human Interface Guidelines or are conventions taken from Finder and
Xcode.

## Principles

1. **The Mac is the design system.** We do not invent a look. Fonts, sizes,
   colours, selection and motion come from the system, and follow the user's
   settings (appearance, accent colour, reduced motion, scroll bar style).
2. **Content first, chrome quiet.** Files and their data are the interface.
   Chrome is monochrome. Colour is reserved for selection, the default action
   and status. Codecs and stream types are plain text, never rainbow badges.
3. **Dense and calm.** This is a pro tool. 13pt body, 24pt rows, 11pt
   inspector. Density comes from small type and tight rows on a strict grid,
   not from cramming.
4. **Native where the OS has an answer.** Menu bar, context menus, open and
   save panels, Quick Look, Trash, file icons, Finder sort order, drag and
   drop: use the system's own. Custom drawing is for content only (lists,
   inspector, charts).
5. **Keyboard-complete.** Everything can be done from the keyboard with the
   shortcuts Finder taught. Every toolbar button is also a menu command.
6. **Never block.** No file, probe or database call on the main thread. Lists
   are virtualised. Long work is a background job that can be cancelled, and
   the app stays usable while it runs.
7. **One component per job.** One list row, one inspector row, one button, one
   field. If a new screen needs a variant, change the component or use it as
   is. No one-off styling at call sites; all values come from `theme`.
8. **Honest state.** Progress is determinate when it can be. Errors appear
   next to what failed, in plain words, with what to do next. No toasts, no
   alert storms. Empty states say what the pane is for.
9. **Reversible by default.** Delete means Move to Trash. Operations that
   rewrite files write to a temporary file and swap, and can be undone where
   the OS allows.
10. **It remembers.** Window frame, pane widths, sort order, last folder and
    selected inspector tab are restored.

## Window

- Three panes: sidebar (places), content (file list), inspector (details).
  Sidebar and inspector collapse; the content pane never does.
- Unified compact titlebar, 40pt (m). Traffic lights are drawn by the system;
  we reserve 78pt at the leading edge. Leading: back, forward, folder title.
  Trailing: search, inspector toggle.
- Sidebar 200pt default, 160 to 320. Inspector 300pt default, 260 to 420.
  Content minimum 320pt. Window minimum 760 x 440.
- Dividers are 1px hairlines with an 8pt invisible drag zone and the resize
  cursor. Double-click resets the width.
- A 24pt status bar at the bottom of the content pane holds only the item
  count, selection summary and running-job indicator.
- The sidebar uses window vibrancy. Content and inspector are opaque.
- When the window is not key: selection turns grey, toolbar icons dim to 50%.

## Type

System font (SF Pro) everywhere. Monospaced digits for every number.

| Role | Size / line | Weight |
|---|---|---|
| Body, list rows, menus, window title | 13 / 16 | Regular |
| Emphasis, default file name in inspector header | 13 / 16 | Semibold |
| Inspector values, table headers, small controls | 11 / 14 | Regular |
| Section headers (sidebar, inspector) | 11 / 14 | Semibold, secondary colour |
| Chart axes, captions | 10 / 13 | Regular |

Nothing is smaller than 10pt. Nothing in the main window is larger than 15pt.

## Metrics

- Grid: 4pt. Related controls 8pt apart. Pane padding 12pt. Row inset 8pt.
- List row 24pt (m). Table header 28pt (m). Sidebar row 28pt. Inspector row
  20pt.
- Controls: small 20pt / 11pt type, regular 24pt / 13pt type (m). Radius 6pt.
  Icon-only toolbar buttons are 28 x 24 with a 16pt glyph.
- Selection is an inset rounded rectangle: 8pt side inset, 6pt radius.
- Icons are 16pt in rows, stroked, in the label colour.
- Focus ring: 3pt, accent colour at 50%, outside the control. Text fields
  only; lists show focus through the selection colour.

## Colour

All colours are semantic tokens in `theme`, resolved for light and dark and
re-resolved when the system changes. Values (m), light / dark:

| Token | Light | Dark |
|---|---|---|
| label | black 85% | white 85% |
| secondary label | black 50% | white 55% |
| tertiary label | black 26% | white 25% |
| separator | black 10% | white 12% |
| content background | #FFFFFF | #1E1E1E |
| alternate row | #F4F5F5 | white 5% |
| selection, focused | system `selectedContentBackgroundColor` | same |
| selection, unfocused | #DCDCDC | #464646 |
| text on focused selection | white | white |

Accent comes from `NSColor.controlAccentColor` and is used only for focused
selection, the default button, focus rings, on-state toggles and the primary
chart series. Status colours are the system red, orange and green.

## Behaviour

- **Selection.** Click selects on mouse-down. Cmd-click toggles. Shift-click
  extends from the anchor. Arrows move, Shift-arrows extend, Cmd-A selects
  all. Typing jumps to the first match (1s buffer). Clicking empty space
  clears. Right-click on an unselected row targets that row.
- **No hover highlight** on list rows or bezelled buttons. Borderless icon
  buttons get a faint fill on hover. The cursor is always the arrow, except
  the I-beam over editable text and resize cursors on dividers.
- **Keys.** Cmd-Down or double-click opens. Cmd-Up goes to the enclosing
  folder. Cmd-[ and Cmd-] go back and forward. Return renames. Space is Quick
  Look. Cmd-Delete moves to Trash. Cmd-Shift-N new folder. Cmd-Shift-. toggles
  hidden files. Cmd-F search. Cmd-Opt-I inspector. Cmd-Opt-S sidebar. Cmd-,
  settings. Cmd-R reveal in Finder.
- **Menus.** App, File, Edit, View, Go, Window, Help. Unavailable items are
  disabled, not hidden. Commands that need more input end with an ellipsis.
- **Tables.** Click a header to sort, again to reverse. Numeric columns are
  right-aligned. File names truncate in the middle, everything else at the
  tail. Sizes are decimal (as Finder), with one unit per cell.
- **Modality.** Sheets for a task on the current window (bulk rename, stream
  cleanup). Popovers for transient detail (jobs). Alerts only for destructive
  or failed actions, with verb-labelled buttons. Settings is its own window
  and applies immediately.
- **Motion.** 0.2s ease-out for pane collapse and disclosure. Nothing else
  animates. Reduce Motion turns these off.
- **Jobs.** Non-modal. A toolbar indicator opens the activity popover: one row
  per job with a determinate bar, a cancel button and inline errors.

## Tells we refuse

Row hover highlights. Pointer cursors on buttons. 14px+ body text. 40px rows.
Selection that stays blue in an inactive window. Custom scroll bars. Toasts.
A settings page inside the main window. Badges in six colours. Fade-ins on
everything. Instant tooltips. A menu bar with only "Quit" in it.

## Engineering rules

- **Stack.** Rust, GPUI via `gpui-kit` (pinned exactly). FFmpeg libraries
  in-process for probing, packet reads and lossless remuxing; the `ffmpeg`
  binary only for re-encoding. SQLite (`rusqlite`, bundled) for cache and job history. `objc2`
  for AppKit, ImageIO and Quick Look.
- **Layers.** `seer-core` knows nothing about UI: file listing, probing,
  bitrate analysis, rename rules, jobs, cache. It is plain synchronous Rust
  with unit tests. `seer` is the GPUI app and calls core only from background
  tasks.
- **One source of truth per value.** Sizes, spacing and colours live in
  `theme.rs`. Formatting of sizes, durations and bitrates lives in
  `seer-core::format`. No literals at call sites.
- **Performance budget.** A folder of 10,000 entries is listed and sorted in
  under 50ms (35ms measured; `cargo test -p seer-core --release -- --ignored
  listing_speed`). Lists are virtualised, so frame cost does not grow with
  folder size. Probe results are cached by file fingerprint (size, mtime,
  head and tail hash), so nothing is probed twice.
