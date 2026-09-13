// src/state/menu.rs

use crate::ipc::hypr::{HyprClient, HyprContext, HyprWindow};
use crate::state::actions::{ActionId, SliceItem, SubTierType};
use crate::state::anim::AnimState;
use crate::state::config::RadialConfig;
use crate::state::context::{is_in_special_workspace, resolve_context, Context};
use crate::state::drag::DragState;

#[derive(Debug, Clone, PartialEq)]
pub enum MenuPhase {
    Hidden,
    Opening,
    Open,
    ClosingAnimated { pending_action: Option<ActionId> },
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, Default, PartialEq)]
pub struct BrowserTab {
    #[serde(default)]
    pub index: usize,
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MenuState {
    pub phase: MenuPhase,
    pub center_x: f32,
    pub center_y: f32,
    pub context: Context,
    pub window_info: HyprWindow,
    /// Raw PID from j/activewindow at the time the menu was opened.
    /// Used as the authoritative pid for terminal actions (e.g., Open CWD in Dolphin)
    /// even when the cursor was on empty space and window_info.pid is 0.
    pub active_window_pid: i64,
    /// Raw title from j/activewindow at the time the menu was opened.
    pub active_window_title: String,
    pub current_slices: Vec<SliceItem>,
    pub browser_tabs: Vec<BrowserTab>,
    pub active_clients: Vec<HyprClient>,
    pub hovered_index: i32,
    pub center_hovered: bool,
    pub outer_hovered_index: i32,
    pub active_sub_tier: Option<SubTierType>,
    pub parent_slice_index: i32,
    pub sub_slices: Vec<SliceItem>,
    pub drag: DragState,
    pub flick_mode_armed: bool,
    pub cursor_moved_flick: bool,
    pub flick_open_time: std::time::Instant,
    pub anim: AnimState,
    pub is_low_end_gpu: bool,
    pub is_special_workspace: bool,
    pub config: RadialConfig,
}

impl Default for MenuState {
    fn default() -> Self {
        Self::new(RadialConfig::default(), false)
    }
}

/// Helper function to resolve icon name from application class
pub fn resolve_app_icon(app_class: &str) -> &'static str {
    let c = app_class.to_lowercase();
    if c.contains("firefox") || c.contains("zen") || c.contains("chrome") || c.contains("chromium") || c.contains("brave") || c.contains("browser") {
        return "globe";
    }
    if c.contains("kitty") || c.contains("terminal") || c.contains("alacritty") || c.contains("foot") || c.contains("konsole") || c.contains("xterm") {
        return "terminal";
    }
    if c.contains("code") || c.contains("cursor") || c.contains("antigravity") || c.contains("nvim") || c.contains("studio") || c.contains("dev") {
        return "code";
    }
    if c.contains("kate") || c.contains("gedit") || c.contains("sublime") || c.contains("text") || c.contains("note") {
        return "edit_note";
    }
    if c.contains("monitor") || c.contains("task") || c.contains("btop") || c.contains("htop") {
        return "monitoring";
    }
    if c.contains("discord") || c.contains("vesktop") || c.contains("telegram") || c.contains("slack") || c.contains("whatsapp") {
        return "chat";
    }
    if c.contains("spotify") || c.contains("music") || c.contains("rhythmbox") {
        return "music_note";
    }
    if c.contains("dolphin") || c.contains("nautilus") || c.contains("thunar") || c.contains("nemo") || c.contains("file") {
        return "folder";
    }
    if c.contains("mpv") || c.contains("vlc") || c.contains("video") || c.contains("media") {
        return "movie";
    }
    if c.contains("calc") || c.contains("calculator") || c.contains("kcalc") {
        return "calculate";
    }
    if c.contains("settings") || c.contains("control") {
        return "settings";
    }
    "window"
}

