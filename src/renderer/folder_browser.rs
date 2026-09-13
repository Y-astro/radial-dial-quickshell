//! In-dial directory picker modal dialog renderer.
//!
//! Provides:
//! - State management for the folder navigation modal (`FolderBrowserState`)
//! - Modal dialog 2D rasterization onto `tiny_skia::Pixmap` (`render_folder_browser`)
//!   including backdrop scrim, breadcrumb bar, places sidebar/chips, folder list,
//!   search filter, and action buttons.

use crate::font::FontRenderer;
use crate::ipc::folder::{DirListing, FolderEntry, PlaceEntry};
use crate::renderer::customizer::CustomizerColors;
use crate::renderer::text::{draw_icon, draw_text, draw_text_left};
use tiny_skia::{
    Color, FillRule, GradientStop, LinearGradient, Paint, PathBuilder, Pixmap, PixmapPaint, Point,
    SpreadMode, Stroke, Transform,
};

#[derive(Debug, Clone, Default)]
pub struct FolderBrowserState {
    pub is_open: bool,
    pub current_path: String,
    pub listing: Option<DirListing>,
    pub places: Vec<PlaceEntry>,
    pub scroll_offset: f32,
    pub selected_index: i32,
    pub search_query: String,
    pub search_focused: bool,
    pub anim_elapsed: f32,
    pub anim_progress: f32,
    /// Phase 4: modal close animation
    pub is_closing: bool,
    pub close_elapsed: f32,
}

impl FolderBrowserState {
    /// Creates a default closed `FolderBrowserState`.
    pub fn new() -> Self {
        Self {
            is_open: false,
            current_path: String::new(),
            listing: None,
            places: crate::ipc::folder::get_places_sync(),
            scroll_offset: 0.0,
            selected_index: -1,
            search_query: String::new(),
            search_focused: false,
            anim_elapsed: 0.0,
            anim_progress: 0.0,
            is_closing: false,
            close_elapsed: 0.0,
        }
    }

    /// Opens the folder browser targeted at `initial_path`.
    pub fn open(&mut self, initial_path: &str) {
        self.is_open = true;
        self.is_closing = false;
        self.close_elapsed = 0.0;
        self.current_path = initial_path.to_string();
        self.search_query.clear();
        self.search_focused = false;
        self.scroll_offset = 0.0;
        self.selected_index = -1;
        self.anim_elapsed = 0.0;
        self.anim_progress = 0.0;
        if self.places.is_empty() {
            self.places = crate::ipc::folder::get_places_sync();
        }
    }

    /// Begins the modal close animation. Caller must poll `is_closing` and remove when done.
    pub fn begin_close(&mut self) {
        if !self.is_closing {
            self.is_closing = true;
            self.close_elapsed = 0.0;
        }
    }

    /// Closes the modal immediately (no animation).
    pub fn close(&mut self) {
        self.is_open = false;
        self.is_closing = false;
    }

    /// Updates the current directory listing.
    pub fn set_listing(&mut self, listing: DirListing) {
        self.current_path = listing.path.clone();
        self.listing = Some(listing);
        self.scroll_offset = 0.0;
        self.selected_index = if self.filtered_folders().is_empty() { -1 } else { 0 };
    }

    /// Updates the list of system places and external storage devices.
    pub fn set_places(&mut self, places: Vec<PlaceEntry>) {
        self.places = places;
    }

    /// Sets the search filter query and resets selection.
    pub fn set_search_query(&mut self, query: &str) {
        self.search_query = query.to_string();
        self.scroll_offset = 0.0;
        self.selected_index = if self.filtered_folders().is_empty() { -1 } else { 0 };
    }

    /// Returns the filtered subfolders matching `search_query`.
    pub fn filtered_folders(&self) -> Vec<&FolderEntry> {
        let Some(listing) = &self.listing else {
            return Vec::new();
        };
        let query = self.search_query.trim().to_lowercase();
        if query.is_empty() {
            listing.folders.iter().collect()
        } else {
            listing
                .folders
                .iter()
                .filter(|f| f.name.to_lowercase().contains(&query))
                .collect()
        }
    }

