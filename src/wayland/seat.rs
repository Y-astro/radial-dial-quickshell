//! `wl_seat` input handling — pointer zones, gestures & key dispatch.
//!
//! Handles raw Wayland pointer and keyboard events, classifies pointer
//! hit-test zones relative to the radial menu geometry, drives drag-and-drop
//! reordering, and dispatches actions to [`MenuState`].

#[allow(unused_imports)]
use crate::renderer::pie::{HUB_RADIUS, SLICE_INNER_R, SUB_INNER_R, SUB_OUTER_R};
use crate::state::actions::{ActionId, SubTierType};
use crate::state::menu::{MenuPhase, MenuState};

// ─────────────────────────────────────────────────────────────────────────────
// Linux Input Event Codes & Keysyms
// ─────────────────────────────────────────────────────────────────────────────

pub const BTN_LEFT: u32 = 0x110;
pub const BTN_RIGHT: u32 = 0x111;
pub const BTN_MIDDLE: u32 = 0x112;

/// Threshold constants from radial dial geometry.
pub const CENTER_HUB_RADIUS: f32 = 48.0;
pub const SUB_INNER_HIT_R: f32 = SUB_INNER_R - 6.0; // 162.0
pub const SUB_OUTER_HIT_R: f32 = SUB_OUTER_R + 14.0; // 242.0
pub const FLICK_DISTANCE_THRESHOLD: f32 = 32.0;

// ─────────────────────────────────────────────────────────────────────────────
// Pointer events & Zones
// ─────────────────────────────────────────────────────────────────────────────

/// High-level pointer events emitted by the seat handler.
#[derive(Debug, Clone, PartialEq)]
pub enum PointerEvent {
    /// Pointer motion in surface coordinates (logical pixels).
    Motion {
        x: f64,
        y: f64,
    },

    /// Button press or release.
    Button {
        /// Linux input event code (e.g. `BTN_LEFT = 0x110`).
        button: u32,
        /// Whether the button was pressed or released.
        state: ButtonState,
    },

    /// Scroll axis event (vertical or horizontal wheel).
    Axis {
        /// Scroll delta in logical pixels (positive = down/right, negative = up/left).
        delta: f64,
    },
}

/// Button press/release state (mirrors `wl_pointer.button_state`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    /// Button was pressed.
    Pressed,
    /// Button was released.
    Released,
}

// ─────────────────────────────────────────────────────────────────────────────
// Keyboard events
// ─────────────────────────────────────────────────────────────────────────────

/// Key press/release state (mirrors `wl_keyboard.key_state`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    /// Key was pressed.
    Pressed,
    /// Key was released.
    Released,
}

/// A keyboard key event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEvent {
    /// XKB keysym.
    pub keysym: u32,
    /// Whether the key was pressed or released.
    pub state: KeyState,
}

// ─────────────────────────────────────────────────────────────────────────────
// Pointer Hit-Test Zones
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PointerZone {
    CenterHub,
    MainRing { slice_index: usize },
    OuterSubRing { sub_index: usize },
    Outside,
}

/// Helper: normalize degrees into `[0.0, 360.0)`.
#[inline]
pub fn normalize_deg(deg: f32) -> f32 {
    let mut d = deg % 360.0;
    if d < 0.0 {
        d += 360.0;
    }
    if d >= 360.0 || d.abs() < 1e-6 {
        0.0
    } else {
        d
    }
}

