use crate::font::FontRenderer;
use crate::renderer::text::{draw_icon, draw_number_badge};
use tiny_skia::{
    Color, FillRule, GradientStop, Paint, PathBuilder, Pixmap, Point, RadialGradient, SpreadMode,
    Stroke, Transform,
};

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

fn fade_color(c: Color, alpha_mul: f32) -> Color {
    Color::from_rgba(c.red(), c.green(), c.blue(), (c.alpha() * alpha_mul).clamp(0.0, 1.0))
        .unwrap_or(c)
}

// Exact geometry constants from QML
pub const HUB_RADIUS: f32 = 44.0;
pub const SLICE_INNER_R: f32 = 54.0;
pub const SLICE_OUTER_R: f32 = 148.0;
pub const ICON_RADIUS: f32 = 101.0; // (54 + 148) / 2
pub const SUB_INNER_R: f32 = 168.0;
pub const SUB_OUTER_R: f32 = 228.0;
pub const SUB_ICON_R: f32 = 198.0;
pub const TOTAL_RADIUS: f32 = 272.0;
pub const GAP_PX: f32 = 8.5;
pub const CORNER_RADIUS_MAIN: f32 = 6.0;
pub const CORNER_RADIUS_SUB: f32 = 5.0;

/// Convert polar coordinates to cartesian (x, y)
pub fn polar(cx: f32, cy: f32, r: f32, theta: f32) -> (f32, f32) {
    (cx + r * theta.cos(), cy + r * theta.sin())
}

/// Unit vector from a 2D vector (handles zero-length gracefully)
pub fn unit_vec(dx: f32, dy: f32) -> (f32, f32) {
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-6 {
        (0.0, 0.0)
    } else {
        (dx / len, dy / len)
    }
}

/// Approximate a circular arc as polyline segments (tiny-skia has no native arc).
/// Interpolates angles smoothly from `start` to `end`.
pub fn arc_segment(pb: &mut PathBuilder, cx: f32, cy: f32, r: f32, start: f32, end: f32, _ccw: bool) {
    let span = (end - start).abs();
    let segments = ((span / (std::f32::consts::PI / 32.0)).ceil() as usize).max(2);
    let step = (end - start) / segments as f32;
    for i in 0..=segments {
        let theta = start + step * i as f32;
        let x = cx + r * theta.cos();
        let y = cy + r * theta.sin();
        pb.line_to(x, y);
    }
}

