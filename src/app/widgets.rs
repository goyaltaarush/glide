//! Small UI building blocks in the macOS/paper-rs style.

use std::ops::RangeInclusive;

use egui::{Color32, CornerRadius, Response, RichText, Sense, Stroke, Ui, vec2};

use super::theme::{self, pal};

mod slider_math;

/// A titled inspector section.
pub fn section(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    ui.add_space(10.0);
    ui.label(RichText::new(title).font(theme::semibold(theme::SECONDARY)).color(pal(ui).secondary));
    ui.add_space(6.0);
    add(ui);
    ui.add_space(10.0);
    ui.separator();
}

/// Label on the left, control on the right.
pub fn row(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.set_min_height(22.0);
        ui.label(label);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), add);
    });
}

/// Whether a focused control owns the keyboard, so arrow keys go to it and
/// not to the playhead.
pub fn keyboard_owned(ctx: &egui::Context) -> bool {
    ctx.memory(|m| m.focused().is_some())
}

/// A labelled slider in Toolcraft's style; returns true when changed.
/// Click the value to type one, double-click the thumb for `default`, and
/// use ←/→ (Shift for bigger steps) once it has focus.
pub fn slider(ui: &mut Ui, label: &str, v: &mut f64, range: RangeInclusive<f64>, suffix: &str, default: f64) -> bool {
    slider_impl(ui, label, v, range, None, suffix, default)
}

/// `slider` that snaps to `step`, with tick marks that show on hover.
pub fn slider_steps(ui: &mut Ui, label: &str, v: &mut f64, range: RangeInclusive<f64>, step: f64, suffix: &str, default: f64) -> bool {
    slider_impl(ui, label, v, range, Some(step), suffix, default)
}

