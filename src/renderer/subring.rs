// src/renderer/subring.rs
#![allow(dead_code)]

use crate::font::FontRenderer;
use crate::renderer::pie::{draw_floating_wedge, CORNER_RADIUS_SUB, SUB_ICON_R, SUB_INNER_R, SUB_OUTER_R};
use crate::renderer::text::draw_icon;
use crate::state::actions::SliceItem;
use tiny_skia::{
    Color, FillRule, GradientStop, Paint, PathBuilder, Pixmap, Point, RadialGradient, SpreadMode, Stroke, Transform,
};

fn lighten_color(c: Color, factor: f32) -> Color {
    let r = (c.red() * factor).min(1.0);
    let g = (c.green() * factor).min(1.0);
    let b = (c.blue() * factor).min(1.0);
    Color::from_rgba(r, g, b, c.alpha()).unwrap_or(c)
}

fn darken_color(c: Color, factor: f32) -> Color {
    let r = c.red() / factor;
    let g = c.green() / factor;
    let b = c.blue() / factor;
    Color::from_rgba(r, g, b, c.alpha()).unwrap_or(c)
}

/// Renders the localized symmetrical sub-arc wedges over the parent segment.
/// Uses `draw_floating_wedge()` with SUB_INNER_R=168.0, SUB_OUTER_R=228.0, CORNER_RADIUS_SUB=5.0.
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
    primary_col: Color,
    is_low_end: bool,
) {
    let m = sub_slices.len();
    if m == 0 || reveal_progress <= 0.0 {
        return;
    }

    for j in 0..m {
        let p = (reveal_progress - j as f32).clamp(0.0, 1.0);
        if p <= 0.0 {
            continue;
        }

        let sub_ease = if p >= 1.0 {
            1.0
        } else {
            1.0 - (1.0 - p).powi(3)
        };

        let is_outer_hov = j as i32 == hovered_index;
        let r_lift = if is_outer_hov {
            5.0 * hover_factor
        } else {
            0.0
        };

        let current_sub_outer = SUB_INNER_R + (SUB_OUTER_R - SUB_INNER_R) * sub_ease + r_lift;

        let start_deg = start_angle_deg + j as f32 * slice_width_deg;
        let end_deg = start_deg + slice_width_deg;

        let start_rad = start_deg.to_radians();
        let end_rad = end_deg.to_radians();

        let mut pb = PathBuilder::new();
        draw_floating_wedge(
            &mut pb,
            cx,
            cy,
            SUB_INNER_R,
            current_sub_outer,
            start_rad,
            end_rad,
            CORNER_RADIUS_SUB,
        );

        if let Some(path) = pb.finish() {
            if is_outer_hov {
                let light_col = lighten_color(primary_col, 1.30);
                let dark_col = darken_color(primary_col, 1.10);

                let grad = RadialGradient::new(
                    Point::from_xy(cx, cy),
                    Point::from_xy(cx, cy),
                    current_sub_outer + 10.0,
                    vec![
                        GradientStop::new(0.0, light_col),
                        GradientStop::new(0.40, primary_col),
                        GradientStop::new(1.0, dark_col),
                    ],
                    SpreadMode::Pad,
                    Transform::identity(),
                );

                let mut paint = Paint::default();
                paint.anti_alias = true;
                if let Some(shader) = grad {
                    paint.shader = shader;
                } else {
                    paint.set_color(primary_col);
                }
                pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);

                let mut stroke_paint = Paint::default();
                stroke_paint.set_color(light_col);
                stroke_paint.anti_alias = true;
                let stroke = Stroke {
                    width: 1.8,
                    ..Default::default()
                };
                pixmap.stroke_path(&path, &stroke_paint, &stroke, Transform::identity(), None);
            } else {
                let (stop0, stop1) = if is_low_end {
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

                let grad = RadialGradient::new(
                    Point::from_xy(cx, cy),
                    Point::from_xy(cx, cy),
                    current_sub_outer,
                    vec![
                        GradientStop::new(0.0, stop0),
                        GradientStop::new(1.0, stop1),
                    ],
                    SpreadMode::Pad,
                    Transform::identity(),
                );

                let mut paint = Paint::default();
                paint.anti_alias = true;
                if let Some(shader) = grad {
                    paint.shader = shader;
                } else {
                    paint.set_color(stop0);
                }
                pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);

                // Subtle white overlay wash
                let mut wash_paint = Paint::default();
                wash_paint.set_color(Color::from_rgba(1.0, 1.0, 1.0, 0.06).unwrap_or(Color::WHITE));
                wash_paint.anti_alias = true;
                pixmap.fill_path(&path, &wash_paint, FillRule::Winding, Transform::identity(), None);

                // Subtle outer border stroke
                let mut stroke_paint = Paint::default();
                stroke_paint.set_color(Color::from_rgba(1.0, 1.0, 1.0, 0.24).unwrap_or(Color::WHITE));
                stroke_paint.anti_alias = true;
                let stroke = Stroke {
                    width: 1.0,
                    ..Default::default()
                };
                pixmap.stroke_path(&path, &stroke_paint, &stroke, Transform::identity(), None);
            }
        }
    }
}

