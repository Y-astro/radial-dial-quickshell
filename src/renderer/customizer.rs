// src/renderer/customizer.rs
#![allow(dead_code, clippy::too_many_arguments)]

use crate::font::FontRenderer;
use crate::renderer::text::{draw_icon, draw_text, draw_text_left};
use crate::state::actions::ActionDef;
use crate::state::config::FileJumpTarget;
use tiny_skia::{
    Color, FillRule, GradientStop, LinearGradient, Paint, PathBuilder, Pixmap, PixmapPaint, Point,
    SpreadMode, Stroke, Transform,
};

pub const CARD_W: f32 = 480.0;
pub const CARD_H: f32 = 540.0;
pub const CARD_RADIUS: f32 = 22.0;

pub const CATEGORIES: &[&str] = &[
    "All", "Apps", "Tools", "Media", "Capture", "Window", "System",
];

pub const AVAILABLE_ICONS: &[&str] = &[
    "folder", "folder_special", "school", "download",
    "description", "photo", "movie", "code",
    "music_note", "home", "terminal", "work",
    "favorite", "star", "folder_zip", "cloud",
];

// Color palette matching Quickshell RadialMenuCustomizer and theme configuration
#[inline]
pub fn col_primary() -> Color {
    Color::from_rgba8(168, 197, 253, 255) // #a8c5fd
}

#[inline]
pub fn col_on_primary() -> Color {
    Color::from_rgba8(6, 48, 91, 255) // #06305b
}

#[inline]
pub fn col_on_surface() -> Color {
    Color::from_rgba8(230, 230, 237, 255) // #e6e6ed
}

#[inline]
pub fn col_subtext() -> Color {
    Color::from_rgba8(178, 178, 191, 204) // #b2b2bf (80% opacity)
}

#[inline]
pub fn col_card_bg() -> Color {
    Color::from_rgba8(20, 20, 31, 245) // #14141f (96% opacity)
}

#[inline]
pub fn col_card_border() -> Color {
    Color::from_rgba8(255, 255, 255, 46) // 18% white
}

#[inline]
pub fn col_input_bg() -> Color {
    Color::from_rgba8(0, 0, 0, 56) // 22% black translucent cutout
}

#[inline]
pub fn col_input_border() -> Color {
    Color::from_rgba(1.0, 1.0, 1.0, 0.12).unwrap_or(Color::WHITE)
}