    /// Advances the selected folder index forward.
    pub fn select_next(&mut self) {
        let count = self.filtered_folders().len();
        if count > 0 {
            if self.selected_index < 0 {
                self.selected_index = 0;
            } else if (self.selected_index as usize) + 1 < count {
                self.selected_index += 1;
            }
        }
    }

    /// Decrements the selected folder index backward.
    pub fn select_prev(&mut self) {
        let count = self.filtered_folders().len();
        if count > 0 {
            if self.selected_index > 0 {
                self.selected_index -= 1;
            }
        }
    }

    /// Returns a reference to the currently selected `FolderEntry`, if any.
    pub fn selected_folder(&self) -> Option<&FolderEntry> {
        if self.selected_index < 0 {
            return None;
        }
        let filtered = self.filtered_folders();
        filtered.get(self.selected_index as usize).copied()
    }

    /// Returns the active directory's display name.
    pub fn current_folder_name(&self) -> &str {
        self.listing
            .as_ref()
            .map(|l| l.name.as_str())
            .unwrap_or("")
    }

    /// Adjusts the scroll offset within bounds.
    pub fn scroll(&mut self, delta_y: f32, max_scroll: f32) {
        self.scroll_offset = (self.scroll_offset - delta_y).clamp(0.0, max_scroll.max(0.0));
    }
}

/// Appends a rounded rectangle subpath to `PathBuilder`.
pub fn add_rounded_rect(pb: &mut PathBuilder, x: f32, y: f32, w: f32, h: f32, r: f32) {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    if r <= 0.001 {
        pb.move_to(x, y);
        pb.line_to(x + w, y);
        pb.line_to(x + w, y + h);
        pb.line_to(x, y + h);
        pb.close();
        return;
    }
    let k = 0.55228475 * r;
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
}

/// Helper to render a rounded rectangle with fill and optional stroke.
pub fn draw_rounded_rect(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    fill_col: Color,
    stroke: Option<(Color, f32)>,
) {
    let mut pb = PathBuilder::new();
    add_rounded_rect(&mut pb, x, y, w, h, r);
    if let Some(path) = pb.finish() {
        if fill_col.alpha() > 0.001 {
            let mut paint = Paint::default();
            paint.set_color(fill_col);
            paint.anti_alias = true;
            pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
        }
        if let Some((stroke_col, stroke_w)) = stroke {
            if stroke_col.alpha() > 0.001 && stroke_w > 0.001 {
                let mut paint = Paint::default();
                paint.set_color(stroke_col);
                paint.anti_alias = true;
                let s = Stroke {
                    width: stroke_w,
                    ..Default::default()
                };
                pixmap.stroke_path(&path, &paint, &s, Transform::identity(), None);
            }
        }
    }
}

/// Render the folder browser modal dialog (breadcrumbs, places sidebar, folder grid, select button).
pub fn render_folder_browser(
    state: &FolderBrowserState,
    pixmap: &mut tiny_skia::Pixmap,
    screen_w: f32,
    screen_h: f32,
    anchor_x: f32,
    anchor_y: f32,
    colors: CustomizerColors,
) {
    let mut font_renderer = FontRenderer::new();
    render_folder_browser_with_font(state, pixmap, screen_w, screen_h, anchor_x, anchor_y, &mut font_renderer, colors);
}

