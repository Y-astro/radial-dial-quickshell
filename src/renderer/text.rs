// src/renderer/text.rs
#![allow(dead_code)]

use crate::font::{icon_codepoint, FontRenderer};
use cosmic_text::{Attrs, Buffer, Family, Metrics, Shaping};
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, PremultipliedColorU8, Stroke, Transform};

/// Standard Porter-Duff Over alpha blending for PremultipliedColorU8 pixels.
#[inline]
pub fn blend_over(dst: &mut PremultipliedColorU8, src_r: u8, src_g: u8, src_b: u8, src_a: u8) {
    if src_a == 0 {
        return;
    }
    if src_a == 255 {
        if let Some(c) = PremultipliedColorU8::from_rgba(src_r, src_g, src_b, 255) {
            *dst = c;
        }
        return;
    }
    let sa = src_a as u32;
    let sr = (src_r as u32 * sa + 127) / 255;
    let sg = (src_g as u32 * sa + 127) / 255;
    let sb = (src_b as u32 * sa + 127) / 255;

    let da = dst.alpha() as u32;
    let dr = dst.red() as u32;
    let dg = dst.green() as u32;
    let db = dst.blue() as u32;

    let inv_sa = 255 - sa;
    let out_r = (sr + (dr * inv_sa + 127) / 255).min(255) as u8;
    let out_g = (sg + (dg * inv_sa + 127) / 255).min(255) as u8;
    let out_b = (sb + (db * inv_sa + 127) / 255).min(255) as u8;
    let out_a = (sa + (da * inv_sa + 127) / 255).min(255) as u8;

    let out_r = out_r.min(out_a);
    let out_g = out_g.min(out_a);
    let out_b = out_b.min(out_a);

    *dst = PremultipliedColorU8::from_rgba(out_r, out_g, out_b, out_a)
        .unwrap_or(PremultipliedColorU8::TRANSPARENT);
}

/// Renders a single or multi-line text string centered at (cx, cy) onto the pixmap.
pub fn draw_text(
    font_renderer: &mut FontRenderer,
    pixmap: &mut Pixmap,
    text: &str,
    cx: f32,
    cy: f32,
    font_size: f32,
    color: Color,
) {
    if text.is_empty() || color.alpha() <= 0.001 || font_size <= 1.0 {
        return;
    }

    let line_height = font_size * 1.25;
    let mut buffer = Buffer::new(&mut font_renderer.font_system, Metrics::new(font_size, line_height));
    buffer.set_size(&mut font_renderer.font_system, None, None);
    let attrs = Attrs::new().family(Family::Name(&font_renderer.text_font_family));
    buffer.set_text(&mut font_renderer.font_system, text, attrs, Shaping::Advanced);
    buffer.shape_until_scroll(&mut font_renderer.font_system, false);

    let runs: Vec<_> = buffer.layout_runs().collect();
    if runs.is_empty() {
        return;
    }

    let min_top = runs[0].line_top;
    let max_bottom = runs.last().map(|r| r.line_top + r.line_height).unwrap_or(min_top);
    let total_h = max_bottom - min_top;
    let start_y = cy - total_h / 2.0 - min_top;

    let font_color = cosmic_text::Color::rgba(
        (color.red() * 255.0) as u8,
        (color.green() * 255.0) as u8,
        (color.blue() * 255.0) as u8,
        (color.alpha() * 255.0) as u8,
    );

    let width = pixmap.width() as i32;
    let height = pixmap.height() as i32;

    for run in &runs {
        let line_start_x = cx - run.line_w / 2.0;
        for glyph in run.glyphs.iter() {
            let physical = glyph.physical((line_start_x, start_y + run.line_y), 1.0);
            font_renderer.swash_cache.with_pixels(
                &mut font_renderer.font_system,
                physical.cache_key,
                font_color,
                |x, y, glyph_col| {
                    let px = physical.x + x;
                    let py = physical.y + y;
                    if px >= 0 && px < width && py >= 0 && py < height {
                        let (r, g, b, a) = glyph_col.as_rgba_tuple();
                        let eff_a = (a as u16 * (color.alpha() * 255.0) as u16 / 255) as u8;
                        let pixel = &mut pixmap.pixels_mut()[(py as usize) * (width as usize) + (px as usize)];
                        blend_over(pixel, r, g, b, eff_a);
                    }
                },
            );
        }
    }
}

