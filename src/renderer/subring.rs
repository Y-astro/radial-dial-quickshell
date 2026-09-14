// src/renderer/subring.rs
#![allow(dead_code)]

use crate::font::FontRenderer;
use crate::renderer::pie::{
    draw_floating_wedge, CORNER_RADIUS_SUB, SUB_ICON_R, SUB_INNER_R, SUB_OUTER_R,
};
use crate::renderer::text::draw_icon;
use crate::state::actions::SliceItem;
use tiny_skia::{
    Color, FillRule, GradientStop, Paint, PathBuilder, Pixmap, Point, RadialGradient, SpreadMode,
    Stroke, Transform,
};

// ─── Local colour helpers ─────────────────────────────────────────────────────

fn lighten_color(c: Color, factor: f32) -> Color {
    let r = (c.red() * factor).min(1.0);
    let g = (c.green() * factor).min(1.0);
    let b = (c.blue() * factor).min(1.0);
    Color::from_rgba(r, g, b, c.alpha()).unwrap_or(c)
}

fn darken_color(c: Color, factor: f32) -> Color {
    Color::from_rgba(c.red() / factor, c.green() / factor, c.blue() / factor, c.alpha())
        .unwrap_or(c)
}

/// Multiply a color's alpha by `alpha_mul` (0..1).
#[inline(always)]
fn fade_color(c: Color, alpha_mul: f32) -> Color {
    Color::from_rgba(c.red(), c.green(), c.blue(), (c.alpha() * alpha_mul).clamp(0.0, 1.0))
        .unwrap_or(c)
}

// ─── Easing helpers (inlined for performance) ─────────────────────────────────

/// OutBack: springy overshoot. `s` controls overshoot strength (0.0 = no overshoot).
#[inline(always)]
fn out_back(t: f32, s: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let inv = t - 1.0;
    1.0 + (s + 1.0) * inv * inv * inv + s * inv * inv
}

/// OutCubic: fast-start slow-end.
#[inline(always)]
fn out_cubic(t: f32) -> f32 {
    let inv = 1.0 - t.clamp(0.0, 1.0);
    1.0 - inv * inv * inv
}

/// InCubic: slow-start fast-end (for collapse).
#[inline(always)]
fn in_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * t
}

// ─── Animation constants ──────────────────────────────────────────────────────

/// ms between successive wedge close starts (last wedge → first wedge)
const CLOSE_STAGGER_MS: f32 = 28.0;
/// ms for each individual wedge to fully collapse
const CLOSE_WEDGE_MS: f32 = 100.0;
/// Gap between wedges as a fraction of their angular width
const GAP_FRACTION: f32 = 0.07;

// ─── Core renderer ────────────────────────────────────────────────────────────

