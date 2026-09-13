// src/state/actions.rs

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionId {
    // Apps & Launchers
    Terminal,
    Calc,
    FileSearch,
    Dolphin,
    Browser,
    Editor,
    Btop,
    Settings,
    // Window management
    ActiveApps,
    ToggleFloat,
    ToggleFullscreen,
    KillWindow,
    PinWindow,
    Scratchpad,
    // Tools
    Clipboard,
    AudioSink,
    ColorPicker,
    ScreenSnip,
    ScreenOcr,
    FileJump,
    Wallpapers,
    Emoji,
    // Media
    MediaPlayPause,
    MediaNext,
    MediaPrev,
    MediaMute,
    VolumeUp,
    VolumeDown,
    BrightnessUp,
    BrightnessDown,
    // Capture
    ScreenshotArea,
    ScreenshotFull,
    ScreenRecord,
    // System
    Session,
    Lock,
    NightLight,
    // Context: Terminal
    KittyNewWindow { pid: i64 },
    KittyAgy,
    LaunchAgyTerminal,
    KittyClear,
    KittyDolphin { pid: i64 },
    // Context: Browser
    BrowserTabs,
    BrowserNewTab,
    BrowserCloseTab,
    BrowserDupTab,
    BrowserReopenTab,
    // Context: Code Editor
    CodeGitStatus,
    CodeTerminal,
    CodeFormat,
    CodePalette,
    CodeRun,
    // Dynamic sub-ring items
    FocusWindow { address: String, workspace: String },
    SwitchToTab(usize),
    JumpToFile(String),
    PasteClip(String),
    SetSink(String),
    MoveToWorkspace(u8),
}

impl ActionId {
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "terminal" => Some(ActionId::Terminal),
            "calc" => Some(ActionId::Calc),
            "filesearch" | "file_search" => Some(ActionId::FileSearch),
            "dolphin" => Some(ActionId::Dolphin),
            "browser" => Some(ActionId::Browser),
            "editor" => Some(ActionId::Editor),
            "btop" => Some(ActionId::Btop),
            "settings" => Some(ActionId::Settings),
            "active_apps" => Some(ActionId::ActiveApps),
            "toggle_float" => Some(ActionId::ToggleFloat),
            "toggle_fullscreen" => Some(ActionId::ToggleFullscreen),
            "kill_window" => Some(ActionId::KillWindow),
            "pin_window" => Some(ActionId::PinWindow),
            "scratchpad" => Some(ActionId::Scratchpad),
            "clipboard" => Some(ActionId::Clipboard),
            "audio_sink" => Some(ActionId::AudioSink),
            "color_picker" | "colorpicker" => Some(ActionId::ColorPicker),
            "screen_snip" => Some(ActionId::ScreenSnip),
            "screen_ocr" => Some(ActionId::ScreenOcr),
            "filejump" | "file_jump" => Some(ActionId::FileJump),
            "wallpapers" => Some(ActionId::Wallpapers),
            "emoji" => Some(ActionId::Emoji),
            "media_play_pause" | "media_play" => Some(ActionId::MediaPlayPause),
            "media_next" => Some(ActionId::MediaNext),
            "media_prev" => Some(ActionId::MediaPrev),
            "media_mute" | "volume_mute" => Some(ActionId::MediaMute),
            "volume_up" => Some(ActionId::VolumeUp),
            "volume_down" => Some(ActionId::VolumeDown),
            "brightness_up" => Some(ActionId::BrightnessUp),
            "brightness_down" => Some(ActionId::BrightnessDown),
            "screenshot_area" => Some(ActionId::ScreenshotArea),
            "screenshot_full" => Some(ActionId::ScreenshotFull),
            "screenrecord" | "screen_record" => Some(ActionId::ScreenRecord),
            "session" => Some(ActionId::Session),
            "lock" => Some(ActionId::Lock),
            "nightlight" | "night_light" => Some(ActionId::NightLight),
            "kitty_new_window" => Some(ActionId::KittyNewWindow { pid: 0 }),
            "kitty_agy" => Some(ActionId::KittyAgy),
            "agy_terminal" | "kitty_launch_agy" | "launch_agy" => Some(ActionId::LaunchAgyTerminal),
            "kitty_clear" => Some(ActionId::KittyClear),
            "kitty_dolphin" => Some(ActionId::KittyDolphin { pid: 0 }),
            "browsertabs" | "browser_tabs" => Some(ActionId::BrowserTabs),
            "browser_new_tab" => Some(ActionId::BrowserNewTab),
            "browser_close_tab" => Some(ActionId::BrowserCloseTab),
            "browser_dup_tab" => Some(ActionId::BrowserDupTab),
            "browser_reopen_tab" => Some(ActionId::BrowserReopenTab),
            "code_git_status" => Some(ActionId::CodeGitStatus),
            "code_terminal" => Some(ActionId::CodeTerminal),
            "code_format" => Some(ActionId::CodeFormat),
            "code_palette" => Some(ActionId::CodePalette),
            "code_run" => Some(ActionId::CodeRun),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubTierType {
    ActiveApps,
    Clipboard,
    AudioSink,
    Scratchpad,
    BrowserTabs,
    FileJump,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SliceItem {
    pub id: String,
    pub label: String,
    pub icon: String, // Material Symbols icon name e.g. "terminal"
    pub has_sub_tier: bool,
    pub sub_tier_type: Option<SubTierType>,
    pub action: Option<ActionId>,
    pub slot_index: usize,
    // Extra fields for file jump
    pub target_path: Option<String>,
    pub target_index: Option<usize>,
    pub is_add_button: bool,
}

impl SliceItem {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        icon: impl Into<String>,
        slot_index: usize,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: icon.into(),
            has_sub_tier: false,
            sub_tier_type: None,
            action: None,
            slot_index,
            target_path: None,
            target_index: None,
            is_add_button: false,
        }
    }