impl MenuState {
    pub fn new(config: RadialConfig, is_low_end_gpu: bool) -> Self {
        Self {
            phase: MenuPhase::Hidden,
            center_x: 0.0,
            center_y: 0.0,
            context: Context::Default,
            window_info: HyprWindow::default(),
            active_window_pid: 0,
            active_window_title: String::new(),
            current_slices: Vec::new(),
            browser_tabs: Vec::new(),
            active_clients: Vec::new(),
            hovered_index: -1,
            center_hovered: false,
            outer_hovered_index: -1,
            active_sub_tier: None,
            parent_slice_index: -1,
            sub_slices: Vec::new(),
            drag: DragState::default(),
            flick_mode_armed: false,
            cursor_moved_flick: false,
            flick_open_time: std::time::Instant::now(),
            anim: AnimState::default(),
            is_low_end_gpu,
            is_special_workspace: false,
            config,
        }
    }

    /// Angle span for each main slice in degrees
    pub fn slice_angle(&self) -> f32 {
        360.0 / self.current_slices.len().max(1) as f32
    }

    /// Localized angle span for each sub-slice in degrees
    pub fn sub_slice_width(&self) -> f32 {
        let count = self.sub_slices.len() as f32;
        if count > 0.0 {
            (110.0 / count).clamp(22.0, 36.0)
        } else {
            28.0
        }
    }

    /// Mid-angle of parent slice in degrees
    pub fn parent_mid_angle(&self) -> f32 {
        if self.parent_slice_index >= 0 {
            (self.parent_slice_index as f32 + 0.5) * self.slice_angle() - 90.0
        } else {
            0.0
        }
    }

    /// Total span of sub-slices in degrees
    pub fn sub_total_span(&self) -> f32 {
        self.sub_slices.len() as f32 * self.sub_slice_width()
    }

    /// Starting angle of sub-slice fan in degrees
    pub fn sub_start_angle(&self) -> f32 {
        self.parent_mid_angle() - self.sub_total_span() / 2.0
    }

    /// Transition to Opening phase, resolving context and loading active slices
    pub fn transition_open(&mut self, ctx: HyprContext) {
        self.context = resolve_context(&ctx.window.class, &ctx.window.title);
        self.center_x = ctx.cursor.x;
        self.center_y = ctx.cursor.y;
        self.window_info = ctx.window;
        // Use active_window_pid from context (raw j/activewindow pid) as the authoritative
        // terminal pid, falling back to window_info.pid if not set.
        self.active_window_pid = if ctx.active_window_pid > 0 {
            ctx.active_window_pid
        } else {
            self.window_info.pid
        };
        self.active_window_title = if !ctx.active_window_title.is_empty() {
            ctx.active_window_title
        } else {
            self.window_info.title.clone()
        };
        self.active_clients = ctx.clients;

        self.is_special_workspace = ctx.is_special_workspace
            || is_in_special_workspace(
                self.window_info.workspace.id,
                &self.window_info.workspace.name,
            );
        let in_special = self.is_special_workspace;
        let slice_ids = self.config.get_active_slice_ids(&self.context.to_string());
        // Use active_window_pid and active_window_title for terminal CWD actions
        // (more reliable than window_info which may be empty when cursor was on empty space)
        let effective_win_pid = self.active_window_pid;
        let mut effective_win = self.window_info.clone();
        if effective_win.pid == 0 && effective_win_pid > 0 {
            effective_win.pid = effective_win_pid;
        }
        if effective_win.title.is_empty() && !self.active_window_title.is_empty() {
            effective_win.title = self.active_window_title.clone();
        }
        self.current_slices = slice_ids
            .iter()
            .enumerate()
            .map(|(idx, id)| {
                crate::state::actions::resolve_slice_item_with_window(
                    id,
                    idx,
                    in_special,
                    Some(&effective_win),
                )
            })
            .collect();

        self.phase = MenuPhase::Opening;
        self.flick_mode_armed = true;
        self.flick_open_time = std::time::Instant::now();
        self.cursor_moved_flick = false;

        self.hovered_index = -1;
        self.center_hovered = false;
        self.outer_hovered_index = -1;
        self.active_sub_tier = None;
        self.parent_slice_index = -1;
        self.sub_slices.clear();
        self.drag.reset();

        self.anim = AnimState::default();
        self.anim.overall_scale = 1.0;
        self.anim.overall_opacity = 1.0;
    }

