# Seer design principles

Seer is a desktop app for looking at, pruning and tidying media files. It is
drawn with GPUI, so nothing is native by default and nothing has to be: Seer
has one look of its own on every OS. The OS supplies behaviour and the parts
it draws itself; everything else comes from `theme` and `ui`.

Numbers marked (m) were measured from AppKit on macOS 27.2. Platform values
come from each vendor's guidelines (see Platforms). The rest are Seer's.

## Principles

1. **Seer has one look.** The OS supplies behaviour, window controls, the menu
   bar (on the Mac), file dialogs, the UI font, the accent colour, the
   scroll-bar preference and reduced motion. Everything else comes from
   `theme`, the same on the Mac, Windows and Linux, including context menus,
   which are Seer's overlays. When a decision is not
   covered here: do what a careful user of that OS expects the key or gesture
   to do, and draw it the Seer way. Finder remains the reference for file
   *behaviour* on the Mac (selection, rename, keys), not for appearance.
2. **Content first, chrome quiet.** Files and their data are the interface.
   Chrome is neutral grey on four surfaces. Colour is reserved for selection,
   the default action, focus, status, and two fixed data hues used only for
   media data. Codecs and stream types are plain text, never badges.
3. **Seer's signature.** The dense media table with two-tone media cells, the
   bit-rate trace, and the video and audio hues. Build these up; never sand
   them off.
4. **Dense and calm.** This is a pro tool. 13pt body, 24pt rows, 11pt
   inspector. Hierarchy comes from three clear levels (title, body, meta),
   weight and tone, not from size alone.
5. **Keyboard-complete.** Everything can be done from the keyboard with the
   shortcuts each OS taught. Every toolbar button is also a menu command and
   names its shortcut in its tooltip.
6. **Never block.** No file, probe or database call on the main thread. Lists
   are virtualised. Long work is a background job that can be cancelled, and
   the app stays usable while it runs.
7. **One component per job.** One list row, one inspector field, one button,
   one sheet, one empty state. If a new screen needs a variant, change the
   component or use it as is. No one-off styling at call sites; all values
   come from `theme`, all controls from `ui`.
8. **Honest state.** Progress is determinate when it can be. Errors appear
   next to what failed, in plain words, with what to do next. Finished work
   says so briefly. No toasts, no alert storms. Empty states say what the pane
   is for and offer the next step.
9. **Reversible by default.** Delete means Move to Trash. Operations that
   rewrite files write to a temporary file and swap, and can be undone where
   the OS allows.
10. **It remembers.** Window frame, pane widths, sort order, last folder and
    selected inspector tab are restored.

## Window

- Three panes: sidebar (places), content (file list), inspector (details).
  Sidebar and inspector collapse; the content pane never does.
- Titlebar: height and insets from `platform::chrome()` (40pt on the Mac).
  Leading cluster, right after the window controls and fixed there whether or
  not the sidebar is open: sidebar toggle, back, forward. Then the
  breadcrumb (`ui::Breadcrumb`): the current folder as a title, before it
  its parent in secondary, and earlier ancestors always folded into a "…"
  menu button ("Parent folders", 28pt, nearest first). The parent folds too
  when there is no room for it. A parent is capped at 140pt and the current
  folder at the room left, both truncated in the middle with the full name
  in a tooltip. Home shows as its name. Trailing: search, a labelled "Clean
  Up…" button (icon only, with a tooltip, when narrow), inspector toggle.
- Search yields before the folder title does: 180pt, then 144pt when the
  titlebar is tight, then a 28pt search button that expands into the field
  on click or Find. Its placeholder names the folder ("Search Movies"), in
  `secondary_label`. It is disabled in a folder with no items.
  While it filters, the status bar says "2 matches" ("2 matches · 1
  selected").
- 48pt of bare titlebar (`size::DRAG_MIN`, `Chrome::drag_min`) is always
  kept for dragging the window; the breadcrumb gives up room first.
- The titlebar over a pane is that pane's: the sidebar's width of it is
  `surface_window`, and while the inspector is open its width is
  `surface_raised` with its divider running to the top.
- On desktops without a global menu bar, a "Main Menu" icon button
  (`ui::MenuButton` filled by `ui::command_menu_with`) holds every command,
  then About Seer; see Platforms for its place. F10, or Alt pressed and
  released on its own, opens it (`ui::open_menu`); its tooltip never shows
  while it is open.
- Sidebar 200pt default, 160 to 320. Inspector 300pt default, 260 to 420.
  Content minimum 320pt. Window minimum 760 x 440.
- Adaptive panes: effective widths are computed from the window width in one
  place, without changing the saved preference. The list always keeps Name
  plus the folder's primary media column (Video, or Audio in an audio
  folder), Duration and Size: at 760pt with the inspector open all four
  show. **Panes yield for the fifth column**: before the list settles for
  four columns, the inspector and then the sidebar narrow toward their
  minimums (saved widths are kept) to give the list the width of the
  folder's five (`browser::list_comfort`): Name at 200pt plus Video, Audio,
  Duration and Size in a mixed folder (648pt); Audio, Duration, Size and
  Date Modified in a folder of audio (692pt), so at the default window the
  inspector gives 14pt and the dates show. **Places stays docked while the
  essentials fit**: when the list cannot have four columns beside a 200pt
  Name either, both panes keep their narrowest widths as long as the list
  keeps the essentials with Name at its floor (`browser::list_essential`,
  499pt), so at 940pt the sidebar stays beside the inspector and the list
  shows four columns (S4-4; the sidebar beats the fifth column). Narrower
  than that (935pt on the Mac) the sidebar leaves its dock; the inspector
  stays. The sidebar button then opens Places as an overlay panel over the
  list (`surface_overlay`, its shadow; Escape, a click elsewhere or choosing
  a place closes it) instead of evicting the inspector. Both dock again when
  the window widens. A floating Places panel is only as tall as its
  content, up to the window, and scrolls past that.