    pub fn scratchpad(in_special: bool, slot_index: usize) -> Self {
        Self {
            id: "scratchpad".to_string(),
            label: if in_special {
                "Move from Scratchpad"
            } else {
                "To Scratchpad"
            }
            .to_string(),
            icon: if in_special {
                "unarchive"
            } else {
                "move_to_inbox"
            }
            .to_string(),
            has_sub_tier: in_special,
            sub_tier_type: if in_special {
                Some(SubTierType::Scratchpad)
            } else {
                None
            },
            action: if in_special {
                None
            } else {
                Some(ActionId::Scratchpad)
            },
            slot_index,
            target_path: None,
            target_index: None,
            is_add_button: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActionDef {
    pub id: &'static str,
    pub label: &'static str,
    pub icon: &'static str,
    pub category: &'static str,
    pub desc: &'static str,
    pub has_sub_tier: bool,
    pub sub_tier_type: Option<SubTierType>,
}

/// Returns the full function catalogue (port of getFunctionCatalogue())
pub fn function_catalogue() -> Vec<ActionDef> {
    vec![
        // Apps & Launchers
        ActionDef {
            id: "terminal",
            label: "Terminal",
            icon: "terminal",
            category: "Apps",
            desc: "Open terminal emulator",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "calc",
            label: "Calculator",
            icon: "calculate",
            category: "Apps",
            desc: "Open calculator app",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "filesearch",
            label: "File Search",
            icon: "search",
            category: "Apps",
            desc: "Search files with launcher (fuzzel/krunner/rofi)",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "dolphin",
            label: "File Manager",
            icon: "folder_open",
            category: "Apps",
            desc: "Open default file manager",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "browser",
            label: "Web Browser",
            icon: "globe",
            category: "Apps",
            desc: "Launch default web browser",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "editor",
            label: "Code Editor",
            icon: "code",
            category: "Apps",
            desc: "Open code editor (Code/Cursor/Neovim)",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "btop",
            label: "Task Manager",
            icon: "monitoring",
            category: "Apps",
            desc: "Open Btop system resource monitor",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "settings",
            label: "Settings",
            icon: "settings",
            category: "Apps",
            desc: "Open system settings",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        // Window Management & Active Apps
        ActionDef {
            id: "active_apps",
            label: "Active Apps",
            icon: "apps",
            category: "Window",
            desc: "Switch to any running application across all workspaces",
            has_sub_tier: true,
            sub_tier_type: Some(SubTierType::ActiveApps),
        },
        // Tools & Features
        ActionDef {
            id: "clipboard",
            label: "Clipboard",
            icon: "content_paste",
            category: "Tools",
            desc: "Quick paste recent clipboard history snippets",
            has_sub_tier: true,
            sub_tier_type: Some(SubTierType::Clipboard),
        },
        ActionDef {
            id: "audio_sink",
            label: "Audio Output",
            icon: "volume_up",
            category: "Media",
            desc: "Switch default audio output device (Headphones/Speakers)",
            has_sub_tier: true,
            sub_tier_type: Some(SubTierType::AudioSink),
        },
        ActionDef {
            id: "color_picker",
            label: "Color Picker",
            icon: "palette",
            category: "Tools",
            desc: "Inspect on-screen color and copy HEX code to clipboard",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "screen_snip",
            label: "Screen Snip",
            icon: "screenshot_region",
            category: "Capture",
            desc: "Interactive area screenshot directly to clipboard",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "screen_ocr",
            label: "Screen OCR",
            icon: "document_scanner",
            category: "Tools",
            desc: "Optical character recognition: grab unselectable text from screen",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        // Code Editor Functions
        ActionDef {
            id: "code_git_status",
            label: "Git Changes",
            icon: "commit",
            category: "Tools",
            desc: "Open Git source control in editor",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "code_terminal",
            label: "Editor Terminal",
            icon: "terminal",
            category: "Tools",
            desc: "Toggle integrated terminal in editor",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "code_format",
            label: "Format Document",
            icon: "format_align_left",
            category: "Tools",
            desc: "Format document with code formatter",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "code_palette",
            label: "Command Palette",
            icon: "terminal",
            category: "Tools",
            desc: "Open editor command palette",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "code_run",
            label: "Run File",
            icon: "play_arrow",
            category: "Tools",
            desc: "Run file without debugging in editor",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        // Media Controls
        ActionDef {
            id: "media_play_pause",
            label: "Play / Pause",
            icon: "play_arrow",
            category: "Media",
            desc: "Toggle media playback",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "media_next",
            label: "Next Track",
            icon: "skip_next",
            category: "Media",
            desc: "Skip to next media track",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "media_prev",
            label: "Previous Track",
            icon: "skip_previous",
            category: "Media",
            desc: "Skip to previous media track",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "media_mute",
            label: "Mute Toggle",
            icon: "volume_off",
            category: "Media",
            desc: "Toggle audio mute",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "volume_up",
            label: "Volume +5%",
            icon: "volume_up",
            category: "Media",
            desc: "Increase master volume",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "volume_down",
            label: "Volume -5%",
            icon: "volume_down",
            category: "Media",
            desc: "Decrease master volume",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "brightness_up",
            label: "Brightness +",
            icon: "brightness_high",
            category: "Tools",
            desc: "Increase display brightness",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "brightness_down",
            label: "Brightness -",
            icon: "brightness_low",
            category: "Tools",
            desc: "Decrease display brightness",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        // Radial Tools
        ActionDef {
            id: "filejump",
            label: "File Jump",
            icon: "drive_file_move",
            category: "Tools",
            desc: "Customizable quick folder jump & drop",
            has_sub_tier: true,
            sub_tier_type: Some(SubTierType::FileJump),
        },
        ActionDef {
            id: "wallpapers",
            label: "Wallpapers",
            icon: "wallpaper",
            category: "Tools",
            desc: "Open wallpaper selector",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "emoji",
            label: "Emoji Picker",
            icon: "sentiment_satisfied",
            category: "Tools",
            desc: "Search and insert emojis",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "scratchpad",
            label: "Scratchpad",
            icon: "move_to_inbox",
            category: "Window",
            desc: "Send or retrieve window from scratchpad",
            has_sub_tier: true,
            sub_tier_type: Some(SubTierType::Scratchpad),
        },
        // Capture
        ActionDef {
            id: "screenshot_area",
            label: "Snip Area",
            icon: "screenshot_region",
            category: "Capture",
            desc: "Capture selected screen region to clipboard",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "screenshot_full",
            label: "Screenshot",
            icon: "fullscreen",
            category: "Capture",
            desc: "Capture entire screen to clipboard",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "screenrecord",
            label: "Screen Record",
            icon: "videocam",
            category: "Capture",
            desc: "Toggle screen recording",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        // Window Management
        ActionDef {
            id: "toggle_float",
            label: "Toggle Float",
            icon: "picture_in_picture_alt",
            category: "Window",
            desc: "Toggle floating mode for active window",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "toggle_fullscreen",
            label: "Fullscreen",
            icon: "fullscreen",
            category: "Window",
            desc: "Toggle fullscreen mode for active window",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "kill_window",
            label: "Close Window",
            icon: "close",
            category: "Window",
            desc: "Close the currently focused window",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "pin_window",
            label: "Pin Window",
            icon: "push_pin",
            category: "Window",
            desc: "Pin window across all workspaces",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        // System & Session
        ActionDef {
            id: "session",
            label: "Session Menu",
            icon: "power_settings_new",
            category: "System",
            desc: "Open power and session dialog",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "lock",
            label: "Lock Screen",
            icon: "lock",
            category: "System",
            desc: "Lock Hyprland session",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "nightlight",
            label: "Night Light",
            icon: "nightlight",
            category: "System",
            desc: "Toggle blue light filter",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        // Context Specific: Terminal
        ActionDef {
            id: "kitty_new_window",
            label: "New Window",
            icon: "open_in_new",
            category: "Apps",
            desc: "Open new terminal in current working directory",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "kitty_agy",
            label: "Run agy",
            icon: "robot_2",
            category: "Tools",
            desc: "Run Antigravity AI coding assistant in terminal",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "agy_terminal",
            label: "Agy Terminal",
            icon: "robot_2",
            category: "Tools",
            desc: "Open Kitty terminal running agy --dangerously-skip-permissions",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "kitty_clear",
            label: "Clear Terminal",
            icon: "mop",
            category: "Tools",
            desc: "Send Ctrl+L to clear active terminal",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "kitty_dolphin",
            label: "Open CWD in Dolphin",
            icon: "folder_open",
            category: "Tools",
            desc: "Open terminal's current directory in file manager",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        // Context Specific: Browser
        ActionDef {
            id: "browsertabs",
            label: "Switch Tab",
            icon: "tabs",
            category: "Apps",
            desc: "Open localized tab switcher petal fan",
            has_sub_tier: true,
            sub_tier_type: Some(SubTierType::BrowserTabs),
        },
        ActionDef {
            id: "browser_new_tab",
            label: "New Tab",
            icon: "tab",
            category: "Apps",
            desc: "Open a new browser tab",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "browser_close_tab",
            label: "Close Tab",
            icon: "tab_close",
            category: "Apps",
            desc: "Close the active browser tab",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "browser_dup_tab",
            label: "Duplicate Tab",
            icon: "tab_duplicate",
            category: "Apps",
            desc: "Duplicate current browser tab",
            has_sub_tier: false,
            sub_tier_type: None,
        },
        ActionDef {
            id: "browser_reopen_tab",
            label: "Reopen Tab",
            icon: "history",
            category: "Apps",
            desc: "Reopen last closed browser tab",
            has_sub_tier: false,
            sub_tier_type: None,
        },
    ]
}

/// Looks up an ActionDef from the catalogue by its string identifier.
pub fn get_action_def(id: &str) -> Option<ActionDef> {
    function_catalogue().into_iter().find(|a| a.id == id)
}

/// Port of resolveSliceItem(fnId, slotIdx, win) from RadialMenuActions.qml lines 1282-1309
pub fn resolve_slice_item(fn_id: &str, slot_idx: usize, in_special: bool) -> SliceItem {
    resolve_slice_item_with_window(fn_id, slot_idx, in_special, None)
}

pub fn resolve_slice_item_with_window(
    fn_id: &str,
    slot_idx: usize,
    in_special: bool,
    window: Option<&crate::ipc::hypr::HyprWindow>,
) -> SliceItem {
    if fn_id == "scratchpad" {
        return SliceItem::scratchpad(in_special, slot_idx);
    }
    let win_pid = window.map(|w| w.pid).unwrap_or(0);
    if let Some(def) = get_action_def(fn_id) {
        let action = if def.has_sub_tier {
            None
        } else {
            match def.id {
                "kitty_dolphin" => Some(ActionId::KittyDolphin { pid: win_pid }),
                "kitty_new_window" => Some(ActionId::KittyNewWindow { pid: win_pid }),
                _ => ActionId::from_id(def.id),
            }
        };
        SliceItem {
            id: def.id.to_string(),
            label: def.label.to_string(),
            icon: def.icon.to_string(),
            has_sub_tier: def.has_sub_tier,
            sub_tier_type: def.sub_tier_type,
            action,
            slot_index: slot_idx,
            target_path: None,
            target_index: None,
            is_add_button: false,
        }
    } else {
        let action = match fn_id {
            "kitty_dolphin" => Some(ActionId::KittyDolphin { pid: win_pid }),
            "kitty_new_window" => Some(ActionId::KittyNewWindow { pid: win_pid }),
            _ => ActionId::from_id(fn_id),
        };
        SliceItem {
            id: fn_id.to_string(),
            label: fn_id.to_string(),
            icon: "extension".to_string(),
            has_sub_tier: false,
            sub_tier_type: None,
            action,
            slot_index: slot_idx,
            target_path: None,
            target_index: None,
            is_add_button: false,
        }
    }
}

/// Execute a bash command detached (port of exec() in RadialMenuActions.qml)
pub fn exec(cmd: &str) {
    let _ = std::process::Command::new("bash")
        .args(["-c", cmd])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

/// Resolve the live current working directory (CWD) of the shell or active foreground process
/// running inside the window associated with `pid`.
pub fn resolve_terminal_cwd(pid: i64) -> std::path::PathBuf {
    let fallback = std::path::PathBuf::from(shellexpand::tilde("~").to_string());

    let effective_pid = if pid > 0 {
        pid
    } else if let Ok(output) = std::process::Command::new("hyprctl")
        .args(["activewindow", "-j"])
        .output()
    {
        if output.status.success() {
            if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                val.get("pid").and_then(|p| p.as_i64()).unwrap_or(0)
            } else {
                0
            }
        } else {
            0
        }
    } else {
        0
    };

    if effective_pid <= 0 {
        return fallback;
    }

    fn get_child_pids(p: i64) -> Vec<i64> {
        let task_children = format!("/proc/{}/task/{}/children", p, p);
        if let Ok(content) = std::fs::read_to_string(&task_children) {
            let pids: Vec<i64> = content
                .split_whitespace()
                .filter_map(|s| s.parse::<i64>().ok())
                .collect();
            if !pids.is_empty() {
                return pids;
            }
        }
        let mut children = Vec::new();
        if let Ok(entries) = std::fs::read_dir("/proc") {
            for entry in entries.flatten() {
                if let Ok(cpid) = entry.file_name().to_string_lossy().parse::<i64>() {
                    let stat_path = format!("/proc/{}/stat", cpid);
                    if let Ok(stat_content) = std::fs::read_to_string(&stat_path) {
                        if let Some(after_paren) = stat_content.rfind(')') {
                            let rest = &stat_content[after_paren + 1..];
                            let parts: Vec<&str> = rest.split_whitespace().collect();
                            if parts.len() > 1 {
                                if let Ok(ppid) = parts[1].parse::<i64>() {
                                    if ppid == p {
                                        children.push(cpid);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        children
    }

    fn is_pts_attached(p: i64) -> bool {
        for fd in 0..=2 {
            let fd_path = format!("/proc/{}/fd/{}", p, fd);
            if let Ok(target) = std::fs::read_link(&fd_path) {
                if let Some(target_str) = target.to_str() {
                    if target_str.starts_with("/dev/pts/") {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn get_tpgid(p: i64) -> i64 {
        let stat_path = format!("/proc/{}/stat", p);
        if let Ok(stat_content) = std::fs::read_to_string(&stat_path) {
            if let Some(after_paren) = stat_content.rfind(')') {
                let rest = &stat_content[after_paren + 1..];
                let parts: Vec<&str> = rest.split_whitespace().collect();
                if parts.len() > 5 {
                    if let Ok(tpgid) = parts[5].parse::<i64>() {
                        return tpgid;
                    }
                }
            }
        }
        -1
    }

    fn get_pid_cwd(p: i64) -> Option<std::path::PathBuf> {
        let cwd_link = format!("/proc/{}/cwd", p);
        if let Ok(target) = std::fs::read_link(&cwd_link) {
            if target.is_dir() {
                return Some(target);
            }
        }
        None
    }

    let mut descendants = Vec::new();
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(effective_pid);
    while let Some(curr) = queue.pop_front() {
        let children = get_child_pids(curr);
        for child in children {
            if !descendants.contains(&child) {
                descendants.push(child);
                queue.push_back(child);
            }
        }
    }

    let mut pts_pids: Vec<i64> = descendants
        .iter()
        .copied()
        .filter(|&p| is_pts_attached(p))
        .collect();

    if is_pts_attached(effective_pid) && !pts_pids.contains(&effective_pid) {
        pts_pids.push(effective_pid);
    }

    // A. Check tpgid (foreground process group) of PTS-attached processes (deepest first)
    for &p in pts_pids.iter().rev() {
        let tpgid = get_tpgid(p);
        if tpgid > 0 {
            if let Some(cwd) = get_pid_cwd(tpgid) {
                return cwd;
            }
        }
    }

    // B. Check cwd of PTS-attached processes themselves (deepest first)
    for &p in pts_pids.iter().rev() {
        if let Some(cwd) = get_pid_cwd(p) {
            return cwd;
        }
    }

    // C. Check cwd of any descendant process (deepest first)
    for &p in descendants.iter().rev() {
        if let Some(cwd) = get_pid_cwd(p) {
            return cwd;
        }
    }

    // D. Check root process itself
    if let Some(cwd) = get_pid_cwd(effective_pid) {
        return cwd;
    }

    fallback
}

pub fn open_terminal_in_dolphin(pid: i64) {
    let cwd = resolve_terminal_cwd(pid);
    let cwd_str = cwd.to_string_lossy().replace('"', "\\\"");
    let cmd = format!("(dolphin \"{}\" || xdg-open \"{}\" || nautilus \"{}\" || thunar \"{}\") &", cwd_str, cwd_str, cwd_str, cwd_str);
    exec(&cmd);
}

pub fn open_terminal_new_window(pid: i64) {
    let cwd = resolve_terminal_cwd(pid);
    let cwd_str = cwd.to_string_lossy().replace('"', "\\\"");
    let cmd = format!("(kitty --directory \"{}\" || alacritty --working-directory \"{}\" || foot -D \"{}\" || kitty || alacritty || foot) &", cwd_str, cwd_str, cwd_str);
    exec(&cmd);
}

/// Get the shell command associated with an ActionId, if any.
pub fn get_command(action: &ActionId) -> Option<String> {
    match action {
        ActionId::Terminal => Some("kitty || alacritty || foot || konsole || xterm &".into()),
        ActionId::Calc => Some("kcalc || qalculate-gtk || gnome-calculator &".into()),
        ActionId::FileSearch => Some("fuzzel || krunner || rofi -show drun &".into()),
        ActionId::Dolphin => Some("dolphin || nautilus || thunar || xdg-open ~ &".into()),
        ActionId::Browser => Some("xdg-open 'https://' &".into()),
        ActionId::Editor => Some("code || cursor || kitty -e nvim || xdg-open ~ &".into()),
        ActionId::Btop => Some("kitty -e btop || alacritty -e btop || foot btop &".into()),
        ActionId::Settings => Some("systemsettings || gnome-control-center &".into()),
        ActionId::ColorPicker => Some("hyprpicker -a &".into()),
        ActionId::ScreenSnip => Some("grim -g \"$(slurp)\" - | wl-copy && notify-send -a 'Radial Menu' 'Screenshot' 'Area copied to clipboard' &".into()),
        ActionId::ScreenOcr => Some("tmp=\"/tmp/ocr_$$\"; grim -g \"$(slurp)\" \"$tmp.png\" && tesseract \"$tmp.png\" \"$tmp\" -l eng 2>/dev/null && cat \"$tmp.txt\" | tr -d '\\f' | wl-copy && rm -f \"$tmp\"* && notify-send -a 'Radial Menu' 'Screen OCR' 'Recognized text copied to clipboard' &".into()),
        ActionId::MediaPlayPause => Some("playerctl play-pause &".into()),
        ActionId::MediaNext => Some("playerctl next &".into()),
        ActionId::MediaPrev => Some("playerctl previous &".into()),
        ActionId::MediaMute => Some("wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle &".into()),
        ActionId::VolumeUp => Some("wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%+ || pactl set-sink-volume @DEFAULT_SINK@ +5%".into()),
        ActionId::VolumeDown => Some("wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%- || pactl set-sink-volume @DEFAULT_SINK@ -5%".into()),
        ActionId::BrightnessUp => Some("brightnessctl s 5%+".into()),
        ActionId::BrightnessDown => Some("brightnessctl s 5%-".into()),
        ActionId::ScreenshotArea => Some("grimblast --freeze copy area || hyprshot -m region --clipboard-only &".into()),
        ActionId::ScreenshotFull => Some("grimblast copy output || hyprshot -m output --clipboard-only &".into()),
        ActionId::ScreenRecord => Some("pkill -SIGINT wf-recorder || wf-recorder -g \"$(slurp)\" -f ~/Videos/recording_$(date +%s).mp4 &".into()),
        ActionId::ToggleFloat => Some("hyprctl dispatch 'hl.dsp.window.float()' 2>/dev/null || hyprctl dispatch togglefloating".into()),
        ActionId::ToggleFullscreen => Some("hyprctl dispatch 'hl.dsp.window.fullscreen()' 2>/dev/null || hyprctl dispatch fullscreen 1".into()),
        ActionId::KillWindow => Some("hyprctl dispatch 'hl.dsp.window.kill()' 2>/dev/null || hyprctl dispatch killactive".into()),
        ActionId::PinWindow => Some("hyprctl dispatch 'hl.dsp.window.pin()' 2>/dev/null || hyprctl dispatch pin".into()),
        ActionId::Lock => Some("loginctl lock-session || hyprlock &".into()),
        ActionId::NightLight => Some("pkill gammastep || gammastep -O 4000 || hyprsunset &".into()),
        ActionId::Session => Some("hyprctl dispatch \"hl.dsp.global('quickshell:sessionToggle')\" &".into()),
        ActionId::Wallpapers => Some("hyprctl dispatch \"hl.dsp.global('quickshell:wallpaperSelectorToggle')\" &".into()),
        ActionId::Emoji => Some("rofimoji || fuzzel &".into()),
        ActionId::CodeGitStatus => Some("wtype -M ctrl -M shift -k g -m shift -m ctrl &".into()),
        ActionId::CodeTerminal => Some("wtype -M ctrl -k grave -m ctrl &".into()),
        ActionId::CodeFormat => Some("wtype -M shift -M alt -k f -m alt -m shift &".into()),
        ActionId::CodePalette => Some("wtype -M ctrl -M shift -k p -m shift -m ctrl &".into()),
        ActionId::CodeRun => Some("wtype -M ctrl -k F5 -m ctrl &".into()),
        ActionId::BrowserNewTab => Some("wtype -M ctrl -k t -m ctrl &".into()),
        ActionId::BrowserCloseTab => Some("wtype -M ctrl -k w -m ctrl &".into()),
        ActionId::BrowserDupTab => Some("wtype -M alt -k d -m alt && sleep 0.06 && wtype -M alt -k Return -m alt &".into()),
        ActionId::BrowserReopenTab => Some("wtype -M ctrl -M shift -k t -m shift -m ctrl &".into()),
        ActionId::KittyNewWindow { pid } => {
            let cwd = resolve_terminal_cwd(*pid);
            let cwd_str = cwd.to_string_lossy().replace('"', "\\\"");
            Some(format!(
                "(kitty --directory \"{}\" || alacritty --working-directory \"{}\" || foot -D \"{}\" || kitty || alacritty || foot) &",
                cwd_str, cwd_str, cwd_str
            ))
        }
        ActionId::KittyAgy => Some("sleep 0.05 && wtype 'agy --dangerously-skip-permissions' -k Return".into()),
        ActionId::LaunchAgyTerminal => Some("kitty & sleep 0.35 && wtype 'agy --dangerously-skip-permissions' -k Return".into()),
        ActionId::KittyClear => Some("sleep 0.05 && wtype -M ctrl -k l -m ctrl".into()),
        ActionId::KittyDolphin { pid } => {
            let cwd = resolve_terminal_cwd(*pid);
            let cwd_str = cwd.to_string_lossy().replace('"', "\\\"");
            Some(format!(
                "(dolphin \"{}\" || xdg-open \"{}\" || nautilus \"{}\" || thunar \"{}\") &",
                cwd_str, cwd_str, cwd_str, cwd_str
            ))
        }
        ActionId::FocusWindow { address, workspace } => {
            let ws_dispatch = if !workspace.is_empty() {
                if workspace.starts_with("special:") {
                    format!("hyprctl dispatch 'hl.dsp.focus({{ workspace = \"{}\" }})'; ", workspace)
                } else if let Ok(num) = workspace.parse::<i32>() {
                    format!("hyprctl dispatch 'hl.dsp.focus({{ workspace = {} }})'; ", num)
                } else {
                    format!("hyprctl dispatch 'hl.dsp.focus({{ workspace = \"{}\" }})'; ", workspace)
                }
            } else {
                String::new()
            };
            Some(format!(
                "{}hyprctl dispatch 'hl.dsp.focus({{ window = \"address:{}\" }})' || hyprctl dispatch focuswindow address:{}",
                ws_dispatch, address, address
            ))
        }
        ActionId::MoveToWorkspace(n) => Some(format!(
            "hyprctl dispatch \"hl.dsp.window.move({{ workspace = {} }})\" 2>/dev/null || hyprctl dispatch movetoworkspace {}",
            n, n
        )),
        ActionId::SwitchToTab(idx) => {
            if *idx >= 1 && *idx <= 8 {
                Some(format!("wtype -M alt -k {} -m alt", idx))
            } else {
                Some("wtype -M alt -k 1 -m alt".into())
            }
        },
        ActionId::PasteClip(id) => Some(format!("cliphist decode {} | wl-copy && sleep 0.05 && wtype -M ctrl -k v -m ctrl", id)),
        ActionId::SetSink(id) => Some(format!("wpctl set-default {}", id)),
        ActionId::JumpToFile(path) => {
            let exp = shellexpand::tilde(path).to_string();
            Some(format!("(dolphin \"{}\" || xdg-open \"{}\") &", exp, exp))
        },
        ActionId::Scratchpad => Some("hyprctl dispatch \"hl.dsp.window.move({ workspace = 'special:special' })\" 2>/dev/null || hyprctl dispatch movetoworkspace special:special".into()),
        _ => None,
    }
}

/// Execute an action. Uses exact same shell commands as the original QML.
pub fn execute(action: &ActionId) {
    match action {
        ActionId::JumpToFile(path) => {
            perform_file_jump(path);
        }
        ActionId::KittyDolphin { pid } => {
            open_terminal_in_dolphin(*pid);
        }
        ActionId::KittyNewWindow { pid } => {
            open_terminal_new_window(*pid);
        }
        _ => {
            if let Some(cmd) = get_command(action) {
                exec(&cmd);
            }
        }
    }
}

/// If target destination is on an unmounted drive/partition, auto-mount via udisksctl
pub fn auto_mount_if_needed(target_path: &str) -> String {
    let expanded = shellexpand::tilde(target_path).to_string();
    let p = std::path::Path::new(&expanded);
    if p.exists() {
        return expanded;
    }

    // Attempt auto-mounting via lsblk -J
    if let Ok(output) = std::process::Command::new("lsblk")
        .args(["-J", "-o", "NAME,LABEL,UUID,MOUNTPOINTS,FSTYPE"])
        .output()
    {
        if output.status.success() {
            if let Ok(data) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                let mut devices = Vec::new();
                fn collect_devs(val: &serde_json::Value, out: &mut Vec<serde_json::Value>) {
                    if let Some(arr) = val.as_array() {
                        for item in arr {
                            out.push(item.clone());
                            if let Some(children) = item.get("children") {
                                collect_devs(children, out);
                            }
                        }
                    }
                }
                if let Some(blockdevices) = data.get("blockdevices") {
                    collect_devs(blockdevices, &mut devices);
                }

                for dev in devices {
                    let name = dev.get("name").and_then(|n| n.as_str());
                    let label = dev.get("label").and_then(|l| l.as_str());
                    let uuid = dev.get("uuid").and_then(|u| u.as_str());

                    let matches = match (label, uuid) {
                        (Some(l), _) if !l.is_empty() && expanded.contains(l) => true,
                        (_, Some(u)) if !u.is_empty() && expanded.contains(u) => true,
                        _ => false,
                    };

                    if matches {
                        if let Some(dev_name) = name {
                            let is_mounted = dev
                                .get("mountpoints")
                                .and_then(|m| m.as_array())
                                .map(|arr| {
                                    arr.iter().any(|v| {
                                        v.as_str()
                                            .map(|s| !s.is_empty() && !s.starts_with('['))
                                            .unwrap_or(false)
                                    })
                                })
                                .unwrap_or(false);

                            if !is_mounted {
                                let dev_node = format!("/dev/{}", dev_name);
                                log::info!(
                                    "Auto-mounting block device {} for path {}",
                                    dev_node,
                                    expanded
                                );
                                let _ = std::process::Command::new("udisksctl")
                                    .args(["mount", "-b", &dev_node])
                                    .output();
                                std::thread::sleep(std::time::Duration::from_millis(200));
                            }
                        }
                        break;
                    }
                }
            }
        }
    }

    expanded
}

/// Universal File Jump handler matching QML performFileJump:
/// 1. Auto-mount removable/unmounted drives via udisksctl if needed
/// 2. If clipboard has file URIs, copy them to destination and notify
/// 3. Open directory in file manager (dolphin, fallback xdg-open)
pub fn perform_file_jump(path: &str) {
    let resolved_path = auto_mount_if_needed(path);
    let p = std::path::Path::new(&resolved_path);

    // If destination parent exists, create destination directory if it doesn't exist yet
    if !p.exists() {
        if let Some(parent) = p.parent() {
            if parent.is_dir() {
                let _ = std::fs::create_dir_all(p);
            }
        }
    }

    // Check clipboard for copied files/URIs
    if let Ok(output) = std::process::Command::new("wl-paste")
        .args(["-t", "text/uri-list"])
        .output()
    {
        if output.status.success() {
            let uris_str = String::from_utf8_lossy(&output.stdout);
            let mut copied = 0;
            let dest_dir = std::path::Path::new(&resolved_path);
            if dest_dir.is_dir() {
                for line in uris_str.lines() {
                    let line = line.trim();
                    if let Some(encoded) = line.strip_prefix("file://") {
                        let decoded = decode_percent(encoded);
                        let src_path = std::path::Path::new(&decoded);
                        if src_path.exists() {
                            if let Some(file_name) = src_path.file_name() {
                                let target_file = dest_dir.join(file_name);
                                if src_path.is_dir() {
                                    if copy_dir_all(src_path, &target_file).is_ok() {
                                        copied += 1;
                                    }
                                } else if std::fs::copy(src_path, &target_file).is_ok() {
                                    copied += 1;
                                }
                            }
                        }
                    }
                }
            }
            if copied > 0 {
                let _ = std::process::Command::new("notify-send")
                    .args([
                        "-a",
                        "Radial Menu",
                        "File Jump",
                        &format!("Copied {} item(s) to {}", copied, resolved_path),
                    ])
                    .spawn();
            }
        }
    }

    let target = if std::path::Path::new(&resolved_path).exists() {
        resolved_path.as_str()
    } else if let Some(parent) = std::path::Path::new(&resolved_path).parent() {
        if parent.exists() {
            parent.to_str().unwrap_or(&resolved_path)
        } else {
            "~"
        }
    } else {
        "~"
    };

    let cmd = format!("(dolphin \"{}\" || xdg-open \"{}\") &", target, target);
    exec(&cmd);
}

fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}

fn decode_percent(s: &str) -> String {
    let mut bytes = Vec::new();
    let mut chars = s.bytes().peekable();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let h1 = chars.next();
            let h2 = chars.next();
            if let (Some(h1), Some(h2)) = (h1, h2) {
                let hex_str = [h1, h2];
                if let Ok(byte) = u8::from_str_radix(std::str::from_utf8(&hex_str).unwrap_or(""), 16) {
                    bytes.push(byte);
                    continue;
                }
            }
        }
        bytes.push(b);
    }
    String::from_utf8(bytes).unwrap_or_else(|_| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_catalogue_not_empty() {
        let cat = function_catalogue();
        assert!(cat.len() >= 30, "expected at least 30 actions, got {}", cat.len());
        assert!(cat.iter().any(|a| a.id == "terminal"));
        assert!(cat.iter().any(|a| a.id == "clipboard"));
        assert!(cat.iter().any(|a| a.id == "active_apps"));
        assert!(cat.iter().any(|a| a.id == "kitty_dolphin"));
    }

    #[test]
    fn test_action_id_from_id() {
        assert_eq!(ActionId::from_id("terminal"), Some(ActionId::Terminal));
        assert_eq!(ActionId::from_id("calc"), Some(ActionId::Calc));
        assert_eq!(ActionId::from_id("filesearch"), Some(ActionId::FileSearch));
        assert_eq!(ActionId::from_id("file_search"), Some(ActionId::FileSearch));
        assert_eq!(ActionId::from_id("dolphin"), Some(ActionId::Dolphin));
        assert_eq!(ActionId::from_id("browser"), Some(ActionId::Browser));
        assert_eq!(ActionId::from_id("editor"), Some(ActionId::Editor));
        assert_eq!(ActionId::from_id("btop"), Some(ActionId::Btop));
        assert_eq!(ActionId::from_id("settings"), Some(ActionId::Settings));
        assert_eq!(ActionId::from_id("active_apps"), Some(ActionId::ActiveApps));
        assert_eq!(ActionId::from_id("toggle_float"), Some(ActionId::ToggleFloat));
        assert_eq!(ActionId::from_id("toggle_fullscreen"), Some(ActionId::ToggleFullscreen));
        assert_eq!(ActionId::from_id("kill_window"), Some(ActionId::KillWindow));
        assert_eq!(ActionId::from_id("pin_window"), Some(ActionId::PinWindow));
        assert_eq!(ActionId::from_id("scratchpad"), Some(ActionId::Scratchpad));
        assert_eq!(ActionId::from_id("clipboard"), Some(ActionId::Clipboard));
        assert_eq!(ActionId::from_id("audio_sink"), Some(ActionId::AudioSink));
        assert_eq!(ActionId::from_id("color_picker"), Some(ActionId::ColorPicker));
        assert_eq!(ActionId::from_id("screen_snip"), Some(ActionId::ScreenSnip));
        assert_eq!(ActionId::from_id("screen_ocr"), Some(ActionId::ScreenOcr));
        assert_eq!(ActionId::from_id("filejump"), Some(ActionId::FileJump));
        assert_eq!(ActionId::from_id("wallpapers"), Some(ActionId::Wallpapers));
        assert_eq!(ActionId::from_id("emoji"), Some(ActionId::Emoji));
        assert_eq!(ActionId::from_id("media_play_pause"), Some(ActionId::MediaPlayPause));
        assert_eq!(ActionId::from_id("media_next"), Some(ActionId::MediaNext));
        assert_eq!(ActionId::from_id("media_prev"), Some(ActionId::MediaPrev));
        assert_eq!(ActionId::from_id("media_mute"), Some(ActionId::MediaMute));
        assert_eq!(ActionId::from_id("volume_up"), Some(ActionId::VolumeUp));
        assert_eq!(ActionId::from_id("volume_down"), Some(ActionId::VolumeDown));
        assert_eq!(ActionId::from_id("brightness_up"), Some(ActionId::BrightnessUp));
        assert_eq!(ActionId::from_id("brightness_down"), Some(ActionId::BrightnessDown));
        assert_eq!(ActionId::from_id("screenshot_area"), Some(ActionId::ScreenshotArea));
        assert_eq!(ActionId::from_id("screenshot_full"), Some(ActionId::ScreenshotFull));
        assert_eq!(ActionId::from_id("screenrecord"), Some(ActionId::ScreenRecord));
        assert_eq!(ActionId::from_id("session"), Some(ActionId::Session));
        assert_eq!(ActionId::from_id("lock"), Some(ActionId::Lock));
        assert_eq!(ActionId::from_id("nightlight"), Some(ActionId::NightLight));
        assert_eq!(ActionId::from_id("kitty_new_window"), Some(ActionId::KittyNewWindow { pid: 0 }));
        assert_eq!(ActionId::from_id("kitty_agy"), Some(ActionId::KittyAgy));
        assert_eq!(ActionId::from_id("agy_terminal"), Some(ActionId::LaunchAgyTerminal));
        assert_eq!(ActionId::from_id("kitty_clear"), Some(ActionId::KittyClear));
        assert_eq!(ActionId::from_id("kitty_dolphin"), Some(ActionId::KittyDolphin { pid: 0 }));
        assert_eq!(ActionId::from_id("browsertabs"), Some(ActionId::BrowserTabs));
        assert_eq!(ActionId::from_id("browser_new_tab"), Some(ActionId::BrowserNewTab));
        assert_eq!(ActionId::from_id("browser_close_tab"), Some(ActionId::BrowserCloseTab));
        assert_eq!(ActionId::from_id("browser_dup_tab"), Some(ActionId::BrowserDupTab));
        assert_eq!(ActionId::from_id("browser_reopen_tab"), Some(ActionId::BrowserReopenTab));
        assert_eq!(ActionId::from_id("code_git_status"), Some(ActionId::CodeGitStatus));
        assert_eq!(ActionId::from_id("code_terminal"), Some(ActionId::CodeTerminal));
        assert_eq!(ActionId::from_id("code_format"), Some(ActionId::CodeFormat));
        assert_eq!(ActionId::from_id("code_palette"), Some(ActionId::CodePalette));
        assert_eq!(ActionId::from_id("code_run"), Some(ActionId::CodeRun));
        assert_eq!(ActionId::from_id("non_existent_action"), None);
    }

    #[test]
    fn test_execute_commands_defined() {
        assert!(get_command(&ActionId::Terminal).is_some());
        assert!(get_command(&ActionId::Calc).is_some());
        assert!(get_command(&ActionId::FileSearch).is_some());
        assert!(get_command(&ActionId::Dolphin).is_some());
        assert!(get_command(&ActionId::Browser).is_some());
        assert!(get_command(&ActionId::Editor).is_some());
        assert!(get_command(&ActionId::Btop).is_some());
        assert!(get_command(&ActionId::Settings).is_some());
        assert!(get_command(&ActionId::ColorPicker).is_some());
        assert!(get_command(&ActionId::ScreenSnip).is_some());
        assert!(get_command(&ActionId::ScreenOcr).is_some());
        assert!(get_command(&ActionId::MediaPlayPause).is_some());
        assert!(get_command(&ActionId::MediaNext).is_some());
        assert!(get_command(&ActionId::MediaPrev).is_some());
        assert!(get_command(&ActionId::MediaMute).is_some());
        assert!(get_command(&ActionId::VolumeUp).is_some());
        assert!(get_command(&ActionId::VolumeDown).is_some());
        assert!(get_command(&ActionId::BrightnessUp).is_some());
        assert!(get_command(&ActionId::BrightnessDown).is_some());
        assert!(get_command(&ActionId::ScreenshotArea).is_some());
        assert!(get_command(&ActionId::ScreenshotFull).is_some());
        assert!(get_command(&ActionId::ScreenRecord).is_some());
        assert!(get_command(&ActionId::ToggleFloat).is_some());
        assert!(get_command(&ActionId::ToggleFullscreen).is_some());
        assert!(get_command(&ActionId::KillWindow).is_some());
        assert!(get_command(&ActionId::PinWindow).is_some());
        assert!(get_command(&ActionId::Lock).is_some());
        assert!(get_command(&ActionId::NightLight).is_some());
        assert!(get_command(&ActionId::Session).is_some());
        assert!(get_command(&ActionId::Wallpapers).is_some());
        assert!(get_command(&ActionId::Emoji).is_some());
        assert!(get_command(&ActionId::CodeGitStatus).is_some());
        assert!(get_command(&ActionId::CodeTerminal).is_some());
        assert!(get_command(&ActionId::CodeFormat).is_some());
        assert!(get_command(&ActionId::CodePalette).is_some());
        assert!(get_command(&ActionId::CodeRun).is_some());
        assert!(get_command(&ActionId::BrowserNewTab).is_some());
        assert!(get_command(&ActionId::BrowserCloseTab).is_some());
        assert!(get_command(&ActionId::BrowserDupTab).is_some());
        assert!(get_command(&ActionId::BrowserReopenTab).is_some());
        assert!(get_command(&ActionId::KittyNewWindow { pid: 0 }).is_some());
        assert!(get_command(&ActionId::KittyAgy).is_some());
        assert!(get_command(&ActionId::LaunchAgyTerminal).is_some());
        assert!(get_command(&ActionId::KittyClear).is_some());
        assert!(get_command(&ActionId::KittyDolphin { pid: 0 }).is_some());
        assert!(get_command(&ActionId::Scratchpad).is_some());

        // Dynamic actions
        let focus = ActionId::FocusWindow {
            address: "0x1234abcd".into(),
            workspace: "1".into(),
        };
        assert_eq!(
            get_command(&focus),
            Some("hyprctl dispatch 'hl.dsp.focus({ workspace = 1 })'; hyprctl dispatch 'hl.dsp.focus({ window = \"address:0x1234abcd\" })' || hyprctl dispatch focuswindow address:0x1234abcd".into())
        );

        let move_ws = ActionId::MoveToWorkspace(3);
        assert_eq!(
            get_command(&move_ws),
            Some("hyprctl dispatch \"hl.dsp.window.move({ workspace = 3 })\" 2>/dev/null || hyprctl dispatch movetoworkspace 3".into())
        );

        let tab3 = ActionId::SwitchToTab(3);
        assert_eq!(
            get_command(&tab3),
            Some("wtype -M alt -k 3 -m alt".into())
        );

        let tab9 = ActionId::SwitchToTab(9);
        assert_eq!(
            get_command(&tab9),
            Some("wtype -M alt -k 1 -m alt".into())
        );

        let paste = ActionId::PasteClip("42".into());
        assert_eq!(
            get_command(&paste),
            Some("cliphist decode 42 | wl-copy && sleep 0.05 && wtype -M ctrl -k v -m ctrl".into())
        );

        let sink = ActionId::SetSink("55".into());
        assert_eq!(
            get_command(&sink),
            Some("wpctl set-default 55".into())
        );

        let jump = ActionId::JumpToFile("/home/astro/Downloads".into());
        assert_eq!(
            get_command(&jump),
            Some("(dolphin \"/home/astro/Downloads\" || xdg-open \"/home/astro/Downloads\") &".into())
        );
    }

    #[test]
    fn test_resolve_slice_item() {
        let item = resolve_slice_item("terminal", 2, false);
        assert_eq!(item.id, "terminal");
        assert_eq!(item.label, "Terminal");
        assert_eq!(item.icon, "terminal");
        assert_eq!(item.slot_index, 2);
        assert!(!item.has_sub_tier);
        assert_eq!(item.action, Some(ActionId::Terminal));

        let custom = resolve_slice_item("custom_app", 5, false);
        assert_eq!(custom.id, "custom_app");
        assert_eq!(custom.icon, "extension");
        assert_eq!(custom.slot_index, 5);
    }

    #[test]
    fn test_scratchpad_slice() {
        let normal = resolve_slice_item("scratchpad", 0, false);
        assert_eq!(normal.label, "To Scratchpad");
        assert_eq!(normal.icon, "move_to_inbox");
        assert!(!normal.has_sub_tier);
        assert_eq!(normal.action, Some(ActionId::Scratchpad));

        let special = resolve_slice_item("scratchpad", 0, true);
        assert_eq!(special.label, "Move from Scratchpad");
        assert_eq!(special.icon, "unarchive");
        assert!(special.has_sub_tier);
        assert_eq!(special.sub_tier_type, Some(SubTierType::Scratchpad));
        assert_eq!(special.action, None);
    }

    #[test]
    fn test_decode_percent() {
        assert_eq!(decode_percent("hello%20world"), "hello world");
        assert_eq!(decode_percent("normal_path/file.txt"), "normal_path/file.txt");
        assert_eq!(decode_percent("%2Fhome%2Fuser"), "/home/user");
    }

    #[test]
    fn test_auto_mount_existing_path() {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        let res = auto_mount_if_needed(&home);
        assert_eq!(res, home);
    }

    #[test]
    fn test_resolve_terminal_cwd() {
        let current_pid = std::process::id() as i64;
        let cwd = resolve_terminal_cwd(current_pid);
        assert!(cwd.is_dir());
        let fallback_cwd = resolve_terminal_cwd(999_999_999);
        assert!(fallback_cwd.is_dir());
    }
}