/// Renders icons on top of sub-ring wedges.
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
    on_primary_col: Color,
) {
    let m = sub_slices.len();
    if m == 0 || reveal_progress <= 0.0 {
        return;
    }

    for j in 0..m {
        let p = (reveal_progress - j as f32).clamp(0.0, 1.0);
        if p <= 0.0 {
            continue;
        }

        let is_outer_hov = j as i32 == hovered_index;
        let r_lift = if is_outer_hov {
            5.0 * hover_factor
        } else {
            0.0
        };

        let cur_icon_r = SUB_ICON_R + r_lift / 2.0;
        let mid_rad = (start_angle_deg + (j as f32 + 0.5) * slice_width_deg).to_radians();

        let raw_icon_x = cx + cur_icon_r * mid_rad.cos();
        let raw_icon_y = cy + cur_icon_r * mid_rad.sin();

        let icon_size = if is_outer_hov { 26.0 } else { 22.0 };
        let icon_color = if is_outer_hov {
            on_primary_col
        } else {
            Color::from_rgba(1.0, 1.0, 1.0, 0.95 * p).unwrap_or(Color::WHITE)
        };

        draw_icon(
            font_renderer,
            pixmap,
            &sub_slices[j].icon,
            raw_icon_x,
            raw_icon_y,
            icon_size,
            icon_color,
        );
    }
}

/// Helper to render both sub-ring wedges and their icons in a single pass.
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
    primary_col: Color,
    on_primary_col: Color,
    is_low_end: bool,
) {
    draw_sub_ring(
        pixmap,
        cx,
        cy,
        sub_slices,
        start_angle_deg,
        slice_width_deg,
        hovered_index,
        hover_factor,
        reveal_progress,
        primary_col,
        is_low_end,
    );

    draw_sub_ring_icons(
        font_renderer,
        pixmap,
        cx,
        cy,
        sub_slices,
        start_angle_deg,
        slice_width_deg,
        hovered_index,
        hover_factor,
        reveal_progress,
        on_primary_col,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sub_ring_drawing() {
        let mut pixmap = Pixmap::new(600, 600).unwrap();
        let cx = 300.0;
        let cy = 300.0;

        let slices = vec![
            SliceItem::new("tab1", "Tab 1", "counter_1", 0),
            SliceItem::new("tab2", "Tab 2", "counter_2", 1),
            SliceItem::new("tab3", "Tab 3", "counter_3", 2),
            SliceItem::new("tab4", "Tab 4", "counter_4", 3),
        ];

        let start_angle = -45.0;
        let slice_width = 22.5;
        let primary = Color::from_rgba8(137, 180, 250, 255);

        // Draw fully revealed sub-ring (reveal_progress = 4.0 covers all 4 slices)
        draw_sub_ring(
            &mut pixmap,
            cx,
            cy,
            &slices,
            start_angle,
            slice_width,
            1, // Hovered index 1
            1.0,
            4.0,
            primary,
            false,
        );

        let non_zero_pixels = pixmap.pixels().iter().filter(|p| p.alpha() > 0).count();
        assert!(
            non_zero_pixels > 2000,
            "Rendered sub-ring should produce non-zero pixels (got {})",
            non_zero_pixels
        );
    }
}