- **Places and the keyboard.** Places is one tab stop: the current place
  (the lit one), or the place the arrows last moved to. Up and Down (Home
  and End) move between places across Favourites, Locations and Devices;
  Return opens one; Tab from a disk's row reaches its eject button; the
  context-menu key opens the row's menu. Opening the floating panel puts
  the keyboard on the current place; Escape returns it to where it was.
  When the panel docks (the window widened) focus stays on the same place
  in the docked sidebar; when a docked sidebar goes (the window narrowed,
  or it was hidden) focus goes to the list.
- Dividers are 1pt hairlines with an 8pt invisible drag zone and the resize
  cursor. Double-click resets the width.
- A 24pt status bar at the bottom of the content pane: the count or selection
  summary at the leading edge, running or just-finished work at the trailing
  edge. Never the path.
- When the window is not key: selection turns grey, titlebar controls dim to
  50%.

## Surfaces

Four surfaces, back to front. In dark, each step is lighter than the one
behind it; an overlay is always lighter than what it floats over.

| Token | Light | Dark | Used for |
|---|---|---|---|
| `surface_window` | #F2F2F0 | #161617 | Sidebar and the titlebar strip above it. 94% over the window blur on the Mac, opaque elsewhere |
| `surface_content` | #FFFFFF | #1C1C1E | File list and the titlebar over it |
| `surface_raised` | #F7F7F5 | #232326 | Inspector, table header, status bar, segmented track |
| `surface_overlay` | #FFFFFF | #2B2B2F | Popovers, sheets, tooltips, menus Seer draws. Always with `border_overlay` (ink 10%) and the overlay shadow |
| `scrim` | black 22% | black 45% | Over the whole window behind a sheet |
| `alternate_row` | #F5F5F4 | #222224 | Every other list row. Stripes end with the data: the empty area below the last row is plain `surface_content` |

## Type

The host's UI font everywhere: the system font on the Mac, Segoe UI
Variable on Windows, Adwaita Sans then Cantarell on GNOME
(`platform::Chrome::fonts`). `theme::apply` takes the first one installed
(`Theme::font`, `theme::font`) and falls back to the system font, so a
family that is missing never leaves text unset. Measured text
(`ui::text_width`, through `theme::font_in_use`), sheets and tooltips use
that same resolved family, so what is measured is what is drawn. Monospaced digits for every
number. Real typography: "—" not "--", "…" not "...", one space either side
of "·". British spelling.

Sizes follow the OS's own type scale (`platform::TypeScale`): the table
below is the Mac's; Windows and GNOME set Title at 16 / 22 and Body, Small
and Caption at 14 / 12 / 11 on 20 / 16 / 14 lines, in 28pt list rows,
32pt sidebar rows and 24pt Small buttons. Title stays at least two points
above Body on every OS (a test holds it), so a pane title never reads as
body text set one point larger. `theme::text` and
`size::ROW` are constants of the build target, so they apply to a real
Windows or Linux build; the `SEER_PLATFORM` simulation on the Mac keeps the
Mac's scale and font (see Platforms).

| Role | Token | Size / line | Weight | Used for |
|---|---|---|---|---|
| Display | `text::DISPLAY` | 20 / 24 | Semibold | Empty-state titles in the main pane only |
| Title | `text::TITLE` | 15 / 20 (16 / 22 where Body is 14) | Semibold | Current folder in the titlebar, file name in the inspector header, sheet titles |
| Body | `text::BODY` | 13 / 16 | Regular | List rows, menus, Regular buttons, breadcrumb parents |
| Small | `text::SMALL` | 11 / 14 | Regular | Inspector values and labels, table headers, status bar, Small buttons |
| Section header | `text::SMALL` | 11 / 14 | Semibold, `label` | Sidebar and inspector group names |
| Caption | `text::CAPTION` | 10 / 13 | Regular | Chart axes |

Nothing is smaller than 10pt or larger than 20pt.

## Colour

All colours are semantic tokens in `theme`, resolved for light and dark and
re-resolved when the system changes. gpui-kit's own colours (search and
rename fields, popovers, menus) are mapped from these by `theme::apply`.

**Contrast floor.** Text that carries information meets 4.5:1 on its surface
in both appearances, including alternate rows and selection. Non-text marks
that must be seen (the focus ring) meet 3:1 against every surface next to
them. Unit tests in `theme.rs` hold this.

**Text inks are opaque.** `label`, `secondary_label` and `tertiary_label`
are greys resolved over `surface_content` (`theme::text_inks`), not
translucent black or white. GPUI keeps a text run's base alpha when another
run highlights part of it, so translucent inks drew two-tone cells in one
tone. A test proves the two tones resolve to two colours.

