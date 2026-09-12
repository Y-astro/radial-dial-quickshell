// src/renderer/customizer.rs
#![allow(dead_code, clippy::too_many_arguments)]

use crate::font::FontRenderer;
use crate::renderer::text::{draw_icon, draw_text, draw_text_left};
use crate::state::actions::ActionDef;
use crate::state::config::FileJumpTarget;
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

pub const CARD_W: f32 = 480.0;
pub const CARD_H: f32 = 560.0;
pub const CARD_RADIUS: f32 = 22.0;

pub const CATEGORIES: &[&str] = &[
    "All", "Apps", "Tools", "Media", "Window", "System",
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
    Color::from_rgba8(0, 0, 0, 89) // 35% black
}

#[inline]
pub fn col_input_border() -> Color {
    Color::from_rgba8(255, 255, 255, 36) // 14% white
}

#[inline]
pub fn col_danger() -> Color {
    Color::from_rgba8(255, 107, 107, 255) // #ff6b6b
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
    pub active_category: String, // "All", "Apps", "Tools", "Media", "Window", "System"
    pub search_query: String,
    pub scroll_offset: f32,
    pub selected_item: Option<String>,
    // Input field buffers for file target editing:
    pub input_label: String,
    pub input_path: String,
    pub input_icon: String,
    pub focused_field: usize, // 0=label, 1=path, 2=icon
}

