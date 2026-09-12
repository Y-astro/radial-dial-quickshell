// src/font/mod.rs
#![allow(dead_code)]

use cosmic_text::{FontSystem, SwashCache};
use std::path::Path;

/// Maps Material Symbols icon names to their Unicode codepoints.
/// Covers 60+ symbols used across radial-dial.
pub fn icon_codepoint(name: &str) -> char {
    match name {
        // Essential / System
        "terminal" => '\u{eb8e}',
        "close" | "✕" | "x" => '\u{e5cd}',
        "search" => '\u{e8b6}',
        "settings" => '\u{e8b8}',
        "apps" | "active_apps" => '\u{e5c3}',
        "calculate" | "calc" => '\u{ea5f}',
        "code" => '\u{e86f}',
        "monitoring" | "low_end" | "btop" | "htop" => '\u{f190}',
        "globe" | "firefox" | "browser" => '\u{e64c}',
        "window" => '\u{f088}',
        "power_settings_new" | "session" => '\u{e8ac}',
        "lock" => '\u{e88d}',
        "nightlight" => '\u{ef5e}',

        // Window Management
        "picture_in_picture_alt" | "toggle_float" => '\u{e911}',
        "fullscreen" | "toggle_fullscreen" => '\u{e5d0}',
        "push_pin" | "pin_window" => '\u{f10d}',
        "move_to_inbox" => '\u{e168}',
        "unarchive" => '\u{e169}',

        // Clipboard & Audio & Media
        "content_paste" | "clipboard" | "no_clips" => '\u{e14f}',
        "volume_up" | "audio_sink" | "audio" => '\u{e050}',
        "volume_down" => '\u{e04d}',
        "volume_off" | "media_mute" => '\u{e04f}',
        "play_arrow" | "media_play" | "media_play_pause" => '\u{e037}',
        "skip_next" | "media_next" => '\u{e044}',
        "skip_previous" | "media_prev" => '\u{e045}',
        "brightness_high" | "brightness_up" => '\u{e1ac}',
        "brightness_low" | "brightness_down" => '\u{e1ad}',

        // Tools & Capture
        "palette" | "color_picker" | "colorpicker" => '\u{e3b7}',
        "screenshot_region" | "screen_snip" | "snip" => '\u{f7d2}',
        "document_scanner" | "screen_ocr" | "ocr" => '\u{e5fa}',
        "videocam" | "screen_record" => '\u{e04b}',
        "commit" => '\u{eaf5}',
        "format_align_left" => '\u{e236}',
        "drive_file_move" | "filejump" | "file_jump" => '\u{e675}',
        "wallpaper" | "wallpapers" => '\u{e1bc}',
        "sentiment_satisfied" | "emoji" => '\u{e0ed}',

        // Terminal Context
        "open_in_new" | "launch" => '\u{e895}',
        "robot_2" | "kitty_agy" | "robot" => '\u{f5d0}',
        "mop" | "kitty_clear" => '\u{e28d}',

        // Browser Context
        "tabs" | "browsertabs" => '\u{e9ee}',
        "tab" | "browser_new_tab" => '\u{e8d8}',
        "tab_close" | "browser_close_tab" => '\u{f745}',
        "tab_duplicate" | "browser_dup_tab" => '\u{f744}',
        "history" | "browser_reopen_tab" => '\u{e28e}',

        // Files & Navigation
        "folder" => '\u{e2c7}',
        "folder_open" => '\u{e2c8}',
        "folder_delete" => '\u{eb34}',
        "folder_special" => '\u{e617}',
        "folder_zip" => '\u{eb2c}',
        "folder_off" => '\u{eb31}',
        "download" | "downloads" => '\u{e171}',
        "description" | "documents" => '\u{e683}',
        "photo" | "pictures" => '\u{e410}',
        "music_note" => '\u{e3a1}',
        "movie" => '\u{e02c}',
        "home" => '\u{e88a}',
        "hard_drive" => '\u{f80e}',
        "computer" => '\u{e30a}',
        "cloud" => '\u{e2bd}',
        "arrow_upward" => '\u{e5d8}',
        "chevron_right" => '\u{e5cc}',
        "progress_activity" => '\u{e048}',

        // Chips & General Icons
        "school" => '\u{e80c}',
        "work" => '\u{e8f9}',
        "favorite" => '\u{e87d}',
        "star" => '\u{e838}',
        "add" | "add_file_target" => '\u{e145}',
        "colorize" => '\u{e3b8}',
        "info" => '\u{e88e}',
        "assignment" => '\u{e85d}',
        "content_paste_off" => '\u{e4f8}',
        "edit_note" => '\u{e745}',
        "chat" => '\u{e0b7}',
        "check" => '\u{e5ca}',
        "arrow_back" => '\u{e5c4}',
        "arrow_forward" => '\u{e5c8}',
        "refresh" => '\u{e5d5}',
        "more_vert" => '\u{e5d4}',
        "extension" => '\u{e87b}',
        "delete" => '\u{e872}',
        "edit" => '\u{e150}',
        "swap_horiz" => '\u{e8d4}',
        "remove" => '\u{e15b}',
        "check_circle" => '\u{e86c}',
        "create_new_folder" => '\u{e2cc}',
        "link" => '\u{e157}',
        "visibility" => '\u{e417}',
        "visibility_off" => '\u{e8f5}',
        "copy" | "content_copy" => '\u{e14d}',
        "file_copy" => '\u{e173}',
        "close_small" => '\u{f508}',
        "terminal_2" => '\u{fff8e}',

        // Tab Counter Badges
        "counter_1" => '\u{f784}',
        "counter_2" => '\u{f783}',
        "counter_3" => '\u{f782}',
        "counter_4" => '\u{f781}',
        "counter_5" => '\u{f780}',
        "counter_6" => '\u{f77f}',
        "counter_7" => '\u{f77e}',
        "counter_8" => '\u{f77d}',

        _ => '\u{e000}', // Fallback
    }
}