| Token | Light | Dark | Used for |
|---|---|---|---|
| `label` | #262626 (black 85% over white) | #E1E1E2 (white 87% over content) | Names, values, titles, section headers |
| `secondary_label` | #666666 (60%) | #A9A9AA (62%) | Field labels, metadata, hints, empty-state detail, status text, "Waiting" |
| `tertiary_label` | #9E9E9E (38%) | #6E6E6F (36%) | Disabled labels and glyphs (at full strength). Never information, never a mark that must be seen (empty-state glyphs are `secondary_label`). Placeholders are `secondary_label`: "Search Movies" says what the field acts on |
| `separator` | black 8% | white 9% | Hairlines between panes and groups |
| `border` | black 13% | white 14% | Around controls |
| `fill`, `fill_hover`, `fill_pressed`, `fill_on` | black 5 / 6 / 10 / 9% | white 8 / 10 / 16 / 14% | Borderless controls, tracks, on-state toggles and on-state Secondary buttons, in both appearances. A segment's hover is half of `fill_hover`, so it never reads as a second chosen segment |
| `control` | #FFFFFF | white 9% | Secondary and Destructive buttons, unchecked checkbox |
| `selection` | system selection colour, lightness tuned for contrast | same | Focused selection |
| `selection_inactive` | #DCDCDC | #464646 | Unfocused selection |
| `on_selection`, `on_selection_secondary` | white or near-black, per fill, opaque | same | Text on selection; both ≥ 4.5:1 |
| `primary`, `on_primary` | accent (lightness tuned), or #262626 when the accent is grey | accent, or #E8E8E8 when grey | The default button, checked checkboxes, progress |
| `focus_ring` | accent, darkened until 3:1 on every surface | accent, lightened until 3:1 | Keyboard focus, 2pt outside the control. Opaque, so a Graphite accent still shows (`theme::ring_on`) |
| `text_selection` | accent 28% | accent 40% | Selected text in a field; softer than the ring on purpose |
| `red`, `orange`, `green` | #D70015, #B25000, #1E7F37 | #FF6961, #FFA142, #4CD964 | Status: failed, warning, done. Always with words |
| `destructive` | `red` | `red`, lightened until 4.5:1 | A Destructive button's label, readable on the button's own fill and its hover (`theme::readable_on`) |
| `data_video` | #0E756E | #3FB8AF | Video series, share bar and stream mark |
| `data_audio` | #A86B0B | #E0A33E | Audio share bar and stream mark |

Selection and primary foregrounds are resolved against the actual fill
(`theme::ink_on`): white is preferred; if neither white nor near-black reads
at 5.6:1, the fill's lightness moves (`theme::legible_fill`) while its hue
stays the system's. So a Graphite accent still gives readable selection and
a primary button that outranks a Secondary one.

The accent is for selection, the primary button, focus rings and on-state
toggles only. Media data uses the two data hues, never the accent, so the
chart is not grey on grey when the accent is Graphite.

## Metrics

- Grid: 4pt. Related controls 8pt apart, groups 16pt. Pane padding 12pt. Row
  inset 8pt. Sheet margin 20pt.
- List row 24pt (m). Table header 28pt (m). Sidebar row 28pt. Inspector row
  20pt.
- Table columns: 16pt apart, 24pt before a left-aligned column that follows a
  numeric one. Widths are tokens: Video 112, Audio 104, Duration 64, Size
  72, Date Modified 148 (`size::COLUMN_VIDEO`, `COLUMN_AUDIO`,
  `COLUMN_DURATION`, `COLUMN_SIZE`, `COLUMN_MODIFIED`). Name is as wide as
  the widest visible name plus 24pt (`size::NAME_PADDING`), clamped to
  200–440pt; slack goes after the last column. **Name yields first**: as
  the list narrows Name shrinks to its 200pt minimum (truncating in the
  middle) before any column drops. Only then do columns drop, in this
  order: Date Modified, Audio (beside Video), Duration, the primary media
  column; Size is the last to go. To keep Size, the primary media column
  and Duration, Name may go below 200pt, down to what the narrowest list
  leaves (171pt at 760pt with a 260pt inspector). At the default 1180pt
  window with both panes open, Name, Video, Audio, Duration and Size all
  show whatever the longest name. At 760pt the four essentials (Name, the
  primary media column, Duration, Size) show **under every sort**: when the
  sorted column cannot fit, its header folds and the list still says how
  it is sorted ("Sorted by Audio ↓"), and View → Sort by… keeps its
  checkmark. An audio-only folder never shows Video; opening one while
  sorted by Video falls back visibly to a sort it can show, never to a
  hidden one.
- Radius scale: 4 inline (checkbox, segment, crumb), 6 control (buttons,
  fields, selection, tooltips), 10 card (popovers, menus), 14 overlay
  (sheets). No radius literals. Menus share the card corner with
  popovers: gpui-kit draws a menu's outside with its one `radius`, which
  `theme::apply` sets to `RADIUS_CARD`; the kit caps a menu item's
  highlight at 8pt inside it. Fields take the control radius back in
  `ui::text_field`.
- Hairlines are 1pt (`size::HAIRLINE`), never half.
- Buttons: Small 22pt with 11pt text (inspector, popovers, side-pane
  empty states; 24pt where Small text is 12pt); Regular 28pt with 13pt text
  (titlebar, sheet footers, and the recovery action of a main-pane empty
  state: Clear Search, Open Folder…, Show Hidden Files, under a 20pt
  Display title). Icon buttons are
  28 x 28 with a 16pt glyph; small ones 22 x 22 with a 12pt glyph.
  Segmented track 24pt.
- Selection is an inset rounded rectangle: 8pt side inset, 6pt radius.
  Adjacent selected rows draw as one block: only a run's outer corners are
  round, and no stripe shows through.