impl Default for CustomizerState {
    fn default() -> Self {
        Self {
            is_open: false,
            mode: CustomizerMode::SliceSwap { slot_index: 0 },
            active_category: "All".to_string(),
            search_query: String::new(),
            scroll_offset: 0.0,
            selected_item: None,
            input_label: String::new(),
            input_path: String::new(),
            input_icon: "folder".to_string(),
            focused_field: 0,
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
        self.scroll_offset = 0.0;
        self.selected_item = None;
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

/// Render the customizer modal: 480x560 px rounded card centered on screen.
pub fn render_customizer(
    state: &CustomizerState,
    catalogue: &[ActionDef],
    pixmap: &mut Pixmap,
    screen_w: f32,
    screen_h: f32,
) {
    if !state.is_open || screen_w <= 0.0 || screen_h <= 0.0 {
        return;
    }
    CUSTOMIZER_FONT_RENDERER.with(|fr| {
        render_customizer_with_font(state, catalogue, pixmap, screen_w, screen_h, &mut fr.borrow_mut());
    });
}

/// Render the customizer modal using an explicitly supplied `FontRenderer`.
pub fn render_customizer_with_font(
    state: &CustomizerState,
    catalogue: &[ActionDef],
    pixmap: &mut Pixmap,
    screen_w: f32,
    screen_h: f32,
    font_renderer: &mut FontRenderer,
) {
    if !state.is_open || screen_w <= 0.0 || screen_h <= 0.0 {
        return;
    }

    // 1. Dim Backdrop
    let mut backdrop_paint = Paint::default();
    backdrop_paint.set_color(Color::from_rgba8(0, 0, 0, 115)); // ~45% dim
    if let Some(rect) = tiny_skia::Rect::from_xywh(0.0, 0.0, screen_w, screen_h) {
        pixmap.fill_rect(rect, &backdrop_paint, Transform::identity(), None);
    }

    // 2. Card Dimensions & Placement (480x560 px centered)
    let card_w = CARD_W.min(screen_w);
    let card_h = CARD_H.min(screen_h);
    let card_x = ((screen_w - card_w) / 2.0).max(0.0);
    let card_y = ((screen_h - card_h) / 2.0).max(0.0);

    // Subtle drop shadow outline
    stroke_rounded_rect(
        pixmap,
        card_x - 1.0,
        card_y - 1.0,
        card_w + 2.0,
        card_h + 2.0,
        CARD_RADIUS + 1.0,
        Color::from_rgba8(0, 0, 0, 70),
        3.0,
    );

    // Card background fill
    fill_rounded_rect(pixmap, card_x, card_y, card_w, card_h, CARD_RADIUS, col_card_bg());

    // Card border
    stroke_rounded_rect(
        pixmap,
        card_x,
        card_y,
        card_w,
        card_h,
        CARD_RADIUS,
        col_card_border(),
        1.5,
    );

    // Top frosted highlight reflection
    let mut highlight_pb = PathBuilder::new();
    highlight_pb.move_to(card_x + CARD_RADIUS, card_y + 1.5);
    highlight_pb.line_to(card_x + card_w - CARD_RADIUS, card_y + 1.5);
    if let Some(p) = highlight_pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(255, 255, 255, 46));
        let stroke = Stroke {
            width: 1.0,
            ..Default::default()
        };
        pixmap.stroke_path(&p, &paint, &stroke, Transform::identity(), None);
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
        Color::from_rgba8(168, 197, 253, 46),
    );
    stroke_rounded_rect(
        pixmap,
        icon_box_x,
        icon_box_y,
        icon_box_size,
        icon_box_size,
        12.0,
        col_primary(),
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
        col_primary(),
    );

    let title_x = icon_box_x + icon_box_size + 12.0;
    draw_text_left(
        font_renderer,
        pixmap,
        mode_title,
        title_x,
        icon_box_y + 11.0,
        15.0,
        col_on_surface(),
    );
    draw_text_left(
        font_renderer,
        pixmap,
        &mode_subtitle,
        title_x,
        icon_box_y + 27.0,
        11.0,
        col_subtext(),
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
        col_on_surface(),
    );

    // 4. Body Content per Mode
    match &state.mode {
        CustomizerMode::SliceSwap { .. } => {
            render_slice_swap_body(
                state,
                catalogue,
                content_x,
                card_y,
                content_w,
                card_h,
                pixmap,
                font_renderer,
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
            );
        }
    }
}

/// Render SliceSwap mode body:
/// Category tabs, search bar, scrollable catalogue action list, and Reset to Defaults footer.
fn render_slice_swap_body(
    state: &CustomizerState,
    catalogue: &[ActionDef],
    content_x: f32,
    card_y: f32,
    content_w: f32,
    _card_h: f32,
    pixmap: &mut Pixmap,
    font_renderer: &mut FontRenderer,
) {
    // 1. Category Tabs (All, Apps, Tools, Media, Window, System)
    let tabs_y = card_y + 64.0;
    let tab_h = 28.0;
    let num_tabs = CATEGORIES.len() as f32;
    let tab_gap = 6.0;
    let tab_w = (content_w - (num_tabs - 1.0) * tab_gap) / num_tabs;

    for (i, cat) in CATEGORIES.iter().enumerate() {
        let tx = content_x + i as f32 * (tab_w + tab_gap);
        let is_active = state.active_category.eq_ignore_ascii_case(cat);

        if is_active {
            fill_rounded_rect(pixmap, tx, tabs_y, tab_w, tab_h, 14.0, col_primary());
            draw_text(
                font_renderer,
                pixmap,
                cat,
                tx + tab_w / 2.0,
                tabs_y + tab_h / 2.0,
                11.0,
                col_on_primary(),
            );
        } else {
            fill_rounded_rect(
                pixmap,
                tx,
                tabs_y,
                tab_w,
                tab_h,
                14.0,
                Color::from_rgba8(255, 255, 255, 13),
            );
            stroke_rounded_rect(
                pixmap,
                tx,
                tabs_y,
                tab_w,
                tab_h,
                14.0,
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
                col_on_surface(),
            );
        }
    }

    // 2. Search Bar
    let search_y = card_y + 100.0;
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
    stroke_rounded_rect(
        pixmap,
        content_x,
        search_y,
        content_w,
        search_h,
        10.0,
        col_input_border(),
        1.0,
    );

    // Search Icon
    draw_icon(
        font_renderer,
        pixmap,
        "search",
        content_x + 18.0,
        search_y + search_h / 2.0,
        16.0,
        Color::from_rgba8(255, 255, 255, 102),
    );

    let query_x = content_x + 36.0;
    if state.search_query.is_empty() {
        draw_text_left(
            font_renderer,
            pixmap,
            "Search function catalogue...",
            query_x,
            search_y + search_h / 2.0,
            12.0,
            Color::from_rgba8(255, 255, 255, 89),
        );
    } else {
        draw_text_left(
            font_renderer,
            pixmap,
            &state.search_query,
            query_x,
            search_y + search_h / 2.0,
            12.0,
            col_on_surface(),
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
    let list_y = card_y + 142.0;
    let list_h = 368.0;
    let items = state.filtered_catalogue(catalogue);

    if items.is_empty() {
        draw_text(
            font_renderer,
            pixmap,
            "No matching functions found",
            content_x + content_w / 2.0,
            list_y + list_h / 2.0,
            13.0,
            col_subtext(),
        );
    } else {
        let item_h = 50.0;
        let item_gap = 6.0;
        let step = item_h + item_gap;
        let start_y = list_y - state.scroll_offset;

        for (idx, item) in items.iter().enumerate() {
            let cur_y = start_y + idx as f32 * step;

            // Viewport bounds check: skip items completely outside list bounds
            if cur_y + item_h < list_y || cur_y > list_y + list_h {
                continue;
            }

            let is_selected = state.selected_item.as_deref() == Some(item.id);

            // Item card background & border
            if is_selected {
                fill_rounded_rect(
                    pixmap,
                    content_x,
                    cur_y,
                    content_w,
                    item_h,
                    12.0,
                    Color::from_rgba8(168, 197, 253, 51),
                );
                stroke_rounded_rect(
                    pixmap,
                    content_x,
                    cur_y,
                    content_w,
                    item_h,
                    12.0,
                    col_primary(),
                    1.5,
                );
            } else {
                fill_rounded_rect(
                    pixmap,
                    content_x,
                    cur_y,
                    content_w,
                    item_h,
                    12.0,
                    Color::from_rgba8(0, 0, 0, 56),
                );
                stroke_rounded_rect(
                    pixmap,
                    content_x,
                    cur_y,
                    content_w,
                    item_h,
                    12.0,
                    Color::from_rgba8(255, 255, 255, 20),
                    1.0,
                );
            }

            // Left icon box
            let ib_x = content_x + 8.0;
            let ib_y = cur_y + 8.0;
            let ib_size = 34.0;
            let ib_bg = if is_selected {
                Color::from_rgba8(168, 197, 253, 46)
            } else {
                Color::from_rgba8(255, 255, 255, 15)
            };
            fill_rounded_rect(pixmap, ib_x, ib_y, ib_size, ib_size, 10.0, ib_bg);

            let icon_col = if is_selected {
                col_primary()
            } else {
                col_on_surface()
            };
            draw_icon(
                font_renderer,
                pixmap,
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
                pixmap,
                item.label,
                text_x,
                cur_y + 16.0,
                13.0,
                col_on_surface(),
            );

            let desc = if item.desc.is_empty() {
                item.category
            } else {
                item.desc
            };
            draw_text_left(
                font_renderer,
                pixmap,
                desc,
                text_x,
                cur_y + 33.0,
                10.0,
                col_subtext(),
            );

            // Right action button (+ Add or - Remove)
            let btn_w = 64.0;
            let btn_h = 26.0;
            let btn_x = content_x + content_w - btn_w - 10.0;
            let btn_y = cur_y + 12.0;

            if is_selected {
                fill_rounded_rect(
                    pixmap,
                    btn_x,
                    btn_y,
                    btn_w,
                    btn_h,
                    13.0,
                    Color::from_rgba8(255, 107, 107, 51),
                );
                stroke_rounded_rect(
                    pixmap,
                    btn_x,
                    btn_y,
                    btn_w,
                    btn_h,
                    13.0,
                    col_danger(),
                    1.0,
                );
                draw_text(
                    font_renderer,
                    pixmap,
                    "Remove",
                    btn_x + btn_w / 2.0,
                    btn_y + btn_h / 2.0,
                    10.0,
                    col_danger(),
                );
            } else {
                fill_rounded_rect(pixmap, btn_x, btn_y, btn_w, btn_h, 13.0, col_primary());
                draw_text(
                    font_renderer,
                    pixmap,
                    "+ Add",
                    btn_x + btn_w / 2.0,
                    btn_y + btn_h / 2.0,
                    10.0,
                    col_on_primary(),
                );
            }
        }
    }

    // 4. Footer: Reset to Defaults & count
    let footer_y = card_y + 518.0;
    let divider_y = footer_y - 4.0;

    let mut div_pb = PathBuilder::new();
    div_pb.move_to(content_x, divider_y);
    div_pb.line_to(content_x + content_w, divider_y);
    if let Some(path) = div_pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(255, 255, 255, 25));
        let stroke = Stroke {
            width: 1.0,
            ..Default::default()
        };
        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }

    // Reset to Defaults button
    let reset_btn_w = 145.0;
    let reset_btn_h = 28.0;
    fill_rounded_rect(
        pixmap,
        content_x,
        footer_y,
        reset_btn_w,
        reset_btn_h,
        8.0,
        Color::from_rgba8(255, 255, 255, 15),
    );
    stroke_rounded_rect(
        pixmap,
        content_x,
        footer_y,
        reset_btn_w,
        reset_btn_h,
        8.0,
        Color::from_rgba8(255, 255, 255, 30),
        1.0,
    );
    draw_icon(
        font_renderer,
        pixmap,
        "refresh",
        content_x + 16.0,
        footer_y + reset_btn_h / 2.0,
        14.0,
        col_subtext(),
    );
    draw_text_left(
        font_renderer,
        pixmap,
        "Reset to Defaults",
        content_x + 28.0,
        footer_y + reset_btn_h / 2.0,
        11.0,
        col_on_surface(),
    );

    // Right side count indicator
    let count_str = format!("{} functions", items.len());
    draw_text_left(
        font_renderer,
        pixmap,
        &count_str,
        content_x + content_w - 75.0,
        footer_y + reset_btn_h / 2.0,
        11.0,
        col_subtext(),
    );
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
        col_subtext(),
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
        (col_primary(), 1.5)
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
            col_on_surface(),
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
        col_subtext(),
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
        (col_primary(), 1.5)
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
            col_on_surface(),
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
        Color::from_rgba8(255, 255, 255, 20),
    );
    stroke_rounded_rect(
        pixmap,
        browse_x,
        path_box_y,
        browse_w,
        path_box_h,
        10.0,
        Color::from_rgba8(255, 255, 255, 46),
        1.0,
    );
    draw_icon(
        font_renderer,
        pixmap,
        "folder_open",
        browse_x + 22.0,
        path_box_y + path_box_h / 2.0,
        16.0,
        col_primary(),
    );
    draw_text_left(
        font_renderer,
        pixmap,
        "Browse",
        browse_x + 38.0,
        path_box_y + path_box_h / 2.0,
        11.0,
        col_on_surface(),
    );

