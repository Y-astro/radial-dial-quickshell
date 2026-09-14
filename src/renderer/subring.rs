// src/renderer/subring.rs
#![allow(dead_code)]

use crate::font::FontRenderer;
use crate::renderer::pie::{
    draw_floating_wedge, CORNER_RADIUS_SUB, SUB_INNER_R, SUB_OUTER_R,
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

// ─── Easing helpers (inlined for zero-overhead hot path) ──────────────────────

/// OutBack: springy bloom. `s` controls overshoot strength.
#[inline(always)]
fn out_back(t: f32, s: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let inv = t - 1.0;
    1.0 + (s + 1.0) * inv * inv * inv + s * inv * inv
}

/// OutCubic: fast initial pop, decelerates smoothly into place.
#[inline(always)]
fn out_cubic(t: f32) -> f32 {
    let inv = 1.0 - t.clamp(0.0, 1.0);
    1.0 - inv * inv * inv
}

/// InCubic: slow start, accelerates as it retracts into main ring.
#[inline(always)]
fn in_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * t
}

/// InQuad: gentle quadratic acceleration for fading out.
#[inline(always)]
fn in_quad(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t
}

// ─── Animation constants ──────────────────────────────────────────────────────

/// Gap between wedges as a fraction of their angular width
const GAP_FRACTION: f32 = 0.07;

// ─── Core wedge renderer ──────────────────────────────────────────────────────

/// Renders sub-arc wedges with symmetrical spring-bloom open and smooth collapse.
///
/// # Symmetrical Arc Blossom:
/// Wedges directly over the parent slice (the center of the arc) bloom outward first.
/// The outer wedges unfurl symmetrically to the left and right, like flower petals
/// opening directly out of the parent slice.
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

    // Normalized open progress across 0.0..1.0
    let norm_open = (reveal_progress / m as f32).clamp(0.0, 1.0);
    let mid_idx = (m - 1) as f32 / 2.0;
    let max_dist = mid_idx.max(0.5);

    for j in 0..m {
        // Distance from center of arc (0.0 = center over parent slice, 1.0 = extreme ends)
        let norm_dist = ((j as f32 - mid_idx).abs() / max_dist).clamp(0.0, 1.0);

        // ── Symmetrical Open: center blooms at t=0, edges bloom by t=0.22 ──
        let open_delay = norm_dist * 0.22;
        let wedge_open_raw = ((norm_open - open_delay) / (1.0 - 0.22)).clamp(0.0, 1.0);
        if wedge_open_raw <= 0.0 {
            continue;
        }

        // ── Symmetrical Close: edges retract first towards center ────────────
        let close_delay = (1.0 - norm_dist) * 0.18;
        let wedge_close_raw = if sub_closing_progress > 0.0 {
            ((sub_closing_progress - close_delay) / (1.0 - 0.18)).clamp(0.0, 1.0)
        } else {
            0.0
        };

        // Open easings:
        // OutBack(0.32) gives an energetic spring bounce that overshoots SUB_OUTER_R slightly
        let open_radial = if is_low_end { wedge_open_raw } else { out_back(wedge_open_raw, 0.32) };
        let open_alpha  = out_cubic(wedge_open_raw);

        // Close easings:
        let close_radial = if sub_closing_progress > 0.0 { in_cubic(wedge_close_raw) } else { 0.0 };
        let close_alpha  = if sub_closing_progress > 0.0 { 1.0 - in_quad(wedge_close_raw) } else { 1.0 };

        // Combined radial travel factor and opacity
        let net_radial = (open_radial * (1.0 - close_radial)).max(0.0);
        let alpha_mul  = (open_alpha * close_alpha).clamp(0.0, 1.0);

        if alpha_mul < 0.01 {
            continue;
        }

        // Hover lift
        let is_hov = j as i32 == hovered_index;
        let r_lift = if is_hov { 5.0 * hover_factor } else { 0.0 };

        // Radial bloom: outer edge pushes outward from SUB_INNER_R past SUB_OUTER_R with spring overshoot
        let travel = SUB_OUTER_R - SUB_INNER_R;
        let current_outer = (SUB_INNER_R + travel * net_radial + r_lift).max(SUB_INNER_R + 2.0);

        // Angular squeeze: wedge unfurls from 50% width to full width as it blooms
        let squeeze = if is_low_end || sub_closing_progress > 0.0 {
            1.0
        } else {
            0.50 + 0.50 * out_cubic(wedge_open_raw)
        };

        let arc_center = start_angle_deg + (j as f32 + 0.5) * slice_width_deg;
        let arc_half   = slice_width_deg * 0.5 * squeeze;
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

            // Translucent white wash
            let mut wp = Paint::default();
            wp.set_color(Color::from_rgba(1.0, 1.0, 1.0, 0.06 * alpha_mul).unwrap_or(Color::WHITE));
            wp.anti_alias = true;
            pixmap.fill_path(&path, &wp, FillRule::Winding, Transform::identity(), None);

            // Subtle border stroke
            let mut sp = Paint::default();
            sp.set_color(Color::from_rgba(1.0, 1.0, 1.0, 0.24 * alpha_mul).unwrap_or(Color::WHITE));
            sp.anti_alias = true;
            pixmap.stroke_path(&path, &sp, &Stroke { width: 1.0, ..Default::default() },
                               Transform::identity(), None);
        }
    }
}