fn slider_impl(ui: &mut Ui, label: &str, v: &mut f64, range: RangeInclusive<f64>, step: Option<f64>, suffix: &str, default: f64) -> bool {
    use slider_math as m;
    // Taken before anything is added, so sliders sharing a label don't share state.
    let id = ui.next_auto_id().with(label);
    let p = pal(ui);
    let k = theme::kit(&p);
    let (lo, hi) = (*range.start(), *range.end());
    let decimals = m::decimals(lo, hi);
    let fix = |n: f64| step.map_or(n.clamp(lo, hi), |s| m::snap(n, lo, hi, s));
    let mut changed = false;
    let mut set = |v: &mut f64, n: f64| {
        if n != *v {
            *v = n;
            changed = true;
        }
    };

    // Name, and the value (click to type).
    ui.horizontal(|ui| {
        ui.label(label);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (edit_id, open_id) = (id.with("edit"), id.with("edit-open"));
            match ui.data(|d| d.get_temp::<String>(edit_id)) {
                Some(mut text) => {
                    let out = egui::TextEdit::singleline(&mut text)
                        .desired_width(56.0)
                        .horizontal_align(egui::Align::RIGHT)
                        .font(egui::FontId::proportional(12.0))
                        .show(ui);
                    let r = out.response;
                    let fresh = !ui.data(|d| d.get_temp::<bool>(open_id).unwrap_or(false));
                    if fresh {
                        // Open with everything selected, so typing replaces it.
                        r.request_focus();
                        let mut state = out.state;
                        let all = egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(text.chars().count()));
                        state.cursor.set_char_range(Some(all));
                        state.store(ui.ctx(), r.id);
                        ui.data_mut(|d| d.insert_temp(open_id, true));
                    }
                    let (enter, esc) = ui.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
                    let close = esc || enter || (!fresh && r.lost_focus());
                    if close {
                        if !esc && let Some(n) = m::parse_typed(&text, suffix, lo, hi) {
                            set(v, fix(n));
                        }
                        ui.data_mut(|d| {
                            d.remove::<String>(edit_id);
                            d.remove::<bool>(open_id);
                        });
                    } else {
                        ui.data_mut(|d| d.insert_temp(edit_id, text));
                    }
                }
                None => {
                    let r = tabular_value(ui, &m::format_value(*v, decimals, suffix), p.secondary);
                    if r.hovered() && ui.is_enabled() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
                    }
                    if r.clicked() {
                        ui.data_mut(|d| d.insert_temp(edit_id, m::format_value(*v, decimals, "")));
                    }
                }
            }
        });
    });

    // The track.
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 18.0), Sense::click_and_drag());
    let track = rect.shrink2(vec2(5.0, 0.0));
    let x_at = |t: f32| egui::lerp(track.x_range(), t);
    let on_thumb = |t: f32, pos: egui::Pos2| egui::Rect::from_center_size(egui::pos2(x_at(t), track.center().y), vec2(18.0, 18.0)).contains(pos);
    let pointer = resp.interact_pointer_pos();
    let grab_id = id.with("grab");
    if resp.double_clicked() && pointer.is_some_and(|pos| on_thumb(m::frac(*v, lo, hi), pos)) {
        set(v, fix(default));
    } else if resp.is_pointer_button_down_on()
        && let Some(pos) = pointer
    {
        // Pressing the thumb grabs it where it is; pressing the track jumps.
        if ui.input(|i| i.pointer.primary_pressed()) {
            resp.request_focus();
            let t = m::frac(*v, lo, hi);
            let offset = if on_thumb(t, pos) { x_at(t) - pos.x } else { 0.0 };
            ui.data_mut(|d| d.insert_temp(grab_id, offset));
        }
        let offset: f32 = ui.data(|d| d.get_temp(grab_id).unwrap_or(0.0));
        let t = (pos.x + offset - track.left()) / track.width();
        let n = fix(m::from_frac(t, lo, hi));
        if (m::frac(n, lo, hi) - m::frac(*v, lo, hi)).abs() > f32::EPSILON || resp.dragged() {
            set(v, n);
        }
    }
    if resp.has_focus() {
        ui.memory_mut(|mem| mem.set_focus_lock_filter(resp.id, egui::EventFilter { horizontal_arrows: true, ..Default::default() }));
        for (key, dir) in [(egui::Key::ArrowLeft, -1), (egui::Key::ArrowRight, 1)] {
            for big in [false, true] {
                let mods = if big { egui::Modifiers::SHIFT } else { egui::Modifiers::NONE };
                if ui.input_mut(|i| i.consume_key(mods, key)) {
                    let n = fix(m::nudge(*v, lo, hi, dir, big));
                    set(v, n);
                }
            }
        }
    }

    // Glide to jumps; follow the pointer exactly while dragging.
    let dragging = resp.dragged();
    let shown = theme::motion::tween(ui.ctx(), id.with("pos"), m::frac(*v, lo, hi), if dragging { 0.0 } else { theme::motion::BASE });
    let over = resp.hover_pos().is_some_and(|pos| on_thumb(shown, pos));
    let active = ui.is_enabled() && (dragging || over);
    let grow = theme::motion::tween(ui.ctx(), id.with("grow"), if active { 1.0 } else { 0.0 }, theme::motion::HOVER);
    if active {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let alpha = if ui.is_enabled() { 1.0 } else { 0.25 };
    let painter = ui.painter_at(rect.expand(4.0));
    let y = track.center().y;
    let x = x_at(shown);
    painter.line_segment([egui::pos2(track.left(), y), egui::pos2(track.right(), y)], Stroke::new(1.0, k.track.gamma_multiply(alpha)));
    painter.line_segment([egui::pos2(track.left(), y), egui::pos2(x, y)], Stroke::new(1.0, k.active.gamma_multiply(alpha)));
    if let Some(s) = step {
        let fade = theme::motion::tween(ui.ctx(), id.with("ticks"), if ui.is_enabled() && (resp.hovered() || dragging) { 1.0 } else { 0.0 }, theme::motion::BASE);
        let count = ((hi - lo) / s).round() as usize;
        if fade > 0.0 && (2..=40).contains(&count) {
            for i in 1..count {
                let tx = x_at(i as f32 / count as f32);
                painter.line_segment([egui::pos2(tx, y - 3.0), egui::pos2(tx, y + 3.0)], Stroke::new(1.0, k.active.gamma_multiply(fade * alpha)));
            }
        }
    }
    let size = 9.0 * egui::lerp(1.0..=1.4, grow);
    painter.rect_filled(egui::Rect::from_center_size(egui::pos2(x, y), vec2(size, size)), CornerRadius::same(2), k.active.gamma_multiply(alpha));
    changed
}

