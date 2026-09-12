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

    #[serde(default)]
    pub colors: Option<ColorsConfig>,
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
    pub fn default_primary()    -> &'static str { "#cbc4cb" }
    pub fn default_on_primary() -> &'static str { "#322f34" }
    pub fn default_surface()    -> &'static str { "#141313" }
    pub fn default_subtext()    -> &'static str { "#948f94" }

    pub fn primary_hex(&self)    -> &str { self.primary.as_deref().unwrap_or(Self::default_primary()) }
    pub fn on_primary_hex(&self) -> &str { self.on_primary.as_deref().unwrap_or(Self::default_on_primary()) }
    pub fn surface_hex(&self)    -> &str { self.surface.as_deref().unwrap_or(Self::default_surface()) }
    pub fn subtext_hex(&self)    -> &str { self.subtext.as_deref().unwrap_or(Self::default_subtext()) }
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

        if cfg.colors.as_ref().and_then(|c| c.primary.as_ref()).is_none() {
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
            if let Some(prim) = detected_primary {
                let mut colors = cfg.colors.unwrap_or_default();
                colors.primary = Some(prim);
                if let Some(on_p) = detected_on_primary {
                    colors.on_primary = Some(on_p);
                }
                cfg.colors = Some(colors);
            }
        }

        cfg
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
            colors: None,
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

        let custom_colors = ColorsConfig {
            primary: Some("#ff0000".into()),
            on_primary: Some("#00ff00".into()),
            surface: Some("#0000ff".into()),
            subtext: Some("#ffff00".into()),
        };
        assert_eq!(custom_colors.primary_hex(), "#ff0000");
        assert_eq!(custom_colors.on_primary_hex(), "#00ff00");
        assert_eq!(custom_colors.surface_hex(), "#0000ff");
        assert_eq!(custom_colors.subtext_hex(), "#ffff00");
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