    // 3. Choose Icon Picker Grid (2 rows of 8 icons)
    let icon_hdr_y = card_y + 208.0;
    draw_text_left(
        font_renderer,
        pixmap,
        "Choose Icon",
        content_x,
        icon_hdr_y,
        11.0,
        col_subtext(),
    );

    let grid_y = card_y + 226.0;
    let cols = 8;
    let gap_x = 8.0;
    let gap_y = 8.0;
    let icon_w = (content_w - (cols as f32 - 1.0) * gap_x) / cols as f32;
    let icon_h = 38.0;

    for (idx, icon_name) in AVAILABLE_ICONS.iter().enumerate() {
        let row = idx / cols;
        let col = idx % cols;
        let ix = content_x + col as f32 * (icon_w + gap_x);
        let iy = grid_y + row as f32 * (icon_h + gap_y);
        let is_selected = state.input_icon == *icon_name;

        if is_selected {
            fill_rounded_rect(pixmap, ix, iy, icon_w, icon_h, 10.0, col_primary());
            draw_icon(
                font_renderer,
                pixmap,
                icon_name,
                ix + icon_w / 2.0,
                iy + icon_h / 2.0,
                20.0,
                col_on_primary(),
            );
        } else {
            fill_rounded_rect(
                pixmap,
                ix,
                iy,
                icon_w,
                icon_h,
                10.0,
                Color::from_rgba8(255, 255, 255, 13),
            );
            stroke_rounded_rect(
                pixmap,
                ix,
                iy,
                icon_w,
                icon_h,
                10.0,
                Color::from_rgba8(255, 255, 255, 30),
                1.0,
            );
            draw_icon(
                font_renderer,
                pixmap,
                icon_name,
                ix + icon_w / 2.0,
                iy + icon_h / 2.0,
                20.0,
                col_on_surface(),
            );
        }
    }

