# Toolcraft-style inspector kit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every control in the editor's right-hand inspector looks and moves like Toolcraft's, natively in egui.

**Architecture:** Restyle in place. Shared motion and colour tokens go in `src/app/theme.rs`. The bodies of the existing `widgets.rs` functions are rewritten, with pure logic split out so it can be unit-tested. Only `slider` changes signature (it gains `default`). egui style tweaks are applied inside the inspector panel only.

**Tech Stack:** Rust 2024, egui 0.36, objc2-app-kit 0.3.

**Spec:** `docs/superpowers/specs/2026-10-14-toolcraft-inspector-kit-design.md`

## Global Constraints

- Branch `feat/toolcraft-inspector-kit`. Build and test in **release** (a debug build fails inside whisper-rs-sys): `export PATH="$HOME/.cargo/bin:$PATH"; . scripts/env.sh; cargo test --release --lib`.
- Motion: `BASE = 0.2` s and `HOVER = 0.1` s, ease-out `1 - (1-t)^3`. Every duration becomes 0 when macOS Reduce motion is on.
- Colours: `track = secondary × 0.38`, `hover_tint = text × 0.10`, `active = text`. Disabled controls draw at 25 % opacity.
- Out of scope: timeline, toolbar, transport, home, export sheet. No style change may leak into them.
- Match the surrounding code's comment density and idiom. No new crates.
- The spike slider currently in `widgets.rs` (`Tween`, `ease_out`, `slider_decimals`) is replaced, not kept alongside.

## Review Focus

1. **Arrow keys on a focused slider.** `Editor::handle_keys` runs first and seeks the playhead on ←/→. A focused slider must nudge only, with no seek. (Task 3)
2. **Two sliders with the same label in one panel** (e.g. Background "Blur" and Shadow "Blur"; the crop and mask "Left"/"Top"). Their typed-edit and tween state must not collide. (Task 3)
3. **A stored value outside the slider's range** (e.g. stiffness 2000 on a 5–1500 slider). The thumb pins to the end and the value is not rewritten on an idle frame. (Task 3)
4. **Retargeting mid-animation.** Clicking a new position or segment while a glide is in flight continues from the current shown value, with no jump. (Task 1)
5. **Typed input edge cases:** `"abc"`, `""`, `"NaN"`, `"1e400"`, `"0,5"`, `"16 px"`. Garbage is ignored, the value is clamped, the suffix is stripped and a comma counts as a decimal point. (Task 2)

---

### Task 1: Motion and colour tokens

**Files:**
- Modify: `src/app/theme.rs` (new `pub mod motion` at the end of the file, plus `KitColors`)
- Modify: `src/app/macos.rs` (add `reduce_motion()`)