/// Render the folder browser modal dialog reusing an existing `FontRenderer`.
pub fn render_folder_browser_with_font(
    state: &FolderBrowserState,
    pixmap: &mut tiny_skia::Pixmap,
    screen_w: f32,
    screen_h: f32,
    anchor_x: f32,
    anchor_y: f32,
    font_renderer: &mut FontRenderer,
    colors: CustomizerColors,
) {
    if !state.is_open {
        return;
    }

    // Material theme palette matching radial-dial and system colors
    let col_primary = colors.primary;
    let col_on_primary = colors.on_primary;
    let col_on_surface = colors.on_surface;
    let col_subtext = colors.subtext;
    let surface = colors.surface_base;

    // Animation progress (0.01 to 1.0)
    let anim_t = state.anim_progress.clamp(0.01, 1.0);
    let scale = 0.92 + 0.08 * anim_t;

    // 2. Main Modal Card (anchor-relative positioning)
    let base_card_w = 480.0_f32.min(screen_w - 20.0);
    let base_card_h = 540.0_f32.min(screen_h - 20.0);
    let preferred_x = anchor_x - base_card_w / 2.0 + (screen_w / 2.0 - anchor_x).signum() * 60.0;
    let preferred_y = anchor_y - base_card_h / 2.0 + (screen_h / 2.0 - anchor_y).signum() * 60.0;
    let base_card_x = preferred_x.clamp(10.0, screen_w - base_card_w - 10.0);
    let base_card_y = preferred_y.clamp(10.0, screen_h - base_card_h - 10.0);

    let card_w = base_card_w * scale;
    let card_h = base_card_h * scale;
    let card_x = base_card_x + (base_card_w - card_w) / 2.0;
    let card_y = base_card_y + (base_card_h - card_h) / 2.0;
    let card_radius = 22.0;

    // Translucent Frosted Glass Base Fill (vertical gradient matching customizer & radial menu blur)
    let mut card_pb = PathBuilder::new();
    add_rounded_rect(&mut card_pb, card_x, card_y, card_w, card_h, card_radius);
    if let Some(card_path) = card_pb.finish() {
        // Subtle drop shadow outline
        draw_rounded_rect(
            pixmap,
            card_x - 1.0,
            card_y - 1.0,
            card_w + 2.0,
            card_h + 2.0,
            card_radius + 1.0,
            Color::TRANSPARENT,
            Some((Color::from_rgba8(0, 0, 0, 60), 2.0)),
        );

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
        draw_rounded_rect(
            pixmap,
            card_x,
            card_y,
            card_w,
            card_h,
            card_radius,
            Color::TRANSPARENT,
            Some((Color::from_rgba(1.0, 1.0, 1.0, 0.18).unwrap_or(Color::WHITE), 1.5)),
        );

        // Top frosted highlight reflection
        let mut highlight_pb = PathBuilder::new();
        highlight_pb.move_to(card_x + card_radius, card_y + 1.5);
        highlight_pb.line_to(card_x + card_w - card_radius, card_y + 1.5);
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

    let margin = 18.0;
    let content_w = card_w - margin * 2.0;

    // 3. Header Bar
    // Up / Parent button
    let back_btn_x = card_x + margin;
    let back_btn_y = card_y + 16.0;
    let back_btn_size = 36.0;
    let back_bg = Color::from_rgba(col_primary.red(), col_primary.green(), col_primary.blue(), 0.14)
        .unwrap_or(col_primary);
    let back_border = Color::from_rgba(col_primary.red(), col_primary.green(), col_primary.blue(), 0.35)
        .unwrap_or(col_primary);
    draw_rounded_rect(
        pixmap,
        back_btn_x,
        back_btn_y,
        back_btn_size,
        back_btn_size,
        18.0,
        back_bg,
        Some((back_border, 1.0)),
    );
    let parent_path = state.listing.as_ref().map(|l| l.parent.as_str()).unwrap_or("");
    let back_icon = if !parent_path.is_empty() && parent_path != state.current_path {
        "arrow_back"
    } else {
        "folder_open"
    };
    draw_icon(
        font_renderer,
        pixmap,
        back_icon,
        back_btn_x + 18.0,
        back_btn_y + 18.0,
        18.0,
        col_primary,
    );

    // Title & Subtitle
    let title_x = back_btn_x + back_btn_size + 12.0;
    draw_text_left(
        font_renderer,
        pixmap,
        "Select Folder",
        title_x,
        back_btn_y + 10.0,
        15.0,
        col_on_surface,
    );
    let folder_subtitle = if !state.current_folder_name().is_empty() {
        state.current_folder_name()
    } else {
        "Browse directories"
    };
    draw_text_left(
        font_renderer,
        pixmap,
        folder_subtitle,
        title_x,
        back_btn_y + 26.0,
        11.0,
        col_subtext,
    );

    // Close button
    let close_size = 32.0;
    let close_x = card_x + card_w - margin - close_size;
    let close_y = card_y + 18.0;
    let close_bg = Color::from_rgba(1.0, 1.0, 1.0, 0.08).unwrap_or(Color::BLACK);
    draw_rounded_rect(
        pixmap,
        close_x,
        close_y,
        close_size,
        close_size,
        16.0,
        close_bg,
        Some((Color::from_rgba(1.0, 1.0, 1.0, 0.14).unwrap_or(Color::WHITE), 1.0)),
    );
    draw_icon(
        font_renderer,
        pixmap,
        "close",
        close_x + 16.0,
        close_y + 16.0,
        16.0,
        col_on_surface,
    );

    // 4. Breadcrumb / Path Bar
    let path_bar_y = card_y + 60.0;
    let path_bar_h = 36.0;
    let path_box_bg = Color::from_rgba8(0, 0, 0, 56);
    let path_box_border = Color::from_rgba(1.0, 1.0, 1.0, 0.12).unwrap_or(Color::WHITE);
    draw_rounded_rect(
        pixmap,
        card_x + margin,
        path_bar_y,
        content_w,
        path_bar_h,
        10.0,
        path_box_bg,
        Some((path_box_border, 1.0)),
    );

    // Arrow Upward Icon
    let up_icon_color = if !parent_path.is_empty() {
        col_primary
    } else {
        Color::from_rgba(col_on_surface.red(), col_on_surface.green(), col_on_surface.blue(), 0.3)
            .unwrap_or(col_on_surface)
    };
    draw_icon(
        font_renderer,
        pixmap,
        "arrow_upward",
        card_x + margin + 18.0,
        path_bar_y + 18.0,
        16.0,
        up_icon_color,
    );

    // Path String display
    let path_text = if state.current_path.is_empty() {
        "~"
    } else {
        &state.current_path
    };
    draw_text_left(
        font_renderer,
        pixmap,
        path_text,
        card_x + margin + 38.0,
        path_bar_y + 18.0,
        11.0,
        col_on_surface,
    );

    // Refresh Icon
    draw_icon(
        font_renderer,
        pixmap,
        "refresh",
        card_x + card_w - margin - 18.0,
        path_bar_y + 18.0,
        16.0,
        col_subtext,
    );

    // 5. Places Quick Navigation Bar (Horizontal chips)
    let places_y = card_y + 104.0;
    let mut chip_x = card_x + margin;
    let chip_h = 28.0;

    for place in &state.places {
        let chip_w = (place.name.len() as f32 * 6.5 + 28.0).clamp(58.0, 92.0);
        if chip_x + chip_w > card_x + card_w - margin {
            break;
        }
        let is_selected = state.current_path == place.path;
        let (chip_fill, chip_stroke, icon_color, text_color) = if is_selected {
            (
                col_primary,
                None,
                col_on_primary,
                col_on_primary,
            )
        } else {
            (
                Color::from_rgba(1.0, 1.0, 1.0, 0.08).unwrap_or(Color::BLACK),
                Some((Color::from_rgba(1.0, 1.0, 1.0, 0.14).unwrap_or(Color::WHITE), 1.0)),
                col_primary,
                col_on_surface,
            )
        };

        draw_rounded_rect(
            pixmap,
            chip_x,
            places_y,
            chip_w,
            chip_h,
            14.0,
            chip_fill,
            chip_stroke,
        );
        draw_icon(
            font_renderer,
            pixmap,
            &place.icon,
            chip_x + 13.0,
            places_y + 14.0,
            14.0,
            icon_color,
        );
        draw_text_left(
            font_renderer,
            pixmap,
            &place.name,
            chip_x + 23.0,
            places_y + 14.0,
            10.5,
            text_color,
        );

        chip_x += chip_w + 6.0;
    }

    // 6. Search Filter Input Bar
    let filter_y = card_y + 140.0;
    let filter_h = 32.0;
    let filter_box_bg = Color::from_rgba8(0, 0, 0, 56);
    let filter_border = Color::from_rgba(1.0, 1.0, 1.0, 0.12).unwrap_or(Color::WHITE);
    draw_rounded_rect(
        pixmap,
        card_x + margin,
        filter_y,
        content_w,
        filter_h,
        10.0,
        filter_box_bg,
        Some((filter_border, 1.0)),
    );

    draw_icon(
        font_renderer,
        pixmap,
        "search",
        card_x + margin + 16.0,
        filter_y + 16.0,
        15.0,
        col_primary,
    );

    if state.search_query.is_empty() {
        let placeholder_col = Color::from_rgba(col_on_surface.red(), col_on_surface.green(), col_on_surface.blue(), 0.35)
            .unwrap_or(col_subtext);
        draw_text_left(
            font_renderer,
            pixmap,
            "Filter subfolders...",
            card_x + margin + 34.0,
            filter_y + 16.0,
            12.0,
            placeholder_col,
        );
    } else {
        draw_text_left(
            font_renderer,
            pixmap,
            &state.search_query,
            card_x + margin + 34.0,
            filter_y + 16.0,
            12.0,
            col_on_surface,
        );

        // Clear query button
        draw_icon(
            font_renderer,
            pixmap,
            "close",
            card_x + card_w - margin - 16.0,
            filter_y + 16.0,
            12.0,
            col_on_surface,
        );
    }

    // 7. Folders List / Grid Area
    let list_y = card_y + 180.0;
    let bottom_bar_h = 64.0;
    let list_h = card_h - (list_y - card_y) - bottom_bar_h;
    let list_bg = Color::from_rgba8(0, 0, 0, 51);
    let list_border = Color::from_rgba(1.0, 1.0, 1.0, 0.10).unwrap_or(Color::WHITE);
    draw_rounded_rect(
        pixmap,
        card_x + margin,
        list_y,
        content_w,
        list_h,
        12.0,
        list_bg,
        Some((list_border, 1.0)),
    );

    let folders = state.filtered_folders();
    let vp_w = content_w.round() as u32;
    let vp_h = list_h.round() as u32;

    if vp_w > 0 && vp_h > 0 {
        if let Some(mut list_pixmap) = Pixmap::new(vp_w, vp_h) {
            if folders.is_empty() {
                // Empty state indicator
                draw_icon(
                    font_renderer,
                    &mut list_pixmap,
                    "folder_off",
                    content_w / 2.0,
                    list_h / 2.0 - 12.0,
                    32.0,
                    col_subtext,
                );
                let empty_msg = if state.search_query.is_empty() {
                    "No subfolders in this directory"
                } else {
                    "No matching folders"
                };
                draw_text(
                    font_renderer,
                    &mut list_pixmap,
                    empty_msg,
                    content_w / 2.0,
                    list_h / 2.0 + 16.0,
                    12.0,
                    col_subtext,
                );
            } else {
                let item_h = 36.0;
                let item_gap = 2.0;
                let step = item_h + item_gap;
                let inner_margin = 4.0;
                let item_w = content_w - inner_margin * 2.0;

                for (i, folder) in folders.iter().enumerate() {
                    let item_y = inner_margin + (i as f32 * step) - state.scroll_offset;
                    // Cull items outside the visible list bounds
                    if item_y + item_h < 0.0 || item_y > list_h {
                        continue;
                    }

                    let is_selected = i as i32 == state.selected_index;
                    let item_x = inner_margin;

                    if is_selected {
                        let sel_bg = Color::from_rgba(col_primary.red(), col_primary.green(), col_primary.blue(), 0.22)
                            .unwrap_or(col_primary);
                        let sel_border = Color::from_rgba(col_primary.red(), col_primary.green(), col_primary.blue(), 0.40)
                            .unwrap_or(col_primary);
                        draw_rounded_rect(
                            &mut list_pixmap,
                            item_x,
                            item_y,
                            item_w,
                            item_h,
                            8.0,
                            sel_bg,
                            Some((sel_border, 1.0)),
                        );
                    }

                    draw_icon(
                        font_renderer,
                        &mut list_pixmap,
                        "folder",
                        item_x + 18.0,
                        item_y + 18.0,
                        18.0,
                        col_primary,
                    );
                    draw_text_left(
                        font_renderer,
                        &mut list_pixmap,
                        &folder.name,
                        item_x + 36.0,
                        item_y + 18.0,
                        12.0,
                        col_on_surface,
                    );
                    draw_icon(
                        font_renderer,
                        &mut list_pixmap,
                        "chevron_right",
                        item_x + item_w - 18.0,
                        item_y + 18.0,
                        16.0,
                        if is_selected { col_primary } else { col_subtext },
                    );
                }
            }

            pixmap.draw_pixmap(
                (card_x + margin).round() as i32,
                list_y.round() as i32,
                list_pixmap.as_ref(),
                &PixmapPaint::default(),
                Transform::identity(),
                None,
            );
        }
    }

    // 8. Bottom Action Bar
    let bar_y = card_y + card_h - 52.0;

    // Cancel Button
    let cancel_w = 90.0;
    let cancel_h = 36.0;
    let cancel_bg = Color::from_rgba(1.0, 1.0, 1.0, 0.08).unwrap_or(Color::BLACK);
    let cancel_border = Color::from_rgba(1.0, 1.0, 1.0, 0.16).unwrap_or(Color::WHITE);
    draw_rounded_rect(
        pixmap,
        card_x + margin,
        bar_y,
        cancel_w,
        cancel_h,
        10.0,
        cancel_bg,
        Some((cancel_border, 1.0)),
    );
    draw_text(
        font_renderer,
        pixmap,
        "Cancel",
        card_x + margin + cancel_w / 2.0,
        bar_y + cancel_h / 2.0,
        12.0,
        col_on_surface,
    );

    // Primary Select Folder Button
    let select_h = 36.0;
    let current_name = state.current_folder_name();
    let select_label = if !current_name.is_empty() {
        format!("Select \"{}\"", current_name)
    } else {
        "Select Folder".to_string()
    };
    let select_w = (select_label.len() as f32 * 7.0 + 44.0).clamp(140.0, 220.0);
    let select_x = card_x + card_w - margin - select_w;

    draw_rounded_rect(
        pixmap,
        select_x,
        bar_y,
        select_w,
        select_h,
        10.0,
        col_primary,
        None,
    );
    draw_icon(
        font_renderer,
        pixmap,
        "check",
        select_x + 18.0,
        bar_y + select_h / 2.0,
        16.0,
        col_on_primary,
    );
    draw_text(
        font_renderer,
        pixmap,
        &select_label,
        select_x + select_w / 2.0 + 8.0,
        bar_y + select_h / 2.0,
        12.0,
        col_on_primary,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::folder::Breadcrumb;

    #[test]
    fn test_folder_browser_state_init() {
        let state = FolderBrowserState::new();
        assert!(!state.is_open);
        assert!(state.current_path.is_empty());
        assert!(state.listing.is_none());
        assert!(!state.places.is_empty());
        assert_eq!(state.scroll_offset, 0.0);
        assert_eq!(state.selected_index, -1);
        assert!(state.search_query.is_empty());

        let default_state = FolderBrowserState::default();
        assert!(!default_state.is_open);
    }

    #[test]
    fn test_folder_browser_filter_and_selection() {
        let mut state = FolderBrowserState::new();
        state.open("/test/path");
        assert!(state.is_open);
        assert_eq!(state.current_path, "/test/path");

        let listing = DirListing {
            path: "/test/path".to_string(),
            parent: "/test".to_string(),
            name: "path".to_string(),
            crumbs: vec![
                Breadcrumb { name: "Root (/)".to_string(), path: "/".to_string() },
                Breadcrumb { name: "test".to_string(), path: "/test".to_string() },
                Breadcrumb { name: "path".to_string(), path: "/test/path".to_string() },
            ],
            folders: vec![
                FolderEntry { name: "Alpha".to_string(), path: "/test/path/Alpha".to_string() },
                FolderEntry { name: "Alphabet".to_string(), path: "/test/path/Alphabet".to_string() },
                FolderEntry { name: "Beta".to_string(), path: "/test/path/Beta".to_string() },
                FolderEntry { name: "Gamma".to_string(), path: "/test/path/Gamma".to_string() },
            ],
            error: None,
        };

        state.set_listing(listing);
        assert_eq!(state.selected_index, 0);
        assert_eq!(state.filtered_folders().len(), 4);
        assert_eq!(state.selected_folder().unwrap().name, "Alpha");

        // Navigation
        state.select_next();
        assert_eq!(state.selected_index, 1);
        assert_eq!(state.selected_folder().unwrap().name, "Alphabet");

        state.select_prev();
        assert_eq!(state.selected_index, 0);

        // Filter query
        state.set_search_query("beta");
        let filtered = state.filtered_folders();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "Beta");
    }

    #[test]
    fn test_render_folder_browser_closed() {
        let state = FolderBrowserState::new();
        let mut pixmap = Pixmap::new(800, 600).unwrap();
        pixmap.fill(Color::TRANSPARENT);

        render_folder_browser(&state, &mut pixmap, 800.0, 600.0, 400.0, 300.0, CustomizerColors::default());

        let non_zero = pixmap.pixels().iter().filter(|p| p.alpha() > 0).count();
        assert_eq!(non_zero, 0, "Closed folder browser should render nothing");
    }

    #[test]
    fn test_render_folder_browser_open() {
        let mut state = FolderBrowserState::new();
        state.open("/home/user");
        state.set_places(vec![
            PlaceEntry {
                name: "Home".to_string(),
                path: "/home/user".to_string(),
                icon: "home".to_string(),
                is_device: false,
                dev_node: None,
            },
            PlaceEntry {
                name: "Root (/)".to_string(),
                path: "/".to_string(),
                icon: "computer".to_string(),
                is_device: false,
                dev_node: None,
            },
        ]);
        state.set_listing(DirListing {
            path: "/home/user".to_string(),
            parent: "/home".to_string(),
            name: "user".to_string(),
            crumbs: vec![
                Breadcrumb { name: "Root (/)".to_string(), path: "/".to_string() },
                Breadcrumb { name: "home".to_string(), path: "/home".to_string() },
                Breadcrumb { name: "user".to_string(), path: "/home/user".to_string() },
            ],
            folders: vec![
                FolderEntry { name: "Documents".to_string(), path: "/home/user/Documents".to_string() },
                FolderEntry { name: "Downloads".to_string(), path: "/home/user/Downloads".to_string() },
            ],
            error: None,
        });

        let mut pixmap = Pixmap::new(800, 600).unwrap();
        pixmap.fill(Color::TRANSPARENT);

        render_folder_browser(&state, &mut pixmap, 800.0, 600.0, 400.0, 300.0, CustomizerColors::default());

        let non_zero = pixmap.pixels().iter().filter(|p| p.alpha() > 0).count();
        assert!(non_zero > 1000, "Open folder browser modal must render visible pixels onto pixmap");
    }
}