/// A value whose digits all take the width of the widest one, so numbers
/// don't shift as they change (tabular figures without font features).
fn tabular_value(ui: &mut Ui, text: &str, color: Color32) -> Response {
    let font = egui::FontId::proportional(12.0);
    let (digit_w, widths): (f32, Vec<f32>) = ui.ctx().fonts_mut(|f| {
        let digit_w = ('0'..='9').map(|c| f.glyph_width(&font, c)).fold(0.0, f32::max);
        (digit_w, text.chars().map(|c| f.glyph_width(&font, c)).collect())
    });
    let cells: Vec<f32> = text.chars().zip(&widths).map(|(c, w)| if c.is_ascii_digit() { digit_w } else { *w }).collect();
    let (rect, resp) = ui.allocate_exact_size(vec2(cells.iter().sum(), 16.0), Sense::click());
    let mut x = rect.left();
    for ((c, w), cell) in text.chars().zip(&widths).zip(&cells) {
        ui.painter().text(egui::pos2(x + (cell - w) / 2.0, rect.center().y), egui::Align2::LEFT_CENTER, c, font.clone(), color);
        x += cell;
    }
    resp
}

/// Toolcraft-style switch: a small pill whose thumb eases across.
pub fn switch(ui: &mut Ui, on: &mut bool) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(vec2(28.0, 16.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = theme::motion::tween(ui.ctx(), resp.id, if *on { 1.0 } else { 0.0 }, theme::motion::BASE);
    let alpha = if ui.is_enabled() { 1.0 } else { 0.25 };
    // Toolcraft's faint off track disappears on these panels; use a solid grey.
    let off = if ui.visuals().dark_mode { Color32::from_rgb(0x4a, 0x4a, 0x4d) } else { Color32::from_rgb(0xd9, 0xd9, 0xdc) };
    let bg = lerp_color(off, theme::accent(), t);
    ui.painter().rect_filled(rect, CornerRadius::same(8), bg.gamma_multiply(alpha));
    let x = egui::lerp(rect.left() + 8.0..=rect.right() - 8.0, t);
    ui.painter().circle_filled(egui::pos2(x, rect.center().y), 7.0, Color32::WHITE.gamma_multiply(alpha));
    resp
}

/// Label + switch on one row.
pub fn toggle_row(ui: &mut Ui, label: &str, on: &mut bool) -> bool {
    let mut changed = false;
    row(ui, label, |ui| changed = switch(ui, on).changed());
    changed
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    // Color32 stores premultiplied components; mixing them as such keeps
    // translucent colours right (opaque ones are unaffected).
    Color32::from_rgba_premultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

/// Segmented control in Toolcraft's outline style: equal items across the
/// full width, with the selection sliding between them. Returns true when
/// the selection changed.
pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, value: &mut T, options: &[(T, &str)]) -> bool {
    let id = ui.next_auto_id().with("segmented");
    let p = pal(ui);
    let k = theme::kit(&p);
    let alpha = if ui.is_enabled() { 1.0 } else { 0.25 };
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::hover());
    let inner = rect.shrink(2.0);
    let w = inner.width() / options.len().max(1) as f32;
    let cell = |i: usize| egui::Rect::from_min_size(egui::pos2(inner.left() + i as f32 * w, inner.top()), vec2(w, inner.height()));
    let mut changed = false;
    let mut hovered = None;
    for (i, (v, _)) in options.iter().enumerate() {
        let r = ui.interact(cell(i), id.with(i), Sense::click());
        if r.hovered() {
            hovered = Some(i);
        }
        if r.clicked() && *value != *v {
            *value = *v;
            changed = true;
        }
    }
    let selected = options.iter().position(|(v, _)| *v == *value);

    let painter = ui.painter();
    painter.rect_stroke(rect, CornerRadius::same(6), Stroke::new(1.0, p.control_border.gamma_multiply(alpha)), egui::StrokeKind::Inside);
    if let Some(h) = hovered.filter(|h| Some(*h) != selected) {
        painter.rect_filled(cell(h), CornerRadius::same(4), k.hover_tint.gamma_multiply(alpha));
    }
    if let Some(sel) = selected {
        let at = theme::motion::tween(ui.ctx(), id.with("at"), sel as f32, theme::motion::BASE);
        let r = egui::Rect::from_min_size(egui::pos2(inner.left() + at * w, inner.top()), vec2(w, inner.height()));
        ui.painter().rect_filled(r, CornerRadius::same(4), p.text.gamma_multiply(0.16 * alpha));
    }
    for (i, (_, label)) in options.iter().enumerate() {
        let color = if Some(i) == selected { p.text } else { p.secondary };
        ui.painter().text(cell(i).center(), egui::Align2::CENTER_CENTER, *label, egui::FontId::proportional(12.0), color.gamma_multiply(alpha));
    }
    changed
}

