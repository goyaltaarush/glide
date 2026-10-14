# Toolcraft-style inspector kit — design

Date: 2026-10-14 · Status: approved in chat, awaiting spec review

## Goal

Make every control in the editor's right-hand inspector look and move like
Toolcraft's controls (`/Users/taarushgoyal/toolcraft/src/toolcraft/ui`),
staying native egui. Success: the inspector reads as one consistent
Toolcraft-style panel, sliders feel like Toolcraft's (glide, grow, type a
value, reset), and nothing else in the app changes behaviour.

Out of scope: timeline, toolbar, transport bar, home screen, export sheet.

## Approach

Restyle in place. The public functions in `src/app/widgets.rs` keep their
names and roles; their bodies are rewritten. Call sites in
`src/app/inspector.rs` change only where a signature changes (slider
`default`). Shared motion and colour tokens live in `src/app/theme.rs`.
egui's global style is tuned so the egui controls that stay (buttons,
ComboBox, TextEdit, colour picker) blend in.

## Tokens (`theme.rs`)

- `motion::BASE = 0.2` s ease-out (cubic: `1 - (1-t)^3`), `motion::HOVER = 0.1` s.
- `motion::reduced()` — true when macOS "Reduce motion" is on
  (`NSWorkspace.accessibilityDisplayShouldReduceMotion`, read once at start
  and cached; needs the `NSAccessibility`/workspace feature in
  `objc2-app-kit` if not already enabled). When true, every kit animation
  duration is 0.
- `tween(ctx, id, target, duration) -> f32`: ease-out tween stored in egui
  temp data (`{from, to, t0}`), restarting from the current value when the
  target changes, requesting repaint while in flight. Replaces the spike's
  inline `Tween`.
- Colours derived from the palette: `track = secondary × 0.38`,
  `hover_tint = text × 0.10`, `active = text`.

## Controls

### Slider — `slider(ui, label, &mut v, range, suffix, default) -> bool`

- Row 1: label (left), value (right). Row 2: full-width 18 px hit area,
  1 px track, 1 px fill in `active` from start to thumb.
- Thumb: 9 × 9, 2 px radius, scales to 1.4 over `HOVER` when the pointer is
  within its 18 px hit area or while dragging.
- Pointer: press on the track jumps there; the thumb and fill glide over
  `BASE`. While dragging the thumb tracks the pointer with no tween.
- Reset: double-click the thumb → `default`. Defaults come from the model's
  `Default` impls (e.g. `Style::default().padding`), passed at each call site.
  Sliders bound to locals or item fields (crop edges, a zoom's strength)
  use the matching item's `Default` (`Zoom::default()`, …) or 0 for crop.
- Typed value: click the value → single-line field; Enter/blur commits
  (clamped, suffix stripped), Esc cancels.
- Keyboard: the slider takes focus on click; ←/→ nudge by 1 % of the range,
  Shift by 10 %.
- Value digits: drawn glyph by glyph with each digit given the width of the
  widest digit (tabular figures without OpenType features), in the
  proportional UI font, `secondary` colour, 12 px.
- Decimals: span ≥ 100 → 0, ≥ 10 → 1, else 2.
- Disabled (`!ui.is_enabled()`): drawn at 25 % opacity, no interaction.

### Stepped slider — `slider_steps(ui, label, &mut v, range, step, suffix, default) -> bool`

Same as `slider`, values snap to `step`; tick marks (1 × 6 px, `active`)
at each step except the ends fade in over `BASE` on hover/drag. Used where a
slider is genuinely discrete; no call site is required to switch.

### Segmented — `segmented(ui, &mut value, options) -> bool` (unchanged signature)

Outline group: 1 px `control_border`, 6 px radius, 2 px inner padding.
Equal-width items fill the row. The selected item's background
(`hover_tint × 1.6`) slides between items over `BASE`; text of the selected
item is `text`, others `secondary`, unselected hover gets `hover_tint`.

### Switch — `switch` / `toggle_row` (unchanged signatures)

28 × 16 pill, 14 px thumb, off = `text × 0.20`, on = accent; thumb travel
and colour over `BASE`.

### Section and row

`section`: title 11 px semibold `secondary`, 10 px above, 6 px below,
hairline separator. `row`: label 12 px `text`, control right-aligned,
22 px min height. Slider rows use the same label style.

### Global egui style (`theme::apply`)

Buttons, ComboBox, TextEdit, colour button: 6 px radius, `hover_tint` on
hover, no hover expansion, 12 px body text, popups with `popover` fill and
6 px radius.

## Data flow and undo

Unchanged. Widgets mutate the inspector's `draft`; `Editor::commit`
coalesces changes within 600 ms into one undo step, so a drag or a series
of arrow-key nudges is one step.

## Error handling

Typed values that don't parse are discarded (field closes, value
unchanged). Empty or inverted ranges: slider draws at the start and
ignores input (no panic).

## Testing

- Unit tests (`widgets.rs`): decimals rule, typed-value parsing (suffix,
  clamping, garbage), step snapping, tween reaches target and holds,
  reduced motion gives duration 0.
- Snapshot check: `glide snapshot <project> --tab <tab>` for every tab,
  dark and light, reviewed by eye against the Toolcraft reference.
- Manual: drag, click-to-jump, double-click reset, type value, arrow keys,
  undo after a drag, with Reduce motion on and off.

## Files

- `src/app/theme.rs` — tokens, `tween`, reduced-motion flag, global style.
- `src/app/widgets.rs` — slider, slider_steps, segmented, switch, section, row.
- `src/app/inspector.rs` — pass `default` to each `slider` call.
- `src/app/macos.rs` — reduced-motion query.