    /// Transition to ClosingAnimated phase with optional pending action
    pub fn transition_close_animated(&mut self, action: Option<ActionId>) {
        self.phase = MenuPhase::ClosingAnimated {
            pending_action: action,
        };
        self.flick_mode_armed = false;
    }

    /// Immediately close menu without animations
    pub fn transition_close_immediate(&mut self) {
        self.phase = MenuPhase::Hidden;
        self.flick_mode_armed = false;
        self.active_sub_tier = None;
        self.parent_slice_index = -1;
        self.sub_slices.clear();
        self.hovered_index = -1;
        self.center_hovered = false;
        self.outer_hovered_index = -1;
        self.drag.reset();
        self.is_special_workspace = false;
    }

    /// Open concentric sub-tier for the specified parent slice index
    pub fn open_sub_tier(&mut self, parent_index: i32, tier: SubTierType) {
        self.active_sub_tier = Some(tier);
        self.parent_slice_index = parent_index;
        self.sub_slices = self.generate_sub_slices(tier);
        self.outer_hovered_index = -1;
        self.anim.sub_reveal_progress = 0.0;
    }

    /// Close currently active sub-tier
    pub fn close_sub_tier(&mut self) {
        self.active_sub_tier = None;
        self.parent_slice_index = -1;
        self.sub_slices.clear();
        self.outer_hovered_index = -1;
        self.anim.sub_reveal_progress = 0.0;
    }

    /// Toggle sub-tier: close if already open on same slice, otherwise open
    pub fn toggle_sub_tier(&mut self, parent_index: i32, tier: SubTierType) {
        if self.active_sub_tier == Some(tier) && self.parent_slice_index == parent_index {
            self.close_sub_tier();
        } else {
            self.open_sub_tier(parent_index, tier);
        }
    }

    /// Re-resolves and updates current slices from config for the active context
    pub fn refresh_current_slices(&mut self) {
        let in_special = self.is_special_workspace
            || is_in_special_workspace(
                self.window_info.workspace.id,
                &self.window_info.workspace.name,
            );
        let slice_ids = self.config.get_active_slice_ids(&self.context.to_string());
        let effective_win_pid = self.active_window_pid;
        let mut effective_win = self.window_info.clone();
        if effective_win.pid == 0 && effective_win_pid > 0 {
            effective_win.pid = effective_win_pid;
        }
        if effective_win.title.is_empty() && !self.active_window_title.is_empty() {
            effective_win.title = self.active_window_title.clone();
        }
        self.current_slices = slice_ids
            .iter()
            .enumerate()
            .map(|(idx, id)| {
                crate::state::actions::resolve_slice_item_with_window(
                    id,
                    idx,
                    in_special,
                    Some(&effective_win),
                )
            })
            .collect();
    }

    /// Re-generates sub slices for the active sub tier if open
    pub fn refresh_sub_slices(&mut self) {
        if let Some(tier) = self.active_sub_tier {
            self.sub_slices = self.generate_sub_slices(tier);
        }
    }

    /// Handles Escape key: closes sub-tier if open (returns false), or closes menu (returns true)
    pub fn handle_escape(&mut self) -> bool {
        if self.active_sub_tier.is_some() {
            self.close_sub_tier();
            false
        } else {
            self.transition_close_animated(None);
            true
        }
    }