/// Primary (accent) button.
pub fn primary(ui: &mut Ui, text: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(text).color(Color32::WHITE).font(theme::semibold(theme::BODY)))
            .fill(theme::accent())
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(7))
            .min_size(vec2(0.0, 26.0)),
    )
}

/// Big round record button.
pub fn record_button(ui: &mut Ui, size: f32, recording: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let hover = resp.hovered();
    let c = rect.center();
    ui.painter().circle_filled(c, size / 2.0, if hover { Color32::from_rgb(255, 80, 70) } else { Color32::from_rgb(255, 59, 48) });
    if recording {
        let s = size * 0.28;
        ui.painter().rect_filled(egui::Rect::from_center_size(c, vec2(s, s)), CornerRadius::same(3), Color32::WHITE);
    } else {
        ui.painter().circle_filled(c, size * 0.18, Color32::WHITE);
    }
    resp
}

/// sRGB colour picker on a project colour.
pub fn color_edit(ui: &mut Ui, c: &mut crate::project::model::Color) -> bool {
    let mut col = Color32::from_rgba_unmultiplied(c.0[0], c.0[1], c.0[2], c.0[3]);
    let changed = egui::color_picker::color_edit_button_srgba(ui, &mut col, egui::color_picker::Alpha::Opaque).changed();
    if changed {
        let [r, g, b, a] = col.to_srgba_unmultiplied();
        c.0 = [r, g, b, a];
    }
    changed
}

/// Small secondary caption text.
pub fn caption(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(theme::SECONDARY).color(pal(ui).secondary));
}

/// Format milliseconds as m:ss.t
pub fn fmt_time(ms: f64) -> String {
    let s = (ms / 1000.0).max(0.0);
    format!("{}:{:04.1}", (s / 60.0) as u64, s % 60.0)
}

#[derive(Clone, Copy, PartialEq)]
pub enum Icon {
    Undo,
    Redo,
    Restart,
    Close,
    Pause,
    Play,
}