- Icons are 16pt in rows, stroked, in `secondary_label` (`label` when on).
- **File glyphs are Seer's** (`ui::kind_glyph`, `ui::Glyph`). Every file
  list row leads with one 16pt glyph from one family: a stroked document
  with a folded corner (a folder silhouette for folders), 1pt stroke on
  half points, with a small inner mark for its kind: a play triangle
  (video), a wave (audio), lines (text), two centred caption lines
  (subtitles), a frame (images), nothing (other files). The outline is
  `secondary_label`; a video's mark is `data_video` and an audio file's
  `data_audio`, so the first thing on a row shares the media columns'
  two hues; every other mark is neutral. On a selected row the whole
  glyph takes the row's on-selection ink (`ui::kind_glyph_in` with
  `on_selection` or `on_selection_inactive`). Every glyph reaches 3:1
  against plain, striped, hovered and selected rows in both appearances
  and in any accent (a test holds it). The system's file icons appear in
  one place only, the inspector header for a selected *file*, where the
  icon is that file's own. A folder there takes Seer's folder glyph at
  32pt (`ui::kind_glyph_large`, the chart's 1.5pt stroke) and a disk the
  sidebar's drive glyph, both in `secondary_label`: the system's folder
  and volume icons differ per OS and the volume's was 1.2:1 in light
  (F4-1). The list therefore looks the same on every OS.
- Empty-state glyphs are 28pt tall in `secondary_label` (3:1 or more, like
  the trace: a mark to be seen, never the disabled ink). **Seer's mark** is
  a small bit-rate trace over a baseline (`ui::trace_glyph`, 40 x 28, the
  chart's 1.5pt stroke): the trace in `secondary_label` (3:1 or more on
  every surface; it is a mark, never styled as disabled), the baseline a
  `separator` rule that recedes. It appears above an empty folder and "No
  matches", on the Bit Rate tab before analysis (what is about to appear),
  and in the About sheet beside the name. States about something else (a
  copy arriving, an error) keep an icon.
- Inspector fields (`ui::field_with_rail`): one grammar for every row: label 72pt
  (58pt while the inspector is narrower than 280pt), left-aligned, in
  `secondary_label`; value in `label`.
- Bars: progress 4pt in `primary`; stream-share 6pt in the data hues, never
  thinner than 2pt when non-zero. The folder bar is one 6pt segment per
  file, sized by bytes, in its kind's hue (`data_video`, `data_audio`,
  neutral), a hairline apart; files too small for a 2pt segment share one
  neutral "others" segment at the end. Hovering a segment names the file
  and its size.
- Sheets: width `min(640, window − 48)`, height to content up to 560, then
  the body scrolls. A sheet with no body is compact: 420 wide.
- Menus Seer draws (context menus, the Main Menu, Parent folders): 13/16
  text, 24pt items, a 4pt inset between the edge and the highlight, a 10pt
  outside corner, `surface_overlay` with a hairline ring in ink 10% (the
  same as `border_overlay`) and a shadow. The kit puts 2pt between items,
  so they sit on a 26pt pitch; it has no setting for that gap. Shortcut
  hints at the trailing edge in `secondary_label` with tabular figures;
  checkmarks for toggles and the current sort. gpui-kit sizes in rems, so
  `theme::apply` sets its rem to `text::KIT_REM` (`BODY` / 0.875): the
  kit's body text is then `BODY` and its small text `SMALL`.
- Other sizes with a token: titlebar search 180 / 144pt, crumb 140pt, drag
  strip 48pt, transfer label and bar 320 / 64pt, chart height 168pt with a
  1.5pt stroke and 3pt dash, legend swatch 8pt, inspector header icon 32pt,
  trace glyph 40pt. Views never keep size constants of their own.

## Components

All in `ui.rs`; see its doc comments for signatures.

- **Button.** Variants Primary, Secondary (raised fill and a 1pt border),
  Ghost, Destructive (`destructive` text on the Secondary fill, 4.5:1 in
  both appearances). Exactly one Primary per surface. Hover and pressed
  fills; a focus ring when focused from the keyboard; Return or Space
  presses it. A Secondary button that is on (the choice in effect) takes
  `fill_on` with its usual border in both appearances, so it never reads
  as focused. Disabled: the Secondary shape with its fill at 40%, the label
  in `tertiary_label` at full strength, no handler. Opacity is for fills
  only, never for text or glyphs.
- **IconButton.** Borderless; hover fill; an on state for toggles whose pane
  is open (`pressed`, which also reports it to assistive technology);
  always a tooltip with the command's name and shortcut. Its accessible
  name is the tooltip's, or a steady one where the tooltip changes with
  the state ("Sidebar" for "Show Sidebar" and "Hide Sidebar"). Disabled:
  the glyph in `tertiary_label`. Inside a group with one roving stop (a
  disk's eject button) it is a tab stop only while its row is.
- **MenuButton.** An IconButton that opens a menu (a glyph or one text
  character such as "…"); Return or Space opens it, and `ui::open_menu`
  opens it from a key without focusing it, so Escape returns focus to
  where it was (the search field, the list), not to the button; its
  tooltip is its name, never shown while its menu is open, and its
  accessible name stays, with whether its menu is expanded. `ui::menu_item` builds an item in
  Seer's type with the command's shortcut; `ui::command_menu_with` fills a
  menu from the command table in `actions.rs`, the same table the Mac menu
  bar is built from (`actions::menu_bar`), with enabled and checked states
  from an `actions::Availability`.
- **Text field.** `ui::text_field`: the kit's field in the control radius.
  While it has focus its border takes the opaque `focus_ring` and Seer's
  2pt ring sits outside it, as on buttons (the kit's own translucent ring
  is off).
- **Tooltips** appear after 600ms on the overlay surface. Every icon-only
  control has one. A Button's, IconButton's or MenuButton's tooltip opens
  `GAP_SMALL` below its control when the control is in the upper half of
  the window (the titlebar), centred under it and kept `GAP_SMALL` inside
  the window, so it never covers the control or its neighbours in the
  row; lower down it keeps GPUI's placement by the pointer, flipped above
  near the bottom edge.
- **Checkbox.** Square, 14pt, for independent choices (stream removal,
  Clean Up rows). It always has an accessible label in words (a debug
  build asserts it): a stream's says which track it removes ("Remove Audio
  2, AAC, Tamil", the number among tracks of its kind, its language and
  title) and, while checked, "Will be removed". The whole row is the mouse target; the box alone takes
  keyboard focus, and the focus ring is drawn around the 14pt mark only
  (2pt outside it, `RADIUS_INLINE`), never around a long label. Space
  toggles it; Return does not (in a sheet, Return belongs to the focused
  button). Up and Down move to the previous and next checkbox (a sheet's
  review rows) and stop at the first and last. A ring means "choose
  one"; Seer has no radio groups outside `segmented`.