    /// Handles 1-based number key (1..9): triggers corresponding slice or sub-tier
    pub fn handle_number_key(&mut self, num: usize) -> Option<ActionId> {
        let target_idx = num.checked_sub(1)?;

        // Case A: Sub-dial is currently open
        if self.active_sub_tier.is_some()
            && !self.sub_slices.is_empty()
            && self.parent_slice_index >= 0
        {
            if target_idx < self.sub_slices.len() {
                let sub = &self.sub_slices[target_idx];
                if let Some(action) = &sub.action {
                    self.outer_hovered_index = target_idx as i32;
                    let act = action.clone();
                    self.transition_close_animated(Some(act.clone()));
                    return Some(act);
                }
            }
            return None;
        }

        // Case B: Main wheel is active
        if target_idx < self.current_slices.len() {
            let slice = &self.current_slices[target_idx];
            if slice.has_sub_tier {
                if let Some(tier) = slice.sub_tier_type {
                    self.hovered_index = target_idx as i32;
                    self.open_sub_tier(target_idx as i32, tier);
                    return None;
                }
            } else if let Some(action) = &slice.action {
                self.hovered_index = target_idx as i32;
                let act = action.clone();
                self.transition_close_animated(Some(act.clone()));
                return Some(act);
            }
        }

        None
    }

    /// Dynamic active hover content (icon, label) for center hub text/icon
    pub fn active_hover_hub_content(&self) -> (Option<String>, String) {
        // 1. Dragging slice
        if self.drag.is_dragging && self.drag.from_index >= 0 && self.drag.target_index >= 0 {
            let from_label = self
                .current_slices
                .get(self.drag.from_index as usize)
                .map(|s| s.label.as_str())
                .unwrap_or("Slice");

            if self.drag.from_index == self.drag.target_index {
                return (None, format!("Moving {}", from_label));
            } else {
                let slot = self.drag.target_index + 1;
                return (None, format!("Move {} → Slot {} (Key {})", from_label, slot, slot));
            }
        }

        // 2. Outer sub-ring hovered
        if self.outer_hovered_index >= 0 {
            if let Some(sub) = self.sub_slices.get(self.outer_hovered_index as usize) {
                if self.active_sub_tier == Some(SubTierType::ActiveApps) {
                    return (Some(sub.icon.clone()), sub.label.clone());
                } else {
                    return (None, sub.label.clone());
                }
            }
        }

        // 3. Main ring hovered
        if self.hovered_index >= 0 {
            if let Some(slice) = self.current_slices.get(self.hovered_index as usize) {
                return (None, slice.label.clone());
            }
        }

        // 4. Nothing hovered
        (None, String::new())
    }

    /// Dynamic active hover label for center hub text
    pub fn active_hover_label(&self) -> String {
        self.active_hover_hub_content().1
    }

    /// Reorder current slices in memory and update config
    pub fn reorder_current_slices(&mut self, from: usize, to: usize) {
        if from == to || from >= self.current_slices.len() || to >= self.current_slices.len() {
            return;
        }
        let ctx_str = self.context.to_string();
        self.config.reorder_slice(&ctx_str, from, to);
        let item = self.current_slices.remove(from);
        self.current_slices.insert(to, item);
        for (idx, slice) in self.current_slices.iter_mut().enumerate() {
            slice.slot_index = idx;
        }
    }

    /// Complete drag-and-drop reorder operation
    pub fn finish_drag_reorder(&mut self) -> bool {
        if self.drag.is_dragging && self.drag.from_index >= 0 && self.drag.target_index >= 0 {
            let from = self.drag.from_index as usize;
            let to = self.drag.target_index as usize;
            if from != to && from < self.current_slices.len() && to < self.current_slices.len() {
                self.reorder_current_slices(from, to);
                let _ = self.config.save();
                self.hovered_index = to as i32;
                self.anim.hover_factor = 1.0;
                self.anim.hover_elapsed = 160.0;
                self.drag.reset();
                return true;
            }
        }
        self.drag.reset();
        false
    }