**Interfaces:**
- Produces:
  - `theme::motion::{BASE: f32 = 0.2, HOVER: f32 = 0.1}`
  - `motion::ease_out(t: f32) -> f32`
  - `motion::duration(d: f32) -> f32` (0 when reduced)
  - `motion::reduced() -> bool`, cached in a `OnceLock` from `macos::reduce_motion()`
  - `#[derive(Clone, Copy, Default)] pub struct Tween { from: f32, to: f32, t0: f64 }`
  - `motion::step(prev: Option<Tween>, target: f32, now: f64, dur: f32) -> (f32, Tween, bool /*in flight*/)` (pure)
  - `motion::tween(ctx: &egui::Context, id: egui::Id, target: f32, dur: f32) -> f32` (stores `Tween` in temp data, requests repaint while in flight, applies `duration()`)
  - `theme::KitColors { track, hover_tint, active: Color32 }` and `theme::kit(p: &Palette) -> KitColors`
  - `macos::reduce_motion() -> bool`, via `NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()` (`NSAccessibility` module, enabled by objc2-app-kit's default features)

- [ ] **Step 1: Write failing tests** in `theme.rs` `#[cfg(test)] mod tests`:
```rust
assert_eq!(ease_out(0.0), 0.0); assert_eq!(ease_out(1.0), 1.0); assert!((ease_out(0.5) - 0.875).abs() < 1e-6);
// first frame snaps, no animation
let (v, s, busy) = step(None, 0.4, 10.0, 0.2); assert_eq!(v, 0.4); assert!(!busy);
// new target animates from current
let (v, s, busy) = step(Some(s), 1.0, 10.0, 0.2); assert_eq!(v, 0.4); assert!(busy);
let (v, _, busy) = step(Some(s), 1.0, 10.2, 0.2); assert_eq!(v, 1.0); assert!(!busy);
// retarget mid-flight continues from shown value (Review Focus 4)
let (mid, s2, _) = step(Some(s), 1.0, 10.1, 0.2);
let (v, _, _) = step(Some(s2), 0.0, 10.1, 0.2); assert!((v - mid).abs() < 1e-6);
// zero duration snaps
let (v, _, busy) = step(Some(s), 0.0, 10.0, 0.0); assert_eq!(v, 0.0); assert!(!busy);
```
- [ ] **Step 2: Run** `cargo test --release --lib theme`. Expected: FAIL (items not defined).
- [ ] **Step 3: Implement** the items listed under Interfaces. The progress clamp in `step` must handle `dur == 0` without dividing by zero.
- [ ] **Step 4: Run** `cargo test --release --lib theme`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(app): motion and colour tokens for the inspector kit`.

### Task 2: Slider logic (pure)

**Files:**
- Create: `src/app/widgets/slider_math.rs`. Turn `widgets.rs` into `widgets/mod.rs` only if a submodule needs it; otherwise put this in a `mod slider_math` inside `widgets.rs` and keep the file.

**Interfaces:**
- Produces (all pure, `f64`):
  - `decimals(lo, hi) -> usize`: span ≥ 100 → 0, ≥ 10 → 1, else 2
  - `frac(v, lo, hi) -> f32`: clamped to 0..=1, and 0 when `hi <= lo`
  - `from_frac(t, lo, hi) -> f64`
  - `snap(v, lo, hi, step) -> f64`: to the nearest `lo + k·step`, clamped
  - `nudge(v, lo, hi, dir: i8, big: bool) -> f64`: 1 % of the span, or 10 % when big, clamped
  - `parse_typed(text, suffix, lo, hi) -> Option<f64>`: trim, strip `suffix.trim()`, replace `,` with `.`, parse, reject non-finite, clamp
  - `format_value(v, decimals, suffix) -> String`

- [ ] **Step 1: Write failing tests:**
```rust
assert_eq!(decimals(5.0, 1500.0), 0); assert_eq!(decimals(0.0, 40.0), 1); assert_eq!(decimals(0.0, 1.0), 2);
assert_eq!(frac(2000.0, 5.0, 1500.0), 1.0); assert_eq!(frac(1.0, 3.0, 3.0), 0.0);   // out of range / empty range
assert_eq!(snap(0.37, 0.0, 1.0, 0.25), 0.25); assert_eq!(snap(9.0, 0.0, 1.0, 0.25), 1.0);
assert_eq!(nudge(0.5, 0.0, 1.0, 1, false), 0.51); assert_eq!(nudge(0.95, 0.0, 1.0, 1, true), 1.0);
assert_eq!(parse_typed("16 px", " px", 0.0, 60.0), Some(16.0));
assert_eq!(parse_typed("0,5", "", 0.0, 1.0), Some(0.5));
assert_eq!(parse_typed("99", "%", 0.0, 40.0), Some(40.0));
for bad in ["abc", "", "NaN", "1e400"] { assert_eq!(parse_typed(bad, "", 0.0, 1.0), None); }
assert_eq!(format_value(16.25, 1, "%"), "16.2%");
```
(Compare floats with a 1e-9 tolerance wherever rounding applies.)
- [ ] **Step 2: Run** `cargo test --release --lib slider_math`. Expected: FAIL.
- [ ] **Step 3: Implement** the functions.
- [ ] **Step 4: Run** the same command. Expected: PASS.
- [ ] **Step 5: Commit** `feat(app): slider value logic`.

### Task 3: Slider widget, stepped slider, call sites

**Files:**
- Modify: `src/app/widgets.rs` (replace the spike `slider`; add `slider_steps` and `paint_tabular`)
- Modify: `src/app/inspector.rs` (every `slider(` call gets `default`)
- Modify: `src/app/editor.rs` (`handle_keys` arrow guard)

**Interfaces:**
- Consumes: Task 1 `motion::*`, `kit()`; Task 2 `slider_math::*`.
- Produces:
  - `pub fn slider(ui, label: &str, v: &mut f64, range: RangeInclusive<f64>, suffix: &str, default: f64) -> bool`
  - `pub fn slider_steps(ui, label, v, range, step: f64, suffix, default) -> bool`
  - `fn paint_tabular(painter: &Painter, right_center: Pos2, text: &str, font: FontId, color: Color32) -> Rect`: lays out right-aligned and gives every ASCII digit the width of the widest digit `0–9` in `font`.

Behaviour (copy every value from the spec's "Slider" section):
- Hit area 18 px high. Track 1 px, `kit().track`. Fill 1 px, `active`.
- Thumb 9 × 9 with a 2 px radius. It grows to 1.4 over `HOVER` when the pointer is in its 18 px hit area or while dragging.
- Pressing the track jumps; the fill and thumb glide over `BASE` via `motion::tween` (target `frac`). While dragging, call with `dur = 0`.
- Double-clicking the thumb sets `default`.
- Clicking the value opens a `TextEdit` (`parse_typed` on Enter or blur; Esc cancels).
- The track takes focus on click (`resp.request_focus()`). While it has focus, ←/→ call `nudge` (Shift = big), and the keys are consumed with `consume_key`.
- The value uses `paint_tabular`, proportional font at 12 px, `secondary` colour.
- `slider_steps` snaps every change through `snap`. Its ticks (1 × 6 px, `active`, all except the ends) fade in via `motion::tween(.., BASE)` on hover or drag.
- Widget state ids: `ui.next_auto_id().with(label)` taken at the very start of the call (Review Focus 2).
- `changed` is true only when `*v` actually changed this frame (Review Focus 3).
- `handle_keys` arrow guard: skip the ArrowLeft/ArrowRight seek when `ctx.memory(|m| m.focused().is_some())` (Review Focus 1).

`default` per call site: use the field's value from the owning type's `Default` (`Style::default()`, `CursorStyle::default()`, `CameraStyle::default()`, `AudioStyle::default()`, `MotionBlur::default()`, `CaptionStyle::default()`, `ShortcutStyle::default()`, `Zoom::default()`, `Mask::default()`). Exceptions:

| Call site | default |
|---|---|
| Crop Left/Right/Top/Bottom | `0.0` |
| Gradient "Angle" | the `Background` gradient default's angle, else `135.0` |
| "Idle after" | `2000.0` |
| "Loop over" | `1000.0` |
| "Freeze for" | `1000.0` |
| Ripple "Strength" | `2.0` |
| Manual zoom X/Y | `0.5` |
| Layout "Camera size" | `0.35` |
| Mask amount (Blur, Pixelate, Dim) | `Mask::default_amount(m.kind)` |
| Mask rect Left/Top/Width/Height | from `Mask::default().rect` |
| Recording mic/system volume | `1.0` |

Hoist each `Default` into a local at the top of its panel function, so it isn't constructed once per slider.

- [ ] **Step 1: Write failing headless tests** in `widgets.rs` tests. Drive `egui::Context::default().run(RawInput{..}, |ctx| CentralPanel::default().show(ctx, |ui| ..))` with synthetic `Event::PointerButton` and `Event::Key` input. Helpers stay local to the test module.
  - `out_of_range_value_is_not_rewritten`: v = 2000.0 on 5..=1500, one idle frame → returns false, v == 2000.0.
  - `same_label_sliders_keep_separate_state`: two `slider(ui, "Blur", ..)`; click the first one's value text, then type `"7"` + Enter → only the first changes.
  - `double_click_thumb_resets`: v = 30 on 0..=60, default 16, double-click at the thumb centre → v == 16.
  - `arrow_nudges_focused_slider`: click the track, release, then ArrowRight → v increased by 1 % of the span.
  - `dragging_past_end_clamps`: press inside, drag 500 px right → v == hi.
- [ ] **Step 2: Run** `cargo test --release --lib widgets`. Expected: FAIL (signature mismatch / behaviour).
- [ ] **Step 3: Implement** `slider`, `slider_steps`, `paint_tabular`, the call-site defaults and the `handle_keys` guard.
- [ ] **Step 4: Run** `cargo test --release --lib`. Expected: all PASS, and `cargo build --release` gives no warnings.
- [ ] **Step 5: Snapshot check.** `glide snapshot "<latest ~/Movies/Glide/*.glide>" --tab background -o /tmp/kit/bg.png`, plus `--light`. Expected: sliders show label/value on one row, hairline track and square thumb, and digits line up from row to row.
- [ ] **Step 6: Commit** `feat(app): Toolcraft-style sliders in the inspector`.

### Task 4: Segmented control and switch

**Files:**
- Modify: `src/app/widgets.rs` (`segmented`, `switch`)

**Interfaces:**
- Consumes: Task 1 tokens.
- Produces: unchanged signatures `segmented<T: PartialEq + Copy>(ui, &mut T, &[(T, &str)]) -> bool` and `switch(ui, &mut bool) -> Response`.

Behaviour (spec values):
- Segmented: outline 1 px `control_border`, 6 px radius, 2 px padding. Equal-width items fill `ui.available_width()`, laid out left to right regardless of the parent's direction (this replaces the current RTL reversal). The selected background `hover_tint × 1.6` slides via `motion::tween` on the selected index's x over `BASE`. Selected text is `text`, others `secondary`. An unselected item gets `hover_tint` on hover.
- Switch: 28 × 16 pill with a 14 px thumb. Off = `text × 0.20`, on = accent. Thumb position and colour use `motion::tween` over `BASE`. Disabled draws at 25 % opacity.

- [ ] **Step 1: Write failing headless tests:**
  - `segmented_click_selects`: three options; click the third item's centre → value == third, returns true.
  - `segmented_items_fill_width`: in a 300 px-wide Ui, the response rect width ≈ 300 (±1).
  - `switch_toggles_on_click`.
- [ ] **Step 2: Run** `cargo test --release --lib widgets`. Expected: FAIL on the width test.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test --release --lib`. Expected: PASS.
- [ ] **Step 5: Snapshot** the cursor and camera tabs, dark and light. Check: segmented rows span the panel; switches are the smaller pill.
- [ ] **Step 6: Commit** `feat(app): Toolcraft-style segmented control and switch`.

### Task 5: Sections, rows and inspector-scoped style

**Files:**
- Modify: `src/app/widgets.rs` (`section`, `row`)
- Modify: `src/app/theme.rs` (add `pub fn inspector_style(s: &mut egui::Style, dark: bool)`)
- Modify: `src/app/inspector.rs` (apply it at the top of `inspector::ui` via `ui.style_mut()`)

**Interfaces:**
- Consumes: Task 1 `kit()`.
- Produces: `theme::inspector_style`. `section`/`row` keep their signatures.

Values:
- `section` title: 11 px semibold `secondary`, 10 px above, 6 px below, hairline separator.
- `row` label: 12 px `text`, 22 px min height.
- `inspector_style`: widget corner radius 6 px; hovered `bg_fill`/`weak_bg_fill` = `hover_tint` over the inactive fill; `expansion = 0` in every widget state; Body and Button text 12 px; popup/menu fill `popover`, radius 6 px.

- [ ] **Step 1: Write failing test** `inspector_style_is_scoped`: apply `inspector_style` to a clone of the context style and check the radius is 6 and the expansion 0; check the context's own style is unchanged.
- [ ] **Step 2: Run** `cargo test --release --lib theme`. Expected: FAIL.
- [ ] **Step 3: Implement** and apply it in `inspector::ui`.
- [ ] **Step 4: Run** `cargo test --release --lib`. Expected: PASS.
- [ ] **Step 5: Snapshot every tab** (background, cursor, camera, captions, audio, keys, motion) dark and light, plus `--select zoom`, `--select mask` and `--select slice`. Also snapshot the home view and check it against `docs/screenshots/home.png`: the toolbar and home must look unchanged.
- [ ] **Step 6: Commit** `feat(app): Toolcraft spacing and control style in the inspector`.

### Task 6: App build and manual pass

- [ ] **Step 1:** `scripts/build.sh`. Expected: `built dist/Glide.app`.
- [ ] **Step 2: Manual checklist** in the app:
  - drag, click-to-jump glide, double-click reset, type a value (Enter / Esc / garbage)
  - arrow keys on a focused slider (the playhead must not move)
  - one ⌘Z undoes a whole drag
  - segmented slide, switch
  - System Settings → Accessibility → Display → Reduce motion on, then relaunch → no animations
- [ ] **Step 3:** Fix anything found, with a test where one fits, then commit.