/// Classify pointer relative to menu center (dx = px - cx, dy = py - cy).
/// - dist < 48.0 -> CenterHub
/// - dist > 238.0 (SUB_OUTER_R + 10) -> Outside
/// - dist >= 162.0 (SUB_INNER_R - 6) -> OuterSubRing (if active_sub_tier is Some)
/// - otherwise -> MainRing
pub fn classify_pointer(
    dx: f32,
    dy: f32,
    slice_count: usize,
    sub_slice_count: usize,
    sub_start_deg: f32,
    sub_slice_width_deg: f32,
    sub_tier_active: bool,
) -> PointerZone {
    let dist = (dx * dx + dy * dy).sqrt();

    // 1. Center Hub (< 48.0 px)
    if dist < CENTER_HUB_RADIUS {
        return PointerZone::CenterHub;
    }

    // 2. Outside entire dial (> 238.0 px = SUB_OUTER_R + 10.0)
    if dist > SUB_OUTER_HIT_R {
        return PointerZone::Outside;
    }

    // 3. Localized Outer Sub-Ring (dist >= 162.0 px = SUB_INNER_R - 6.0)
    if dist >= SUB_INNER_HIT_R {
        if sub_tier_active && sub_slice_count > 0 && sub_slice_width_deg > 0.0 {
            let raw_angle = dy.atan2(dx).to_degrees();
            let canvas_angle = normalize_deg(raw_angle);
            let norm_start = normalize_deg(sub_start_deg);
            let rel_angle = normalize_deg(canvas_angle - norm_start);
            let sub_total_span = sub_slice_count as f32 * sub_slice_width_deg;

            if rel_angle <= sub_total_span + 1e-3 {
                let sub_idx = (rel_angle / sub_slice_width_deg).floor() as usize;
                let sub_index = sub_idx.min(sub_slice_count.saturating_sub(1));
                return PointerZone::OuterSubRing { sub_index };
            }
        }
        return PointerZone::Outside;
    }

    // 4. Main Ring (48.0 <= dist < 162.0)
    if slice_count == 0 {
        return PointerZone::Outside;
    }

    let raw_angle = dy.atan2(dx).to_degrees();
    let clock_angle = normalize_deg(raw_angle + 90.0);
    let slice_angle = 360.0 / slice_count as f32;
    let slice_index = ((clock_angle / slice_angle).floor() as usize) % slice_count;

    PointerZone::MainRing { slice_index }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers for button & keysym matching
// ─────────────────────────────────────────────────────────────────────────────

#[inline]
pub fn is_left_button(button: u32) -> bool {
    button == BTN_LEFT || button == 1
}

#[inline]
pub fn is_right_button(button: u32) -> bool {
    button == BTN_RIGHT || button == 2 || button == 3
}

#[inline]
pub fn is_escape(keysym: u32) -> bool {
    matches!(keysym, 0xff1b | 27)
}

#[inline]
pub fn is_super_or_tab(keysym: u32) -> bool {
    matches!(
        keysym,
        0xffeb  // XK_Super_L
        | 0xffec // XK_Super_R
        | 0xffe7 // XK_Meta_L
        | 0xffe8 // XK_Meta_R
        | 0xff09 // XK_Tab
        | 0xfe20 // XK_ISO_Left_Tab
        | 9      // ASCII '\t'
    )
}

#[inline]
pub fn parse_number_key(keysym: u32) -> Option<usize> {
    match keysym {
        1..=9 => Some(keysym as usize),
        0x31..=0x39 => Some((keysym - 0x31 + 1) as usize),
        0xffb1..=0xffb9 => Some((keysym - 0xffb1 + 1) as usize),
        _ => None,
    }
}

/// Convert an XKB keysym into a printable character if applicable.
#[inline]
pub fn keysym_to_char(keysym: u32) -> Option<char> {
    match keysym {
        0x20..=0x7e => char::from_u32(keysym),
        0x00a0..=0x00ff => char::from_u32(keysym),
        0xffb0..=0xffb9 => char::from_u32('0' as u32 + (keysym - 0xffb0)),
        0xffac => Some('*'),
        0xffab => Some('+'),
        0xffad => Some('-'),
        0xffae => Some('.'),
        0xffaf => Some('/'),
        0x01000100..=0x0110ffff => char::from_u32(keysym - 0x01000000),
        _ => None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Seat Handler
// ─────────────────────────────────────────────────────────────────────────────

/// Seat input handler managing pointer and keyboard events.
#[derive(Debug, Clone, Default)]
pub struct SeatHandler {
    pub last_x: f32,
    pub last_y: f32,
    pub customizer_open: bool,
    pub click_handled_on_press: bool,
}

impl SeatHandler {
    /// Create a new seat handler.
    pub fn new() -> Self {
        Self::default()
    }

    /// Handles pointer motion in compositor/window coordinates.
    ///
    /// - Calculates `dx = x - menu.center_x`, `dy = y - menu.center_y`.
    /// - Checks hold-and-flick threshold: if dist > 32.0 px, sets `cursor_moved_flick = true`.
    /// - Updates drag position if dragging.
    /// - Classifies zone and updates `hovered_index`, `center_hovered`, `outer_hovered_index`.
    pub fn handle_pointer_motion(&mut self, menu: &mut MenuState, x: f32, y: f32) {
        self.last_x = x;
        self.last_y = y;

        let dx = x - menu.center_x;
        let dy = y - menu.center_y;
        let dist = (dx * dx + dy * dy).sqrt();

        // Check hold-and-flick threshold
        if dist > FLICK_DISTANCE_THRESHOLD {
            menu.cursor_moved_flick = true;
        }

        // Handle adding slice cursor tracking around main dial
        if let Some(adding) = &mut menu.adding_slice {
            adding.current_x = x;
            adding.current_y = y;
            let slice_count = menu.current_slices.len().max(1);
            let slice_angle = 360.0 / slice_count as f32;
            let raw_angle = dy.atan2(dx).to_degrees();
            let clock_angle = (raw_angle + 90.0).rem_euclid(360.0);
            let clock_shifted = (clock_angle + slice_angle * 0.5).rem_euclid(360.0);
            let target_index = ((clock_shifted / slice_angle).floor() as usize) % slice_count;
            adding.target_index = target_index;
            menu.hovered_index = -1;
            menu.center_hovered = false;
            menu.outer_hovered_index = -1;
            return;
        }

        // Handle drag-to-reorder if dragging
        if menu.drag.is_dragging {
            menu.drag.update_position(x, y);
            if menu.drag.is_sub_drag {
                let valid_count = menu.config.file_jump_targets.len();
                let total_slices = menu.sub_slices.len();
                let sub_start = menu.sub_start_angle();
                let sub_width = menu.sub_slice_width();
                menu.drag.update_sub_target_slot(
                    menu.center_x,
                    menu.center_y,
                    valid_count,
                    total_slices,
                    sub_start,
                    sub_width,
                );
            } else {
                menu.drag.update_target_slot(
                    menu.center_x,
                    menu.center_y,
                    menu.current_slices.len(),
                    menu.slice_angle(),
                );
            }
            return;
        } else if menu.drag.from_index >= 0 {
            menu.drag.update_position(x, y);
            if menu.drag.threshold_met {
                menu.drag.is_dragging = true;
                menu.anim.drag_elapsed = 0.0;
                menu.anim.drag_pluck_progress = 0.0;
                menu.anim.drag_indicator_alpha = 0.0;
                if menu.drag.is_sub_drag {
                    let valid_count = menu.config.file_jump_targets.len();
                    let total_slices = menu.sub_slices.len();
                    let sub_start = menu.sub_start_angle();
                    let sub_width = menu.sub_slice_width();
                    menu.drag.update_sub_target_slot(
                        menu.center_x,
                        menu.center_y,
                        valid_count,
                        total_slices,
                        sub_start,
                        sub_width,
                    );
                } else {
                    if menu.active_sub_tier.is_some() {
                        menu.close_sub_tier();
                    }
                    menu.drag.update_target_slot(
                        menu.center_x,
                        menu.center_y,
                        menu.current_slices.len(),
                        menu.slice_angle(),
                    );
                }
                return;
            }
        }

        // Classify zone
        let zone = classify_pointer(
            dx,
            dy,
            menu.current_slices.len(),
            menu.sub_slices.len(),
            menu.sub_start_angle(),
            menu.sub_slice_width(),
            menu.active_sub_tier.is_some(),
        );

        match zone {
            PointerZone::CenterHub => {
                menu.center_hovered = true;
                if menu.hovered_index != -1 {
                    menu.hovered_index = -1;
                    menu.anim.hover_fade_start = menu.anim.hover_factor;
                    menu.anim.hover_elapsed = 0.0;
                }
                if menu.outer_hovered_index != -1 {
                    menu.outer_hovered_index = -1;
                    menu.anim.outer_hover_fade_start = menu.anim.outer_hover_factor;
                    menu.anim.outer_hover_elapsed = 0.0;
                }
            }
            PointerZone::MainRing { slice_index } => {
                menu.center_hovered = false;
                let new_idx = slice_index as i32;
                if menu.hovered_index != new_idx {
                    menu.hovered_index = new_idx;
                    menu.anim.hover_factor = 0.0;
                    menu.anim.hover_elapsed = 0.0;
                }
                if menu.outer_hovered_index != -1 {
                    menu.outer_hovered_index = -1;
                    menu.anim.outer_hover_fade_start = menu.anim.outer_hover_factor;
                    menu.anim.outer_hover_elapsed = 0.0;
                }
            }
            PointerZone::OuterSubRing { sub_index } => {
                menu.center_hovered = false;
                if menu.parent_slice_index >= 0 && menu.hovered_index != menu.parent_slice_index {
                    menu.hovered_index = menu.parent_slice_index;
                    menu.anim.hover_factor = 1.0;
                    menu.anim.hover_elapsed = 160.0;
                }
                let new_outer = sub_index as i32;
                if menu.outer_hovered_index != new_outer {
                    menu.outer_hovered_index = new_outer;
                    menu.anim.outer_hover_factor = 0.0;
                    menu.anim.outer_hover_elapsed = 0.0;
                }
            }
            PointerZone::Outside => {
                menu.center_hovered = false;
                if menu.hovered_index != -1 {
                    menu.hovered_index = -1;
                    menu.anim.hover_fade_start = menu.anim.hover_factor;
                    menu.anim.hover_elapsed = 0.0;
                }
                if menu.outer_hovered_index != -1 {
                    menu.outer_hovered_index = -1;
                    menu.anim.outer_hover_fade_start = menu.anim.outer_hover_factor;
                    menu.anim.outer_hover_elapsed = 0.0;
                }
            }
        }
    }

    /// Handles pointer button events.
    ///
    /// - Left click in CenterHub -> close menu
    /// - Left click in MainRing -> toggle sub-tier or return ActionId to execute + close
    /// - Left click in OuterSubRing -> return ActionId to execute + close
    /// - Left click in Outside -> close menu
    /// - Right click -> toggle in-dial customizer modal
    pub fn handle_pointer_button(
        &mut self,
        menu: &mut MenuState,
        button: u32,
        state: ButtonState,
    ) -> Option<ActionId> {
        if is_right_button(button) {
            if state == ButtonState::Pressed {
                self.customizer_open = !self.customizer_open;
            }
            return None;
        }

        if !is_left_button(button) {
            return None;
        }

        if state == ButtonState::Pressed {
            // Drop slice being added into the dial at the target location!
            if let Some(adding) = menu.adding_slice.take() {
                menu.config.insert_slice(&menu.context.to_string(), adding.target_index, &adding.action_id);
                let _ = menu.config.save();
                menu.refresh_current_slices();
                menu.hovered_index = adding.target_index as i32;
                menu.anim.hover_factor = 1.0;
                menu.anim.drag_pluck_progress = 0.0;
                menu.anim.drag_indicator_alpha = 0.0;
                menu.anim.drag_elapsed = 0.0;
                self.click_handled_on_press = true;
                return None;
            }

            // Sub-ring FileJump target pressed -> start potential sub-ring drag-to-reorder.
            // Do NOT execute click on press! Defer to ButtonState::Released so user can drag.
            if menu.active_sub_tier == Some(SubTierType::FileJump)
                && menu.outer_hovered_index >= 0
                && (menu.outer_hovered_index as usize) < menu.sub_slices.len()
            {
                let idx = menu.outer_hovered_index as usize;
                if !menu.sub_slices[idx].is_add_button {
                    menu.drag.start_sub_drag(menu.outer_hovered_index, self.last_x, self.last_y);
                    self.click_handled_on_press = false;
                    return None;
                }
            }

            // Main ring slice pressed -> start potential drag-to-reorder.
            // Do NOT execute click on press! Defer to ButtonState::Released so user can drag.
            if menu.hovered_index >= 0 && menu.outer_hovered_index < 0 && !menu.center_hovered {
                menu.drag.start_drag(menu.hovered_index, self.last_x, self.last_y);
                self.click_handled_on_press = false;
                return None;
            }

            self.click_handled_on_press = true;
            return self.execute_left_click(menu);
        }

        if state == ButtonState::Released {
            if menu.drag.is_dragging {
                if menu.drag.is_sub_drag {
                    menu.finish_sub_drag_reorder();
                } else {
                    menu.finish_drag_reorder();
                    let _ = menu.config.save();
                }
                menu.anim.drag_pluck_progress = 0.0;
                menu.anim.drag_indicator_alpha = 0.0;
                menu.anim.drag_elapsed = 0.0;
                menu.drag.reset();
                self.click_handled_on_press = false;
                return None;
            }

            menu.drag.reset();

            if self.click_handled_on_press {
                self.click_handled_on_press = false;
                return None;
            }

            if !matches!(menu.phase, MenuPhase::ClosingAnimated { .. } | MenuPhase::Hidden) {
                return self.execute_left_click(menu);
            }
        }

        None
    }

    fn execute_left_click(&mut self, menu: &mut MenuState) -> Option<ActionId> {
        // 1. CenterHub
        if menu.center_hovered {
            menu.transition_close_animated(None);
            return None;
        }

        // 2. OuterSubRing
        if menu.active_sub_tier.is_some() && menu.outer_hovered_index >= 0 {
            let idx = menu.outer_hovered_index as usize;
            if idx < menu.sub_slices.len() {
                if let Some(action) = &menu.sub_slices[idx].action {
                    let act = action.clone();
                    menu.transition_close_animated(Some(act.clone()));
                    return Some(act);
                }
            }
            menu.transition_close_animated(None);
            return None;
        }

        // 3. MainRing
        if menu.hovered_index >= 0 {
            let idx = menu.hovered_index as usize;
            if idx < menu.current_slices.len() {
                let slice = &menu.current_slices[idx];
                if slice.has_sub_tier {
                    if let Some(tier) = slice.sub_tier_type {
                        menu.toggle_sub_tier(menu.hovered_index, tier);
                        return None;
                    }
                } else if let Some(action) = &slice.action {
                    let act = action.clone();
                    menu.transition_close_animated(Some(act.clone()));
                    return Some(act);
                }
            }
            return None;
        }

        // 4. Outside
        menu.transition_close_animated(None);
        None
    }

    /// Handles keyboard key events.
    ///
    /// - Escape -> calls `menu.handle_escape()`
    /// - 1-9 keys -> calls `menu.handle_number_key(n)`
    /// - Key release of Super or Tab -> if `flick_mode_armed && cursor_moved_flick`: execute currently hovered slice immediately
    pub fn handle_key_event(
        &mut self,
        menu: &mut MenuState,
        keysym: u32,
        state: KeyState,
    ) -> Option<ActionId> {
        // 1. Super / Tab key release in flick mode
        if state == KeyState::Released && is_super_or_tab(keysym) {
            if menu.flick_mode_armed && menu.cursor_moved_flick {
                menu.flick_mode_armed = false;
                if menu.adding_slice.is_some() {
                    return None;
                }
                if menu.outer_hovered_index >= 0
                    && (menu.outer_hovered_index as usize) < menu.sub_slices.len()
                {
                    if let Some(action) = &menu.sub_slices[menu.outer_hovered_index as usize].action {
                        let act = action.clone();
                        menu.transition_close_animated(Some(act.clone()));
                        return Some(act);
                    }
                } else if menu.hovered_index >= 0
                    && (menu.hovered_index as usize) < menu.current_slices.len()
                {
                    let slice = &menu.current_slices[menu.hovered_index as usize];
                    if slice.has_sub_tier {
                        if let Some(tier) = slice.sub_tier_type {
                            menu.open_sub_tier(menu.hovered_index, tier);
                            return None;
                        }
                    } else if let Some(action) = &slice.action {
                        let act = action.clone();
                        menu.transition_close_animated(Some(act.clone()));
                        return Some(act);
                    }
                }
            }
            return None;
        }

        // 2. Escape
        if is_escape(keysym) {
            if state == KeyState::Pressed {
                menu.handle_escape();
            }
            return None;
        }

        // 3. Number keys 1..9
        if let Some(n) = parse_number_key(keysym) {
            if state == KeyState::Pressed {
                if menu.adding_slice.is_some() {
                    return None;
                }
                return menu.handle_number_key(n);
            }
            return None;
        }

        None
    }

    /// Handles pointer scroll wheel axis events.
    ///
    /// - Over volume slice: `adjust_volume(delta)`
    /// - Over brightness slice: `adjust_brightness(delta)`
    /// - Over sub-tier: cycle outer hovered index
    pub fn handle_pointer_axis(&mut self, menu: &mut MenuState, delta: f64) -> Option<ActionId> {
        if menu.hovered_index >= 0 && (menu.hovered_index as usize) < menu.current_slices.len() {
            let slice = &menu.current_slices[menu.hovered_index as usize];
            let fid = slice.id.to_lowercase();

            if fid.contains("volume")
                || fid.contains("media")
                || fid.contains("audio")
                || fid.contains("sound")
            {
                let act = if delta > 0.0 {
                    ActionId::VolumeUp
                } else if delta < 0.0 {
                    ActionId::VolumeDown
                } else {
                    return None;
                };
                crate::state::actions::execute(&act);
                return Some(act);
            }

            if fid.contains("brightness") || fid.contains("light") {
                let act = if delta > 0.0 {
                    ActionId::BrightnessUp
                } else if delta < 0.0 {
                    ActionId::BrightnessDown
                } else {
                    return None;
                };
                crate::state::actions::execute(&act);
                return Some(act);
            }

            if slice.has_sub_tier
                && menu.active_sub_tier.is_some()
                && !menu.sub_slices.is_empty()
            {
                let count = menu.sub_slices.len() as i32;
                if delta < 0.0 {
                    menu.outer_hovered_index = (menu.outer_hovered_index + 1) % count;
                } else if delta > 0.0 {
                    menu.outer_hovered_index = (menu.outer_hovered_index - 1 + count) % count;
                }
                return None;
            }
        }
        None
    }

    /// High-level pointer event dispatcher.
    pub fn handle_pointer(&mut self, menu: &mut MenuState, event: PointerEvent) -> Option<ActionId> {
        match event {
            PointerEvent::Motion { x, y } => {
                self.handle_pointer_motion(menu, x as f32, y as f32);
                None
            }
            PointerEvent::Button { button, state } => {
                self.handle_pointer_button(menu, button, state)
            }
            PointerEvent::Axis { delta } => {
                self.handle_pointer_axis(menu, delta)
            }
        }
    }

    /// High-level keyboard event dispatcher.
    pub fn handle_key(&mut self, menu: &mut MenuState, event: KeyEvent) -> Option<ActionId> {
        self.handle_key_event(menu, event.keysym, event.state)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::actions::{SliceItem, SubTierType};
    use crate::state::config::RadialConfig;

    #[test]
    fn test_pointer_zone_center_hub() {
        // (0, 0) is dead center
        let zone = classify_pointer(0.0, 0.0, 6, 0, 0.0, 0.0, false);
        assert_eq!(zone, PointerZone::CenterHub);

        // Distance = sqrt(20^2 + 20^2) = 28.28 < 48.0
        let zone = classify_pointer(20.0, 20.0, 6, 0, 0.0, 0.0, false);
        assert_eq!(zone, PointerZone::CenterHub);

        // Distance = 47.9 < 48.0
        let zone = classify_pointer(0.0, -47.9, 6, 0, 0.0, 0.0, false);
        assert_eq!(zone, PointerZone::CenterHub);
    }

    #[test]
    fn test_pointer_zone_main_ring() {
        // 6 slices -> 60 deg each
        // 12 o'clock: dx=0, dy=-100 -> angle 0 deg clock -> slice 0
        let zone = classify_pointer(0.0, -100.0, 6, 0, 0.0, 0.0, false);
        assert_eq!(zone, PointerZone::MainRing { slice_index: 0 });

        // 3 o'clock: dx=100, dy=0 -> angle 90 deg clock -> 90 / 60 = 1 -> slice 1
        let zone = classify_pointer(100.0, 0.0, 6, 0, 0.0, 0.0, false);
        assert_eq!(zone, PointerZone::MainRing { slice_index: 1 });

        // 6 o'clock: dx=0, dy=100 -> angle 180 deg clock -> 180 / 60 = 3 -> slice 3
        let zone = classify_pointer(0.0, 100.0, 6, 0, 0.0, 0.0, false);
        assert_eq!(zone, PointerZone::MainRing { slice_index: 3 });

        // 9 o'clock: dx=-100, dy=0 -> angle 270 deg clock -> 270 / 60 = 4 -> slice 4
        let zone = classify_pointer(-100.0, 0.0, 6, 0, 0.0, 0.0, false);
        assert_eq!(zone, PointerZone::MainRing { slice_index: 4 });

        // 8 slices -> 45 deg each
        // Clock 70 deg: dx=100*sin(70°)=93.97, dy=-100*cos(70°)=-34.20
        // 70 / 45 = 1.55 -> slice 1
        let zone = classify_pointer(93.97, -34.20, 8, 0, 0.0, 0.0, false);
        assert_eq!(zone, PointerZone::MainRing { slice_index: 1 });

        // Clock 330 deg: dx=-50.0, dy=-86.6 -> 330 / 45 = 7.33 -> slice 7
        let zone = classify_pointer(-50.0, -86.6, 8, 0, 0.0, 0.0, false);
        assert_eq!(zone, PointerZone::MainRing { slice_index: 7 });
    }

    #[test]
    fn test_pointer_zone_outer_sub_ring() {
        // Sub-tier active with 3 sub-slices.
        // Parent mid angle at -60° canvas (12 o'clock slice).
        // Span = 3 * 36 = 108°. Sub start = -60 - 54 = -114°.
        // norm_start = normalize_deg(-114) = 246°.
        let sub_start_deg = -114.0;
        let sub_width_deg = 36.0;

        // Pointer at radius 190 (between 162 and 238)
        // Sub-slice 0: canvas angle -100° (normalize = 260°, rel = 260 - 246 = 14°)
        // dx = 190 * cos(-100°) = 190 * -0.1736 = -33.0
        // dy = 190 * sin(-100°) = 190 * -0.9848 = -187.1
        let zone = classify_pointer(-33.0, -187.1, 6, 3, sub_start_deg, sub_width_deg, true);
        assert_eq!(zone, PointerZone::OuterSubRing { sub_index: 0 });

        // Sub-slice 1: canvas angle -60° (normalize = 300°, rel = 300 - 246 = 54°)
        // dx = 190 * cos(-60°) = 95.0, dy = 190 * sin(-60°) = -164.54
        let zone = classify_pointer(95.0, -164.54, 6, 3, sub_start_deg, sub_width_deg, true);
        assert_eq!(zone, PointerZone::OuterSubRing { sub_index: 1 });

        // Sub-slice 2: canvas angle -20° (normalize = 340°, rel = 340 - 246 = 94°)
        // dx = 190 * cos(-20°) = 178.5, dy = 190 * sin(-20°) = -64.98
        let zone = classify_pointer(178.5, -64.98, 6, 3, sub_start_deg, sub_width_deg, true);
        assert_eq!(zone, PointerZone::OuterSubRing { sub_index: 2 });
    }

    #[test]
    fn test_pointer_zone_outside() {
        // Distance > 238.0
        let zone = classify_pointer(0.0, 250.0, 6, 0, 0.0, 0.0, false);
        assert_eq!(zone, PointerZone::Outside);

        let zone = classify_pointer(300.0, 0.0, 6, 0, 0.0, 0.0, false);
        assert_eq!(zone, PointerZone::Outside);

        // Distance 190.0 but sub_tier is NOT active
        let zone = classify_pointer(0.0, 190.0, 6, 0, 0.0, 0.0, false);
        assert_eq!(zone, PointerZone::Outside);

        // Distance 190.0 with sub_tier active, but angle is on the opposite side of the fan
        // Fan is at top (-114° to -6°). Pointer at 6 o'clock (dy = 190.0, dx = 0.0, canvas 90°)
        let zone = classify_pointer(0.0, 190.0, 6, 3, -114.0, 36.0, true);
        assert_eq!(zone, PointerZone::Outside);
    }

    fn make_slice(id: &str, label: &str, slot: usize) -> SliceItem {
        SliceItem::new(id, label, "icon", slot)
    }

    #[test]
    fn test_pointer_motion_simulation() {
        let mut seat = SeatHandler::new();
        let mut menu = MenuState::new(RadialConfig::default(), false);
        menu.center_x = 200.0;
        menu.center_y = 200.0;
        menu.current_slices = vec![
            make_slice("term", "Terminal", 0),
            make_slice("calc", "Calculator", 1),
            make_slice("files", "Files", 2),
            make_slice("web", "Browser", 3),
        ];
        menu.phase = MenuPhase::Open;
        menu.flick_mode_armed = true;
        menu.cursor_moved_flick = false;

        // Move cursor 10px from center (< 32px flick threshold, < 48px center hub)
        seat.handle_pointer_motion(&mut menu, 210.0, 200.0);
        assert!(!menu.cursor_moved_flick);
        assert!(menu.center_hovered);
        assert_eq!(menu.hovered_index, -1);
        assert_eq!(menu.outer_hovered_index, -1);

        // Move cursor 100px up (to 12 o'clock: x=200, y=100).
        // Distance = 100px > 32px flick threshold -> cursor_moved_flick = true.
        // Slice 0 (Terminal) should be hovered.
        seat.handle_pointer_motion(&mut menu, 200.0, 100.0);
        assert!(menu.cursor_moved_flick);
        assert!(!menu.center_hovered);
        assert_eq!(menu.hovered_index, 0);
        assert_eq!(menu.outer_hovered_index, -1);

        // Move cursor 300px away (outside dial)
        seat.handle_pointer_motion(&mut menu, 500.0, 200.0);
        assert!(!menu.center_hovered);
        assert_eq!(menu.hovered_index, -1);
        assert_eq!(menu.outer_hovered_index, -1);
    }

    #[test]
    fn test_pointer_motion_hover_ripple_reset() {
        let mut seat = SeatHandler::new();
        let mut menu = MenuState::new(RadialConfig::default(), false);
        menu.center_x = 200.0;
        menu.center_y = 200.0;
        menu.current_slices = vec![
            make_slice("term", "Terminal", 0),
            make_slice("calc", "Calculator", 1),
            make_slice("files", "Files", 2),
            make_slice("web", "Browser", 3),
        ];
        menu.phase = MenuPhase::Open;

        // 1. Hover slice 0 (top, y=100)
        seat.handle_pointer_motion(&mut menu, 200.0, 100.0);
        assert_eq!(menu.hovered_index, 0);
        menu.anim.hover_factor = 1.0;
        menu.anim.hover_elapsed = 160.0;

        // 2. Switch to slice 1 (right, x=300, y=200)
        seat.handle_pointer_motion(&mut menu, 300.0, 200.0);
        assert_eq!(menu.hovered_index, 1);
        // Both hover_factor and hover_elapsed must be reset to 0.0 to trigger ripple
        assert_eq!(menu.anim.hover_factor, 0.0);
        assert_eq!(menu.anim.hover_elapsed, 0.0);

        // 3. Move outside dial
        menu.anim.hover_factor = 0.8;
        seat.handle_pointer_motion(&mut menu, 500.0, 200.0);
        assert_eq!(menu.hovered_index, -1);
        assert_eq!(menu.anim.hover_fade_start, 0.8);
        assert_eq!(menu.anim.hover_elapsed, 0.0);
    }

    #[test]
    fn test_pointer_button_center_hub_closes() {
        let mut seat = SeatHandler::new();
        let mut menu = MenuState::new(RadialConfig::default(), false);
        menu.center_x = 200.0;
        menu.center_y = 200.0;
        menu.phase = MenuPhase::Open;

        seat.handle_pointer_motion(&mut menu, 210.0, 200.0);
        assert!(menu.center_hovered);

        let action = seat.handle_pointer_button(&mut menu, BTN_LEFT, ButtonState::Pressed);
        assert_eq!(action, None);
        assert_eq!(
            menu.phase,
            MenuPhase::ClosingAnimated {
                pending_action: None
            }
        );
    }

    #[test]
    fn test_pointer_button_main_ring_action() {
        let mut seat = SeatHandler::new();
        let mut menu = MenuState::new(RadialConfig::default(), false);
        menu.center_x = 200.0;
        menu.center_y = 200.0;
        menu.phase = MenuPhase::Open;
        let mut term = make_slice("term", "Terminal", 0);
        term.action = Some(ActionId::Terminal);
        menu.current_slices = vec![term];

        seat.handle_pointer_motion(&mut menu, 200.0, 100.0);
        assert_eq!(menu.hovered_index, 0);

        // Press initiates potential drag -> no action yet
        let action_press = seat.handle_pointer_button(&mut menu, BTN_LEFT, ButtonState::Pressed);
        assert_eq!(action_press, None);
        assert_eq!(menu.phase, MenuPhase::Open);

        // Release without dragging executes action
        let action = seat.handle_pointer_button(&mut menu, BTN_LEFT, ButtonState::Released);
        assert_eq!(action, Some(ActionId::Terminal));
        assert_eq!(
            menu.phase,
            MenuPhase::ClosingAnimated {
                pending_action: Some(ActionId::Terminal)
            }
        );
    }

    #[test]
    fn test_pointer_button_main_ring_toggle_sub_tier() {
        let mut seat = SeatHandler::new();
        let mut menu = MenuState::new(RadialConfig::default(), false);
        menu.center_x = 200.0;
        menu.center_y = 200.0;
        menu.phase = MenuPhase::Open;
        let mut scratch = make_slice("scratchpad", "Scratchpad", 0);
        scratch.has_sub_tier = true;
        scratch.sub_tier_type = Some(SubTierType::Scratchpad);
        menu.current_slices = vec![scratch];

        seat.handle_pointer_motion(&mut menu, 200.0, 100.0);
        assert_eq!(menu.hovered_index, 0);

        // Press initiates potential drag
        let action = seat.handle_pointer_button(&mut menu, BTN_LEFT, ButtonState::Pressed);
        assert_eq!(action, None);
        assert_eq!(menu.active_sub_tier, None);

        // Release toggles sub-tier open
        let action = seat.handle_pointer_button(&mut menu, BTN_LEFT, ButtonState::Released);
        assert_eq!(action, None);
        assert_eq!(menu.active_sub_tier, Some(SubTierType::Scratchpad));
        assert!(!menu.sub_slices.is_empty());
        assert_eq!(menu.phase, MenuPhase::Open);

        // Click again (Press + Release) toggles sub-tier closed
        let action = seat.handle_pointer_button(&mut menu, BTN_LEFT, ButtonState::Pressed);
        assert_eq!(action, None);
        let action = seat.handle_pointer_button(&mut menu, BTN_LEFT, ButtonState::Released);
        assert_eq!(action, None);
        assert_eq!(menu.active_sub_tier, None);
    }

    #[test]
    fn test_pointer_button_outer_sub_ring() {
        let mut seat = SeatHandler::new();
        let mut menu = MenuState::new(RadialConfig::default(), false);
        menu.center_x = 200.0;
        menu.center_y = 200.0;
        menu.phase = MenuPhase::Open;

        let mut sub1 = make_slice("ws1", "Workspace 1", 0);
        sub1.action = Some(ActionId::MoveToWorkspace(1));
        menu.sub_slices = vec![sub1];
        menu.active_sub_tier = Some(SubTierType::Scratchpad);
        menu.parent_slice_index = 0;
        menu.outer_hovered_index = 0;

        let action = seat.handle_pointer_button(&mut menu, BTN_LEFT, ButtonState::Pressed);
        assert_eq!(action, Some(ActionId::MoveToWorkspace(1)));
        assert_eq!(
            menu.phase,
            MenuPhase::ClosingAnimated {
                pending_action: Some(ActionId::MoveToWorkspace(1))
            }
        );
    }

    #[test]
    fn test_pointer_button_outside_closes() {
        let mut seat = SeatHandler::new();
        let mut menu = MenuState::new(RadialConfig::default(), false);
        menu.center_x = 200.0;
        menu.center_y = 200.0;
        menu.phase = MenuPhase::Open;

        seat.handle_pointer_motion(&mut menu, 600.0, 200.0);
        let action = seat.handle_pointer_button(&mut menu, BTN_LEFT, ButtonState::Pressed);
        assert_eq!(action, None);
        assert_eq!(
            menu.phase,
            MenuPhase::ClosingAnimated {
                pending_action: None
            }
        );
    }

    #[test]
    fn test_drag_reorder_workflow() {
        let mut seat = SeatHandler::new();
        let mut menu = MenuState::new(RadialConfig::default(), false);
        menu.center_x = 200.0;
        menu.center_y = 200.0;
        menu.phase = MenuPhase::Open;
        let mut s0 = make_slice("slice0", "Slice 0", 0);
        s0.action = Some(ActionId::Terminal);
        menu.current_slices = vec![
            s0,
            make_slice("slice1", "Slice 1", 1),
            make_slice("slice2", "Slice 2", 2),
            make_slice("slice3", "Slice 3", 3),
        ];

        // Hover slice 0 (12 o'clock, x=200, y=100)
        seat.handle_pointer_motion(&mut menu, 200.0, 100.0);
        assert_eq!(menu.hovered_index, 0);

        // Press left button to start drag tracking -> does NOT fire Terminal!
        let press_act = seat.handle_pointer_button(&mut menu, BTN_LEFT, ButtonState::Pressed);
        assert_eq!(press_act, None);
        assert_eq!(menu.drag.from_index, 0);

        // Move pointer 50px right (x=250, y=100) -> distance from drag start > 10px
        seat.handle_pointer_motion(&mut menu, 250.0, 100.0);
        assert!(menu.drag.is_dragging);

        // Move to 3 o'clock (x=300, y=200) -> slot 1
        seat.handle_pointer_motion(&mut menu, 300.0, 200.0);
        assert_eq!(menu.drag.target_index, 1);

        // Release button to complete reorder
        let action = seat.handle_pointer_button(&mut menu, BTN_LEFT, ButtonState::Released);
        assert_eq!(action, None);
        assert!(!menu.drag.is_dragging);
        assert_eq!(menu.current_slices[0].id, "slice1");
        assert_eq!(menu.current_slices[1].id, "slice0");
    }

    #[test]
    fn test_sub_drag_file_jump_workflow() {
        let mut seat = SeatHandler::new();
        let mut menu = MenuState::new(RadialConfig::default(), false);
        menu.center_x = 200.0;
        menu.center_y = 200.0;
        menu.phase = MenuPhase::Open;
        menu.open_sub_tier(0, SubTierType::FileJump);
        assert!(menu.sub_slices.len() >= 3);

        let target_0_label = menu.config.file_jump_targets[0].label.clone();
        let target_1_label = menu.config.file_jump_targets[1].label.clone();

        // 1. Hover sub-slice 0
        menu.outer_hovered_index = 0;
        seat.last_x = 200.0;
        seat.last_y = 200.0 + 195.0; // on outer sub-ring radius

        // 2. Press left button -> does NOT open folder! Defers for potential drag.
        let press_act = seat.handle_pointer_button(&mut menu, BTN_LEFT, ButtonState::Pressed);
        assert_eq!(press_act, None);
        assert!(menu.drag.is_sub_drag);
        assert_eq!(menu.drag.from_index, 0);

        // 3. Move > 10px -> threshold met -> is_dragging becomes true
        seat.handle_pointer_motion(&mut menu, 230.0, 200.0 + 195.0);
        assert!(menu.drag.is_dragging);
        assert_eq!(menu.active_sub_tier, Some(SubTierType::FileJump)); // Sub-tier MUST stay open!

        // Force target slot 1
        menu.drag.target_index = 1;

        // 4. Release button to complete reorder
        let release_act = seat.handle_pointer_button(&mut menu, BTN_LEFT, ButtonState::Released);
        assert_eq!(release_act, None); // Does NOT execute action
        assert!(!menu.drag.is_dragging);
        assert_eq!(menu.config.file_jump_targets[0].label, target_1_label);
        assert_eq!(menu.config.file_jump_targets[1].label, target_0_label);
        assert_eq!(menu.sub_slices[0].label, target_1_label);
        assert_eq!(menu.sub_slices[1].label, target_0_label);

        // 5. Normal click (without drag) executes action on release
        menu.outer_hovered_index = 0;
        seat.last_x = 200.0;
        seat.last_y = 395.0;
        let press_act2 = seat.handle_pointer_button(&mut menu, BTN_LEFT, ButtonState::Pressed);
        assert_eq!(press_act2, None); // Deferred
        assert!(!menu.drag.is_dragging);
        let release_act2 = seat.handle_pointer_button(&mut menu, BTN_LEFT, ButtonState::Released);
        assert!(matches!(release_act2, Some(ActionId::JumpToFile(_))));
    }

    #[test]
    fn test_keyboard_events() {
        let mut seat = SeatHandler::new();
        let mut menu = MenuState::new(RadialConfig::default(), false);
        menu.phase = MenuPhase::Open;
        let mut term = make_slice("term", "Terminal", 0);
        term.action = Some(ActionId::Terminal);
        let mut scratch = make_slice("scratch", "Scratchpad", 1);
        scratch.has_sub_tier = true;
        scratch.sub_tier_type = Some(SubTierType::Scratchpad);
        menu.current_slices = vec![term, scratch];

        // Number key '1' (0x31) triggers slice 1 (Terminal)
        let action = seat.handle_key_event(&mut menu, 0x31, KeyState::Pressed);
        assert_eq!(action, Some(ActionId::Terminal));

        // Reset menu to open
        menu.phase = MenuPhase::Open;
        // Number key '2' (0x32) opens sub-tier for Scratchpad
        let action = seat.handle_key_event(&mut menu, 0x32, KeyState::Pressed);
        assert_eq!(action, None);
        assert_eq!(menu.active_sub_tier, Some(SubTierType::Scratchpad));

        // Escape (0xff1b) closes sub-tier first
        let action = seat.handle_key_event(&mut menu, 0xff1b, KeyState::Pressed);
        assert_eq!(action, None);
        assert_eq!(menu.active_sub_tier, None);
        assert_eq!(menu.phase, MenuPhase::Open);

        // Second Escape closes menu
        let action = seat.handle_key_event(&mut menu, 0xff1b, KeyState::Pressed);
        assert_eq!(action, None);
        assert_eq!(
            menu.phase,
            MenuPhase::ClosingAnimated {
                pending_action: None
            }
        );
    }

    #[test]
    fn test_keyboard_flick_release() {
        let mut seat = SeatHandler::new();
        let mut menu = MenuState::new(RadialConfig::default(), false);
        menu.phase = MenuPhase::Open;
        menu.flick_mode_armed = true;
        menu.cursor_moved_flick = true;
        let mut term = make_slice("term", "Terminal", 0);
        term.action = Some(ActionId::Terminal);
        menu.current_slices = vec![term];
        menu.hovered_index = 0;

        // Super key release (0xffeb) in flick mode executes hovered slice immediately
        let action = seat.handle_key_event(&mut menu, 0xffeb, KeyState::Released);
        assert_eq!(action, Some(ActionId::Terminal));
        assert_eq!(
            menu.phase,
            MenuPhase::ClosingAnimated {
                pending_action: Some(ActionId::Terminal)
            }
        );
        assert!(!menu.flick_mode_armed);
    }

    #[test]
    fn test_pointer_axis_volume_and_brightness() {
        let mut seat = SeatHandler::new();
        let mut menu = MenuState::new(RadialConfig::default(), false);
        menu.phase = MenuPhase::Open;
        let vol_slice = make_slice("volume_up", "Volume", 0);
        let bright_slice = make_slice("brightness_up", "Brightness", 1);
        menu.current_slices = vec![vol_slice, bright_slice];

        // Hover volume slice (index 0)
        menu.hovered_index = 0;
        let action = seat.handle_pointer_axis(&mut menu, 10.0);
        assert_eq!(action, Some(ActionId::VolumeUp));
        let action = seat.handle_pointer_axis(&mut menu, -10.0);
        assert_eq!(action, Some(ActionId::VolumeDown));

        // Hover brightness slice (index 1)
        menu.hovered_index = 1;
        let action = seat.handle_pointer_axis(&mut menu, 10.0);
        assert_eq!(action, Some(ActionId::BrightnessUp));
        let action = seat.handle_pointer_axis(&mut menu, -10.0);
        assert_eq!(action, Some(ActionId::BrightnessDown));
    }

    #[test]
    fn test_high_level_pointer_and_key_dispatch() {
        let mut seat = SeatHandler::new();
        let mut menu = MenuState::new(RadialConfig::default(), false);
        menu.center_x = 200.0;
        menu.center_y = 200.0;
        menu.phase = MenuPhase::Open;
        let mut term = make_slice("term", "Terminal", 0);
        term.action = Some(ActionId::Terminal);
        let vol = make_slice("volume_up", "Volume", 1);
        menu.current_slices = vec![term, vol];

        // Motion via PointerEvent
        seat.handle_pointer(
            &mut menu,
            PointerEvent::Motion {
                x: 200.0,
                y: 100.0,
            },
        );
        assert_eq!(menu.hovered_index, 0);

        // Axis via PointerEvent (on volume slice)
        menu.hovered_index = 1;
        let axis_act = seat.handle_pointer(
            &mut menu,
            PointerEvent::Axis { delta: 5.0 },
        );
        assert_eq!(axis_act, Some(ActionId::VolumeUp));

        // Button via PointerEvent (Press + Release cycle for main ring slice)
        menu.hovered_index = 0;
        menu.phase = MenuPhase::Open;
        let press_act = seat.handle_pointer(
            &mut menu,
            PointerEvent::Button {
                button: BTN_LEFT,
                state: ButtonState::Pressed,
            },
        );
        assert_eq!(press_act, None);
        let btn_act = seat.handle_pointer(
            &mut menu,
            PointerEvent::Button {
                button: BTN_LEFT,
                state: ButtonState::Released,
            },
        );
        assert_eq!(btn_act, Some(ActionId::Terminal));

        // Middle button ignored
        assert_eq!(
            seat.handle_pointer_button(&mut menu, BTN_MIDDLE, ButtonState::Pressed),
            None
        );

        // Key via KeyEvent
        menu.hovered_index = -1;
        let act = seat.handle_key(
            &mut menu,
            KeyEvent {
                keysym: 0x31,
                state: KeyState::Pressed,
            },
        );
        assert_eq!(act, Some(ActionId::Terminal));
    }

    #[test]
    fn test_adding_slice_pointer_tracking_and_drop_click() {
        let mut seat = SeatHandler::new();
        let mut menu = MenuState::new(RadialConfig::default(), false);
        menu.center_x = 200.0;
        menu.center_y = 200.0;
        menu.phase = MenuPhase::Open;
        menu.current_slices = vec![
            make_slice("slice0", "Slice 0", 0),
            make_slice("slice1", "Slice 1", 1),
            make_slice("slice2", "Slice 2", 2),
            make_slice("slice3", "Slice 3", 3),
        ];

        // Activate adding slice
        menu.adding_slice = Some(crate::state::menu::AddingSliceState {
            action_id: "firefox".to_string(),
            icon: "globe".to_string(),
            label: "Firefox".to_string(),
            target_index: 0,
            current_x: 200.0,
            current_y: 200.0,
        });

        // 1. Pointer motion to 3 o'clock (x=300, y=200) -> 90 clock degrees
        // slice_angle = 360 / 4 = 90 deg.
        // clock_shifted = (90 + 45) = 135 deg -> floor(135/90) = 1
        seat.handle_pointer_motion(&mut menu, 300.0, 200.0);
        let adding = menu.adding_slice.as_ref().unwrap();
        assert_eq!(adding.current_x, 300.0);
        assert_eq!(adding.current_y, 200.0);
        assert_eq!(adding.target_index, 1);
        assert_eq!(menu.hovered_index, -1);
        assert_eq!(menu.center_hovered, false);

        // 2. Left click drops the slice!
        let press_act = seat.handle_pointer_button(&mut menu, BTN_LEFT, ButtonState::Pressed);
        assert_eq!(press_act, None);
        assert!(menu.adding_slice.is_none(), "adding_slice must be consumed on drop");
        assert_eq!(menu.hovered_index, 1);
        assert_eq!(menu.anim.hover_factor, 1.0);
        assert_eq!(menu.anim.drag_pluck_progress, 0.0);

        // Verify it was inserted at index 1 in config
        let slices = menu.config.get_active_slice_ids(&menu.context.to_string());
        assert!(slices.contains(&"firefox".to_string()));
        assert_eq!(slices[1], "firefox");

        // 3. Mouse release consumes click without executing
        let release_act = seat.handle_pointer_button(&mut menu, BTN_LEFT, ButtonState::Released);
        assert_eq!(release_act, None);
        assert!(!matches!(menu.phase, MenuPhase::ClosingAnimated { .. }));
    }

    #[test]
    fn test_adding_slice_blocks_number_and_flick_keys() {
        let mut seat = SeatHandler::new();
        let mut menu = MenuState::new(RadialConfig::default(), false);
        menu.center_x = 200.0;
        menu.center_y = 200.0;
        menu.phase = MenuPhase::Open;
        menu.current_slices = vec![
            make_slice("slice0", "Slice 0", 0),
            make_slice("slice1", "Slice 1", 1),
        ];

        menu.adding_slice = Some(crate::state::menu::AddingSliceState {
            action_id: "firefox".to_string(),
            icon: "globe".to_string(),
            label: "Firefox".to_string(),
            target_index: 0,
            current_x: 200.0,
            current_y: 200.0,
        });

        // Pressing number key '1' while adding does not execute
        let key_act = seat.handle_key_event(&mut menu, 0x31, KeyState::Pressed);
        assert_eq!(key_act, None);

        // Releasing super/tab in flick mode while adding does not execute
        menu.flick_mode_armed = true;
        menu.cursor_moved_flick = true;
        menu.hovered_index = 0;
        let flick_act = seat.handle_key_event(&mut menu, 0xffeb, KeyState::Released);
        assert_eq!(flick_act, None);
    }
}