    /// Generates the sub-slice items for a given SubTierType
    pub fn generate_sub_slices(&self, tier: SubTierType) -> Vec<SliceItem> {
        match tier {
            SubTierType::Scratchpad => (1..=8)
                .map(|i| {
                    let mut item = SliceItem::new(
                        format!("scratchpad_ws_{}", i),
                        format!("Workspace {}", i),
                        format!("counter_{}", i),
                        i - 1,
                    );
                    item.action = Some(ActionId::MoveToWorkspace(i as u8));
                    item
                })
                .collect(),

            SubTierType::BrowserTabs => {
                if !self.browser_tabs.is_empty() {
                    self.browser_tabs
                        .iter()
                        .enumerate()
                        .map(|(i, tab)| {
                            let idx = if tab.index > 0 { tab.index } else { i + 1 };
                            let icon = if idx <= 8 {
                                format!("counter_{}", idx)
                            } else {
                                "tab".to_string()
                            };
                            let label = if !tab.title.is_empty() {
                                tab.title.clone()
                            } else {
                                format!("Tab {}", idx)
                            };
                            let mut item =
                                SliceItem::new(format!("browser_tab_{}", idx), label, icon, i);
                            item.action = Some(ActionId::SwitchToTab(idx));
                            item
                        })
                        .collect()
                } else {
                    (1..=8)
                        .map(|i| {
                            let mut item = SliceItem::new(
                                format!("browser_tab_{}", i),
                                format!("Tab {}", i),
                                format!("counter_{}", i),
                                i - 1,
                            );
                            item.action = Some(ActionId::SwitchToTab(i));
                            item
                        })
                        .collect()
                }
            }

            SubTierType::FileJump => {
                let mut slices: Vec<SliceItem> = self
                    .config
                    .file_jump_targets
                    .iter()
                    .enumerate()
                    .map(|(idx, t)| {
                        let mut item = SliceItem::new(
                            if t.id.is_empty() {
                                format!("target_{}", idx)
                            } else {
                                t.id.clone()
                            },
                            t.label.clone(),
                            if t.icon.is_empty() {
                                "folder".to_string()
                            } else {
                                t.icon.clone()
                            },
                            idx,
                        );
                        item.target_path = Some(t.path.clone());
                        item.target_index = Some(idx);
                        item.action = Some(ActionId::JumpToFile(t.path.clone()));
                        item
                    })
                    .collect();

                let mut add_btn =
                    SliceItem::new("add_file_target", "Add Target", "add", slices.len());
                add_btn.is_add_button = true;
                slices.push(add_btn);
                slices
            }

            SubTierType::ActiveApps => {
                let valid_clients: Vec<&HyprClient> = self
                    .active_clients
                    .iter()
                    .filter(|c| {
                        !c.address.is_empty()
                            && !c.workspace.name.starts_with("special:quickshell")
                    })
                    .collect();

                if valid_clients.is_empty() {
                    return vec![SliceItem::new("no_windows", "No Open Apps", "info", 0)];
                }

                valid_clients
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        let raw_class = if c.class.is_empty() { "App" } else { &c.class };

                        let ws_display = if c.workspace.name.starts_with("special") || c.workspace.id < 0 {
                            "WS X".to_string()
                        } else if !c.workspace.name.is_empty() {
                            format!("WS {}", c.workspace.name)
                        } else if c.workspace.id != 0 {
                            format!("WS {}", c.workspace.id)
                        } else {
                            "WS ?".to_string()
                        };

                        let raw_ws = if !c.workspace.name.is_empty() {
                            c.workspace.name.clone()
                        } else if c.workspace.id != 0 {
                            c.workspace.id.to_string()
                        } else {
                            String::new()
                        };

                        let icon = resolve_app_icon(raw_class);
                        let mut item =
                            SliceItem::new(format!("app_{}", c.address), ws_display, icon, i);
                        item.action = Some(ActionId::FocusWindow {
                            address: c.address.clone(),
                            workspace: raw_ws,
                        });
                        item
                    })
                    .collect()
            }