/// Small borderless toolbar button with a vector icon (Inter has no arrows).
pub fn icon_button(ui: &mut Ui, icon: Icon, enabled: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(26.0, 22.0), if enabled { Sense::click() } else { Sense::hover() });
    let p = pal(ui);
    if enabled && resp.hovered() {
        let fill = if resp.is_pointer_button_down_on() { p.pressed } else { p.hover };
        ui.painter().rect_filled(rect, CornerRadius::same(5), fill);
    }
    let color = if enabled { p.text } else { p.tertiary };
    let stroke = Stroke::new(1.5, color);
    let c = rect.center();
    let painter = ui.painter();
    let arc = |from: f32, to: f32, r: f32, cy: f32| -> Vec<egui::Pos2> {
        (0..=16)
            .map(|i| {
                let a = from + (to - from) * i as f32 / 16.0;
                egui::pos2(c.x + r * a.cos(), cy - r * a.sin())
            })
            .collect()
    };
    use std::f32::consts::PI;
    match icon {
        Icon::Undo | Icon::Redo => {
            let flip = if icon == Icon::Redo { -1.0 } else { 1.0 };
            let mut pts = vec![egui::pos2(c.x + flip * 5.5, c.y + 5.0)];
            pts.extend(arc(0.0, PI, 5.5, c.y + 1.5).into_iter().map(|q| egui::pos2(c.x + (q.x - c.x) * flip, q.y)));
            let end = *pts.last().unwrap();
            painter.add(egui::Shape::line(pts, stroke));
            painter.line_segment([end, egui::pos2(end.x - 3.0, end.y - 3.0)], stroke);
            painter.line_segment([end, egui::pos2(end.x + 3.0, end.y - 3.0)], stroke);
        }
        Icon::Restart => {
            let pts = arc(PI * 0.35, PI * 2.15, 6.0, c.y);
            let end = pts[0];
            painter.add(egui::Shape::line(pts, stroke));
            painter.line_segment([end, egui::pos2(end.x - 3.5, end.y - 0.5)], stroke);
            painter.line_segment([end, egui::pos2(end.x + 0.5, end.y + 3.5)], stroke);
        }
        Icon::Close => {
            let d = 4.5;
            painter.line_segment([c + vec2(-d, -d), c + vec2(d, d)], stroke);
            painter.line_segment([c + vec2(-d, d), c + vec2(d, -d)], stroke);
        }
        Icon::Pause => {
            for dx in [-3.0, 3.0] {
                painter.rect_filled(egui::Rect::from_center_size(c + vec2(dx, 0.0), vec2(3.0, 11.0)), CornerRadius::same(1), color);
            }
        }
        Icon::Play => {
            let pts = vec![c + vec2(-4.0, -6.0), c + vec2(6.0, 0.0), c + vec2(-4.0, 6.0)];
            painter.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
        }
    }
    resp
}

/// Outline of a camera bubble filling `rect`, clockwise: a rounded rect with
/// corner radius `radius`, or a superellipse when `n` > 0.
pub fn bubble_outline(rect: egui::Rect, radius: f32, n: f32) -> Vec<egui::Pos2> {
    let c = rect.center();
    let (hw, hh) = (rect.width() / 2.0, rect.height() / 2.0);
    if n > 0.0 {
        return (0..128)
            .map(|i| {
                let t = i as f32 / 128.0 * std::f32::consts::TAU;
                let (ct, st) = (t.cos(), t.sin());
                c + vec2(ct.signum() * ct.abs().powf(2.0 / n) * hw, st.signum() * st.abs().powf(2.0 / n) * hh)
            })
            .collect();
    }
    let r = radius.clamp(0.0, hw.min(hh));
    let mut pts = Vec::with_capacity(4 * 17);
    // Corner centres, starting top right and going clockwise (y down).
    for (k, (sx, sy)) in [(1.0, -1.0), (1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0)].into_iter().enumerate() {
        let cc = c + vec2(sx * (hw - r), sy * (hh - r));
        let a0 = -std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::FRAC_PI_2;
        for j in 0..=16 {
            let a = a0 + j as f32 / 16.0 * std::f32::consts::FRAC_PI_2;
            pts.push(cc + vec2(a.cos(), a.sin()) * r);
        }
    }
    pts
}