    // 4. Action Buttons Row (Delete / Cancel / Save)
    let btns_y = card_y + 505.0;
    let btns_h = 38.0;

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
    let save_w = 110.0;
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
        Color::from_rgba8(255, 255, 255, 20),
    );
    stroke_rounded_rect(
        pixmap,
        cancel_x,
        btns_y,
        cancel_w,
        btns_h,
        10.0,
        Color::from_rgba8(255, 255, 255, 40),
        1.0,
    );
    draw_text(
        font_renderer,
        pixmap,
        "Cancel",
        cancel_x + cancel_w / 2.0,
        btns_y + btns_h / 2.0,
        12.0,
        col_on_surface(),
    );

    // Save / Add Target button
    fill_rounded_rect(pixmap, save_x, btns_y, save_w, btns_h, 10.0, col_primary());
    draw_icon(
        font_renderer,
        pixmap,
        "check",
        save_x + 22.0,
        btns_y + btns_h / 2.0,
        16.0,
        col_on_primary(),
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
        save_x + 36.0,
        btns_y + btns_h / 2.0,
        12.0,
        col_on_primary(),
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

        // 1. When is_open is false, pixmap remains blank (0 non-zero pixels)
        let mut pixmap = Pixmap::new(800, 600).unwrap();
        render_customizer(&state, &catalogue, &mut pixmap, 800.0, 600.0);
        let non_zero_closed = pixmap.pixels().iter().filter(|p| p.alpha() > 0).count();
        assert_eq!(
            non_zero_closed, 0,
            "Pixmap should remain untouched when customizer is closed"
        );

        // 2. When is_open is true (SliceSwap mode), pixmap has pixels rendered
        state.is_open = true;
        render_customizer(&state, &catalogue, &mut pixmap, 800.0, 600.0);
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
        render_customizer(&state, &catalogue, &mut pixmap_edit, 800.0, 600.0);
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
        render_customizer(&state, &catalogue, &mut pixmap_add, 800.0, 600.0);
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