/// Renders a single or multi-line text string with left alignment at (left_x, cy) onto the pixmap.
pub fn draw_text_left(
    font_renderer: &mut FontRenderer,
    pixmap: &mut Pixmap,
    text: &str,
    left_x: f32,
    cy: f32,
    font_size: f32,
    color: Color,
) {
    if text.is_empty() || color.alpha() <= 0.001 || font_size <= 1.0 {
        return;
    }

    let line_height = font_size * 1.25;
    let mut buffer = Buffer::new(&mut font_renderer.font_system, Metrics::new(font_size, line_height));
    buffer.set_size(&mut font_renderer.font_system, None, None);
    let attrs = Attrs::new().family(Family::Name(&font_renderer.text_font_family));
    buffer.set_text(&mut font_renderer.font_system, text, attrs, Shaping::Advanced);
    buffer.shape_until_scroll(&mut font_renderer.font_system, false);

    let runs: Vec<_> = buffer.layout_runs().collect();
    if runs.is_empty() {
        return;
    }

    let min_top = runs[0].line_top;
    let max_bottom = runs.last().map(|r| r.line_top + r.line_height).unwrap_or(min_top);
    let total_h = max_bottom - min_top;
    let start_y = cy - total_h / 2.0 - min_top;

    let font_color = cosmic_text::Color::rgba(
        (color.red() * 255.0) as u8,
        (color.green() * 255.0) as u8,
        (color.blue() * 255.0) as u8,
        (color.alpha() * 255.0) as u8,
    );

    let width = pixmap.width() as i32;
    let height = pixmap.height() as i32;

    for run in &runs {
        let line_start_x = left_x;
        for glyph in run.glyphs.iter() {
            let physical = glyph.physical((line_start_x, start_y + run.line_y), 1.0);
            font_renderer.swash_cache.with_pixels(
                &mut font_renderer.font_system,
                physical.cache_key,
                font_color,
                |x, y, glyph_col| {
                    let px = physical.x + x;
                    let py = physical.y + y;
                    if px >= 0 && px < width && py >= 0 && py < height {
                        let (r, g, b, a) = glyph_col.as_rgba_tuple();
                        let eff_a = (a as u16 * (color.alpha() * 255.0) as u16 / 255) as u8;
                        let pixel = &mut pixmap.pixels_mut()[(py as usize) * (width as usize) + (px as usize)];
                        blend_over(pixel, r, g, b, eff_a);
                    }
                },
            );
        }
    }
}

/// Renders a Material Symbols icon centered at (cx, cy) onto the pixmap.
pub fn draw_icon(
    font_renderer: &mut FontRenderer,
    pixmap: &mut Pixmap,
    icon_name: &str,
    cx: f32,
    cy: f32,
    size: f32,
    color: Color,
) {
    if icon_name.is_empty() || color.alpha() <= 0.001 || size <= 1.0 {
        return;
    }

    let cp = icon_codepoint(icon_name);
    let s = cp.to_string();

    let mut buffer = Buffer::new(&mut font_renderer.font_system, Metrics::new(size, size));
    buffer.set_size(&mut font_renderer.font_system, None, None);
    let attrs = Attrs::new().family(Family::Name(&font_renderer.icon_font_family));
    buffer.set_text(&mut font_renderer.font_system, &s, attrs, Shaping::Advanced);
    buffer.shape_until_scroll(&mut font_renderer.font_system, false);

    let font_color = cosmic_text::Color::rgba(
        (color.red() * 255.0) as u8,
        (color.green() * 255.0) as u8,
        (color.blue() * 255.0) as u8,
        (color.alpha() * 255.0) as u8,
    );

    let width = pixmap.width() as i32;
    let height = pixmap.height() as i32;
    let mut pixels_drawn = 0;

    for run in buffer.layout_runs() {
        for glyph in run.glyphs.iter() {
            let physical = glyph.physical((0.0, 0.0), 1.0);
            if let Some(img) = font_renderer
                .swash_cache
                .get_image(&mut font_renderer.font_system, physical.cache_key)
            {
                let w = img.placement.width as f32;
                let h = img.placement.height as f32;
                let left = img.placement.left as f32;
                let top = img.placement.top as f32;

                let origin_x = cx - (left + w / 2.0);
                let origin_y = cy - (-top + h / 2.0);

                let phys_placed = glyph.physical((origin_x, origin_y), 1.0);
                font_renderer.swash_cache.with_pixels(
                    &mut font_renderer.font_system,
                    phys_placed.cache_key,
                    font_color,
                    |x, y, glyph_col| {
                        let px = phys_placed.x + x;
                        let py = phys_placed.y + y;
                        if px >= 0 && px < width && py >= 0 && py < height {
                            let (r, g, b, a) = glyph_col.as_rgba_tuple();
                            let eff_a = (a as u16 * (color.alpha() * 255.0) as u16 / 255) as u8;
                            let pixel = &mut pixmap.pixels_mut()[(py as usize) * (width as usize) + (px as usize)];
                            blend_over(pixel, r, g, b, eff_a);
                            pixels_drawn += 1;
                        }
                    },
                );
            }
        }
    }

    // Geometric fallback if font shaping produced no visible pixels
    if pixels_drawn == 0 {
        let arm = (size * 0.35).max(3.0);
        let stroke_w = (size * 0.12).clamp(1.5, 3.0);
        let mut pb = PathBuilder::new();
        match icon_name {
            "close" | "✕" | "x" => {
                pb.move_to(cx - arm, cy - arm);
                pb.line_to(cx + arm, cy + arm);
                pb.move_to(cx - arm, cy + arm);
                pb.line_to(cx + arm, cy - arm);
            }
            "arrow_back" => {
                pb.move_to(cx + arm, cy);
                pb.line_to(cx - arm, cy);
                pb.move_to(cx - arm * 0.4, cy - arm * 0.6);
                pb.line_to(cx - arm, cy);
                pb.line_to(cx - arm * 0.4, cy + arm * 0.6);
            }
            "arrow_upward" => {
                pb.move_to(cx, cy + arm);
                pb.line_to(cx, cy - arm);
                pb.move_to(cx - arm * 0.6, cy - arm * 0.4);
                pb.line_to(cx, cy - arm);
                pb.line_to(cx + arm * 0.6, cy - arm * 0.4);
            }
            "chevron_right" => {
                pb.move_to(cx - arm * 0.4, cy - arm * 0.7);
                pb.line_to(cx + arm * 0.4, cy);
                pb.line_to(cx - arm * 0.4, cy + arm * 0.7);
            }
            "check" => {
                pb.move_to(cx - arm * 0.8, cy);
                pb.line_to(cx - arm * 0.2, cy + arm * 0.6);
                pb.line_to(cx + arm * 0.8, cy - arm * 0.6);
            }
            _ => {}
        }
        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            paint.set_color(color);
            paint.anti_alias = true;
            let stroke = Stroke {
                width: stroke_w,
                ..Default::default()
            };
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    }
}