/// Font shaping and rasterization engine using `cosmic-text`.
pub struct FontRenderer {
    pub font_system: FontSystem,
    pub swash_cache: SwashCache,
    pub icon_font_family: String,
    pub text_font_family: String,
}

impl Default for FontRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl FontRenderer {
    /// Discovers system fonts and initializes FontSystem and SwashCache.
    pub fn new() -> Self {
        let mut font_system = FontSystem::new();
        let swash_cache = SwashCache::new();

        discover_and_load_fonts(&mut font_system);

        let icon_font_family = resolve_icon_family(&font_system);
        let text_font_family = resolve_text_family(&font_system);

        Self {
            font_system,
            swash_cache,
            icon_font_family,
            text_font_family,
        }
    }
}

/// Checks known paths for Material Symbols and system fonts, loading them into `FontSystem`.
fn discover_and_load_fonts(font_system: &mut FontSystem) {
    let font_paths = [
        "/usr/share/fonts/ttf-material-symbols-variable/MaterialSymbolsRounded[FILL,GRAD,opsz,wght].ttf",
        "/usr/share/fonts/ttf-material-symbols-variable/MaterialSymbolsRounded.ttf",
        "/usr/share/fonts/ttf-material-symbols-variable/MaterialSymbolsOutlined[FILL,GRAD,opsz,wght].ttf",
        "/usr/share/fonts/ttf-material-symbols-variable/MaterialSymbolsSharp[FILL,GRAD,opsz,wght].ttf",
        "/usr/share/fonts/MaterialSymbolsRounded.ttf",
        "/usr/share/fonts/TTF/MaterialSymbolsRounded.ttf",
        "/usr/share/fonts/noto/NotoSans-Regular.ttf",
    ];

    for path_str in &font_paths {
        let path = Path::new(path_str);
        if path.exists() {
            let _ = font_system.db_mut().load_font_file(path);
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        let user_paths = [
            format!("{}/.local/share/fonts/MaterialSymbolsRounded.ttf", home),
            format!("{}/.fonts/MaterialSymbolsRounded.ttf", home),
        ];
        for p in &user_paths {
            let path = Path::new(p);
            if path.exists() {
                let _ = font_system.db_mut().load_font_file(path);
            }
        }
    }
}

/// Finds the best available Material Symbols or icon font family name in `FontSystem`.
fn resolve_icon_family(font_system: &FontSystem) -> String {
    let priority = [
        "Material Symbols Rounded",
        "Material Symbols Outlined",
        "Material Symbols Sharp",
        "Noto Sans Symbols",
        "Noto Sans Symbols 2",
    ];
    for name in &priority {
        for face in font_system.db().faces() {
            for (family_name, _) in &face.families {
                if family_name == *name {
                    return name.to_string();
                }
            }
        }
    }
    "Material Symbols Rounded".to_string()
}

/// Finds the best available standard text font family name in `FontSystem`.
fn resolve_text_family(font_system: &FontSystem) -> String {
    let priority = [
        "Noto Sans",
        "Inter",
        "Roboto",
        "Cantarell",
        "DejaVu Sans",
        "Liberation Sans",
    ];
    for name in &priority {
        for face in font_system.db().faces() {
            for (family_name, _) in &face.families {
                if family_name == *name {
                    return name.to_string();
                }
            }
        }
    }
    "sans-serif".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_icon_codepoint_mappings() {
        assert_eq!(icon_codepoint("terminal"), '\u{eb8e}');
        assert_eq!(icon_codepoint("close"), '\u{e5cd}');
        assert_eq!(icon_codepoint("✕"), '\u{e5cd}');
        assert_eq!(icon_codepoint("search"), '\u{e8b6}');
        assert_eq!(icon_codepoint("settings"), '\u{e8b8}');
        assert_eq!(icon_codepoint("calculate"), '\u{ea5f}');
        assert_eq!(icon_codepoint("calc"), '\u{ea5f}');
        assert_eq!(icon_codepoint("apps"), '\u{e5c3}');
        assert_eq!(icon_codepoint("content_paste"), '\u{e14f}');
        assert_eq!(icon_codepoint("clipboard"), '\u{e14f}');
        assert_eq!(icon_codepoint("volume_up"), '\u{e050}');
        assert_eq!(icon_codepoint("audio_sink"), '\u{e050}');
        assert_eq!(icon_codepoint("palette"), '\u{e3b7}');
        assert_eq!(icon_codepoint("screenshot_region"), '\u{f7d2}');
        assert_eq!(icon_codepoint("document_scanner"), '\u{e5fa}');
        assert_eq!(icon_codepoint("commit"), '\u{eaf5}');
        assert_eq!(icon_codepoint("format_align_left"), '\u{e236}');
        assert_eq!(icon_codepoint("play_arrow"), '\u{e037}');
        assert_eq!(icon_codepoint("skip_next"), '\u{e044}');
        assert_eq!(icon_codepoint("skip_previous"), '\u{e045}');
        assert_eq!(icon_codepoint("volume_off"), '\u{e04f}');
        assert_eq!(icon_codepoint("volume_down"), '\u{e04d}');
        assert_eq!(icon_codepoint("brightness_high"), '\u{e1ac}');
        assert_eq!(icon_codepoint("brightness_low"), '\u{e1ad}');
        assert_eq!(icon_codepoint("drive_file_move"), '\u{e675}');
        assert_eq!(icon_codepoint("wallpaper"), '\u{e1bc}');
        assert_eq!(icon_codepoint("sentiment_satisfied"), '\u{e0ed}');
        assert_eq!(icon_codepoint("move_to_inbox"), '\u{e168}');
        assert_eq!(icon_codepoint("fullscreen"), '\u{e5d0}');
        assert_eq!(icon_codepoint("videocam"), '\u{e04b}');
        assert_eq!(icon_codepoint("picture_in_picture_alt"), '\u{e911}');
        assert_eq!(icon_codepoint("push_pin"), '\u{f10d}');
        assert_eq!(icon_codepoint("power_settings_new"), '\u{e8ac}');
        assert_eq!(icon_codepoint("lock"), '\u{e88d}');
        assert_eq!(icon_codepoint("nightlight"), '\u{ef5e}');
        assert_eq!(icon_codepoint("open_in_new"), '\u{e895}');
        assert_eq!(icon_codepoint("robot_2"), '\u{f5d0}');
        assert_eq!(icon_codepoint("mop"), '\u{e28d}');
        assert_eq!(icon_codepoint("tabs"), '\u{e9ee}');
        assert_eq!(icon_codepoint("tab"), '\u{e8d8}');
        assert_eq!(icon_codepoint("tab_close"), '\u{f745}');
        assert_eq!(icon_codepoint("tab_duplicate"), '\u{f744}');
        assert_eq!(icon_codepoint("history"), '\u{e28e}');
        assert_eq!(icon_codepoint("edit_note"), '\u{e745}');
        assert_eq!(icon_codepoint("chat"), '\u{e0b7}');
        assert_eq!(icon_codepoint("music_note"), '\u{e3a1}');
        assert_eq!(icon_codepoint("folder"), '\u{e2c7}');
        assert_eq!(icon_codepoint("folder_open"), '\u{e2c8}');
        assert_eq!(icon_codepoint("movie"), '\u{e02c}');
        assert_eq!(icon_codepoint("download"), '\u{e171}');
        assert_eq!(icon_codepoint("description"), '\u{e683}');
        assert_eq!(icon_codepoint("photo"), '\u{e410}');
        assert_eq!(icon_codepoint("home"), '\u{e88a}');
        assert_eq!(icon_codepoint("counter_1"), '\u{f784}');
        assert_eq!(icon_codepoint("counter_8"), '\u{f77d}');
        assert_eq!(icon_codepoint("swap_horiz"), '\u{e8d4}');
        assert_eq!(icon_codepoint("remove"), '\u{e15b}');
        assert_eq!(icon_codepoint("check_circle"), '\u{e86c}');
        assert_eq!(icon_codepoint("create_new_folder"), '\u{e2cc}');

        // Fallback for unknown
        assert_eq!(icon_codepoint("unknown_nonexistent_icon"), '\u{e000}');
    }

    #[test]
    fn test_font_renderer_initialization() {
        let renderer = FontRenderer::new();
        assert!(!renderer.icon_font_family.is_empty());
        assert!(!renderer.text_font_family.is_empty());
    }
}