#[inline]
pub fn col_danger() -> Color {
    Color::from_rgba8(255, 107, 107, 255) // #ff6b6b
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CustomizerColors {
    pub primary: Color,
    pub on_primary: Color,
    pub on_surface: Color,
    pub subtext: Color,
    pub card_bg: Color,
    pub surface_base: Color,
}

impl Default for CustomizerColors {
    fn default() -> Self {
        Self {
            primary: col_primary(),
            on_primary: col_on_primary(),
            on_surface: col_on_surface(),
            subtext: col_subtext(),
            card_bg: col_card_bg(),
            surface_base: col_card_bg(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CustomizerMode {
    SliceSwap { slot_index: usize },
    FileTargetEdit { target_index: usize },
    FileTargetAdd,
}

impl CustomizerMode {
    pub fn is_swap(&self) -> bool {
        matches!(self, CustomizerMode::SliceSwap { .. })
    }

    pub fn is_file_edit(&self) -> bool {
        matches!(self, CustomizerMode::FileTargetEdit { .. })
    }

    pub fn is_file_add(&self) -> bool {
        matches!(self, CustomizerMode::FileTargetAdd)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CustomizerState {
    pub is_open: bool,
    pub mode: CustomizerMode,
    pub active_category: String, // "All", "Apps", "Tools", "Media", "Capture", "Window", "System"
    pub search_query: String,
    pub search_focused: bool,
    pub scroll_offset: f32,
    pub selected_item: Option<String>,
    // Input field buffers for file target editing:
    pub input_label: String,
    pub input_path: String,
    pub input_icon: String,
    pub focused_field: usize, // 0=label, 1=path, 2=icon
    // Opening animation state (180ms OutCubic)
    pub anim_elapsed: f32,
    pub anim_progress: f32,
}

impl Default for CustomizerState {
    fn default() -> Self {
        Self {
            is_open: false,
            mode: CustomizerMode::SliceSwap { slot_index: 0 },
            active_category: "All".to_string(),
            search_query: String::new(),
            search_focused: false,
            scroll_offset: 0.0,
            selected_item: None,
            input_label: String::new(),
            input_path: String::new(),
            input_icon: "folder".to_string(),
            focused_field: 0,
            anim_elapsed: 0.0,
            anim_progress: 0.0,
        }
    }
}

impl CustomizerState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open_slice_swap(&mut self, slot_index: usize) {
        self.mode = CustomizerMode::SliceSwap { slot_index };
        self.is_open = true;
        self.search_query.clear();
        self.search_focused = false;
        self.scroll_offset = 0.0;
        self.selected_item = None;
        self.anim_elapsed = 0.0;
        self.anim_progress = 0.0;
    }

    pub fn open_file_edit(&mut self, target_index: usize, label: &str, path: &str, icon: &str) {
        self.mode = CustomizerMode::FileTargetEdit { target_index };
        self.input_label = label.to_string();
        self.input_path = path.to_string();
        self.input_icon = if icon.is_empty() {
            "folder".to_string()
        } else {
            icon.to_string()
        };
        self.focused_field = 0;
        self.search_focused = false;
        self.anim_elapsed = 0.0;
        self.anim_progress = 0.0;
        self.is_open = true;
    }

    pub fn open_file_edit_target(&mut self, target_index: usize, target: &FileJumpTarget) {
        self.open_file_edit(target_index, &target.label, &target.path, &target.icon);
    }

    pub fn open_file_add(&mut self) {
        self.mode = CustomizerMode::FileTargetAdd;
        self.input_label.clear();
        self.input_path.clear();
        self.input_icon = "folder".to_string();
        self.focused_field = 0;
        self.search_focused = false;
        self.anim_elapsed = 0.0;
        self.anim_progress = 0.0;
        self.is_open = true;
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    pub fn filtered_catalogue<'a>(&self, catalogue: &'a [ActionDef]) -> Vec<&'a ActionDef> {
        filter_catalogue(catalogue, &self.active_category, &self.search_query)
    }
}

/// Filter catalogue by category and search query.
/// If `category == "All"`, all categories are returned.
/// If `search_query` is non-empty, matches against label or desc (case-insensitive).
pub fn filter_catalogue<'a>(
    catalogue: &'a [ActionDef],
    category: &str,
    search_query: &str,
) -> Vec<&'a ActionDef> {
    let q = search_query.trim().to_lowercase();
    catalogue
        .iter()
        .filter(|item| {
            if category != "All" && !item.category.eq_ignore_ascii_case(category) {
                return false;
            }
            if !q.is_empty() {
                let l = item.label.to_lowercase();
                let d = item.desc.to_lowercase();
                if !l.contains(&q) && !d.contains(&q) {
                    return false;
                }
            }
            true
        })
        .collect()
}

/// Helper to build a rounded rectangle path with corner radius `r`.
pub fn rounded_rect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<tiny_skia::Path> {
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    let mut pb = PathBuilder::new();
    if r < 0.5 {
        pb.move_to(x, y);
        pb.line_to(x + w, y);
        pb.line_to(x + w, y + h);
        pb.line_to(x, y + h);
        pb.close();
    } else {
        pb.move_to(x + r, y);
        pb.line_to(x + w - r, y);
        pb.quad_to(x + w, y, x + w, y + r);
        pb.line_to(x + w, y + h - r);
        pb.quad_to(x + w, y + h, x + w - r, y + h);
        pb.line_to(x + r, y + h);
        pb.quad_to(x, y + h, x, y + h - r);
        pb.line_to(x, y + r);
        pb.quad_to(x, y, x + r, y);
        pb.close();
    }
    pb.finish()
}

/// Helper to fill a rounded rectangle.
pub fn fill_rounded_rect(pixmap: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, color: Color) {
    if let Some(path) = rounded_rect_path(x, y, w, h, r) {
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;
        pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

/// Helper to stroke a rounded rectangle.
pub fn stroke_rounded_rect(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    color: Color,
    stroke_w: f32,
) {
    if let Some(path) = rounded_rect_path(x, y, w, h, r) {
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

thread_local! {
    static CUSTOMIZER_FONT_RENDERER: std::cell::RefCell<FontRenderer> = std::cell::RefCell::new(FontRenderer::new());
}

/// Render the customizer modal: 480x560 px rounded card positioned near anchor.
pub fn render_customizer(
    state: &CustomizerState,
    catalogue: &[ActionDef],
    active_slice_ids: &[String],
    pixmap: &mut Pixmap,
    screen_w: f32,
    screen_h: f32,
    anchor_x: f32,
    anchor_y: f32,
) {
    if !state.is_open || screen_w <= 0.0 || screen_h <= 0.0 {
        return;
    }
    CUSTOMIZER_FONT_RENDERER.with(|fr| {
        render_customizer_with_font(
            state,
            catalogue,
            active_slice_ids,
            pixmap,
            screen_w,
            screen_h,
            anchor_x,
            anchor_y,
            &mut fr.borrow_mut(),
            CustomizerColors::default(),
        );
    });
}

/// Render the customizer modal using an explicitly supplied `FontRenderer`.
pub fn render_customizer_with_font(
    state: &CustomizerState,
    catalogue: &[ActionDef],
    active_slice_ids: &[String],
    pixmap: &mut Pixmap,
    screen_w: f32,
    screen_h: f32,
    anchor_x: f32,
    anchor_y: f32,
    font_renderer: &mut FontRenderer,
    colors: CustomizerColors,
) {
    if !state.is_open || screen_w <= 0.0 || screen_h <= 0.0 {
        return;
    }

    // Animation progress (0.01 to 1.0)
    let anim_t = state.anim_progress.clamp(0.01, 1.0);
    let scale = 0.92 + 0.08 * anim_t;

    // 2. Card Dimensions & Placement (480x540 px, anchor-relative)
    let base_card_w = CARD_W.min(screen_w - 20.0);
    let base_card_h = CARD_H.min(screen_h - 20.0);
    let preferred_x = anchor_x - base_card_w / 2.0 + (screen_w / 2.0 - anchor_x).signum() * 60.0;
    let preferred_y = anchor_y - base_card_h / 2.0 + (screen_h / 2.0 - anchor_y).signum() * 60.0;
    let base_card_x = preferred_x.clamp(10.0, screen_w - base_card_w - 10.0);
    let base_card_y = preferred_y.clamp(10.0, screen_h - base_card_h - 10.0);

    let card_w = base_card_w * scale;
    let card_h = base_card_h * scale;
    let card_x = base_card_x + (base_card_w - card_w) / 2.0;
    let card_y = base_card_y + (base_card_h - card_h) / 2.0;

    if let Some(card_path) = rounded_rect_path(card_x, card_y, card_w, card_h, CARD_RADIUS) {
        // Subtle drop shadow outline
        stroke_rounded_rect(
            pixmap,
            card_x - 1.0,
            card_y - 1.0,
            card_w + 2.0,
            card_h + 2.0,
            CARD_RADIUS + 1.0,
            Color::from_rgba8(0, 0, 0, 60),
            2.0,
        );

        // Translucent Frosted Glass Base Fill (vertical gradient matching QML & radial menu blur)
        // Alpha ~0.48 top to ~0.42 bottom allows Hyprland compositor blur to show through without darkening
        let surface = colors.surface_base;
        let card_bg_top = Color::from_rgba(
            (surface.red() * 1.08).min(1.0),
            (surface.green() * 1.08).min(1.0),
            (surface.blue() * 1.08).min(1.0),
            0.48,
        ).unwrap_or(Color::from_rgba8(20, 22, 22, 122));

        let card_bg_bottom = Color::from_rgba(
            (surface.red() * 0.92).min(1.0),
            (surface.green() * 0.92).min(1.0),
            (surface.blue() * 0.92).min(1.0),
            0.42,
        ).unwrap_or(Color::from_rgba8(14, 16, 16, 107));

        let mut bg_paint = Paint::default();
        bg_paint.anti_alias = true;
        let grad = LinearGradient::new(
            Point::from_xy(card_x, card_y),
            Point::from_xy(card_x, card_y + card_h),
            vec![
                GradientStop::new(0.0, card_bg_top),
                GradientStop::new(1.0, card_bg_bottom),
            ],
            SpreadMode::Pad,
            Transform::identity(),
        );
        if let Some(shader) = grad {
            bg_paint.shader = shader;
        } else {
            bg_paint.set_color(card_bg_top);
        }
        pixmap.fill_path(&card_path, &bg_paint, FillRule::Winding, Transform::identity(), None);

        // Subtle white sheen
        let mut sheen_paint = Paint::default();
        sheen_paint.set_color(Color::from_rgba(1.0, 1.0, 1.0, 0.03).unwrap_or(Color::WHITE));
        sheen_paint.anti_alias = true;
        pixmap.fill_path(&card_path, &sheen_paint, FillRule::Winding, Transform::identity(), None);

        // Crisp soft white translucent border
        stroke_rounded_rect(
            pixmap,
            card_x,
            card_y,
            card_w,
            card_h,
            CARD_RADIUS,
            Color::from_rgba(1.0, 1.0, 1.0, 0.18).unwrap_or(Color::WHITE),
            1.5,
        );

        // Top frosted highlight reflection
        let mut highlight_pb = PathBuilder::new();
        highlight_pb.move_to(card_x + CARD_RADIUS, card_y + 1.5);
        highlight_pb.line_to(card_x + card_w - CARD_RADIUS, card_y + 1.5);
        if let Some(p) = highlight_pb.finish() {
            let mut paint = Paint::default();
            paint.set_color(Color::from_rgba(1.0, 1.0, 1.0, 0.20).unwrap_or(Color::WHITE));
            let stroke = Stroke {
                width: 1.5,
                ..Default::default()
            };
            pixmap.stroke_path(&p, &paint, &stroke, Transform::identity(), None);
        }
    }

    // 3. Title Bar / Header (mode description & close button)
    let content_x = card_x + 18.0;
    let content_w = card_w - 36.0;

    let icon_box_x = content_x;
    let icon_box_y = card_y + 16.0;
    let icon_box_size = 38.0;

    fill_rounded_rect(
        pixmap,
        icon_box_x,
        icon_box_y,
        icon_box_size,
        icon_box_size,
        12.0,
        Color::from_rgba(colors.primary.red(), colors.primary.green(), colors.primary.blue(), 0.18)
            .unwrap_or(colors.primary),
    );
    stroke_rounded_rect(
        pixmap,
        icon_box_x,
        icon_box_y,
        icon_box_size,
        icon_box_size,
        12.0,
        colors.primary,
        1.0,
    );

    let (mode_icon, mode_title, mode_subtitle) = match &state.mode {
        CustomizerMode::SliceSwap { slot_index } => (
            "swap_horiz",
            "Customize Radial Functions",
            format!(
                "Slot {} • Click row to replace segment, or Add/Remove from dial",
                slot_index + 1
            ),
        ),
        CustomizerMode::FileTargetEdit { target_index } => (
            "edit",
            "Edit Folder Target",
            format!(
                "Target #{} • Customize folder path, label, and icon",
                target_index + 1
            ),
        ),
        CustomizerMode::FileTargetAdd => (
            "add",
            "Add Folder Target",
            "Customize folder path, label, and icon".to_string(),
        ),
    };

    draw_icon(
        font_renderer,
        pixmap,
        mode_icon,
        icon_box_x + icon_box_size / 2.0,
        icon_box_y + icon_box_size / 2.0,
        22.0,
        colors.primary,
    );

    let title_x = icon_box_x + icon_box_size + 12.0;
    draw_text_left(
        font_renderer,
        pixmap,
        mode_title,
        title_x,
        icon_box_y + 11.0,
        15.0,
        colors.on_surface,
    );
    draw_text_left(
        font_renderer,
        pixmap,
        &mode_subtitle,
        title_x,
        icon_box_y + 27.0,
        11.0,
        colors.subtext,
    );

    // Close button on the top right
    let close_size = 32.0;
    let close_x = content_x + content_w - close_size;
    let close_y = card_y + 19.0;
    fill_rounded_rect(
        pixmap,
        close_x,
        close_y,
        close_size,
        close_size,
        16.0,
        Color::from_rgba8(255, 255, 255, 20),
    );
    stroke_rounded_rect(
        pixmap,
        close_x,
        close_y,
        close_size,
        close_size,
        16.0,
        Color::from_rgba8(255, 255, 255, 30),
        1.0,
    );
    draw_icon(
        font_renderer,
        pixmap,
        "close",
        close_x + close_size / 2.0,
        close_y + close_size / 2.0,
        18.0,
        colors.on_surface,
    );

    // Reset to Defaults button (in SliceSwap mode, next to close button)
    if state.mode.is_swap() {
        let reset_x = close_x - 38.0;
        fill_rounded_rect(
            pixmap,
            reset_x,
            close_y,
            close_size,
            close_size,
            16.0,
            Color::from_rgba8(255, 255, 255, 15),
        );
        stroke_rounded_rect(
            pixmap,
            reset_x,
            close_y,
            close_size,
            close_size,
            16.0,
            Color::from_rgba8(255, 255, 255, 30),
            1.0,
        );
        draw_icon(
            font_renderer,
            pixmap,
            "refresh",
            reset_x + close_size / 2.0,
            close_y + close_size / 2.0,
            18.0,
            colors.subtext,
        );
    }
    draw_icon(
        font_renderer,
        pixmap,
        "close",
        close_x + close_size / 2.0,
        close_y + close_size / 2.0,
        18.0,
        colors.on_surface,
    );

    // 4. Body Content per Mode
    match &state.mode {
        CustomizerMode::SliceSwap { .. } => {
            render_slice_swap_body(
                state,
                catalogue,
                active_slice_ids,
                content_x,
                card_y,
                content_w,
                card_h,
                pixmap,
                font_renderer,
                colors,
            );
        }
        CustomizerMode::FileTargetEdit { target_index } => {
            render_file_target_body(
                state,
                Some(*target_index),
                content_x,
                card_y,
                content_w,
                card_h,
                pixmap,
                font_renderer,
                colors,
            );
        }
        CustomizerMode::FileTargetAdd => {
            render_file_target_body(
                state,
                None,
                content_x,
                card_y,
                content_w,
                card_h,
                pixmap,
                font_renderer,
                colors,
            );
        }
    }
}

/// Render SliceSwap mode body:
/// Category tabs, search bar, scrollable catalogue action list, and Reset to Defaults footer.
fn render_slice_swap_body(
    state: &CustomizerState,
    catalogue: &[ActionDef],
    active_slice_ids: &[String],
    content_x: f32,
    card_y: f32,
    content_w: f32,
    _card_h: f32,
    pixmap: &mut Pixmap,
    font_renderer: &mut FontRenderer,
    colors: CustomizerColors,
) {
    // 1. Category Tabs (All, Apps, Tools, Media, Capture, Window, System)
    let tabs_y = card_y + 66.0;
    let tab_h = 26.0;
    let num_tabs = CATEGORIES.len() as f32;
    let tab_gap = 5.0;
    let tab_w = (content_w - (num_tabs - 1.0) * tab_gap) / num_tabs;

    for (i, cat) in CATEGORIES.iter().enumerate() {
        let tx = content_x + i as f32 * (tab_w + tab_gap);
        let is_active = state.active_category.eq_ignore_ascii_case(cat);

        if is_active {
            fill_rounded_rect(pixmap, tx, tabs_y, tab_w, tab_h, 13.0, colors.primary);
            draw_text(
                font_renderer,
                pixmap,
                cat,
                tx + tab_w / 2.0,
                tabs_y + tab_h / 2.0,
                11.0,
                colors.on_primary,
            );
        } else {
            fill_rounded_rect(
                pixmap,
                tx,
                tabs_y,
                tab_w,
                tab_h,
                13.0,
                Color::from_rgba8(255, 255, 255, 13),
            );
            stroke_rounded_rect(
                pixmap,
                tx,
                tabs_y,
                tab_w,
                tab_h,
                13.0,
                Color::from_rgba8(255, 255, 255, 30),
                1.0,
            );
            draw_text(
                font_renderer,
                pixmap,
                cat,
                tx + tab_w / 2.0,
                tabs_y + tab_h / 2.0,
                11.0,
                colors.on_surface,
            );
        }
    }

    // 2. Search Bar
    let search_y = card_y + 102.0;
    let search_h = 34.0;
    fill_rounded_rect(
        pixmap,
        content_x,
        search_y,
        content_w,
        search_h,
        10.0,
        col_input_bg(),
    );
    let search_border_col = if state.search_focused {
        colors.primary
    } else {
        col_input_border()
    };
    stroke_rounded_rect(
        pixmap,
        content_x,
        search_y,
        content_w,
        search_h,
        10.0,
        search_border_col,
        if state.search_focused { 1.5 } else { 1.0 },
    );

    // Search Icon
    draw_icon(
        font_renderer,
        pixmap,
        "search",
        content_x + 18.0,
        search_y + search_h / 2.0,
        16.0,
        if state.search_focused { colors.primary } else { Color::from_rgba8(255, 255, 255, 102) },
    );

    let query_x = content_x + 36.0;
    if state.search_query.is_empty() {
        let placeholder = if state.search_focused { "|" } else { "Search function catalogue..." };
        draw_text_left(
            font_renderer,
            pixmap,
            placeholder,
            query_x,
            search_y + search_h / 2.0,
            12.0,
            if state.search_focused { colors.primary } else { Color::from_rgba8(255, 255, 255, 89) },
        );
    } else {
        let display_text = if state.search_focused {
            format!("{}|", state.search_query)
        } else {
            state.search_query.clone()
        };
        draw_text_left(
            font_renderer,
            pixmap,
            &display_text,
            query_x,
            search_y + search_h / 2.0,
            12.0,
            colors.on_surface,
        );

        // Clear query icon
        draw_icon(
            font_renderer,
            pixmap,
            "close",
            content_x + content_w - 18.0,
            search_y + search_h / 2.0,
            14.0,
            Color::from_rgba8(255, 255, 255, 128),
        );
    }

    // 3. Scrollable Catalogue Action List
    let list_y = card_y + 146.0;
    let list_h: f32 = 376.0;
    let filtered = state.filtered_catalogue(catalogue);

    // Build item list: each entry is (action_def, is_active)
    let mut ordered_items: Vec<(&ActionDef, bool)> = Vec::new();

    // Active items first (in order they appear in active_slice_ids)
    for active_id in active_slice_ids {
        if let Some(def) = catalogue.iter().find(|d| d.id == active_id.as_str()) {
            let passes_filter = filtered.iter().any(|f| f.id == def.id);
            if passes_filter {
                ordered_items.push((def, true));
            }
        }
    }

    // Then filtered items that are NOT already active
    for def in &filtered {
        let is_active = active_slice_ids.iter().any(|id| id.as_str() == def.id);
        if !is_active {
            ordered_items.push((def, false));
        }
    }

    let vp_w = content_w.round() as u32;
    let vp_h = list_h.round() as u32;

    if vp_w > 0 && vp_h > 0 {
        if let Some(mut list_pixmap) = Pixmap::new(vp_w, vp_h) {
            if ordered_items.is_empty() {
                draw_text(
                    font_renderer,
                    &mut list_pixmap,
                    "No matching functions found",
                    content_w / 2.0,
                    list_h / 2.0,
                    13.0,
                    colors.subtext,
                );
            } else {
                let item_h = 50.0;
                let item_gap = 4.0;
                let step = item_h + item_gap;

                for (idx, (item, is_active)) in ordered_items.iter().enumerate() {
                    let cur_y = idx as f32 * step - state.scroll_offset;

                    // Viewport bounds check: skip items completely outside list bounds
                    if cur_y + item_h < 0.0 || cur_y > list_h {
                        continue;
                    }

                    // Item card background - active items have subtle primary tint
                    if *is_active {
                        fill_rounded_rect(
                            &mut list_pixmap,
                            0.0,
                            cur_y,
                            content_w,
                            item_h,
                            12.0,
                            Color::from_rgba(colors.primary.red(), colors.primary.green(), colors.primary.blue(), 0.12)
                                .unwrap_or(colors.primary),
                        );
                        stroke_rounded_rect(
                            &mut list_pixmap,
                            0.0,
                            cur_y,
                            content_w,
                            item_h,
                            12.0,
                            Color::from_rgba(colors.primary.red(), colors.primary.green(), colors.primary.blue(), 0.35)
                                .unwrap_or(colors.primary),
                            1.0,
                        );
                    } else {
                        fill_rounded_rect(
                            &mut list_pixmap,
                            0.0,
                            cur_y,
                            content_w,
                            item_h,
                            12.0,
                            Color::from_rgba8(0, 0, 0, 56),
                        );
                        stroke_rounded_rect(
                            &mut list_pixmap,
                            0.0,
                            cur_y,
                            content_w,
                            item_h,
                            12.0,
                            Color::from_rgba8(255, 255, 255, 20),
                            1.0,
                        );
                    }

                    // Left icon box
                    let ib_x = 8.0;
                    let ib_y = cur_y + 8.0;
                    let ib_size = 34.0;
                    let ib_bg = if *is_active {
                        Color::from_rgba(colors.primary.red(), colors.primary.green(), colors.primary.blue(), 0.20)
                            .unwrap_or(colors.primary)
                    } else {
                        Color::from_rgba8(255, 255, 255, 15)
                    };
                    fill_rounded_rect(&mut list_pixmap, ib_x, ib_y, ib_size, ib_size, 10.0, ib_bg);

                    let icon_col = if *is_active { colors.primary } else { colors.on_surface };
                    draw_icon(
                        font_renderer,
                        &mut list_pixmap,
                        item.icon,
                        ib_x + ib_size / 2.0,
                        ib_y + ib_size / 2.0,
                        20.0,
                        icon_col,
                    );

                    // Action Label and Description
                    let text_x = ib_x + ib_size + 10.0;
                    draw_text_left(
                        font_renderer,
                        &mut list_pixmap,
                        item.label,
                        text_x,
                        cur_y + 16.0,
                        13.0,
                        colors.on_surface,
                    );

                    let desc = if item.desc.is_empty() { item.category } else { item.desc };
                    draw_text_left(
                        font_renderer,
                        &mut list_pixmap,
                        desc,
                        text_x,
                        cur_y + 33.0,
                        10.0,
                        colors.subtext,
                    );

                    // Right action button
                    if *is_active {
                        // "Active" badge + red remove circle button
                        let badge_w = 64.0;
                        let badge_h = 22.0;
                        let badge_x = content_w - 46.0 - badge_w - 8.0;
                        let badge_y = cur_y + (item_h - badge_h) / 2.0;
                        fill_rounded_rect(
                            &mut list_pixmap,
                            badge_x,
                            badge_y,
                            badge_w,
                            badge_h,
                            11.0,
                            Color::from_rgba(colors.primary.red(), colors.primary.green(), colors.primary.blue(), 0.18)
                                .unwrap_or(colors.primary),
                        );
                        stroke_rounded_rect(
                            &mut list_pixmap,
                            badge_x,
                            badge_y,
                            badge_w,
                            badge_h,
                            11.0,
                            Color::from_rgba(colors.primary.red(), colors.primary.green(), colors.primary.blue(), 0.5)
                                .unwrap_or(colors.primary),
                            1.0,
                        );
                        draw_icon(
                            font_renderer,
                            &mut list_pixmap,
                            "check_circle",
                            badge_x + 14.0,
                            badge_y + badge_h / 2.0,
                            13.0,
                            colors.primary,
                        );
                        draw_text_left(
                            font_renderer,
                            &mut list_pixmap,
                            "Active",
                            badge_x + 24.0,
                            badge_y + badge_h / 2.0,
                            10.0,
                            colors.primary,
                        );

                        // Red remove circle
                        let rm_r = 14.0;
                        let rm_cx = content_w - 10.0 - rm_r;
                        let rm_cy = cur_y + item_h / 2.0;
                        fill_rounded_rect(
                            &mut list_pixmap,
                            rm_cx - rm_r,
                            rm_cy - rm_r,
                            rm_r * 2.0,
                            rm_r * 2.0,
                            rm_r,
                            Color::from_rgba8(255, 80, 80, 200),
                        );
                        draw_icon(
                            font_renderer,
                            &mut list_pixmap,
                            "remove",
                            rm_cx,
                            rm_cy,
                            14.0,
                            Color::from_rgba8(255, 255, 255, 230),
                        );
                    } else {
                        // "+ Add" button
                        let btn_w = 64.0;
                        let btn_h = 26.0;
                        let btn_x = content_w - btn_w - 10.0;
                        let btn_y = cur_y + 12.0;
                        fill_rounded_rect(&mut list_pixmap, btn_x, btn_y, btn_w, btn_h, 13.0, colors.primary);
                        draw_text(
                            font_renderer,
                            &mut list_pixmap,
                            "+ Add",
                            btn_x + btn_w / 2.0,
                            btn_y + btn_h / 2.0,
                            10.0,
                            colors.on_primary,
                        );
                    }
                }
            }

            pixmap.draw_pixmap(
                content_x.round() as i32,
                list_y.round() as i32,
                list_pixmap.as_ref(),
                &PixmapPaint::default(),
                Transform::identity(),
                None,
            );
        }
    }
}

/// Render FileTargetEdit and FileTargetAdd mode body:
/// Label input, path input with Browse button, icon picker grid, and Delete/Cancel/Save action buttons.
fn render_file_target_body(
    state: &CustomizerState,
    target_index: Option<usize>,
    content_x: f32,
    card_y: f32,
    content_w: f32,
    _card_h: f32,
    pixmap: &mut Pixmap,
    font_renderer: &mut FontRenderer,
    colors: CustomizerColors,
) {
    // 1. Display Name Input Field
    let label_hdr_y = card_y + 68.0;
    draw_text_left(
        font_renderer,
        pixmap,
        "Display Name",
        content_x,
        label_hdr_y,
        11.0,
        colors.subtext,
    );

    let label_box_y = card_y + 84.0;
    let label_box_h = 38.0;
    fill_rounded_rect(
        pixmap,
        content_x,
        label_box_y,
        content_w,
        label_box_h,
        10.0,
        col_input_bg(),
    );
    let (border_col, border_w) = if state.focused_field == 0 {
        (colors.primary, 1.5)
    } else {
        (col_input_border(), 1.0)
    };
    stroke_rounded_rect(
        pixmap,
        content_x,
        label_box_y,
        content_w,
        label_box_h,
        10.0,
        border_col,
        border_w,
    );

    let label_text_x = content_x + 12.0;
    if state.input_label.is_empty() {
        draw_text_left(
            font_renderer,
            pixmap,
            "e.g. Projects, Notes, Work, Music...",
            label_text_x,
            label_box_y + label_box_h / 2.0,
            12.0,
            Color::from_rgba8(255, 255, 255, 77),
        );
    } else {
        let text = if state.focused_field == 0 {
            format!("{}|", state.input_label)
        } else {
            state.input_label.clone()
        };
        draw_text_left(
            font_renderer,
            pixmap,
            &text,
            label_text_x,
            label_box_y + label_box_h / 2.0,
            13.0,
            colors.on_surface,
        );
    }

    // 2. Target Path Input Field + Browse Button
    let path_hdr_y = card_y + 138.0;
    draw_text_left(
        font_renderer,
        pixmap,
        "Target Folder or File Path",
        content_x,
        path_hdr_y,
        11.0,
        colors.subtext,
    );

    let path_box_y = card_y + 154.0;
    let path_box_h = 38.0;
    let browse_w = 90.0;
    let path_box_w = content_w - browse_w - 8.0;

    fill_rounded_rect(
        pixmap,
        content_x,
        path_box_y,
        path_box_w,
        path_box_h,
        10.0,
        col_input_bg(),
    );
    let (p_border_col, p_border_w) = if state.focused_field == 1 {
        (colors.primary, 1.5)
    } else {
        (col_input_border(), 1.0)
    };
    stroke_rounded_rect(
        pixmap,
        content_x,
        path_box_y,
        path_box_w,
        path_box_h,
        10.0,
        p_border_col,
        p_border_w,
    );

    let path_text_x = content_x + 12.0;
    if state.input_path.is_empty() {
        draw_text_left(
            font_renderer,
            pixmap,
            "e.g. ~/Projects or /mnt/data/...",
            path_text_x,
            path_box_y + path_box_h / 2.0,
            12.0,
            Color::from_rgba8(255, 255, 255, 77),
        );
    } else {
        let text = if state.focused_field == 1 {
            format!("{}|", state.input_path)
        } else {
            state.input_path.clone()
        };
        draw_text_left(
            font_renderer,
            pixmap,
            &text,
            path_text_x,
            path_box_y + path_box_h / 2.0,
            12.0,
            colors.on_surface,
        );
    }

    // Browse Button
    let browse_x = content_x + path_box_w + 8.0;
    fill_rounded_rect(
        pixmap,
        browse_x,
        path_box_y,
        browse_w,
        path_box_h,
        10.0,
        Color::from_rgba(1.0, 1.0, 1.0, 0.08).unwrap_or(Color::BLACK),
    );
    stroke_rounded_rect(
        pixmap,
        browse_x,
        path_box_y,
        browse_w,
        path_box_h,
        10.0,
        Color::from_rgba(1.0, 1.0, 1.0, 0.14).unwrap_or(Color::WHITE),
        1.0,
    );
    draw_icon(
        font_renderer,
        pixmap,
        "folder_open",
        browse_x + 20.0,
        path_box_y + path_box_h / 2.0,
        16.0,
        colors.on_surface,
    );
    draw_text_left(
        font_renderer,
        pixmap,
        "Browse",
        browse_x + 36.0,
        path_box_y + path_box_h / 2.0,
        11.0,
        colors.on_surface,
    );

    // 3. Choose Icon Picker Grid (Flow layout: 10 columns matching shell reference)
    let icon_hdr_y = card_y + 208.0;
    draw_text_left(
        font_renderer,
        pixmap,
        "Choose Icon",
        content_x,
        icon_hdr_y,
        11.0,
        colors.subtext,
    );

    let grid_y = card_y + 226.0;
    let cols = 10;
    let chip_w = 36.0;
    let chip_h = 36.0;
    let gap = 8.0;

    for (idx, icon_name) in AVAILABLE_ICONS.iter().enumerate() {
        let row = idx / cols;
        let col = idx % cols;
        let ix = content_x + col as f32 * (chip_w + gap);
        let iy = grid_y + row as f32 * (chip_h + gap);
        let is_selected = state.input_icon == *icon_name;

        if is_selected {
            fill_rounded_rect(pixmap, ix, iy, chip_w, chip_h, 10.0, colors.primary);
            draw_icon(
                font_renderer,
                pixmap,
                icon_name,
                ix + chip_w / 2.0,
                iy + chip_h / 2.0,
                20.0,
                colors.on_primary,
            );
        } else {
            fill_rounded_rect(
                pixmap,
                ix,
                iy,
                chip_w,
                chip_h,
                10.0,
                Color::from_rgba(1.0, 1.0, 1.0, 0.05).unwrap_or(Color::BLACK),
            );
            stroke_rounded_rect(
                pixmap,
                ix,
                iy,
                chip_w,
                chip_h,
                10.0,
                Color::from_rgba(1.0, 1.0, 1.0, 0.12).unwrap_or(Color::WHITE),
                1.0,
            );
            draw_icon(
                font_renderer,
                pixmap,
                icon_name,
                ix + chip_w / 2.0,
                iy + chip_h / 2.0,
                20.0,
                colors.on_surface,
            );
        }
    }

    // 4. Action Buttons Row (Delete / Cancel / Save)
    let btns_h = 36.0;
    let btns_y = card_y + 486.0;

    // Delete button (visible only in Edit mode)
    if target_index.is_some() {
        let del_w = 90.0;
        fill_rounded_rect(
            pixmap,
            content_x,
            btns_y,
            del_w,
            btns_h,
            10.0,
            Color::from_rgba8(255, 107, 107, 45),
        );
        stroke_rounded_rect(
            pixmap,
            content_x,
            btns_y,
            del_w,
            btns_h,
            10.0,
            Color::from_rgba8(255, 107, 107, 120),
            1.0,
        );
        draw_icon(
            font_renderer,
            pixmap,
            "delete",
            content_x + 22.0,
            btns_y + btns_h / 2.0,
            16.0,
            col_danger(),
        );
        draw_text_left(
            font_renderer,
            pixmap,
            "Delete",
            content_x + 38.0,
            btns_y + btns_h / 2.0,
            11.0,
            col_danger(),
        );
    }

    // Cancel and Save buttons on the right
    let cancel_w = 80.0;
    let save_w = 106.0;
    let save_x = content_x + content_w - save_w;
    let cancel_x = save_x - cancel_w - 8.0;

    // Cancel button
    fill_rounded_rect(
        pixmap,
        cancel_x,
        btns_y,
        cancel_w,
        btns_h,
        10.0,
        Color::from_rgba(1.0, 1.0, 1.0, 0.08).unwrap_or(Color::BLACK),
    );
    stroke_rounded_rect(
        pixmap,
        cancel_x,
        btns_y,
        cancel_w,
        btns_h,
        10.0,
        Color::from_rgba(1.0, 1.0, 1.0, 0.16).unwrap_or(Color::WHITE),
        1.0,
    );
    draw_text(
        font_renderer,
        pixmap,
        "Cancel",
        cancel_x + cancel_w / 2.0,
        btns_y + btns_h / 2.0,
        12.0,
        colors.on_surface,
    );

    // Save / Add Target button
    fill_rounded_rect(pixmap, save_x, btns_y, save_w, btns_h, 10.0, colors.primary);
    draw_icon(
        font_renderer,
        pixmap,
        "check",
        save_x + 18.0,
        btns_y + btns_h / 2.0,
        16.0,
        colors.on_primary,
    );
    let save_text = if target_index.is_some() {
        "Save"
    } else {
        "Add Target"
    };
    draw_text_left(
        font_renderer,
        pixmap,
        save_text,
        save_x + 32.0,
        btns_y + btns_h / 2.0,
        12.0,
        colors.on_primary,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::actions::function_catalogue;

    #[test]
    fn test_customizer_state_defaults() {
        let state = CustomizerState::default();
        assert!(!state.is_open);
        assert_eq!(state.mode, CustomizerMode::SliceSwap { slot_index: 0 });
        assert_eq!(state.active_category, "All");
        assert!(state.search_query.is_empty());
        assert_eq!(state.scroll_offset, 0.0);
        assert_eq!(state.selected_item, None);
        assert!(state.input_label.is_empty());
        assert!(state.input_path.is_empty());
        assert_eq!(state.input_icon, "folder");
        assert_eq!(state.focused_field, 0);
    }

    #[test]
    fn test_customizer_category_filter() {
        let catalogue = function_catalogue();
        let mut state = CustomizerState::default();

        // "All" returns the entire catalogue
        let all = state.filtered_catalogue(&catalogue);
        assert_eq!(all.len(), catalogue.len());

        // Category "Apps"
        state.active_category = "Apps".to_string();
        let apps = state.filtered_catalogue(&catalogue);
        assert!(!apps.is_empty());
        assert!(apps.iter().all(|a| a.category.eq_ignore_ascii_case("Apps")));
        assert!(apps.iter().any(|a| a.id == "terminal"));

        // Category "Tools"
        state.active_category = "Tools".to_string();
        let tools = state.filtered_catalogue(&catalogue);
        assert!(!tools.is_empty());
        assert!(tools.iter().all(|a| a.category.eq_ignore_ascii_case("Tools")));
        assert!(tools.iter().any(|a| a.id == "clipboard"));

        // Category "Media"
        state.active_category = "Media".to_string();
        let media = state.filtered_catalogue(&catalogue);
        assert!(!media.is_empty());
        assert!(media.iter().all(|a| a.category.eq_ignore_ascii_case("Media")));
        assert!(media.iter().any(|a| a.id == "media_play_pause"));

        // Category "Window"
        state.active_category = "Window".to_string();
        let window = state.filtered_catalogue(&catalogue);
        assert!(!window.is_empty());
        assert!(window.iter().all(|a| a.category.eq_ignore_ascii_case("Window")));
        assert!(window.iter().any(|a| a.id == "active_apps"));

        // Category "System"
        state.active_category = "System".to_string();
        let system = state.filtered_catalogue(&catalogue);
        assert!(!system.is_empty());
        assert!(system.iter().all(|a| a.category.eq_ignore_ascii_case("System")));
        assert!(system.iter().any(|a| a.id == "session"));
    }

    #[test]
    fn test_customizer_search_filter() {
        let catalogue = function_catalogue();
        let mut state = CustomizerState::default();

        // Query matching label
        state.search_query = "term".to_string();
        let results = state.filtered_catalogue(&catalogue);
        assert!(results.iter().any(|a| a.id == "terminal"));

        // Case insensitivity
        state.search_query = "TERMINAL".to_string();
        let res_caps = state.filtered_catalogue(&catalogue);
        assert!(res_caps.iter().any(|a| a.id == "terminal"));

        // Query matching description
        state.search_query = "calculator".to_string();
        let res_desc = state.filtered_catalogue(&catalogue);
        assert!(res_desc.iter().any(|a| a.id == "calc"));

        // Query trimming and empty query matches all
        state.search_query = "   ".to_string();
        let res_empty = state.filtered_catalogue(&catalogue);
        assert_eq!(res_empty.len(), catalogue.len());

        // Non-existent search query returns empty
        state.search_query = "non_existent_random_action_123".to_string();
        let res_none = state.filtered_catalogue(&catalogue);
        assert!(res_none.is_empty());

        // Combined category and search query
        state.active_category = "Apps".to_string();
        state.search_query = "browser".to_string();
        let res_combo = state.filtered_catalogue(&catalogue);
        assert!(res_combo.iter().all(|a| a.category.eq_ignore_ascii_case("Apps")));
        assert!(res_combo.iter().any(|a| a.id == "browser"));
    }

    #[test]
    fn test_render_customizer_to_pixmap() {
        let catalogue = function_catalogue();
        let mut state = CustomizerState::default();
        let active_ids: Vec<String> = Vec::new();

        // 1. When is_open is false, pixmap remains blank (0 non-zero pixels)
        let mut pixmap = Pixmap::new(800, 600).unwrap();
        render_customizer(&state, &catalogue, &active_ids, &mut pixmap, 800.0, 600.0, 400.0, 300.0);
        let non_zero_closed = pixmap.pixels().iter().filter(|p| p.alpha() > 0).count();
        assert_eq!(
            non_zero_closed, 0,
            "Pixmap should remain untouched when customizer is closed"
        );

        // 2. When is_open is true (SliceSwap mode), pixmap has pixels rendered
        state.is_open = true;
        render_customizer(&state, &catalogue, &active_ids, &mut pixmap, 800.0, 600.0, 400.0, 300.0);
        let non_zero_open = pixmap.pixels().iter().filter(|p| p.alpha() > 0).count();
        assert!(
            non_zero_open > 1000,
            "SliceSwap mode should render card, text, and icons, got {}",
            non_zero_open
        );

        // 3. Test FileTargetEdit mode rendering
        let mut pixmap_edit = Pixmap::new(800, 600).unwrap();
        state.mode = CustomizerMode::FileTargetEdit { target_index: 0 };
        state.input_label = "Downloads".to_string();
        state.input_path = "~/Downloads".to_string();
        state.input_icon = "download".to_string();
        render_customizer(&state, &catalogue, &active_ids, &mut pixmap_edit, 800.0, 600.0, 400.0, 300.0);
        let non_zero_edit = pixmap_edit.pixels().iter().filter(|p| p.alpha() > 0).count();
        assert!(
            non_zero_edit > 1000,
            "FileTargetEdit mode should render fields and buttons, got {}",
            non_zero_edit
        );

        // 4. Test FileTargetAdd mode rendering
        let mut pixmap_add = Pixmap::new(800, 600).unwrap();
        state.mode = CustomizerMode::FileTargetAdd;
        state.input_label = "New Folder".to_string();
        state.input_path = "~/NewFolder".to_string();
        state.input_icon = "folder".to_string();
        render_customizer(&state, &catalogue, &active_ids, &mut pixmap_add, 800.0, 600.0, 400.0, 300.0);
        let non_zero_add = pixmap_add.pixels().iter().filter(|p| p.alpha() > 0).count();
        assert!(
            non_zero_add > 1000,
            "FileTargetAdd mode should render fields and buttons, got {}",
            non_zero_add
        );
    }

    #[test]
    fn test_customizer_state_transitions() {
        let mut state = CustomizerState::default();
        assert!(!state.is_open);

        state.open_slice_swap(3);
        assert!(state.is_open);
        assert_eq!(state.mode, CustomizerMode::SliceSwap { slot_index: 3 });

        let target = FileJumpTarget {
            id: "docs".to_string(),
            label: "Documents".to_string(),
            path: "~/Documents".to_string(),
            icon: "description".to_string(),
        };
        state.open_file_edit_target(1, &target);
        assert!(state.is_open);
        assert_eq!(state.mode, CustomizerMode::FileTargetEdit { target_index: 1 });
        assert_eq!(state.input_label, "Documents");
        assert_eq!(state.input_path, "~/Documents");
        assert_eq!(state.input_icon, "description");

        state.open_file_add();
        assert!(state.is_open);
        assert_eq!(state.mode, CustomizerMode::FileTargetAdd);
        assert!(state.input_label.is_empty());
        assert!(state.input_path.is_empty());
        assert_eq!(state.input_icon, "folder");

        state.close();
        assert!(!state.is_open);
    }

    #[test]
    fn test_rounded_rect_path_edge_cases() {
        assert!(rounded_rect_path(0.0, 0.0, 0.0, 100.0, 10.0).is_none());
        assert!(rounded_rect_path(0.0, 0.0, 100.0, 0.0, 10.0).is_none());
        assert!(rounded_rect_path(0.0, 0.0, 100.0, 100.0, 0.0).is_some());
        assert!(rounded_rect_path(0.0, 0.0, 100.0, 100.0, 200.0).is_some());
    }
}