- **Segmented control** (`ui::segmented`, and `ui::tabs` for the
  inspector's tabs): one tab stop, the chosen segment; Left and Right
  choose the neighbouring segment and move focus with it.
- **Field**, **section header**, **progress bar**, **share bar**,
  **breadcrumb**: one each.
- **Accessibility.** The pinned GPUI exposes an AccessKit tree
  (`role`, `aria_label`, `aria_description`, `aria_toggled`,
  `aria_selected`, `aria_expanded`, `aria_position_in_set`,
  `aria_active_descendant`, `aria_value`, …), and every shared control
  uses it, so no control is unnamed:
  - Button, IconButton and MenuButton are buttons named by their label,
    with their shortcut; a toggle reports on or off, a menu button whether
    its menu is open; a disabled one says why (its tooltip) or
    "Unavailable". The stand-in caption buttons are named too.
  - A Checkbox is a checkbox with its checked state, its required
    `accessible_label` and an optional description (Clean Up passes the
    track, the proposed name and the reason; Streams the track and "Will
    be removed").
  - `segmented` is a radio group of radio buttons with their checked
    state, `tabs` a tab list of tabs with the selected one, each with its
    label and position.
  - A Sheet is a dialog named by its title, its status line as the
    description; the floating Places panel is a dialog named "Places".
  - The breadcrumb is a navigation landmark ("Location") of links, the
    current folder a heading.
  - The file list is a list box named by its folder ("Files in Movies").
    Each row is an option identified by its path, named by the file's full
    name, with its position, whether it is selected, and each visible
    column with its value as its description ("Video 1080p H.264, Size
    459 MB"). The list keeps one keyboard stop; the lead row is its active
    descendant, so assistive technology follows the arrow keys and a
    re-sort without the rows taking focus.
  - Each Places group is a list box of places, the current one selected.
  - Views name what only they know (the chart's current time and rate).

  What VoiceOver actually announces has not been verified on this Mac;
  GPUI only builds the tree while an assistive client is attached.
- **Empty state.** Glyph, title, detail, action. Main pane: 28pt glyph
  (Seer's trace glyph for an empty folder and for no matches,
  `EmptyState::trace`), Display title, secondary detail, an optional
  button (Open Folder…, Show Hidden Files, Clear Search) in the Regular
  size, a full-size target under the Display title; an icon glyph is in
  `secondary_label`. Side panes: a
  short top-aligned prompt; the inspector's empty-folder prompt keeps the
  folder summary's header (icon, name). Never two equal-weight empty
  messages side by side.
- **Sheet.** Overlay surface and radius over a scrim covering the whole
  window. Title and one status line; body; footer with a note at the leading
  edge and buttons at the trailing edge, one Primary. With no body it is
  compact: 420 wide, no separator, no footer band, the buttons under the
  status line. The keyboard stays inside it: Tab and Shift-Tab cycle its
  controls (the kit's focus trap). It opens with focus on its first control
  (the first checkbox of a review), and moves focus there again when its
  content is replaced or the focused control goes away. A focused button
  takes Return and Space itself, so a sheet with a body applies a change
  only from its focused Fix button, never from Return elsewhere; a
  checkbox takes Space only. A compact sheet's default (Done) also runs on
  Return when the sheet itself holds focus. Escape and the default button
  close it; focus returns to where it was (the list, a menu's trigger).
  Assistive technology reads it as a dialog named by its title.
- **Context menu.** Seer's, not the OS's (see Metrics, Menus). The row it
  was opened on keeps its focused selection colour while it is open. It
  opens from the keyboard too (`ui::KeyMenu`, the same builder): the
  context-menu key (Shift+F10 or the Menu key on Windows and GNOME, ⌃↩ on
  the Mac; Edit › Show Context Menu) opens the list's menu under the lead
  row's name (scrolling it into view first), the folder's menu at the top
  of the list when nothing is selected, and a place's menu under that
  place. Escape gives the keyboard back to the row.
- **Popover.** Overlay surface, card radius, 12pt padding, rises above what
  it covers. Outside click and Escape dismiss; focus returns to what opened
  it.

## Behaviour

- **Selection.** Click selects on mouse-down. Cmd-click (Ctrl-click elsewhere)
  toggles. Shift-click extends from the anchor. Arrows move, Shift-arrows
  extend, Select All selects all. Typing jumps to the first match (1s
  buffer). Clicking empty space clears. Right-click on an unselected row
  targets that row.
- **No hover highlight** on list rows. Buttons, icon buttons, crumbs and
  sortable headers get a hover fill. The cursor is always the arrow, except
  the I-beam over editable text and resize cursors on dividers.
- **Keys** come from `platform::keys`, per OS. On the Mac: Cmd-Down or
  double-click opens. Cmd-Up goes to the enclosing folder. Cmd-[ and Cmd-] go
  back and forward. Return renames. Space is Quick Look. Cmd-Delete moves to
  Trash. Cmd-Shift-N new folder. Cmd-Shift-. toggles hidden files. Cmd-F
  search. Cmd-Opt-I inspector. Cmd-Opt-S sidebar. Cmd-R reveal in Finder.
  Cmd-B analyse bit rate. Cmd-Shift-K clean up stream names. Cmd-Shift-J
  shows transfers. Ctrl-Return opens the focused row's context menu
  (Shift+F10 or the Menu key elsewhere).
- **Menus.** App and Window (Mac only), File, Edit, View, Go, from one table
  (`actions::command_menus`). Unavailable items are disabled, not hidden,
  from one `actions::Availability` (selection, clipboard, history, Clean Up
  eligibility, transfers) that the Main Menu, the context menu and the
  window's action handlers all read; each command names what it
  `actions::Needs`. On the Mac, AppKit greys out a menu-bar item whose
  action has no handler, so a view registers a command's handler only
  when the command is available. Toggles and the current sort carry a
  checkmark. Edit ends with "Show Context Menu" (`ShowContextMenu`), the
  keyboard's way to what a right-click shows; it is greyed out where
  nothing focused has a menu. View lists every sort the list has, in column order (Name,
  Video, Audio, Duration, Size, Date Modified; `actions::Sort`), so a
  sort whose header has folded at a narrow width can still be seen,
  chosen and reversed from the keyboard. "About Seer" opens a compact sheet (name, version, the
  one-line description, `actions::APP`): first in the app menu on the Mac,
  last in the Main Menu elsewhere. View has "Show Transfers", enabled only
  while a transfer runs or a receipt is retained (`Devices::has_details`);
  dismissing the last receipt disables it.
  Commands that need more input end with an ellipsis. Wording comes from
  `platform::label`; the folder chooser is "Open Folder…" in the menu and
  on the empty-folder button alike. Disclosures are sentence case ("Show 4
  technical tags"); commands are title case.
- **Tables.** Click a header to sort, again to reverse. Numeric columns are
  right-aligned, with the sort arrow on their leading side. File names
  truncate in the middle, everything else at the tail. Sizes are decimal,
  with one unit per cell. Media cells are two-tone and lead with the strong
  word, so each media column has one bright left edge: resolution class
  and channel layout in `label`, then the codec in secondary ("**1080p**
  H.264", "**Stereo** AAC", "**5.1** E-AC-3 +1"). Every header sorts, with
  a hover fill and the arrow when
  active: Video by resolution height then codec, Audio by codec then
  channels, Duration by length.
- **Blank, dash, Unreadable.** A cell is blank when its column does not
  apply to that kind of item (folders, text, images, subtitles; a folder's
  Size) and while a probe is pending. "—" in tertiary appears only on a
  probed media file that lacks that stream. A media file that could not be
  read says "Unreadable" in `orange` in its first media cell, with the
  reason in a tooltip.
- **Inspector.** Media first: under the header and tabs, duration, video and
  audio; then File details (kind, size, modified, one-line location with
  reveal and copy); then Naming; then Tags (humanised keys; container
  plumbing behind "Show 4 technical tags"). A stream row's value is the
  codec alone; profile and the rest go on its detail line. Any "a · b · c"
  run (folder header, Codecs, stream details) is packed by measuring its
  parts: a line never starts or ends with "·". A lone "Default" on a file's
  only stream of that kind is left out. Location shows two path components,
  then one, never a second ellipsis. Nothing is truncated at the default
  width.
- **Nothing selected shows the folder.** The inspector summarises the
  current folder: its name, "113 MB · 7 videos · 3 folders" (size first),
  total duration, the folder bar (one segment per file, see Metrics) with a
  legend, and the codecs ("H.264 3 · HEVC 1"). It fills in as probes
  finish. Totals say what they count: with unreadable or still-probing
  files the figure says so beside it ("0:35 · 1 unreadable", "2 of 3
  videos"), and Codecs says whether it counts streams or files. While a
  copy targets the folder the summary says "Copying 4 items here…", never
  "This folder is empty", and below it the inspector adds what the list
  cannot: the bytes so far ("1.20 GB of 6.38 GB copied", or "moved") and
  the files that have arrived ("2 of 6 files"), not the list's sentence
  again. Bytes in flight are copied, not arrived; only finished files have
  arrived. A multi-selection gets the same Codecs line and the same
  coverage wording. Every partial figure uses one definition of "read":
  successful probes; failures are counted beside it ("1 unreadable").
  While a search filters the list, the summary is labelled as the whole
  folder ("Whole folder · 11 items"). Clearing a search by any route
  (Escape, the clear button, Clear Search, deleting the text) restores the
  same finished summary, never a stuck "Reading…".
- **Inspector title.** The selected file's name wraps to a second Title
  line before it truncates, so the full name reads at 760pt; a wrapped
  metadata line is rebalanced rather than leaving one word alone.
- **Chart.** One timeline: trace, peak marker, pointer readout and time
  ticks all derive from it. The average and peak headline stays put; the
  pointer's sample has its own readout line. The chart is a focus stop with
  the focus ring: Left and Right move the readout one sample (Shift for
  larger steps), Home and End jump to the ends; pointer and keys share the
  readout. The aggregate trace is labelled "All streams"; a mixed
  aggregate is drawn so it cannot be taken for a video-only trace, which is
  `data_video`; audio-only files draw in `data_audio`.
- **Clean Up.** Two-line rows: checkbox, stream identity ("Audio 2 · AAC ·
  Tamil"), the old name struck through, an arrow, the new name; under it the
  reason in small secondary text, never truncated. Status: "5 names to fix
  in 2 videos". A file that could not be read is counted and named ("6
  videos checked · 1 couldn't be read"); "every stream name is tidy" only
  when every candidate was read and nothing was skipped. **Clean Up reviews
  a complete scan**: Fix stays disabled until the scan has finished, and
  folders or files the scan could not open are counted and named ("1
  folder couldn't be read"). Track names are written in one order
  everywhere: the list, Clean Up's proposals and the inspector agree
  ("Mono AAC"). After a fix only the ticked issues show as
  fixed. The status names the scope ("7 videos checked, including
  subfolders") and its numbers agree with the list it shows. The identity
  column is as wide as its widest label; names wrap onto a second line
  before anything is truncated. Clean Up is live: one eligibility rule,
  re-evaluated on every listing, enables the toolbar button, the command,
  the folder context menu and the Main Menu item; it is disabled in a
  folder with no videos. Keyboard: see Sheet.
- **Modality.** Sheets for a task on the current window (bulk rename, stream
  cleanup). Popovers for transient detail (jobs). Alerts only for destructive
  or failed actions, with verb-labelled buttons. Settings is its own window
  and applies immediately.
- **Motion.** 0.2s ease-out for pane collapse and disclosure; tooltips after
  600ms. Nothing else animates. Reduce Motion turns these off.
- **Devices.** An Android device connected by USB appears in the sidebar
  under Devices and is browsed like a folder. Copy, Paste and Move Item Here
  work as in the file manager, in every direction. Nothing is overwritten: a
  taken name arrives as "name 2". A device has no Trash, so deleting there
  asks first. Media on a device is not probed; copy it to the computer to
  inspect it.
- **Jobs.** Non-modal. One status-bar trigger ("Copying 4 items · 44%" with a
  slim bar and a chevron that says it opens) opens the activity popover: one
  row per item with its state at the trailing edge, counts labelled ("4
  items, 6 files"), inline errors, and a labelled Small "Stop Copy" (or
  "Stop Move") button. No circled X means stop. A finished transfer reads
  "6.38 GB · 4 items, 6 files". On completion the status bar keeps a check
  and "Copied 4 items to Archive" for 4 seconds; a failed or stopped result
  keeps a visible cue in the status bar until dismissed, even beside a
  newer job. View → Show Transfers (Cmd-Shift-J, Ctrl-J elsewhere) opens
  the details from the keyboard whenever a transfer runs or a receipt is
  retained, including the last finished one after its notice expires.
  **The receipt stays findable**: once the notice has gone, while the last
  receipt is kept, the status bar's trigger stays as a quiet "Last
  Transfer" in `secondary_label` with its chevron (S4-5), and opens the
  same details. Every ended transfer can be dismissed there, a success
  too; dismissing the last receipt takes the entry away and returns focus
  to where it was.
- **Location.** With the search field active at tight widths, the current
  folder's crumb keeps enough room to be read and the parent folders stay
  reachable (the "…" menu remains). When a menu, popover or
  sheet closes, focus returns to where it was.
- **Volumes.** A mounted volume has its own drive glyph, not the startup
  disk's and not a USB plug, and an eject button that shows on hover and on
  focus. Ejecting goes through `native::eject`, off the main thread.

## Platforms

`platform.rs` holds everything that differs per OS. Views never branch on the
OS and never hard-code "⌘", "Ctrl" or a file manager's name. Only macOS is
implemented; the Windows and GNOME values are data. Debug builds take
`SEER_PLATFORM=windows|gnome` to present that OS's titlebar insets, control
side, wording, shortcut hints, Main Menu (with F10 bound) and font choice
on the Mac, with stand-in window controls (`ui::window_controls`); other
key bindings stay the host's.

`platform::chrome()` also says where the commands live (`menu`), how much
bare titlebar to keep for dragging (`drag_min`, 48pt everywhere) and whether
a hairline with 8pt either side separates Seer's last control from the
window controls (`controls_separator`). `platform::trash()` and
`platform::key_store()` name the Trash and the secret store.

**Scroll bars follow the host.** `platform::scroll_bars` turns the host's
setting, read through GPUI (`should_auto_hide_scrollbars`: AppKit's
preferred scroller style on the Mac), into the kit's mode: kept on screen
when the person asked for that, on every OS; otherwise shown while
scrolling (Windows also on hover). `theme::apply` sets it, and the window
reads it again whenever it becomes active, so a change in System Settings
is picked up.

**Every surface uses the resolved font.** Measured text, sheets, tooltips,
the transfer details and the label that follows a drag all take
`theme::font` (the first installed family of `platform::Chrome::fonts`);
nothing names the compile-time `theme::FONT` at a call site.

**What is still simulation.** On this Mac, a `SEER_PLATFORM` capture shows
another OS's layout, wording and menus, but not:

- its type scale: `theme::text`, `size::ROW`, `size::SIDEBAR_ROW` and
  `size::CONTROL_SMALL` are constants of the build target, so 16 / 14 /
  12 / 11 in 28pt rows appear only in a real Windows or Linux build;
- its font: Segoe UI Variable and Adwaita Sans are not installed on the
  Mac, so the simulation falls back to the system font, as a real build
  would when a family is missing;
- its caption-button reserve, which is a constant here and should be read
  from the window (`LeftInset` / `RightInset`, the user's button layout);
- KDE's server-side decorations, file dialogs and accent source, and the
  bare-Alt, F10, Shift+F10 and Menu-key behaviour of a real window manager.

No Windows or Linux build has been made or captured; those platform rows
are designed, not verified.

**Still open** (out of reach on this Mac, or deferred by decision):

- real Windows 11, GNOME and KDE builds and their captures, with caption
  insets, the user's button layout and text scale read from the host;
- a Settings window (Settings applies immediately and is its own window,
  never a page in the main one) and a sidebar foot to open it from (F4-5);
- a keyboard disclosure under the folder bar listing each file and its
  bytes (today a pointer tooltip names a segment), a keyboard route to an
  inspector title that still clips after two lines, and a disclosure for a
  failed Clean Up rewrite's reason (today a tooltip);
- Clean Up stacking an unusually long old and new name on lines of their
  own;
- the first frame of a video in place of its system icon in the
  inspector header.

| Concern | macOS | Windows 11 | GNOME |
|---|---|---|---|
| Window controls | System traffic lights, leading; 78pt kept clear | System caption buttons, trailing; 3 × 46px | Close button, trailing; 46px kept clear |
| Titlebar | 40pt | 48px (holds interactive content) | 46px header bar |
| Sidebar | Translucent over the window blur | Opaque | Opaque |
| Commands | Global menu bar | "Main Menu" button at the leading end of the titlebar | "Main Menu" button at the trailing end, before the close button |
| Drag strip | 48pt kept | 48px kept | 48px kept |
| Before window controls | — (traffic lights lead) | Hairline, 8pt either side | Hairline, 8pt either side |
| Primary modifier | ⌘ | Ctrl | Ctrl |
| Rename / open / trash | Return / Cmd-Down / Cmd-Delete | F2 / Enter / Delete | F2 / Enter / Delete |
| Back / forward / up | Cmd-[ / Cmd-] / Cmd-Up | Alt-Left / Alt-Right / Alt-Up | Alt-Left / Alt-Right / Alt-Up |
| Hidden files / sidebar / inspector | Cmd-Shift-. / Cmd-Opt-S / Cmd-Opt-I | Ctrl-H / F9 / Alt-Enter | Ctrl-H / F9 / Alt-Enter |
| Show Transfers | Cmd-Shift-J | Ctrl-J | Ctrl-J |
| Open the Main Menu | — (menu bar) | F10, or Alt alone | F10, or Alt alone |
| Open the focused row's context menu | Ctrl-Return | Shift+F10, or the Menu key | Shift+F10, or the Menu key |
| Scroll bars | System setting (AppKit scroller style) | System setting | System setting |
| About Seer | App menu, first | End of the Main Menu | End of the Main Menu |
| UI font | System (SF) | Segoe UI Variable, then Segoe UI | Adwaita Sans, then Cantarell |
| Title / Body / Small / Caption | 15 / 13 / 11 / 10 | 16 / 14 / 12 / 11 | 16 / 14 / 12 / 11 |
| List row / sidebar row / Small button | 24 / 28 / 22pt | 28 / 32 / 24pt | 28 / 32 / 24pt |
| Reveal | Reveal in Finder | Show in File Explorer | Show in Files |
| Delete | Move to Trash | Delete | Move to Trash |
| Where deleted files go | Trash | Recycle Bin | Trash |
| Secret store (API keys) | Keychain | Credential Manager | keyring |
| Root volume | Macintosh HD | This PC | Computer |
| File dialogs, accent | AppKit | System | Portal and toolkit |
| Context menus | Seer's overlay | Seer's overlay | Seer's overlay |

## Tells we refuse

Row hover highlights. Pointer cursors on buttons. 14px+ body text. 40px rows.
Selection that stays saturated in an inactive window. Custom scroll bars.
Toasts. A settings page inside the main window. Badges in six colours.
Stripes below the last row. A column dropped while Name has room to give.
Another vendor's icons leading the rows of the main list. A sort the
keyboard cannot reach. A focus ring around a paragraph. A tooltip over its
own control. A control a screen reader cannot name. A context menu only
the pointer can open. A menu that hands focus to its button instead of
back to where it was.
A tooltip over its own open menu. Return that applies a change from
anywhere but the button that makes it. The
accent used for data. Fade-ins on everything. Instant tooltips. Icon-only
controls without a tooltip. A circled X that stops work. Text below 4.5:1
that carries information. A menu bar with only "Quit" in it. Translucent
text inks. Disabled text faded with opacity. A dash for "does not apply".
A keyboard that can Tab out of a sheet. A context menu bigger than the list
it opens on.

## Engineering rules

- **Stack.** Rust, GPUI via `gpui-kit` (pinned exactly). FFmpeg libraries
  in-process for probing, packet reads and lossless remuxing; the `ffmpeg`
  binary only for re-encoding. SQLite (`rusqlite`, bundled) for cache and job
  history. `objc2` for AppKit, ImageIO and Quick Look.
- **Layers.** `seer-core` knows nothing about UI: file listing, probing,
  bitrate analysis, rename rules, jobs, cache. It is plain synchronous Rust
  with unit tests. `seer` is the GPUI app and calls core only from background
  tasks.
- **One source of truth per value.** Sizes, spacing, radii, colours and
  timings live in `theme.rs`; controls in `ui.rs`; OS differences in
  `platform.rs`. Formatting of sizes, durations and bitrates lives in
  `seer-core::format`. No literals at call sites.
- **Performance budget.** A folder of 10,000 entries is listed and sorted in
  under 50ms (35ms measured; `cargo test -p seer-core --release -- --ignored
  listing_speed`). Lists are virtualised, so frame cost does not grow with
  folder size. Probe results are cached by file fingerprint (size, mtime,
  head and tail hash), so nothing is probed twice.