            SubTierType::Clipboard => {
                vec![SliceItem::new(
                    "no_clips",
                    "Clipboard Empty",
                    "content_paste_off",
                    0,
                )]
            }

            SubTierType::AudioSink => {
                vec![SliceItem::new(
                    "no_sinks",
                    "No Sinks Found",
                    "volume_off",
                    0,
                )]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::hypr::{HyprCursorPos, HyprWorkspace};

    fn make_test_context(class: &str, title: &str) -> HyprContext {
        HyprContext {
            cursor: HyprCursorPos { x: 500.0, y: 400.0 },
            window: HyprWindow {
                address: "0x123".into(),
                class: class.into(),
                initial_class: class.into(),
                title: title.into(),
                pid: 1000,
                workspace: HyprWorkspace {
                    id: 1,
                    name: "1".into(),
                },
                at: vec![100, 100],
                ..Default::default()
            },
            clients: vec![
                HyprClient {
                    address: "0x123".into(),
                    class: "kitty".into(),
                    title: "Terminal".into(),
                    pid: 1000,
                    workspace: HyprWorkspace {
                        id: 1,
                        name: "1".into(),
                    },
                    ..Default::default()
                },
                HyprClient {
                    address: "0x456".into(),
                    class: "firefox".into(),
                    title: "Browser".into(),
                    pid: 2000,
                    workspace: HyprWorkspace {
                        id: 2,
                        name: "2".into(),
                    },
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn test_menu_state_init() {
        let menu = MenuState::new(RadialConfig::default(), false);
        assert_eq!(menu.phase, MenuPhase::Hidden);
        assert_eq!(menu.center_x, 0.0);
        assert_eq!(menu.center_y, 0.0);
        assert_eq!(menu.context, Context::Default);
        assert_eq!(menu.hovered_index, -1);
        assert!(!menu.center_hovered);
        assert_eq!(menu.outer_hovered_index, -1);
        assert_eq!(menu.active_sub_tier, None);
        assert_eq!(menu.parent_slice_index, -1);
        assert!(menu.sub_slices.is_empty());
        assert!(menu.current_slices.is_empty());
        assert!(!menu.drag.is_dragging);
        assert!(!menu.flick_mode_armed);
        assert_eq!(menu.slice_angle(), 360.0);
        assert_eq!(menu.sub_slice_width(), 28.0);
    }

    #[test]
    fn test_transition_open() {
        let mut menu = MenuState::new(RadialConfig::default(), false);
        let ctx = make_test_context("kitty", "~");
        menu.transition_open(ctx);

        assert_eq!(menu.phase, MenuPhase::Opening);
        assert_eq!(menu.context, Context::Terminal);
        assert_eq!(menu.center_x, 500.0);
        assert_eq!(menu.center_y, 400.0);
        assert_eq!(menu.window_info.class, "kitty");
        assert_eq!(menu.active_clients.len(), 2);
        assert!(menu.flick_mode_armed);
        assert!(!menu.cursor_moved_flick);
        assert!(!menu.current_slices.is_empty());
        assert_eq!(menu.hovered_index, -1);
        assert_eq!(menu.outer_hovered_index, -1);
        assert_eq!(menu.active_sub_tier, None);

        // Check slices correspond to kitty slices
        assert_eq!(
            menu.current_slices.len(),
            menu.config.kitty_slices.len()
        );
    }

    #[test]
    fn test_open_and_close_sub_tier() {
        let mut menu = MenuState::new(RadialConfig::default(), false);
        let ctx = make_test_context("kitty", "~");
        menu.transition_open(ctx);

        menu.open_sub_tier(0, SubTierType::Scratchpad);
        assert_eq!(menu.active_sub_tier, Some(SubTierType::Scratchpad));
        assert_eq!(menu.parent_slice_index, 0);
        assert_eq!(menu.sub_slices.len(), 8);
        assert_eq!(menu.sub_slices[0].label, "Workspace 1");
        assert_eq!(menu.sub_slices[7].label, "Workspace 8");

        menu.close_sub_tier();
        assert_eq!(menu.active_sub_tier, None);
        assert_eq!(menu.parent_slice_index, -1);
        assert!(menu.sub_slices.is_empty());
    }

    #[test]
    fn test_escape_key_hierarchical() {
        let mut menu = MenuState::new(RadialConfig::default(), false);
        let ctx = make_test_context("kitty", "~");
        menu.transition_open(ctx);

        menu.open_sub_tier(0, SubTierType::Scratchpad);
        assert_eq!(menu.active_sub_tier, Some(SubTierType::Scratchpad));

        // First escape: closes sub-tier, returns false
        let closed_menu = menu.handle_escape();
        assert!(!closed_menu, "First escape should close sub-tier, not menu");
        assert_eq!(menu.active_sub_tier, None);
        assert_eq!(menu.phase, MenuPhase::Opening);

        // Second escape: closes menu, returns true
        let closed_menu2 = menu.handle_escape();
        assert!(closed_menu2, "Second escape should close menu");
        assert_eq!(
            menu.phase,
            MenuPhase::ClosingAnimated { pending_action: None }
        );
    }

    #[test]
    fn test_number_key_activation() {
        let mut menu = MenuState::new(RadialConfig::default(), false);
        let ctx = make_test_context("kitty", "~");
        menu.transition_open(ctx);

        // Kitty slices default:
        // Slot 0: scratchpad (has_sub_tier = false in normal workspace, action = ActionId::Scratchpad)
        // Slot 1: kitty_new_window (action = ActionId::KittyNewWindow)
        // Key 2 triggers Slot 1 (1-based index)
        let action = menu.handle_number_key(2);
        assert_eq!(action, Some(ActionId::KittyNewWindow { pid: 1000, title: "~".into() }));
        assert_eq!(
            menu.phase,
            MenuPhase::ClosingAnimated {
                pending_action: Some(ActionId::KittyNewWindow { pid: 1000, title: "~".into() })
            }
        );

        // Re-open and test sub-tier activation
        menu.transition_open(make_test_context("kitty", "~"));
        // Open sub-tier manually on slot 0
        menu.open_sub_tier(0, SubTierType::Scratchpad);
        assert_eq!(menu.active_sub_tier, Some(SubTierType::Scratchpad));

        // Key 3 in sub-tier corresponds to Workspace 3
        let sub_action = menu.handle_number_key(3);
        assert_eq!(sub_action, Some(ActionId::MoveToWorkspace(3)));
        assert_eq!(
            menu.phase,
            MenuPhase::ClosingAnimated {
                pending_action: Some(ActionId::MoveToWorkspace(3))
            }
        );

        // Out of bounds key
        let none_action = menu.handle_number_key(99);
        assert_eq!(none_action, None);

        // Key 0 is invalid (1-based)
        let zero_action = menu.handle_number_key(0);
        assert_eq!(zero_action, None);
    }

    #[test]
    fn test_active_hover_label() {
        let mut menu = MenuState::new(RadialConfig::default(), false);
        let ctx = make_test_context("kitty", "~");
        menu.transition_open(ctx);

        // Nothing hovered
        assert_eq!(menu.active_hover_label(), "");

        // Main slice hovered
        menu.hovered_index = 1;
        assert_eq!(menu.active_hover_label(), menu.current_slices[1].label);

        // Sub-tier open and hovered
        menu.open_sub_tier(0, SubTierType::Scratchpad);
        menu.outer_hovered_index = 2;
        assert_eq!(menu.active_hover_label(), "Workspace 3");

        // Active apps hovered returns icon and workspace label
        menu.open_sub_tier(0, SubTierType::ActiveApps);
        menu.sub_slices = vec![SliceItem::new("app_1", "WS X", "terminal", 0)];
        menu.outer_hovered_index = 0;
        assert_eq!(menu.active_hover_hub_content(), (Some("terminal".to_string()), "WS X".to_string()));
        assert_eq!(menu.active_hover_label(), "WS X");

        // Dragging slice
        menu.drag.is_dragging = true;
        menu.drag.from_index = 1;
        menu.drag.target_index = 1;
        let label = menu.current_slices[1].label.clone();
        assert_eq!(menu.active_hover_label(), format!("Moving {}", label));

        menu.drag.target_index = 3;
        assert_eq!(
            menu.active_hover_label(),
            format!("Move {} → Slot 4 (Key 4)", label)
        );
    }

    #[test]
    fn test_drag_reorder() {
        let mut menu = MenuState::new(RadialConfig::default(), false);
        let ctx = make_test_context("kitty", "~");
        menu.transition_open(ctx);

        let slice_0_before = menu.current_slices[0].id.clone();
        let slice_1_before = menu.current_slices[1].id.clone();

        menu.drag.is_dragging = true;
        menu.drag.from_index = 0;
        menu.drag.target_index = 1;

        let reordered = menu.finish_drag_reorder();
        assert!(reordered);
        assert_eq!(menu.current_slices[0].id, slice_1_before);
        assert_eq!(menu.current_slices[1].id, slice_0_before);
        assert!(!menu.drag.is_dragging);
    }

    #[test]
    fn test_browser_tabs_sub_slices() {
        let mut menu = MenuState::new(RadialConfig::default(), false);
        menu.browser_tabs = vec![
            BrowserTab {
                index: 1,
                title: "GitHub".into(),
                url: "https://github.com".into(),
                active: true,
                id: Some(10),
            },
            BrowserTab {
                index: 2,
                title: "YouTube".into(),
                url: "https://youtube.com".into(),
                active: false,
                id: Some(11),
            },
        ];

        let subs = menu.generate_sub_slices(SubTierType::BrowserTabs);
        assert_eq!(subs.len(), 2);
        assert_eq!(subs[0].label, "GitHub");
        assert_eq!(subs[0].action, Some(ActionId::SwitchToTab(1)));
        assert_eq!(subs[1].label, "YouTube");
        assert_eq!(subs[1].action, Some(ActionId::SwitchToTab(2)));
    }

    #[test]
    fn test_active_apps_sub_slices() {
        let mut menu = MenuState::new(RadialConfig::default(), false);
        menu.active_clients = vec![
            HyprClient {
                address: "0x1".into(),
                class: "kitty".into(),
                title: "Terminal 1".into(),
                pid: 101,
                workspace: HyprWorkspace {
                    id: 1,
                    name: "1".into(),
                },
                ..Default::default()
            },
            HyprClient {
                address: "0x2".into(),
                class: "kitty".into(),
                title: "Terminal 2".into(),
                pid: 102,
                workspace: HyprWorkspace {
                    id: 2,
                    name: "2".into(),
                },
                ..Default::default()
            },
            HyprClient {
                address: "0x3".into(),
                class: "firefox".into(),
                title: "Browser".into(),
                pid: 103,
                workspace: HyprWorkspace {
                    id: -99,
                    name: "special:special".into(),
                },
                ..Default::default()
            },
        ];

        let subs = menu.generate_sub_slices(SubTierType::ActiveApps);
        assert_eq!(subs.len(), 3);
        assert_eq!(subs[0].label, "WS 1");
        assert_eq!(subs[0].icon, "terminal");
        assert_eq!(subs[1].label, "WS 2");
        assert_eq!(subs[1].icon, "terminal");
        assert_eq!(subs[2].label, "WS X");
        assert_eq!(subs[2].icon, "globe");
    }
}