/// Renders sub-arc wedges with spring-bloom open and reverse-stagger close.
///
/// # Arguments
/// - `reveal_progress` — `0.0..N` open stagger ramp; wedge `j` becomes visible
///   once `reveal_progress > j`.
/// - `sub_closing_progress` — `0.0..1.0` global close progress (0 = open/opening,
///   >0 = collapsing). Drives a reverse-staggered per-wedge collapse.
pub fn draw_sub_ring(
    pixmap: &mut Pixmap,
    cx: f32,
    cy: f32,
    sub_slices: &[SliceItem],
    start_angle_deg: f32,
    slice_width_deg: f32,
    hovered_index: i32,
    hover_factor: f32,
    reveal_progress: f32,
    sub_closing_progress: f32,
    primary_col: Color,
    is_low_end: bool,
) {
    let m = sub_slices.len();
    if m == 0 || reveal_progress <= 0.0 {
        return;
    }

    // Total close timeline: last wedge starts at t=0, first wedge starts at
    // t = (m-1)*CLOSE_STAGGER_MS, each runs for CLOSE_WEDGE_MS.
    let total_close_span = CLOSE_WEDGE_MS + (m - 1) as f32 * CLOSE_STAGGER_MS;
    let close_elapsed_ms = sub_closing_progress * total_close_span;

    for j in 0..m {
        // ── Open stagger ─────────────────────────────────────────────────────
        let raw_p = (reveal_progress - j as f32).clamp(0.0, 1.0);
        if raw_p <= 0.0 {
            continue;
        }

        // ── Close stagger (reverse: last wedge closes first) ─────────────────
        // delay for wedge j: last wedge (j=m-1) → 0 ms, first (j=0) → (m-1)*STAGGER
        let close_delay = (m - 1 - j) as f32 * CLOSE_STAGGER_MS;
        let wedge_close_t = if sub_closing_progress > 0.0 {
            ((close_elapsed_ms - close_delay) / CLOSE_WEDGE_MS).clamp(0.0, 1.0)
        } else {
            0.0
        };

        // ── Per-wedge open easing ─────────────────────────────────────────────
        // OutBack spring for radial bloom; slight overshoot makes it feel springy
        let open_ease = if is_low_end { raw_p } else { out_back(raw_p, 0.30) };
        // Quadratic opacity ramp — wedge starts transparent and solidifies quickly
        let open_opacity = raw_p * raw_p;

        // ── Per-wedge close easing ────────────────────────────────────────────
        let close_ease    = if sub_closing_progress > 0.0 { in_cubic(wedge_close_t) } else { 0.0 };
        let close_opacity = if sub_closing_progress > 0.0 { 1.0 - wedge_close_t } else { 1.0 };

        // Combined alpha and radial progress
        let alpha_mul = (open_opacity * close_opacity).clamp(0.0, 1.0);
        // radial_t drives SUB_INNER_R → SUB_OUTER_R; allow slight OutBack overshoot
        let radial_t  = (open_ease - close_ease).clamp(0.0, 1.35);

        if alpha_mul < 0.01 {
            continue;
        }

        // ── Hover lift ────────────────────────────────────────────────────────
        let is_hov = j as i32 == hovered_index;
        let r_lift = if is_hov { 5.0 * hover_factor } else { 0.0 };

        // ── Radial bloom ──────────────────────────────────────────────────────
        // Outer edge springs from SUB_INNER_R → SUB_OUTER_R (+ hover lift)
        let travel = SUB_OUTER_R - SUB_INNER_R;
        let current_outer = (SUB_INNER_R + travel * radial_t.min(1.0) + r_lift)
            .max(SUB_INNER_R + 2.0);

        // ── Angular squeeze-open ──────────────────────────────────────────────
        // Wedge starts as a narrow sliver at its angular center then fans out.
        // Only applies during open phase (not while closing — keep full width).
        let squeeze = if is_low_end || sub_closing_progress > 0.0 {
            1.0
        } else {
            GAP_FRACTION + (1.0 - GAP_FRACTION) * out_cubic(open_ease.min(1.0))
        };

        let arc_center = start_angle_deg + (j as f32 + 0.5) * slice_width_deg;
        let arc_half   = slice_width_deg * 0.5 * squeeze;
        // Apply a fixed angular gap between wedges
        let gap_deg    = slice_width_deg * GAP_FRACTION * 0.5;
        let start_deg  = arc_center - arc_half + gap_deg;
        let end_deg    = arc_center + arc_half - gap_deg;

        let mut pb = PathBuilder::new();
        draw_floating_wedge(
            &mut pb,
            cx,
            cy,
            SUB_INNER_R,
            current_outer,
            start_deg.to_radians(),
            end_deg.to_radians(),
            CORNER_RADIUS_SUB,
        );

        let Some(path) = pb.finish() else { continue };

        // ── Fill & stroke ─────────────────────────────────────────────────────
        if is_hov && sub_closing_progress < 0.01 {
            // Hovered wedge: primary-colour gradient
            let light  = fade_color(lighten_color(primary_col, 1.30), alpha_mul);
            let mid    = fade_color(primary_col, alpha_mul);
            let dark   = fade_color(darken_color(primary_col, 1.10), alpha_mul);

            let grad = RadialGradient::new(
                Point::from_xy(cx, cy),
                Point::from_xy(cx, cy),
                current_outer + 10.0,
                vec![
                    GradientStop::new(0.0, light),
                    GradientStop::new(0.40, mid),
                    GradientStop::new(1.0, dark),
                ],
                SpreadMode::Pad,
                Transform::identity(),
            );
            let mut paint = Paint::default();
            paint.anti_alias = true;
            if let Some(shader) = grad { paint.shader = shader; } else { paint.set_color(mid); }
            pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);

            let mut sp = Paint::default();
            sp.set_color(light);
            sp.anti_alias = true;
            pixmap.stroke_path(&path, &sp, &Stroke { width: 1.8, ..Default::default() },
                               Transform::identity(), None);
        } else {
            // Idle wedge: frosted glass
            let (s0_base, s1_base) = if is_low_end {
                (
                    Color::from_rgba(0.11, 0.11, 0.14, 0.90).unwrap_or(Color::BLACK),
                    Color::from_rgba(0.07, 0.07, 0.09, 0.85).unwrap_or(Color::BLACK),
                )
            } else {
                (
                    Color::from_rgba(0.08, 0.08, 0.10, 0.50).unwrap_or(Color::BLACK),
                    Color::from_rgba(0.05, 0.05, 0.06, 0.40).unwrap_or(Color::BLACK),
                )
            };
            let s0 = fade_color(s0_base, alpha_mul);
            let s1 = fade_color(s1_base, alpha_mul);

            let grad = RadialGradient::new(
                Point::from_xy(cx, cy),
                Point::from_xy(cx, cy),
                current_outer,
                vec![GradientStop::new(0.0, s0), GradientStop::new(1.0, s1)],
                SpreadMode::Pad,
                Transform::identity(),
            );
            let mut paint = Paint::default();
            paint.anti_alias = true;
            if let Some(shader) = grad { paint.shader = shader; } else { paint.set_color(s0); }
            pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);

            // White wash overlay
            let mut wp = Paint::default();
            wp.set_color(Color::from_rgba(1.0, 1.0, 1.0, 0.06 * alpha_mul).unwrap_or(Color::WHITE));
            wp.anti_alias = true;
            pixmap.fill_path(&path, &wp, FillRule::Winding, Transform::identity(), None);

            // Border stroke
            let mut sp = Paint::default();
            sp.set_color(Color::from_rgba(1.0, 1.0, 1.0, 0.24 * alpha_mul).unwrap_or(Color::WHITE));
            sp.anti_alias = true;
            pixmap.stroke_path(&path, &sp, &Stroke { width: 1.0, ..Default::default() },
                               Transform::identity(), None);
        }
    }
}