/// A small icon of a camera shape (with the current roundness).
fn shape_icon(ui: &Ui, rect: egui::Rect, c: &crate::project::model::CameraStyle, color: Color32) {
    let (w, h) = if c.shape.is_square() { (16.0, 16.0) } else { (22.0, 13.0) };
    let r = egui::Rect::from_center_size(rect.center(), vec2(w, h));
    let pts = bubble_outline(r, c.radius(w as f64, h as f64) as f32, c.squircle_exponent() as f32);
    ui.painter().add(egui::Shape::closed_line(pts, Stroke::new(1.5, color)));
}

/// The camera bubble's look and position, as in Rec: shape, size,
/// roundness, border, corner, shadow and mirror. Returns true when changed.
pub fn camera_look(ui: &mut Ui, c: &mut crate::project::model::CameraStyle) -> bool {
    use crate::project::model::{CameraShape, CameraStyle};
    let before = *c;
    let p = pal(ui);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let bw = ((ui.available_width() - 16.0) / 5.0).max(30.0);
        for shape in CameraShape::ALL {
            let sel = c.shape == shape;
            let (rect, resp) = ui.allocate_exact_size(vec2(bw, 30.0), Sense::click());
            let fill = if sel { theme::accent() } else if resp.hovered() { p.hover } else { p.control };
            ui.painter().rect_filled(rect, CornerRadius::same(6), fill);
            shape_icon(ui, rect, &CameraStyle { shape, ..*c }, if sel { Color32::WHITE } else { p.text });
            if resp.on_hover_text(shape.label()).clicked() {
                c.shape = shape;
            }
        }
    });
    ui.add_space(4.0);
    let pct = |ui: &mut Ui, label: &str, v: &mut f64, range: std::ops::RangeInclusive<f64>, scale: f64, enabled: bool| {
        ui.horizontal(|ui| {
            ui.label(label);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut x = *v * scale;
                let r = (*range.start() * scale)..=(*range.end() * scale);
                if ui.add_enabled(enabled, egui::Slider::new(&mut x, r).suffix(" %").integer().trailing_fill(true)).changed() {
                    *v = x / scale;
                }
            });
        });
    };
    pct(ui, "Size", &mut c.size, 0.1..=0.8, 100.0, true);
    // Roundness 0..0.5 of the short side shows as 0..100 %.
    pct(ui, "Roundness", &mut c.roundness, 0.0..=0.5, 200.0, c.shape.uses_roundness());
    ui.horizontal(|ui| {
        ui.label("Border");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_enabled_ui(c.border > 0.0, |ui| color_edit(ui, &mut c.border_color));
            ui.add(egui::Slider::new(&mut c.border, 0.0..=20.0).suffix(" px").integer().trailing_fill(true));
        });
    });
    row(ui, "Position", |ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        // Right to left: the last corner is drawn first.
        for (label, tip, pos) in [("↘", "Bottom right", (1.0, 1.0)), ("↙", "Bottom left", (0.0, 1.0)), ("↗", "Top right", (1.0, 0.0)), ("↖", "Top left", (0.0, 0.0))] {
            let sel = (c.position.x, c.position.y) == pos;
            if ui.add(egui::Button::selectable(sel, label).min_size(vec2(26.0, 22.0))).on_hover_text(tip).clicked() {
                c.position = crate::engine::geom::v2(pos.0, pos.1);
            }
        }
    });
    toggle_row(ui, "Shadow", &mut c.shadow);
    toggle_row(ui, "Mirror", &mut c.mirror);
    *c != before
}