/// Draws a numbered circular hotkey badge (1..9) at position (x, y).
pub fn draw_number_badge(
    font_renderer: &mut FontRenderer,
    pixmap: &mut Pixmap,
    num: usize,
    x: f32,
    y: f32,
    is_hovered: bool,
    primary_col: Color,
    on_primary_col: Color,
) {
    let radius = 7.5;

    let bg_color = if is_hovered {
        on_primary_col
    } else {
        Color::from_rgba(0.04, 0.04, 0.06, 0.70).unwrap_or(Color::BLACK)
    };

    let border_color = if is_hovered {
        primary_col
    } else {
        Color::from_rgba(1.0, 1.0, 1.0, 0.28).unwrap_or(Color::WHITE)
    };

    let text_color = if is_hovered {
        primary_col
    } else {
        Color::from_rgba(1.0, 1.0, 1.0, 0.85).unwrap_or(Color::WHITE)
    };

    let mut pb = PathBuilder::new();
    pb.push_circle(x, y, radius);
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(bg_color);
        paint.anti_alias = true;
        pixmap.fill_path(&path, &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);

        let mut stroke_paint = Paint::default();
        stroke_paint.set_color(border_color);
        stroke_paint.anti_alias = true;
        let stroke = Stroke {
            width: 1.0,
            ..Default::default()
        };
        pixmap.stroke_path(&path, &stroke_paint, &stroke, Transform::identity(), None);
    }

    let text = num.to_string();
    draw_text(font_renderer, pixmap, &text, x, y, 9.0, text_color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_draw_text_renders_pixels() {
        let mut font_renderer = FontRenderer::new();
        let mut pixmap = Pixmap::new(100, 100).unwrap();
        let color = Color::from_rgba8(255, 255, 255, 255);

        draw_text(&mut font_renderer, &mut pixmap, "Test", 50.0, 50.0, 14.0, color);

        let non_zero = pixmap.pixels().iter().filter(|p| p.alpha() > 0).count();
        assert!(non_zero > 0, "Text rendering should produce non-zero pixels");
    }

    #[test]
    fn test_draw_icon_renders_pixels() {
        let mut font_renderer = FontRenderer::new();
        let mut pixmap = Pixmap::new(64, 64).unwrap();
        let color = Color::from_rgba8(255, 255, 255, 255);

        draw_icon(&mut font_renderer, &mut pixmap, "terminal", 32.0, 32.0, 24.0, color);

        let non_zero = pixmap.pixels().iter().filter(|p| p.alpha() > 0).count();
        assert!(non_zero > 0, "Icon rendering should produce non-zero pixels");
    }

    #[test]
    fn test_draw_number_badge_renders() {
        let mut font_renderer = FontRenderer::new();
        let mut pixmap = Pixmap::new(30, 30).unwrap();
        let primary = Color::from_rgba8(137, 180, 250, 255);
        let on_primary = Color::from_rgba8(17, 17, 27, 255);

        draw_number_badge(&mut font_renderer, &mut pixmap, 1, 15.0, 15.0, false, primary, on_primary);

        let non_zero = pixmap.pixels().iter().filter(|p| p.alpha() > 0).count();
        assert!(non_zero > 0, "Badge rendering should produce non-zero pixels");
    }

    #[test]
    fn test_draw_text_left_renders_pixels() {
        let mut font_renderer = FontRenderer::new();
        let mut pixmap = Pixmap::new(100, 50).unwrap();
        let color = Color::from_rgba8(255, 255, 255, 255);

        draw_text_left(&mut font_renderer, &mut pixmap, "Left Align", 10.0, 25.0, 12.0, color);

        let non_zero = pixmap.pixels().iter().filter(|p| p.alpha() > 0).count();
        assert!(non_zero > 0, "Left-aligned text rendering should produce non-zero pixels");
    }
}