// ─── Icon renderer ────────────────────────────────────────────────────────────

/// Renders icons on top of sub-ring wedges, matching the same open/close alpha.
pub fn draw_sub_ring_icons(
    font_renderer: &mut FontRenderer,
    pixmap: &mut Pixmap,
    cx: f32,
    cy: f32,
    sub_slices: &[SliceItem],
    start_angle_deg: f32,
    slice_width_deg: f32,
    hovered_index: i32,
    hover_factor: f32,
    reveal_progress: f32,
    sub_closing_progress: f32,
    on_primary_col: Color,
) {
    let m = sub_slices.len();
    if m == 0 || reveal_progress <= 0.0 {
        return;
    }

    let total_close_span = CLOSE_WEDGE_MS + (m - 1) as f32 * CLOSE_STAGGER_MS;
    let close_elapsed_ms = sub_closing_progress * total_close_span;

    for j in 0..m {
        let raw_p = (reveal_progress - j as f32).clamp(0.0, 1.0);
        if raw_p <= 0.0 { continue; }

        let close_delay   = (m - 1 - j) as f32 * CLOSE_STAGGER_MS;
        let wedge_close_t = if sub_closing_progress > 0.0 {
            ((close_elapsed_ms - close_delay) / CLOSE_WEDGE_MS).clamp(0.0, 1.0)
        } else { 0.0 };

        let open_opacity  = raw_p * raw_p;
        let close_opacity = if sub_closing_progress > 0.0 { 1.0 - wedge_close_t } else { 1.0 };
        let alpha = (open_opacity * close_opacity).clamp(0.0, 1.0);
        if alpha < 0.01 { continue; }

        let is_hov   = j as i32 == hovered_index;
        let r_lift   = if is_hov { 5.0 * hover_factor } else { 0.0 };
        let cur_icon_r = SUB_ICON_R + r_lift / 2.0;
        let mid_rad    = (start_angle_deg + (j as f32 + 0.5) * slice_width_deg).to_radians();
        let icon_x     = cx + cur_icon_r * mid_rad.cos();
        let icon_y     = cy + cur_icon_r * mid_rad.sin();

        let icon_size  = if is_hov { 26.0 } else { 22.0 };
        let icon_color = if is_hov {
            fade_color(on_primary_col, alpha)
        } else {
            Color::from_rgba(1.0, 1.0, 1.0, 0.95 * alpha).unwrap_or(Color::WHITE)
        };

        draw_icon(font_renderer, pixmap, &sub_slices[j].icon, icon_x, icon_y, icon_size, icon_color);
    }
}