// ─── Icon renderer ────────────────────────────────────────────────────────────

/// Renders icons on top of sub-ring wedges, positioned dynamically centered on the expanding wedge.
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

    let norm_open = (reveal_progress / m as f32).clamp(0.0, 1.0);
    let mid_idx = (m - 1) as f32 / 2.0;
    let max_dist = mid_idx.max(0.5);

    for j in 0..m {
        let norm_dist = ((j as f32 - mid_idx).abs() / max_dist).clamp(0.0, 1.0);

        let open_delay = norm_dist * 0.22;
        let wedge_open_raw = ((norm_open - open_delay) / (1.0 - 0.22)).clamp(0.0, 1.0);
        if wedge_open_raw <= 0.0 { continue; }

        let close_delay = (1.0 - norm_dist) * 0.18;
        let wedge_close_raw = if sub_closing_progress > 0.0 {
            ((sub_closing_progress - close_delay) / (1.0 - 0.18)).clamp(0.0, 1.0)
        } else { 0.0 };

        let open_radial = out_back(wedge_open_raw, 0.32);
        let open_alpha  = out_cubic(wedge_open_raw);
        let close_radial = if sub_closing_progress > 0.0 { in_cubic(wedge_close_raw) } else { 0.0 };
        let close_alpha  = if sub_closing_progress > 0.0 { 1.0 - in_quad(wedge_close_raw) } else { 1.0 };

        let net_radial = (open_radial * (1.0 - close_radial)).max(0.0);
        let alpha = (open_alpha * close_alpha).clamp(0.0, 1.0);
        if alpha < 0.01 { continue; }

        let is_hov = j as i32 == hovered_index;
        let r_lift = if is_hov { 5.0 * hover_factor } else { 0.0 };

        // Keep icon centered radially on the wedge as it blooms/retracts
        let travel = SUB_OUTER_R - SUB_INNER_R;
        let cur_outer = SUB_INNER_R + travel * net_radial + r_lift;
        let cur_icon_r = SUB_INNER_R + (cur_outer - SUB_INNER_R) * 0.5;

        let mid_rad = (start_angle_deg + (j as f32 + 0.5) * slice_width_deg).to_radians();
        let icon_x  = cx + cur_icon_r * mid_rad.cos();
        let icon_y  = cy + cur_icon_r * mid_rad.sin();

        // Icon scales smoothly up with spring bloom
        let base_size = if is_hov { 26.0 } else { 22.0 };
        let icon_size = base_size * (0.60 + 0.40 * open_radial.min(1.1));

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
        let t = 0.85_f32;
        let v = out_back(t, 0.32);
        assert!(v > 1.0, "OutBack(0.32) should overshoot at t=0.85, got {}", v);
    }
}
