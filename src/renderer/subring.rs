// src/renderer/subring.rs
#![allow(dead_code)]

use crate::font::FontRenderer;
use crate::renderer::pie::{
    draw_drag_insertion_indicator, draw_floating_wedge, draw_plucked_floating_wedge,
    CORNER_RADIUS_SUB, SUB_INNER_R, SUB_OUTER_R,
};
use crate::renderer::text::{draw_icon, draw_number_badge};
use crate::state::actions::SliceItem;
use tiny_skia::{
    Color, FillRule, GradientStop, Paint, PathBuilder, Pixmap, Point, RadialGradient, SpreadMode,
    Stroke, Transform,
};

#[derive(Debug, Clone, Copy)]
pub struct SubRingDragInfo {
    pub from_index: i32,
    pub target_index: i32,
    pub pluck_progress: f32,
    pub indicator_alpha: f32,
    pub cursor_x: f32,
    pub cursor_y: f32,
}

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
    drag_info: Option<SubRingDragInfo>,
) {
    let m = sub_slices.len();
    if m == 0 || reveal_progress <= 0.0 {
        return;
    }

    // Normalized open progress across 0.0..1.0
    let norm_open = (reveal_progress / m as f32).clamp(0.0, 1.0);
    let mid_idx = (m - 1) as f32 / 2.0;
    let max_dist = mid_idx.max(0.5);

    let is_sub_drag = drag_info.is_some();

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
        let is_hov = j as i32 == hovered_index && !is_sub_drag;
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
        let mut start_deg  = arc_center - arc_half + gap_deg;
        let mut end_deg    = arc_center + arc_half - gap_deg;

        // Part neighboring slices at target slot insertion boundary during drag
        if let Some(drag) = drag_info {
            if drag.from_index >= 0 && drag.target_index >= 0 && drag.from_index != drag.target_index && drag.pluck_progress > 0.0 {
                let b = if drag.from_index < drag.target_index {
                    drag.target_index + 1
                } else {
                    drag.target_index
                } as usize;
                let prev_j = (b as i32 - 1).max(0) as usize;
                let next_j = b.min(m.saturating_sub(1));
                if j == prev_j && b > 0 {
                    end_deg -= 2.4 * drag.pluck_progress;
                }
                if j == next_j && b < m {
                    start_deg += 2.4 * drag.pluck_progress;
                }
            }
        }

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

        // Empty recessed socket for plucked segment
        if let Some(drag) = drag_info {
            if drag.from_index >= 0 && j as i32 == drag.from_index {
                let mut socket_paint = Paint::default();
                socket_paint.set_color(Color::from_rgba(0.0, 0.0, 0.0, 0.25 * alpha_mul).unwrap_or(Color::BLACK));
                socket_paint.anti_alias = true;
                pixmap.fill_path(&path, &socket_paint, FillRule::Winding, Transform::identity(), None);

                let mut socket_stroke = Paint::default();
                socket_stroke.set_color(Color::from_rgba(1.0, 1.0, 1.0, 0.14 * alpha_mul).unwrap_or(Color::WHITE));
                socket_stroke.anti_alias = true;
                pixmap.stroke_path(&path, &socket_stroke, &Stroke { width: 1.0, ..Default::default() }, Transform::identity(), None);
                continue;
            }
        }

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

    // Draw illuminated neon insertion indicator line between subdial segments
    if let Some(drag) = drag_info {
        if drag.from_index >= 0 && drag.target_index >= 0 && drag.from_index != drag.target_index && drag.indicator_alpha > 0.005 {
            let b = if drag.from_index < drag.target_index {
                drag.target_index + 1
            } else {
                drag.target_index
            } as usize;
            let boundary_deg = start_angle_deg + b as f32 * slice_width_deg;
            draw_drag_insertion_indicator(
                pixmap,
                cx,
                cy,
                SUB_INNER_R - 5.0,
                SUB_OUTER_R + 8.0,
                boundary_deg.to_radians(),
                primary_col,
                drag.indicator_alpha,
                Transform::identity(),
            );
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
    primary_col: Color,
    on_primary_col: Color,
    drag_info: Option<SubRingDragInfo>,
) {
    let m = sub_slices.len();
    if m == 0 || reveal_progress <= 0.0 {
        return;
    }

    let norm_open = (reveal_progress / m as f32).clamp(0.0, 1.0);
    let mid_idx = (m - 1) as f32 / 2.0;
    let max_dist = mid_idx.max(0.5);

    let is_sub_drag = drag_info.is_some();

    for j in 0..m {
        if let Some(drag) = drag_info {
            if drag.from_index >= 0 && j as i32 == drag.from_index {
                // Plucked out; skip drawing normal icon and badge
                continue;
            }
        }

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

        let is_hov = j as i32 == hovered_index && !is_sub_drag;
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

        // Number badges for hotkeys 1..9 on subdial segments
        if j < 9 && !sub_slices[j].is_add_button && alpha >= 0.5 {
            let badge_x = icon_x + 12.0;
            let badge_y = icon_y - 12.0;
            draw_number_badge(
                font_renderer,
                pixmap,
                j + 1,
                badge_x,
                badge_y,
                is_hov,
                primary_col,
                on_primary_col,
            );
        }
    }

    // Draw the plucked floating folder wedge above the subdial tracking the cursor
    if let Some(drag) = drag_info {
        if drag.from_index >= 0 && (drag.from_index as usize) < sub_slices.len() && drag.pluck_progress > 0.01 {
            let folder_slice = &sub_slices[drag.from_index as usize];
            let dx = drag.cursor_x - cx;
            let dy = drag.cursor_y - cy;
            let pointer_angle_deg = dy.atan2(dx).to_degrees();
            let badge_num = if drag.from_index < 9 && !folder_slice.is_add_button {
                Some((drag.from_index + 1) as usize)
            } else {
                None
            };
            draw_plucked_floating_wedge(
                font_renderer,
                pixmap,
                cx,
                cy,
                SUB_INNER_R,
                SUB_OUTER_R,
                slice_width_deg,
                pointer_angle_deg,
                CORNER_RADIUS_SUB,
                &folder_slice.icon,
                badge_num,
                drag.pluck_progress,
                primary_col,
                on_primary_col,
                Transform::identity(),
            );
        }
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
    drag_info: Option<SubRingDragInfo>,
) {
    draw_sub_ring(
        pixmap, cx, cy, sub_slices, start_angle_deg, slice_width_deg,
        hovered_index, hover_factor, reveal_progress, sub_closing_progress,
        primary_col, is_low_end, drag_info,
    );
    draw_sub_ring_icons(
        font_renderer, pixmap, cx, cy, sub_slices, start_angle_deg, slice_width_deg,
        hovered_index, hover_factor, reveal_progress, sub_closing_progress,
        primary_col, on_primary_col, drag_info,
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
            1, 1.0, 4.0, 0.0, primary, false, None,
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
                          args.2, args.3, 3.0, progress, primary, false, None);
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

    #[test]
    fn test_sub_ring_with_icons_renders_badges() {
        let mut pixmap = Pixmap::new(600, 600).unwrap();
        let mut font_renderer = FontRenderer::new();
        let mut slices = vec![
            SliceItem::new("f1", "Folder 1", "folder", 0),
            SliceItem::new("f2", "Folder 2", "folder", 1),
        ];
        let mut add_btn = SliceItem::new("add", "Add", "add", 2);
        add_btn.is_add_button = true;
        slices.push(add_btn);

        let primary = Color::from_rgba8(137, 180, 250, 255);
        let on_primary = Color::from_rgba8(17, 17, 27, 255);

        draw_sub_ring_with_icons(
            &mut font_renderer,
            &mut pixmap,
            300.0,
            300.0,
            &slices,
            -45.0,
            30.0,
            0,
            1.0,
            3.0,
            0.0,
            primary,
            on_primary,
            false,
            None,
        );

        let non_zero = pixmap.pixels().iter().filter(|p| p.alpha() > 0).count();
        assert!(non_zero > 1000, "Sub ring with icons and badges should render pixels");
    }
}