// ─── Combined pass ────────────────────────────────────────────────────────────

/// Renders both wedges and icons in one call.
pub fn draw_sub_ring_with_icons(
    font_renderer: &mut FontRenderer,
    pixmap: &mut Pixmap,
    cx: f32,
    cy: f32,
    sub_slices: &[SliceItem],
    start_angle_deg: f32,
    slice_width_deg: f32,
    hovered_index: i32,
    hover_factor: f32,
    reveal_progress: f32,
    sub_closing_progress: f32,
    primary_col: Color,
    on_primary_col: Color,
    is_low_end: bool,
) {
    draw_sub_ring(
        pixmap, cx, cy, sub_slices, start_angle_deg, slice_width_deg,
        hovered_index, hover_factor, reveal_progress, sub_closing_progress,
        primary_col, is_low_end,
    );
    draw_sub_ring_icons(
        font_renderer, pixmap, cx, cy, sub_slices, start_angle_deg, slice_width_deg,
        hovered_index, hover_factor, reveal_progress, sub_closing_progress,
        on_primary_col,
    );
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sub_ring_drawing() {
        let mut pixmap = Pixmap::new(600, 600).unwrap();
        let slices = vec![
            SliceItem::new("tab1", "Tab 1", "counter_1", 0),
            SliceItem::new("tab2", "Tab 2", "counter_2", 1),
            SliceItem::new("tab3", "Tab 3", "counter_3", 2),
            SliceItem::new("tab4", "Tab 4", "counter_4", 3),
        ];
        let primary = Color::from_rgba8(137, 180, 250, 255);

        draw_sub_ring(
            &mut pixmap, 300.0, 300.0, &slices, -45.0, 22.5,
            1, 1.0, 4.0, 0.0, primary, false,
        );

        let non_zero = pixmap.pixels().iter().filter(|p| p.alpha() > 0).count();
        assert!(non_zero > 2000, "Expected non-zero pixels, got {}", non_zero);
    }

    #[test]
    fn test_sub_ring_closing_animation() {
        let slices = vec![
            SliceItem::new("t1", "T1", "counter_1", 0),
            SliceItem::new("t2", "T2", "counter_2", 1),
            SliceItem::new("t3", "T3", "counter_3", 2),
        ];
        let primary = Color::from_rgba8(137, 180, 250, 255);
        let args = (-45.0_f32, 30.0_f32, -1_i32, 0.0_f32);

        let count_alpha = |progress: f32| {
            let mut pm = Pixmap::new(600, 600).unwrap();
            draw_sub_ring(&mut pm, 300.0, 300.0, &slices, args.0, args.1,
                          args.2, args.3, 3.0, progress, primary, false);
            pm.pixels().iter().filter(|p| p.alpha() > 10).count()
        };

        let px_open   = count_alpha(0.0);
        let px_mid    = count_alpha(0.5);
        let px_closed = count_alpha(1.0);

        assert!(px_open   > 1000,    "Fully open: {}", px_open);
        assert!(px_mid    < px_open, "Half-closed < open: {} vs {}", px_mid, px_open);
        assert!(px_closed < px_mid,  "Closed < half: {} vs {}", px_closed, px_mid);
    }

    #[test]
    fn test_sub_ring_spring_overshoot() {
        // OutBack(0.30) should allow the outer edge to briefly exceed SUB_OUTER_R.
        // At raw_p=1.0, out_back(1.0, 0.30) == 1.0 (no overshoot at t=1, only mid-travel).
        // At raw_p=0.85, out_back > 1.0 → current_outer > SUB_OUTER_R.
        let t = 0.85_f32;
        let v = out_back(t, 0.30);
        assert!(v > 1.0, "OutBack(0.30) should overshoot at t=0.85, got {}", v);
    }
}