/// Draws a single floating wedge arc with rounded corners and uniform parallel gaps.
/// Equivalent to QML drawFloatingWedge(ctx, cx, cy, r0, r1, th0, th1, cr)
///
/// Parameters:
///   pb:       PathBuilder to append to
///   cx, cy:   center of the circle
///   r0:       inner radius
///   r1:       outer radius
///   th0:      start angle in RADIANS (0 = right, positive = clockwise)
///   th1:      end angle in RADIANS
///   corner_r: desired corner radius in pixels
pub fn draw_floating_wedge(
    pb: &mut PathBuilder,
    cx: f32,
    cy: f32,
    r0: f32,
    r1: f32,
    th0: f32,
    th1: f32,
    corner_r: f32,
) {
    let hg = GAP_PX / 2.0;
    // Angular insets to create gap (asin approximation, clamped to avoid NaN)
    let dth_inner = (hg / r0).min(0.92).asin();
    let dth_outer = (hg / r1).min(0.92).asin();

    let th0_in = th0 + dth_inner;
    let th1_in = th1 - dth_inner;
    let th0_out = th0 + dth_outer;
    let th1_out = th1 - dth_outer;

    // Degenerate case: wedge too thin
    if (th1_in - th0_in) <= 0.02 || (r1 - r0) <= 2.0 {
        let (start_x, start_y) = polar(cx, cy, r1, th0_out);
        pb.move_to(start_x, start_y);
        arc_segment(pb, cx, cy, r1, th0_out, th1_out, false);
        arc_segment(pb, cx, cy, r0, th1_in, th0_in, true);
        pb.close();
        return;
    }

    // Effective corner radius: min of requested, half-thickness, and arc-length-based
    let cr = corner_r
        .min((r1 - r0) / 2.2)
        .min(((th1_in - th0_in) * r0 / 2.5).max(1.0));

    let cth_inner = cr / r0; // angular equiv of corner radius at inner arc
    let cth_outer = cr / r1; // angular equiv of corner radius at outer arc

    // Right edge endpoints
    let (p_in_r_x, p_in_r_y) = polar(cx, cy, r0, th1_in);
    let (p_out_r_x, p_out_r_y) = polar(cx, cy, r1, th1_out);
    let vr = unit_vec(p_out_r_x - p_in_r_x, p_out_r_y - p_in_r_y);

    // Left edge endpoints
    let (p_in_l_x, p_in_l_y) = polar(cx, cy, r0, th0_in);
    let (p_out_l_x, p_out_l_y) = polar(cx, cy, r1, th0_out);
    let vl = unit_vec(p_out_l_x - p_in_l_x, p_out_l_y - p_in_l_y);

    // Start at inner arc start point
    let start_x = cx + r0 * (th0_in + cth_inner).cos();
    let start_y = cy + r0 * (th0_in + cth_inner).sin();
    pb.move_to(start_x, start_y);

    // 1. Inner arc (CW from th0+cth to th1-cth)
    arc_segment(pb, cx, cy, r0, th0_in + cth_inner, th1_in - cth_inner, false);

    // 2. Bottom-right corner: inner arc -> right edge
    pb.quad_to(
        p_in_r_x,
        p_in_r_y,
        p_in_r_x + cr * vr.0,
        p_in_r_y + cr * vr.1,
    );

    // 3. Right edge
    pb.line_to(p_out_r_x - cr * vr.0, p_out_r_y - cr * vr.1);

    // 4. Top-right corner: right edge -> outer arc
    pb.quad_to(
        p_out_r_x,
        p_out_r_y,
        cx + r1 * (th1_out - cth_outer).cos(),
        cy + r1 * (th1_out - cth_outer).sin(),
    );

    // 5. Outer arc (CCW = reversed direction)
    arc_segment(pb, cx, cy, r1, th1_out - cth_outer, th0_out + cth_outer, true);

    // 6. Top-left corner: outer arc -> left edge
    pb.quad_to(
        p_out_l_x,
        p_out_l_y,
        p_out_l_x - cr * vl.0,
        p_out_l_y - cr * vl.1,
    );

    // 7. Left edge
    pb.line_to(p_in_l_x + cr * vl.0, p_in_l_y + cr * vl.1);

    // 8. Bottom-left corner: left edge -> inner arc
    pb.quad_to(
        p_in_l_x,
        p_in_l_y,
        cx + r0 * (th0_in + cth_inner).cos(),
        cy + r0 * (th0_in + cth_inner).sin(),
    );

    pb.close();
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SliceDisp {
    pub start_shift: f32, // degrees to shift start angle
    pub end_shift: f32,   // degrees to shift end angle
    pub r_shift: f32,     // pixels to shift radially outward
}

/// Port of QML getSliceDisplacement(i, n, h, factor)
/// i=slice index, n=total slices, h=hovered index (-1 if none), factor=hover_factor (0..1)
pub fn get_slice_displacement(i: i32, n: i32, h: i32, factor: f32) -> SliceDisp {
    if h < 0 || factor <= 0.0 || n <= 0 {
        return SliceDisp::default();
    }
    // Circular distance (wrapping)
    let mut d = i - h;
    let half_n = n / 2;
    while d > half_n {
        d -= n;
    }
    while d < -half_n {
        d += n;
    }

    let expand = 3.2 * factor;
    if d == 0 {
        SliceDisp {
            start_shift: -expand,
            end_shift: expand,
            r_shift: 7.0 * factor,
        }
    } else {
        let abs_d = d.abs() as f32;
        let push = (expand * 1.3 / abs_d) * if d > 0 { 1.0 } else { -1.0 };
        let r_push = 2.5 / abs_d * factor;
        SliceDisp {
            start_shift: push,
            end_shift: push,
            r_shift: r_push,
        }
    }
}

/// Renders main ring wedges with per-slice blossoming animation and optional drag-reorder parting.
pub fn build_main_ring_paths_animated_with_drag(
    cx: f32,
    cy: f32,
    slice_count: usize,
    hovered: i32,
    hover_factor: f32,
    reveal_progress: f32,
    drag_boundary: Option<usize>,
    drag_pluck: f32,
) -> Vec<(usize, tiny_skia::Path, f32)> {
    if slice_count == 0 {
        return Vec::new();
    }
    let mut paths = Vec::with_capacity(slice_count);
    for i in 0..slice_count {
        let p = (reveal_progress - i as f32).clamp(0.0, 1.0);
        if p <= 0.001 {
            continue;
        }
        let ease = if p >= 1.0 { 1.0 } else { 1.0 - (1.0 - p).powi(3) };
        let mut disp = get_slice_displacement(i as i32, slice_count as i32, hovered, hover_factor);

        if let Some(b) = drag_boundary {
            if drag_pluck > 0.0 && slice_count >= 2 {
                let prev_slot = (b + slice_count - 1) % slice_count;
                let next_slot = b % slice_count;
                if i == prev_slot {
                    disp.end_shift -= 2.6 * drag_pluck;
                }
                if i == next_slot {
                    disp.start_shift += 2.6 * drag_pluck;
                }
            }
        }

        let base_start = (i as f32 * 360.0 / slice_count as f32 - 90.0).to_radians();
        let base_end = ((i as f32 + 1.0) * 360.0 / slice_count as f32 - 90.0).to_radians();
        let th0 = base_start + disp.start_shift.to_radians();
        let th1 = base_end + disp.end_shift.to_radians();
        let base_outer = SLICE_INNER_R + (SLICE_OUTER_R - SLICE_INNER_R) * ease;
        let r_outer = base_outer + disp.r_shift;
        let mut pb = PathBuilder::new();
        draw_floating_wedge(
            &mut pb,
            cx,
            cy,
            SLICE_INNER_R,
            r_outer,
            th0,
            th1,
            CORNER_RADIUS_MAIN,
        );
        if let Some(path) = pb.finish() {
            paths.push((i, path, p));
        }
    }
    paths
}

/// Backwards-compatible wrapper: renders main ring wedges with per-slice blossoming animation.
pub fn build_main_ring_paths_animated(
    cx: f32,
    cy: f32,
    slice_count: usize,
    hovered: i32,
    hover_factor: f32,
    reveal_progress: f32,
) -> Vec<(usize, tiny_skia::Path, f32)> {
    build_main_ring_paths_animated_with_drag(
        cx,
        cy,
        slice_count,
        hovered,
        hover_factor,
        reveal_progress,
        None,
        0.0,
    )
}

/// Draws an illuminated neon insertion indicator line between segments to indicate where a dragged segment will drop.
/// Includes ambient glow, bright inner glow, core line, and circular luminous terminal beads at both inner and outer ends.
pub fn draw_drag_insertion_indicator(
    pixmap: &mut Pixmap,
    cx: f32,
    cy: f32,
    r_inner: f32,
    r_outer: f32,
    boundary_angle_rad: f32,
    primary_col: Color,
    alpha_mul: f32,
    transform: Transform,
) {
    if alpha_mul <= 0.005 {
        return;
    }
    let cos_a = boundary_angle_rad.cos();
    let sin_a = boundary_angle_rad.sin();

    let x_in = cx + r_inner * cos_a;
    let y_in = cy + r_inner * sin_a;
    let x_out = cx + r_outer * cos_a;
    let y_out = cy + r_outer * sin_a;

    let mut pb = PathBuilder::new();
    pb.move_to(x_in, y_in);
    pb.line_to(x_out, y_out);
    let Some(line_path) = pb.finish() else { return };

    // 1. Wide ambient soft glow
    let ambient_col = fade_color(primary_col, 0.22 * alpha_mul);
    let mut ambient_paint = Paint::default();
    ambient_paint.set_color(ambient_col);
    ambient_paint.anti_alias = true;
    let ambient_stroke = Stroke {
        width: 14.0,
        ..Default::default()
    };
    pixmap.stroke_path(&line_path, &ambient_paint, &ambient_stroke, transform, None);

    // 2. Focused vibrant inner glow
    let inner_col = fade_color(lighten_color(primary_col, 1.3), 0.60 * alpha_mul);
    let mut inner_paint = Paint::default();
    inner_paint.set_color(inner_col);
    inner_paint.anti_alias = true;
    let inner_stroke = Stroke {
        width: 5.5,
        ..Default::default()
    };
    pixmap.stroke_path(&line_path, &inner_paint, &inner_stroke, transform, None);

    // 3. Ultra-bright neon core line
    let core_col = fade_color(Color::from_rgba8(255, 255, 255, 245), alpha_mul);
    let mut core_paint = Paint::default();
    core_paint.set_color(core_col);
    core_paint.anti_alias = true;
    let core_stroke = Stroke {
        width: 2.2,
        ..Default::default()
    };
    pixmap.stroke_path(&line_path, &core_paint, &core_stroke, transform, None);

    // 4. Outer terminal bead (luminous circular diamond at outer edge)
    let bead_outer_r = 4.5;
    let mut bead_out_pb = PathBuilder::new();
    bead_out_pb.push_circle(x_out, y_out, bead_outer_r);
    if let Some(bead_path) = bead_out_pb.finish() {
        let mut bead_glow = Paint::default();
        bead_glow.set_color(fade_color(primary_col, 0.75 * alpha_mul));
        bead_glow.anti_alias = true;
        pixmap.fill_path(&bead_path, &bead_glow, FillRule::Winding, transform, None);

        let mut bead_core_pb = PathBuilder::new();
        bead_core_pb.push_circle(x_out, y_out, 2.4);
        if let Some(core_path) = bead_core_pb.finish() {
            let mut bead_core = Paint::default();
            bead_core.set_color(fade_color(Color::WHITE, 0.95 * alpha_mul));
            bead_core.anti_alias = true;
            pixmap.fill_path(&core_path, &bead_core, FillRule::Winding, transform, None);
        }
    }

    // 5. Inner terminal bead (accent pip at inner edge)
    let bead_inner_r = 3.2;
    let mut bead_in_pb = PathBuilder::new();
    bead_in_pb.push_circle(x_in, y_in, bead_inner_r);
    if let Some(bead_path) = bead_in_pb.finish() {
        let mut bead_paint = Paint::default();
        bead_paint.set_color(fade_color(lighten_color(primary_col, 1.25), 0.85 * alpha_mul));
        bead_paint.anti_alias = true;
        pixmap.fill_path(&bead_path, &bead_paint, FillRule::Winding, transform, None);
    }
}

/// Draws the "plucked" segment floating above the dial tracking the pointer cursor.
/// Features an elevated drop shadow, vibrant gradient wedge body, luminous highlight border,
/// centered icon, and optional number badge.
pub fn draw_plucked_floating_wedge(
    font_renderer: &mut FontRenderer,
    pixmap: &mut Pixmap,
    cx: f32,
    cy: f32,
    base_inner_r: f32,
    base_outer_r: f32,
    slice_width_deg: f32,
    pointer_angle_deg: f32,
    corner_radius: f32,
    icon: &str,
    badge_number: Option<usize>,
    pluck_factor: f32,
    primary_col: Color,
    on_primary_col: Color,
    transform: Transform,
) {
    let pluck = pluck_factor.clamp(0.0, 1.3);
    if pluck <= 0.01 {
        return;
    }

    // Floating elevation: segment lifts radially outwards
    let r_inner = base_inner_r + 4.0 * pluck;
    let r_outer = base_outer_r + 14.0 * pluck;

    let th0 = (pointer_angle_deg - slice_width_deg * 0.5).to_radians();
    let th1 = (pointer_angle_deg + slice_width_deg * 0.5).to_radians();

    // 1. Drop shadow underneath the elevated segment
    let shadow_offset_x = 3.0 * pluck;
    let shadow_offset_y = 6.0 * pluck;
    let mut shadow_pb = PathBuilder::new();
    draw_floating_wedge(
        &mut shadow_pb,
        cx + shadow_offset_x,
        cy + shadow_offset_y,
        r_inner - 1.5,
        r_outer + 3.0,
        th0,
        th1,
        corner_radius + 1.0,
    );
    if let Some(shadow_path) = shadow_pb.finish() {
        let mut shadow_paint = Paint::default();
        shadow_paint.set_color(Color::from_rgba8(0, 0, 0, (90.0 * pluck.min(1.0)) as u8));
        shadow_paint.anti_alias = true;
        pixmap.fill_path(&shadow_path, &shadow_paint, FillRule::Winding, transform, None);
    }

    // 2. Floating wedge body
    let mut wedge_pb = PathBuilder::new();
    draw_floating_wedge(
        &mut wedge_pb,
        cx,
        cy,
        r_inner,
        r_outer,
        th0,
        th1,
        corner_radius,
    );
    let Some(wedge_path) = wedge_pb.finish() else { return };

    let light_col = lighten_color(primary_col, 1.40);
    let dark_col = darken_color(primary_col, 1.20);
    let grad = RadialGradient::new(
        Point::from_xy(cx, cy),
        Point::from_xy(cx, cy),
        r_outer + 12.0,
        vec![
            GradientStop::new(0.0, light_col),
            GradientStop::new(0.35, primary_col),
            GradientStop::new(1.0, dark_col),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    );
    let mut wedge_paint = Paint::default();
    wedge_paint.anti_alias = true;
    if let Some(shader) = grad {
        wedge_paint.shader = shader;
    } else {
        wedge_paint.set_color(primary_col);
    }
    pixmap.fill_path(&wedge_path, &wedge_paint, FillRule::Winding, transform, None);

    // 3. Frosted highlight sheen on top of wedge
    let mut sheen_paint = Paint::default();
    sheen_paint.set_color(Color::from_rgba(1.0, 1.0, 1.0, 0.12).unwrap_or(Color::WHITE));
    sheen_paint.anti_alias = true;
    pixmap.fill_path(&wedge_path, &sheen_paint, FillRule::Winding, transform, None);

    // 4. Luminous elevated border stroke
    let mut stroke_paint = Paint::default();
    stroke_paint.set_color(Color::from_rgba(light_col.red(), light_col.green(), light_col.blue(), 0.95).unwrap_or(light_col));
    stroke_paint.anti_alias = true;
    let stroke = Stroke {
        width: 2.2,
        ..Default::default()
    };
    pixmap.stroke_path(&wedge_path, &stroke_paint, &stroke, transform, None);

    // 5. Centered Icon and Number Badge
    let mid_rad = pointer_angle_deg.to_radians();
    let icon_r = (r_inner + r_outer) * 0.5;
    let raw_icon_x = cx + icon_r * mid_rad.cos();
    let raw_icon_y = cy + icon_r * mid_rad.sin();

    // Map through transform
    let icon_x = transform.sx * raw_icon_x + transform.kx * raw_icon_y + transform.tx;
    let icon_y = transform.ky * raw_icon_x + transform.sy * raw_icon_y + transform.ty;

    let icon_size = 28.0 * (1.0 + 0.08 * pluck.min(1.0));
    draw_icon(
        font_renderer,
        pixmap,
        icon,
        icon_x,
        icon_y,
        icon_size,
        on_primary_col,
    );

    if let Some(num) = badge_number {
        let badge_x = icon_x + 12.0;
        let badge_y = icon_y - 12.0;
        draw_number_badge(
            font_renderer,
            pixmap,
            num,
            badge_x,
            badge_y,
            true,
            primary_col,
            on_primary_col,
        );
    }
}

/// Renders main ring wedges as a Vec of finished Paths (one per slice).
pub fn build_main_ring_paths(
    cx: f32,
    cy: f32,
    slice_count: usize,
    hovered: i32, // -1 = none
    hover_factor: f32,
) -> Vec<tiny_skia::Path> {
    build_main_ring_paths_animated(cx, cy, slice_count, hovered, hover_factor, slice_count as f32)
        .into_iter()
        .map(|(_, path, _)| path)
        .collect()
}

/// Renders sub-ring wedges for an active parent slice.
/// Returns a Vec of finished Paths (one per sub-slice).
pub fn build_sub_ring_paths(
    cx: f32,
    cy: f32,
    sub_count: usize,
    start_angle_deg: f32,
    slice_width_deg: f32,
    outer_hovered: i32,
    outer_hover_factor: f32,
) -> Vec<tiny_skia::Path> {
    let mut paths = Vec::with_capacity(sub_count);
    for j in 0..sub_count {
        let is_outer_hov = j as i32 == outer_hovered;
        let r_lift = if is_outer_hov {
            5.0 * outer_hover_factor
        } else {
            0.0
        };
        let r_outer = SUB_OUTER_R + r_lift;
        let start_rad = (start_angle_deg + j as f32 * slice_width_deg).to_radians();
        let end_rad = (start_angle_deg + (j + 1) as f32 * slice_width_deg).to_radians();
        let mut pb = PathBuilder::new();
        draw_floating_wedge(
            &mut pb,
            cx,
            cy,
            SUB_INNER_R,
            r_outer,
            start_rad,
            end_rad,
            CORNER_RADIUS_SUB,
        );
        if let Some(path) = pb.finish() {
            paths.push(path);
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_polar_coordinates() {
        let (x, y) = polar(100.0, 100.0, 50.0, 0.0);
        assert!((x - 150.0).abs() < 1e-5);
        assert!((y - 100.0).abs() < 1e-5);

        let (x, y) = polar(100.0, 100.0, 50.0, std::f32::consts::PI / 2.0);
        assert!((x - 100.0).abs() < 1e-5);
        assert!((y - 150.0).abs() < 1e-5);
    }

    #[test]
    fn test_unit_vec() {
        let (ux, uy) = unit_vec(0.0, 0.0);
        assert_eq!((ux, uy), (0.0, 0.0));

        let (ux, uy) = unit_vec(10.0, 0.0);
        assert!((ux - 1.0).abs() < 1e-5);
        assert!(uy.abs() < 1e-5);

        let (ux, uy) = unit_vec(3.0, 4.0);
        assert!((ux - 0.6).abs() < 1e-5);
        assert!((uy - 0.8).abs() < 1e-5);
    }

    #[test]
    fn test_wedge_produces_path_8_slices() {
        let paths = build_main_ring_paths(272.0, 272.0, 8, -1, 0.0);
        assert_eq!(paths.len(), 8);
        for p in &paths {
            assert!(p.bounds().width() > 0.0);
            assert!(p.bounds().height() > 0.0);
        }
    }

    #[test]
    fn test_wedge_produces_path_6_slices() {
        let paths = build_main_ring_paths(272.0, 272.0, 6, -1, 0.0);
        assert_eq!(paths.len(), 6);
        for p in &paths {
            assert!(p.bounds().width() > 0.0);
            assert!(p.bounds().height() > 0.0);
        }
    }

    #[test]
    fn test_displacement_no_hover() {
        let d = get_slice_displacement(0, 8, -1, 0.0);
        assert_eq!(d.r_shift, 0.0);
        assert_eq!(d.start_shift, 0.0);
        assert_eq!(d.end_shift, 0.0);
    }

    #[test]
    fn test_displacement_zero_factor() {
        let d = get_slice_displacement(0, 8, 0, 0.0);
        assert_eq!(d, SliceDisp::default());
    }

    #[test]
    fn test_displacement_hovered() {
        let d = get_slice_displacement(0, 8, 0, 1.0);
        assert!(d.r_shift > 0.0);
        assert!(d.start_shift < 0.0); // start shifts backwards
        assert!(d.end_shift > 0.0);   // end shifts forwards
    }

    #[test]
    fn test_displacement_neighbor() {
        let d = get_slice_displacement(1, 8, 0, 1.0);
        // Neighbor shifts in same direction (positive d=1, push positive)
        assert!(d.r_shift > 0.0);
        assert!(d.start_shift > 0.0);
        assert!(d.end_shift > 0.0);
    }

    #[test]
    fn test_displacement_wrapping() {
        // In an 8-slice menu, hovering slice 0 should affect slice 7 as d = -1
        let d7 = get_slice_displacement(7, 8, 0, 1.0);
        assert!(d7.r_shift > 0.0);
        assert!(d7.start_shift < 0.0);
        assert!(d7.end_shift < 0.0);
    }

    #[test]
    fn test_degenerate_wedge() {
        let mut pb = PathBuilder::new();
        // r1 - r0 <= 2.0 triggers degenerate branch
        draw_floating_wedge(&mut pb, 272.0, 272.0, 50.0, 51.0, 0.0, 1.0, 6.0);
        let path = pb.finish();
        assert!(path.is_some());
        let p = path.unwrap();
        assert!(p.bounds().width() > 0.0);
    }

    #[test]
    fn test_sub_ring_paths() {
        let paths = build_sub_ring_paths(272.0, 272.0, 4, -45.0, 22.5, 1, 1.0);
        assert_eq!(paths.len(), 4);
        for p in &paths {
            assert!(p.bounds().width() > 0.0);
            assert!(p.bounds().height() > 0.0);
        }
    }

    #[test]
    fn test_build_main_ring_paths_empty() {
        let paths = build_main_ring_paths(272.0, 272.0, 0, -1, 0.0);
        assert!(paths.is_empty());
    }

    #[test]
    fn test_wedge_rasterization_area() {
        use tiny_skia::{Color, FillRule, Paint, Pixmap, Transform};

        let mut pixmap = Pixmap::new(544, 544).unwrap();
        let paths = build_main_ring_paths(272.0, 272.0, 8, -1, 0.0);

        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(255, 255, 255, 255));

        // Fill one wedge
        pixmap.fill_path(&paths[0], &paint, FillRule::Winding, Transform::identity(), None);

        // Count non-zero alpha pixels
        let non_zero_pixels = pixmap
            .pixels()
            .iter()
            .filter(|px| px.alpha() > 0)
            .count();

        // Theoretical area of 1/8 ring: ~7456 pixels (minus gap/rounded corners)
        // Should be around 6000 - 7500 pixels.
        assert!(
            non_zero_pixels > 5000 && non_zero_pixels < 8000,
            "Rendered pixels: {}",
            non_zero_pixels
        );
    }

    #[test]
    fn test_main_ring_paths_drag_parting() {
        let paths = build_main_ring_paths_animated_with_drag(
            272.0, 272.0, 8, -1, 0.0, 8.0, Some(2), 1.0,
        );
        assert_eq!(paths.len(), 8);
        for (_, p, _) in &paths {
            assert!(p.bounds().width() > 0.0);
            assert!(p.bounds().height() > 0.0);
        }
    }

    #[test]
    fn test_draw_drag_insertion_indicator_renders() {
        let mut pixmap = Pixmap::new(600, 600).unwrap();
        let primary = Color::from_rgba8(137, 180, 250, 255);
        draw_drag_insertion_indicator(
            &mut pixmap,
            300.0,
            300.0,
            50.0,
            150.0,
            0.0,
            primary,
            1.0,
            Transform::identity(),
        );
        let non_zero = pixmap.pixels().iter().filter(|p| p.alpha() > 0).count();
        assert!(non_zero > 100, "Insertion indicator should render luminous pixels");
    }

    #[test]
    fn test_draw_plucked_floating_wedge_renders() {
        let mut pixmap = Pixmap::new(600, 600).unwrap();
        let mut font_renderer = FontRenderer::new();
        let primary = Color::from_rgba8(137, 180, 250, 255);
        let on_primary = Color::from_rgba8(17, 17, 27, 255);
        draw_plucked_floating_wedge(
            &mut font_renderer,
            &mut pixmap,
            300.0,
            300.0,
            54.0,
            148.0,
            45.0,
            0.0,
            6.0,
            "folder",
            Some(1),
            1.0,
            primary,
            on_primary,
            Transform::identity(),
        );
        let non_zero = pixmap.pixels().iter().filter(|p| p.alpha() > 0).count();
        assert!(non_zero > 1000, "Plucked wedge should render shadow, body, and icon");
    }
}
