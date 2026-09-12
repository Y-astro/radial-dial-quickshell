// src/renderer/hub.rs
#![allow(dead_code)]

use crate::font::FontRenderer;
use crate::renderer::text::{draw_icon, draw_text};
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

fn lighten_color(c: Color, factor: f32) -> Color {
    let r = (c.red() * factor).min(1.0);
    let g = (c.green() * factor).min(1.0);
    let b = (c.blue() * factor).min(1.0);
    Color::from_rgba(r, g, b, c.alpha()).unwrap_or(c)
}

/// Draws the center floating hub button at (cx, cy).
/// If `hover_label` is empty, renders the close icon ("close").
/// If `hover_label` is non-empty, renders the dynamic slice title text centered.
pub fn draw_center_hub(
    font_renderer: &mut FontRenderer,
    pixmap: &mut Pixmap,
    cx: f32,
    cy: f32,
    radius: f32,
    hover_label: &str,
    is_hovered: bool,
    scale: f32,
    primary_col: Color,
    is_low_end: bool,
) {
    if scale <= 0.001 {
        return;
    }

    let base_r = if radius > 0.0 { radius } else { 44.0 };
    let scale_factor = if scale >= 1.0 && is_hovered { 1.06 } else { scale };
    let r = base_r * scale_factor;
    if r <= 0.5 {
        return;
    }

    // Background fill matching QML
    let fill_alpha = if is_hovered {
        0.22
    } else if !hover_label.is_empty() {
        if is_low_end { 0.92 } else { 0.65 }
    } else if is_low_end {
        0.85
    } else {
        0.45
    } * scale;

    let fill_color = if is_hovered {
        Color::from_rgba(1.0, 1.0, 1.0, fill_alpha).unwrap_or(Color::WHITE)
    } else if !hover_label.is_empty() {
        if is_low_end {
            Color::from_rgba(0.10, 0.11, 0.15, fill_alpha).unwrap_or(Color::BLACK)
        } else {
            Color::from_rgba(0.08, 0.08, 0.12, fill_alpha).unwrap_or(Color::BLACK)
        }
    } else if is_low_end {
        Color::from_rgba(0.08, 0.08, 0.11, fill_alpha).unwrap_or(Color::BLACK)
    } else {
        Color::from_rgba(0.06, 0.06, 0.08, fill_alpha).unwrap_or(Color::BLACK)
    };

    // Border color & width matching QML
    let (border_color, border_width) = if !hover_label.is_empty() {
        let light = lighten_color(primary_col, 1.25);
        let eff_a = (light.alpha() * scale).clamp(0.0, 1.0);
        let col = Color::from_rgba(light.red(), light.green(), light.blue(), eff_a).unwrap_or(light);
        (col, 1.8 * scale.min(1.0))
    } else {
        let eff_a = (0.22 * scale).clamp(0.0, 1.0);
        (Color::from_rgba(1.0, 1.0, 1.0, eff_a).unwrap_or(Color::WHITE), 1.5 * scale.min(1.0))
    };

    // Draw circular hub body
    let mut pb = PathBuilder::new();
    pb.push_circle(cx, cy, r);
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(fill_color);
        paint.anti_alias = true;
        pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);

        let mut stroke_paint = Paint::default();
        stroke_paint.set_color(border_color);
        stroke_paint.anti_alias = true;
        let stroke = Stroke {
            width: border_width,
            ..Default::default()
        };
        pixmap.stroke_path(&path, &stroke_paint, &stroke, Transform::identity(), None);
    }

    // Hub content
    if hover_label.is_empty() {
        // 1. Close icon ("close" / '✕')
        let icon_size = (if is_hovered { 24.0 } else { 22.0 }) * scale;
        let icon_color = Color::from_rgba(1.0, 1.0, 1.0, 0.95 * scale).unwrap_or(Color::WHITE);
        draw_icon(font_renderer, pixmap, "close", cx, cy, icon_size, icon_color);
    } else {
        // 2. Dynamic Segment / Function Name centered
        let font_size = 13.0 * scale;
        let eff_a = (primary_col.alpha() * scale).clamp(0.0, 1.0);
        let text_color = Color::from_rgba(primary_col.red(), primary_col.green(), primary_col.blue(), eff_a).unwrap_or(primary_col);
        draw_text(font_renderer, pixmap, hover_label, cx, cy, font_size, text_color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_center_hub_drawing() {
        let mut font_renderer = FontRenderer::new();
        let mut pixmap = Pixmap::new(200, 200).unwrap();
        let primary = Color::from_rgba8(137, 180, 250, 255);

        // 1. Draw with empty hover_label (Close 'X' icon)
        draw_center_hub(
            &mut font_renderer,
            &mut pixmap,
            100.0,
            100.0,
            44.0,
            "",
            false,
            1.0,
            primary,
            false,
        );

        let non_zero_empty = pixmap.pixels().iter().filter(|p| p.alpha() > 0).count();
        assert!(non_zero_empty > 500, "Center hub with close icon must render visible pixels");

        // 2. Draw with hover label (Text title)
        let mut pixmap_label = Pixmap::new(200, 200).unwrap();
        draw_center_hub(
            &mut font_renderer,
            &mut pixmap_label,
            100.0,
            100.0,
            44.0,
            "Terminal",
            true,
            1.0,
            primary,
            false,
        );

        let non_zero_label = pixmap_label.pixels().iter().filter(|p| p.alpha() > 0).count();
        assert!(non_zero_label > 500, "Center hub with label must render visible pixels");
    }
}
