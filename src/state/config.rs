use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileJumpTarget {
    pub id: String,
    pub label: String,
    pub path: String,
    pub icon: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct PerformanceConfig {
    pub profile: Option<String>, // "low_end" | "high_performance" | null
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ColorsConfig {
    #[serde(default)]
    pub primary: Option<String>,    // hex e.g. "#a8c5fd"
    #[serde(alias = "onPrimary", default)]
    pub on_primary: Option<String>, // hex e.g. "#06305b"
    #[serde(default)]
    pub surface: Option<String>,    // hex e.g. "#e6e6ed"
    #[serde(default)]
    pub subtext: Option<String>,    // hex e.g. "#b2b2bf"
    #[serde(alias = "onSurface", default)]
    pub on_surface: Option<String>, // hex e.g. "#e3e2e2"
    #[serde(alias = "surfaceContainer", default)]
    pub surface_container: Option<String>, // hex e.g. "#1f2020"
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RadialConfig {
    #[serde(rename = "globalSlices", default = "default_global_slices")]
    pub global_slices: Vec<String>,

    #[serde(rename = "kittySlices", default = "default_kitty_slices")]
    pub kitty_slices: Vec<String>,

    #[serde(rename = "browserSlices", default = "default_browser_slices")]
    pub browser_slices: Vec<String>,

    #[serde(rename = "codeSlices", default = "default_code_slices")]
    pub code_slices: Vec<String>,

    #[serde(rename = "mediaSlices", default = "default_media_slices")]
    pub media_slices: Vec<String>,

    #[serde(rename = "fileJumpTargets", default = "default_file_jump_targets")]
    pub file_jump_targets: Vec<FileJumpTarget>,

    #[serde(default)]
    pub performance: Option<PerformanceConfig>,

    /// User-defined explicit override colors (if specified in config.json)
    #[serde(rename = "customColors", alias = "custom_colors", default)]
    pub custom_colors: Option<ColorsConfig>,

    /// Active colors used by the renderer (runtime only; not serialized to avoid locking system themes)
    #[serde(skip_serializing, default)]
    pub colors: Option<ColorsConfig>,

    /// Tracks whether custom colors explicitly override system colors
    #[serde(rename = "hasExplicitColors", alias = "has_explicit_colors", default)]
    pub has_explicit_colors: bool,
}

fn default_global_slices() -> Vec<String> {
    vec!["scratchpad", "terminal", "wallpapers", "btop", "session", "filejump", "calc", "active_apps"]
        .into_iter().map(String::from).collect()
}

fn default_kitty_slices() -> Vec<String> {
    vec!["scratchpad", "kitty_new_window", "kitty_agy", "kitty_clear", "kitty_dolphin", "active_apps"]
        .into_iter().map(String::from).collect()
}

fn default_browser_slices() -> Vec<String> {
    vec!["scratchpad", "browsertabs", "browser_new_tab", "browser_close_tab", "browser_dup_tab", "browser_reopen_tab", "active_apps"]
        .into_iter().map(String::from).collect()
}

fn default_code_slices() -> Vec<String> {
    vec!["scratchpad", "code_palette", "code_terminal", "code_git_status", "code_format", "code_run", "active_apps"]
        .into_iter().map(String::from).collect()
}

fn default_media_slices() -> Vec<String> {
    vec!["scratchpad", "media_play_pause", "media_prev", "media_next", "volume_up", "volume_down", "audio_sink", "active_apps"]
        .into_iter().map(String::from).collect()
}

fn default_file_jump_targets() -> Vec<FileJumpTarget> {
    vec![
        FileJumpTarget { id: "downloads".into(), label: "Downloads".into(), path: "~/Downloads".into(), icon: "download".into() },
        FileJumpTarget { id: "documents".into(), label: "Documents".into(), path: "~/Documents".into(), icon: "description".into() },
        FileJumpTarget { id: "pictures".into(),  label: "Pictures".into(),  path: "~/Pictures".into(),  icon: "photo".into() },
        FileJumpTarget { id: "music".into(),     label: "Music".into(),     path: "~/Music".into(),     icon: "music_note".into() },
        FileJumpTarget { id: "home".into(),      label: "Home".into(),      path: "~".into(),           icon: "home".into() },
        FileJumpTarget { id: "temp".into(),      label: "Temp".into(),      path: "/tmp".into(),        icon: "folder_delete".into() },
    ]
}

impl ColorsConfig {
    pub fn default_primary()           -> &'static str { "#cbc4cb" }
    pub fn default_on_primary()        -> &'static str { "#322f34" }
    pub fn default_surface()           -> &'static str { "#141313" }
    pub fn default_subtext()           -> &'static str { "#948f94" }
    pub fn default_on_surface()        -> &'static str { "#e3e2e2" }
    pub fn default_surface_container() -> &'static str { "#1f2020" }

    pub fn primary_hex(&self)           -> &str { self.primary.as_deref().unwrap_or(Self::default_primary()) }
    pub fn on_primary_hex(&self)        -> &str { self.on_primary.as_deref().unwrap_or(Self::default_on_primary()) }
    pub fn surface_hex(&self)           -> &str { self.surface.as_deref().unwrap_or(Self::default_surface()) }
    pub fn subtext_hex(&self)           -> &str { self.subtext.as_deref().unwrap_or(Self::default_subtext()) }
    pub fn on_surface_hex(&self)        -> &str { self.on_surface.as_deref().unwrap_or(Self::default_on_surface()) }
    pub fn surface_container_hex(&self) -> &str { self.surface_container.as_deref().unwrap_or(Self::default_surface_container()) }
}

impl RadialConfig {
    /// Load from ~/.config/radialMenu/config.json
    /// Returns Default if file doesn't exist or is malformed
    pub fn load() -> Self {
        let path = shellexpand::tilde("~/.config/radialMenu/config.json").to_string();
        Self::load_from(std::path::Path::new(&path))
    }

    /// Load from explicit path. Returns Default if file doesn't exist or is malformed.
    pub fn load_from(path: &std::path::Path) -> Self {
        let mut cfg: Self = std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        if let Some(custom) = &cfg.custom_colors {
            cfg.has_explicit_colors = true;
            cfg.colors = Some(custom.clone());
        } else {
            cfg.has_explicit_colors = false;
            cfg.colors = Self::load_system_colors();
        }

        cfg
    }

    /// Load dynamic system theme colors from generated colors.json or fallback Appearance.qml / gtk.css
    pub fn load_system_colors() -> Option<ColorsConfig> {
        // 1. Primary sources: Matugen generated colors.json
        for json_path in &[
            shellexpand::tilde("~/.local/state/quickshell/user/generated/colors.json").to_string(),
            shellexpand::tilde("~/.cache/quickshell/user/generated/colors.json").to_string(),
        ] {
            if let Ok(content) = std::fs::read_to_string(json_path) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                    let primary = val.get("primary").and_then(|v| v.as_str()).map(String::from);
                    let on_primary = val.get("on_primary").and_then(|v| v.as_str()).map(String::from);
                    let surface = val.get("surface").and_then(|v| v.as_str()).map(String::from);
                    let subtext = val.get("outline").and_then(|v| v.as_str())
                        .or_else(|| val.get("on_surface_variant").and_then(|v| v.as_str()))
                        .map(String::from);
                    let on_surface = val.get("on_surface").and_then(|v| v.as_str()).map(String::from);
                    let surface_container = val.get("surface_container").and_then(|v| v.as_str())
                        .or_else(|| val.get("surface_container_high").and_then(|v| v.as_str()))
                        .map(String::from);

                    if primary.is_some() || on_primary.is_some() || surface.is_some() {
                        return Some(ColorsConfig {
                            primary,
                            on_primary,
                            surface,
                            subtext,
                            on_surface,
                            surface_container,
                        });
                    }
                }
            }
        }

        // 2. Secondary fallback: GTK CSS (gtk-3.0 / gtk-4.0)
        for css_path in &[
            shellexpand::tilde("~/.config/gtk-3.0/gtk.css").to_string(),
            shellexpand::tilde("~/.config/gtk-4.0/gtk.css").to_string(),
        ] {
            if let Ok(content) = std::fs::read_to_string(css_path) {
                let mut primary = None;
                let mut on_primary = None;
                let mut surface = None;
                let mut on_surface = None;
                let mut surface_container = None;

                for line in content.lines() {
                    let line = line.trim();
                    if let Some(rest) = line.strip_prefix("@define-color accent_color ") {
                        primary = rest.strip_suffix(';').map(|s| s.trim().to_string());
                    } else if let Some(rest) = line.strip_prefix("@define-color accent_fg_color ") {
                        on_primary = rest.strip_suffix(';').map(|s| s.trim().to_string());
                    } else if let Some(rest) = line.strip_prefix("@define-color window_bg_color ") {
                        surface = rest.strip_suffix(';').map(|s| s.trim().to_string());
                    } else if let Some(rest) = line.strip_prefix("@define-color window_fg_color ") {
                        on_surface = rest.strip_suffix(';').map(|s| s.trim().to_string());
                    } else if let Some(rest) = line.strip_prefix("@define-color card_bg_color ") {
                        surface_container = rest.strip_suffix(';').map(|s| s.trim().to_string());
                    }
                }

                if primary.is_some() || on_primary.is_some() || surface.is_some() {
                    return Some(ColorsConfig {
                        primary,
                        on_primary,
                        surface,
                        subtext: None,
                        on_surface,
                        surface_container,
                    });
                }
            }
        }

        // 3. Tertiary fallback: Appearance.qml
        let mut detected_primary = None;
        let mut detected_on_primary = None;
        for qml_path in &[
            shellexpand::tilde("~/.config/quickshell/ii/modules/common/Appearance.qml").to_string(),
            shellexpand::tilde("~/.config/quickshell/end4-pC/modules/common/Appearance.qml").to_string(),
        ] {
            if let Ok(content) = std::fs::read_to_string(qml_path) {
                for line in content.lines() {
                    if line.contains("m3primary:") {
                        if let Some(start) = line.find('"') {
                            if let Some(end) = line[start + 1..].find('"') {
                                detected_primary = Some(line[start + 1..start + 1 + end].to_string());
                            }
                        }
                    }
                    if line.contains("m3onPrimary:") {
                        if let Some(start) = line.find('"') {
                            if let Some(end) = line[start + 1..].find('"') {
                                detected_on_primary = Some(line[start + 1..start + 1 + end].to_string());
                            }
                        }
                    }
                }
                if detected_primary.is_some() {
                    break;
                }
            }
        }

        if detected_primary.is_some() || detected_on_primary.is_some() {
            return Some(ColorsConfig {
                primary: detected_primary,
                on_primary: detected_on_primary,
                surface: None,
                subtext: None,
                on_surface: None,
                surface_container: None,
            });
        }

        None
    }

    /// Dynamically reload system theme colors if colors.json was modified and user hasn't specified explicit colors
    pub fn reload_system_colors(&mut self) -> bool {
        if !self.has_explicit_colors {
            if let Some(sys_colors) = Self::load_system_colors() {
                if self.colors.as_ref() != Some(&sys_colors) {
                    log::info!("System colors reloaded dynamically: primary={:?}", sys_colors.primary);
                    self.colors = Some(sys_colors);
                    return true;
                }
            }
        }
        false
    }

    /// Atomic write: write to temp file, then rename (prevents corruption)
    pub fn save(&self) -> anyhow::Result<()> {
        let path = shellexpand::tilde("~/.config/radialMenu/config.json").to_string();
        self.save_to(std::path::Path::new(&path))
    }

    /// Atomic write to explicit path: write to temp file, then rename
    pub fn save_to(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let dir = path.parent()
            .ok_or_else(|| anyhow::anyhow!("no parent dir"))?;
        std::fs::create_dir_all(dir)?;
        let tmp = format!("{}.tmp", path.display());
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&tmp, &json)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// Get active slice IDs for a given context string (port of getActiveSliceIds)
    /// Context strings: "kitty", "browser", "code", "media", "default"
    pub fn get_active_slice_ids(&self, context: &str) -> Vec<String> {
        let mut ids = match context {
            "kitty"   => self.kitty_slices.clone(),
            "browser" => self.browser_slices.clone(),
            "code"    => self.code_slices.clone(),
            "media"   => self.media_slices.clone(),
            _         => self.global_slices.clone(),
        };
        // Ensure "active_apps" always present (port of QML behavior)
        if !ids.contains(&"active_apps".to_string()) {
            ids.push("active_apps".to_string());
        }
        ids
    }

    /// Add a slice to dial for given context
    pub fn add_slice(&mut self, context: &str, function_id: &str) {
        let slices = self.slices_mut(context);
        if !slices.contains(&function_id.to_string()) {
            slices.push(function_id.to_string());
        }
    }

    /// Remove a slice from dial (min 2 slices enforced)
    pub fn remove_slice(&mut self, context: &str, function_id: &str) {
        let slices = self.slices_mut(context);
        if slices.len() > 2 {
            slices.retain(|s| s != function_id);
        }
    }

    /// Swap (replace) slice at slot_index with new function_id
    pub fn swap_slice(&mut self, context: &str, slot_index: usize, new_function_id: &str) {
        let slices = self.slices_mut(context);
        if slot_index < slices.len() {
            slices[slot_index] = new_function_id.to_string();
        } else {
            slices.push(new_function_id.to_string());
        }
    }

    /// Reorder slice from_index -> to_index
    pub fn reorder_slice(&mut self, context: &str, from_index: usize, to_index: usize) {
        if from_index == to_index { return; }
        let slices = self.slices_mut(context);
        if from_index < slices.len() && to_index < slices.len() {
            let item = slices.remove(from_index);
            slices.insert(to_index, item);
        }
    }

    fn slices_mut(&mut self, context: &str) -> &mut Vec<String> {
        match context {
            "kitty"   => &mut self.kitty_slices,
            "browser" => &mut self.browser_slices,
            "code"    => &mut self.code_slices,
            "media"   => &mut self.media_slices,
            _         => &mut self.global_slices,
        }
    }

    /// Add file jump target
    pub fn add_file_jump_target(&mut self, label: &str, path: &str, icon: &str) {
        self.file_jump_targets.push(FileJumpTarget {
            id: format!("target_{}", std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis()),
            label: label.to_string(),
            path: path.to_string(),
            icon: icon.to_string(),
        });
    }

    /// Update file jump target at index
    pub fn update_file_jump_target(&mut self, index: usize, label: &str, path: &str, icon: &str) {
        if let Some(t) = self.file_jump_targets.get_mut(index) {
            t.label = label.to_string();
            t.path = path.to_string();
            t.icon = icon.to_string();
        }
    }

    /// Remove file jump target at index
    pub fn remove_file_jump_target(&mut self, index: usize) {
        if index < self.file_jump_targets.len() {
            self.file_jump_targets.remove(index);
        }
    }

    /// Reset slices for a specific context to default factory slices
    pub fn reset_context(&mut self, context: &str) {
        match context {
            "kitty"   => self.kitty_slices = default_kitty_slices(),
            "browser" => self.browser_slices = default_browser_slices(),
            "code"    => self.code_slices = default_code_slices(),
            "media"   => self.media_slices = default_media_slices(),
            _         => self.global_slices = default_global_slices(),
        }
    }

    /// Reset to factory defaults
    pub fn reset() -> Self {
        Self::default()
    }
}

impl Default for RadialConfig {
    fn default() -> Self {
        RadialConfig {
            global_slices: default_global_slices(),
            kitty_slices: default_kitty_slices(),
            browser_slices: default_browser_slices(),
            code_slices: default_code_slices(),
            media_slices: default_media_slices(),
            file_jump_targets: default_file_jump_targets(),
            performance: None,
            custom_colors: None,
            colors: None,
            has_explicit_colors: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_global_slices() {
        let cfg = RadialConfig::default();
        assert_eq!(cfg.global_slices, vec!["scratchpad","terminal","wallpapers","btop","session","filejump","calc","active_apps"]);
    }

    #[test]
    fn test_default_kitty_slices() {
        let cfg = RadialConfig::default();
        assert_eq!(cfg.kitty_slices, vec!["scratchpad","kitty_new_window","kitty_agy","kitty_clear","kitty_dolphin","active_apps"]);
    }

    #[test]
    fn test_default_browser_slices() {
        let cfg = RadialConfig::default();
        assert_eq!(cfg.browser_slices, vec!["scratchpad","browsertabs","browser_new_tab","browser_close_tab","browser_dup_tab","browser_reopen_tab","active_apps"]);
    }

    #[test]
    fn test_default_code_slices() {
        let cfg = RadialConfig::default();
        assert_eq!(cfg.code_slices, vec!["scratchpad","code_palette","code_terminal","code_git_status","code_format","code_run","active_apps"]);
    }

    #[test]
    fn test_default_media_slices() {
        let cfg = RadialConfig::default();
        assert_eq!(cfg.media_slices, vec!["scratchpad","media_play_pause","media_prev","media_next","volume_up","volume_down","audio_sink","active_apps"]);
    }

    #[test]
    fn test_get_active_slice_ids_adds_active_apps() {
        let mut cfg = RadialConfig::default();
        cfg.global_slices = vec!["terminal".into(), "calc".into()]; // no active_apps
        let ids = cfg.get_active_slice_ids("default");
        assert!(ids.contains(&"active_apps".to_string()));
    }

    #[test]
    fn test_get_active_slice_ids_contexts() {
        let cfg = RadialConfig::default();
        assert_eq!(cfg.get_active_slice_ids("kitty"), cfg.kitty_slices);
        assert_eq!(cfg.get_active_slice_ids("browser"), cfg.browser_slices);
        assert_eq!(cfg.get_active_slice_ids("code"), cfg.code_slices);
        assert_eq!(cfg.get_active_slice_ids("media"), cfg.media_slices);
        assert_eq!(cfg.get_active_slice_ids("other"), cfg.global_slices);
    }

    #[test]
    fn test_save_load_roundtrip() {
        let cfg = RadialConfig::default();
        let json = serde_json::to_string_pretty(&cfg).unwrap();
        let loaded: RadialConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.global_slices, cfg.global_slices);
        assert_eq!(loaded.file_jump_targets.len(), 6);
        assert_eq!(loaded, cfg);
    }

    #[test]
    fn test_add_remove_slice() {
        let mut cfg = RadialConfig::default();
        cfg.add_slice("default", "color_picker");
        assert!(cfg.global_slices.contains(&"color_picker".to_string()));
        // Adding again does not duplicate
        cfg.add_slice("default", "color_picker");
        assert_eq!(cfg.global_slices.iter().filter(|&s| s == "color_picker").count(), 1);

        cfg.remove_slice("default", "color_picker");
        assert!(!cfg.global_slices.contains(&"color_picker".to_string()));
    }

    #[test]
    fn test_remove_slice_min_2_constraint() {
        let mut cfg = RadialConfig::default();
        cfg.kitty_slices = vec!["slice1".into(), "slice2".into()];
        cfg.remove_slice("kitty", "slice1");
        assert_eq!(cfg.kitty_slices.len(), 2, "Must enforce min 2 slices");
    }

    #[test]
    fn test_swap_slice() {
        let mut cfg = RadialConfig::default();
        cfg.swap_slice("kitty", 1, "custom_action");
        assert_eq!(cfg.kitty_slices[1], "custom_action");

        // Out of bounds appends
        let len = cfg.kitty_slices.len();
        cfg.swap_slice("kitty", len + 10, "appended_action");
        assert_eq!(cfg.kitty_slices.last().unwrap(), "appended_action");
    }

    #[test]
    fn test_reorder_slice() {
        let mut cfg = RadialConfig::default();
        let original_second = cfg.global_slices[1].clone();
        cfg.reorder_slice("default", 1, 0);
        assert_eq!(cfg.global_slices[0], original_second);

        // Same index is a no-op
        let before = cfg.global_slices.clone();
        cfg.reorder_slice("default", 2, 2);
        assert_eq!(cfg.global_slices, before);

        // Out of bounds is safely ignored
        cfg.reorder_slice("default", 0, 999);
        assert_eq!(cfg.global_slices, before);
    }

    #[test]
    fn test_file_jump_target_ops() {
        let mut cfg = RadialConfig::default();
        let orig_count = cfg.file_jump_targets.len();
        cfg.add_file_jump_target("Test", "~/test", "folder");
        assert_eq!(cfg.file_jump_targets.len(), orig_count + 1);
        assert_eq!(cfg.file_jump_targets[orig_count].label, "Test");
        assert_eq!(cfg.file_jump_targets[orig_count].path, "~/test");
        assert_eq!(cfg.file_jump_targets[orig_count].icon, "folder");

        cfg.update_file_jump_target(orig_count, "Updated", "~/updated", "folder_special");
        assert_eq!(cfg.file_jump_targets[orig_count].label, "Updated");
        assert_eq!(cfg.file_jump_targets[orig_count].path, "~/updated");
        assert_eq!(cfg.file_jump_targets[orig_count].icon, "folder_special");

        cfg.remove_file_jump_target(orig_count);
        assert_eq!(cfg.file_jump_targets.len(), orig_count);
    }

    #[test]
    fn test_json_field_names_camel_case() {
        let cfg = RadialConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        assert!(json.contains("globalSlices"));
        assert!(json.contains("kittySlices"));
        assert!(json.contains("browserSlices"));
        assert!(json.contains("fileJumpTargets"));
    }

    #[test]
    fn test_colors_config_defaults() {
        let empty_colors = ColorsConfig::default();
        assert_eq!(empty_colors.primary_hex(), "#cbc4cb");
        assert_eq!(empty_colors.on_primary_hex(), "#322f34");
        assert_eq!(empty_colors.surface_hex(), "#141313");
        assert_eq!(empty_colors.subtext_hex(), "#948f94");
        assert_eq!(empty_colors.on_surface_hex(), "#e3e2e2");
        assert_eq!(empty_colors.surface_container_hex(), "#1f2020");

        let custom_colors = ColorsConfig {
            primary: Some("#ff0000".into()),
            on_primary: Some("#00ff00".into()),
            surface: Some("#0000ff".into()),
            subtext: Some("#ffff00".into()),
            on_surface: Some("#ffffff".into()),
            surface_container: Some("#112233".into()),
        };
        assert_eq!(custom_colors.primary_hex(), "#ff0000");
        assert_eq!(custom_colors.on_primary_hex(), "#00ff00");
        assert_eq!(custom_colors.surface_hex(), "#0000ff");
        assert_eq!(custom_colors.subtext_hex(), "#ffff00");
        assert_eq!(custom_colors.on_surface_hex(), "#ffffff");
        assert_eq!(custom_colors.surface_container_hex(), "#112233");
    }

    #[test]
    fn test_system_colors_dynamic_reload() {
        let mut cfg = RadialConfig::default();
        assert!(!cfg.has_explicit_colors);

        // System colors should be reloaded when available
        let _ = cfg.reload_system_colors();

        // If custom colors are set, has_explicit_colors is true and reload is skipped
        cfg.custom_colors = Some(ColorsConfig {
            primary: Some("#123456".into()),
            ..Default::default()
        });
        cfg.has_explicit_colors = true;
        cfg.colors = cfg.custom_colors.clone();
        assert!(!cfg.reload_system_colors());
        assert_eq!(cfg.colors.as_ref().unwrap().primary_hex(), "#123456");
    }

    #[test]
    fn test_reset() {
        let mut cfg = RadialConfig::default();
        cfg.global_slices.clear();
        assert!(cfg.global_slices.is_empty());
        let reset_cfg = RadialConfig::reset();
        assert_eq!(reset_cfg.global_slices, default_global_slices());
    }

    #[test]
    fn test_reset_context() {
        let mut cfg = RadialConfig::default();
        cfg.kitty_slices = vec!["custom1".into(), "custom2".into()];
        cfg.reset_context("kitty");
        assert_eq!(cfg.kitty_slices, default_kitty_slices());

        cfg.global_slices = vec!["custom_g".into()];
        cfg.reset_context("default");
        assert_eq!(cfg.global_slices, default_global_slices());
    }

    #[test]
    fn test_atomic_write_and_load_in_tempdir() {
        let unique_name = format!("radial_dial_test_{}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
        let test_dir = std::env::temp_dir().join(unique_name);
        let test_file = test_dir.join("config.json");

        let mut cfg = RadialConfig::default();
        cfg.add_slice("kitty", "extra_slice");

        let res = cfg.save_to(&test_file);
        assert!(res.is_ok(), "save_to should succeed: {:?}", res);

        let loaded = RadialConfig::load_from(&test_file);
        assert_eq!(loaded.kitty_slices, cfg.kitty_slices);

        // Clean up
        let _ = std::fs::remove_dir_all(&test_dir);
    }
}