#[cfg(test)]
mod tests {
    use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, pos2, vec2};

    /// Drives real egui frames with synthetic input.
    struct Rig {
        ctx: egui::Context,
        time: f64,
    }

    impl Rig {
        fn new() -> Self {
            Rig { ctx: egui::Context::default(), time: 0.0 }
        }

        /// One frame; `add` returns the rects it wants to report.
        fn frame<R>(&mut self, events: Vec<Event>, mut add: impl FnMut(&mut egui::Ui) -> R) -> R {
            self.time += 1.0 / 60.0;
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(300.0, 400.0))),
                time: Some(self.time),
                events,
                ..Default::default()
            };
            let mut out = None;
            let _ = self.ctx.run_ui(input, |ui| {
                egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| out = Some(add(ui)));
            });
            out.unwrap()
        }

        fn idle<R>(&mut self, add: impl FnMut(&mut egui::Ui) -> R) -> R {
            self.frame(vec![], add)
        }

        fn click<R>(&mut self, at: Pos2, mut add: impl FnMut(&mut egui::Ui) -> R) -> R {
            self.frame(vec![Event::PointerMoved(at), press(at, true)], &mut add);
            self.frame(vec![press(at, false)], &mut add)
        }

        fn key<R>(&mut self, key: Key, add: impl FnMut(&mut egui::Ui) -> R) -> R {
            let k = |pressed| Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: Modifiers::NONE };
            self.frame(vec![k(true), k(false)], add)
        }
    }

    fn press(pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
    }

    /// Lays out one slider and reports where its value text and track are:
    /// (value-text point, track rect).
    fn measured(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) -> (Pos2, Rect) {
        let top = ui.cursor().top();
        add(ui);
        let bottom = ui.cursor().top() - ui.spacing().item_spacing.y;
        let w = ui.max_rect().width();
        (pos2(w - 4.0, top + 8.0), Rect::from_min_max(pos2(5.0, bottom - 18.0), pos2(w - 5.0, bottom)))
    }

    fn thumb_x(track: Rect, v: f64, lo: f64, hi: f64) -> f32 {
        egui::lerp(track.x_range(), ((v - lo) / (hi - lo)) as f32)
    }

    #[test]
    fn out_of_range_value_is_not_rewritten() {
        let mut rig = Rig::new();
        let mut v = 2000.0;
        for _ in 0..3 {
            let changed = rig.idle(|ui| super::slider(ui, "Rigidity", &mut v, 5.0..=1500.0, "", 170.0));
            assert!(!changed);
        }
        assert_eq!(v, 2000.0);
    }

    #[test]
    fn same_label_sliders_keep_separate_state() {
        let mut rig = Rig::new();
        let (mut a, mut b) = (30.0, 30.0);
        let mut ui_fn = |ui: &mut egui::Ui| {
            let first = measured(ui, |ui| {
                super::slider(ui, "Blur", &mut a, 0.0..=60.0, " px", 0.0);
            });
            super::slider(ui, "Blur", &mut b, 0.0..=60.0, " px", 0.0);
            first
        };
        let (value_at, _) = rig.idle(&mut ui_fn);
        rig.click(value_at, &mut ui_fn);
        rig.idle(&mut ui_fn);
        rig.frame(vec![Event::Text("7".into())], &mut ui_fn);
        rig.key(Key::Enter, &mut ui_fn);
        rig.idle(&mut ui_fn);
        assert_eq!(a, 7.0);
        assert_eq!(b, 30.0);
    }

    #[test]
    fn double_click_thumb_resets_to_default() {
        let mut rig = Rig::new();
        let mut v = 30.0;
        let mut ui_fn = |ui: &mut egui::Ui| measured(ui, |ui| {
            super::slider(ui, "Padding", &mut v, 0.0..=60.0, "%", 16.0);
        });
        let (_, track) = rig.idle(&mut ui_fn);
        let at = pos2(thumb_x(track, 30.0, 0.0, 60.0), track.center().y);
        rig.click(at, &mut ui_fn);
        rig.click(at, &mut ui_fn);
        rig.idle(&mut ui_fn);
        assert_eq!(v, 16.0);
    }

    #[test]
    fn arrow_nudges_focused_slider_and_owns_the_keyboard() {
        let mut rig = Rig::new();
        let v = std::cell::RefCell::new(30.0);
        let mut ui_fn = |ui: &mut egui::Ui| measured(ui, |ui| {
            super::slider(ui, "Padding", &mut v.borrow_mut(), 0.0..=60.0, "%", 16.0);
        });
        let (_, track) = rig.idle(&mut ui_fn);
        rig.click(pos2(thumb_x(track, 30.0, 0.0, 60.0), track.center().y), &mut ui_fn);
        assert!(super::keyboard_owned(&rig.ctx));
        let before = *v.borrow();
        rig.key(Key::ArrowRight, &mut ui_fn);
        let after = *v.borrow();
        assert!((after - (before + 0.6)).abs() < 1e-9, "{before} -> {after}");
    }

    #[test]
    fn clicking_elsewhere_releases_the_keyboard() {
        let mut rig = Rig::new();
        let mut v = 30.0;
        let mut ui_fn = |ui: &mut egui::Ui| measured(ui, |ui| {
            super::slider(ui, "Padding", &mut v, 0.0..=60.0, "%", 16.0);
        });
        let (_, track) = rig.idle(&mut ui_fn);
        rig.click(track.center(), &mut ui_fn);
        assert!(super::keyboard_owned(&rig.ctx));
        rig.click(pos2(150.0, 350.0), &mut ui_fn);
        rig.idle(&mut ui_fn);
        assert!(!super::keyboard_owned(&rig.ctx));
    }

    #[test]
    fn segmented_click_selects() {
        let mut rig = Rig::new();
        let mut value = 0;
        let mut changed = false;
        let mut ui_fn = |ui: &mut egui::Ui| {
            let top = ui.cursor().top();
            changed |= super::segmented(ui, &mut value, &[(0, "None"), (1, "Circle"), (2, "Ripple")]);
            Rect::from_min_max(pos2(0.0, top), pos2(ui.max_rect().width(), ui.cursor().top() - ui.spacing().item_spacing.y))
        };
        let r = rig.idle(&mut ui_fn);
        rig.click(pos2(r.left() + r.width() * 5.0 / 6.0, r.center().y), &mut ui_fn);
        drop(ui_fn);
        assert_eq!(value, 2);
        assert!(changed);
    }

    #[test]
    fn segmented_items_fill_width() {
        let mut rig = Rig::new();
        let mut value = 0;
        let width = rig.idle(|ui| {
            ui.set_max_width(300.0);
            ui.scope(|ui| super::segmented(ui, &mut value, &[(0, "A"), (1, "B")])).response.rect.width()
        });
        assert!((width - 300.0).abs() <= 1.0, "{width}");
    }

    #[test]
    fn switch_toggles_on_click() {
        let mut rig = Rig::new();
        let mut on = false;
        let mut ui_fn = |ui: &mut egui::Ui| super::switch(ui, &mut on).rect;
        let r = rig.idle(&mut ui_fn);
        assert_eq!(r.size(), vec2(28.0, 16.0));
        rig.click(r.center(), &mut ui_fn);
        drop(ui_fn);
        assert!(on);
    }

    #[test]
    fn lerp_color_keeps_translucent_colours() {
        let faint = egui::Color32::from_rgb(0xec, 0xec, 0xee).gamma_multiply(0.2);
        assert_eq!(super::lerp_color(faint, egui::Color32::BLUE, 0.0), faint);
        assert_eq!(super::lerp_color(faint, egui::Color32::BLUE, 1.0), egui::Color32::BLUE);
    }

    #[test]
    fn dragging_past_the_end_clamps() {
        let mut rig = Rig::new();
        let mut v = 30.0;
        let mut ui_fn = |ui: &mut egui::Ui| measured(ui, |ui| {
            super::slider(ui, "Padding", &mut v, 0.0..=60.0, "%", 16.0);
        });
        let (_, track) = rig.idle(&mut ui_fn);
        let start = track.center();
        rig.frame(vec![Event::PointerMoved(start), press(start, true)], &mut ui_fn);
        for dx in [20.0, 100.0, 300.0, 500.0] {
            rig.frame(vec![Event::PointerMoved(start + vec2(dx, 0.0))], &mut ui_fn);
        }
        rig.frame(vec![press(start + vec2(500.0, 0.0), false)], &mut ui_fn);
        assert_eq!(v, 60.0);
    }
}
