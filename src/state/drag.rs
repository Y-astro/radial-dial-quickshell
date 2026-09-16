// src/state/drag.rs

#[derive(Debug, Clone, PartialEq)]
pub struct DragState {
    pub is_dragging: bool,
    pub is_sub_drag: bool,
    pub from_index: i32,     // -1 if none
    pub target_index: i32,   // -1 if none
    pub drag_start_x: f32,
    pub drag_start_y: f32,
    pub current_x: f32,
    pub current_y: f32,
    pub threshold_met: bool, // drag distance > 10px threshold
}

impl Default for DragState {
    fn default() -> Self {
        Self {
            is_dragging: false,
            is_sub_drag: false,
            from_index: -1,
            target_index: -1,
            drag_start_x: 0.0,
            drag_start_y: 0.0,
            current_x: 0.0,
            current_y: 0.0,
            threshold_met: false,
        }
    }
}

impl DragState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn start_drag(&mut self, index: i32, x: f32, y: f32) {
        self.is_dragging = false;
        self.is_sub_drag = false;
        self.from_index = index;
        self.target_index = index;
        self.drag_start_x = x;
        self.drag_start_y = y;
        self.current_x = x;
        self.current_y = y;
        self.threshold_met = false;
    }

    pub fn start_sub_drag(&mut self, index: i32, x: f32, y: f32) {
        self.is_dragging = false;
        self.is_sub_drag = true;
        self.from_index = index;
        self.target_index = index;
        self.drag_start_x = x;
        self.drag_start_y = y;
        self.current_x = x;
        self.current_y = y;
        self.threshold_met = false;
    }

    pub fn update_position(&mut self, x: f32, y: f32) {
        self.current_x = x;
        self.current_y = y;
        let dx = x - self.drag_start_x;
        let dy = y - self.drag_start_y;
        if (dx * dx + dy * dy).sqrt() >= 10.0 {
            self.threshold_met = true;
        }
    }

    pub fn update_target_slot(&mut self, cx: f32, cy: f32, slice_count: usize, slice_angle: f32) {
        if slice_count == 0 || slice_angle <= 0.0 {
            return;
        }
        let dx = self.current_x - cx;
        let dy = self.current_y - cy;
        let raw_angle = dy.atan2(dx).to_degrees();
        let mut clock_angle = (raw_angle + 90.0) % 360.0;
        if clock_angle < 0.0 {
            clock_angle += 360.0;
        }
        let target = ((clock_angle / slice_angle).floor() as usize) % slice_count;
        self.target_index = target as i32;
    }

    pub fn update_sub_target_slot(
        &mut self,
        cx: f32,
        cy: f32,
        valid_count: usize,
        total_slices: usize,
        sub_start_deg: f32,
        sub_slice_width_deg: f32,
    ) {
        if valid_count == 0 || total_slices == 0 || sub_slice_width_deg <= 0.0 {
            return;
        }
        let dx = self.current_x - cx;
        let dy = self.current_y - cy;
        let raw_angle = dy.atan2(dx).to_degrees();
        let mut canvas_angle = raw_angle % 360.0;
        if canvas_angle < 0.0 {
            canvas_angle += 360.0;
        }
        let mut norm_start = sub_start_deg % 360.0;
        if norm_start < 0.0 {
            norm_start += 360.0;
        }
        let mut rel_angle = (canvas_angle - norm_start) % 360.0;
        if rel_angle < 0.0 {
            rel_angle += 360.0;
        }

        let total_span = (total_slices as f32 * sub_slice_width_deg).min(359.0);
        let dead_zone_mid = (360.0 + total_span) / 2.0;

        if rel_angle <= total_span {
            let slot = (rel_angle / sub_slice_width_deg).floor() as usize;
            self.target_index = slot.min(valid_count.saturating_sub(1)) as i32;
        } else if rel_angle <= dead_zone_mid {
            self.target_index = valid_count.saturating_sub(1) as i32;
        } else {
            self.target_index = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drag_state_threshold() {
        let mut drag = DragState::default();
        assert_eq!(drag.from_index, -1);
        assert_eq!(drag.target_index, -1);
        assert!(!drag.is_dragging);
        assert!(!drag.threshold_met);

        drag.start_drag(2, 100.0, 100.0);
        assert_eq!(drag.from_index, 2);
        assert_eq!(drag.target_index, 2);
        assert_eq!(drag.drag_start_x, 100.0);
        assert_eq!(drag.drag_start_y, 100.0);
        assert!(!drag.threshold_met);

        // Move by 5px (< 10px threshold)
        drag.update_position(105.0, 100.0);
        assert_eq!(drag.current_x, 105.0);
        assert_eq!(drag.current_y, 100.0);
        assert!(!drag.threshold_met);

        // Move to 10px distance
        drag.update_position(110.0, 100.0);
        assert!(drag.threshold_met);

        // Reset
        drag.reset();
        assert_eq!(drag.from_index, -1);
        assert_eq!(drag.target_index, -1);
        assert!(!drag.threshold_met);
        assert!(!drag.is_dragging);
    }

    #[test]
    fn test_drag_target_slot() {
        let mut drag = DragState::default();
        drag.start_drag(0, 200.0, 200.0);
        // Center is (200.0, 200.0)
        // Move to the right: dx=50, dy=0 -> raw_angle=0°, clock_angle = 90° (3 o'clock)
        drag.update_position(250.0, 200.0);
        // For 4 slices, slice_angle = 90°. 90° / 90° = 1
        drag.update_target_slot(200.0, 200.0, 4, 90.0);
        assert_eq!(drag.target_index, 1);

        // Move straight down: dx=0, dy=50 -> raw_angle=90°, clock_angle = 180° (6 o'clock)
        drag.update_position(200.0, 250.0);
        drag.update_target_slot(200.0, 200.0, 4, 90.0);
        assert_eq!(drag.target_index, 2);
    }

    #[test]
    fn test_sub_drag_state() {
        let mut drag = DragState::default();
        drag.start_sub_drag(1, 150.0, 150.0);
        assert!(drag.is_sub_drag);
        assert_eq!(drag.from_index, 1);
        assert_eq!(drag.target_index, 1);
        assert!(!drag.threshold_met);

        drag.update_position(165.0, 150.0);
        assert!(drag.threshold_met);

        drag.reset();
        assert!(!drag.is_sub_drag);
        assert_eq!(drag.from_index, -1);
    }

    #[test]
    fn test_sub_drag_target_slot() {
        let mut drag = DragState::default();
        drag.start_sub_drag(0, 200.0, 200.0);
        // Center is (200.0, 200.0)
        // 4 folders (valid_count=4), plus 1 add button (total_slices=5).
        // sub_start_deg = 0.0, sub_slice_width_deg = 30.0.
        // Fan occupies 0°..150°.
        // angle = 15° (mid slot 0) -> slot 0
        let a0 = 15.0_f32.to_radians();
        drag.update_position(200.0 + 100.0 * a0.cos(), 200.0 + 100.0 * a0.sin());
        drag.update_sub_target_slot(200.0, 200.0, 4, 5, 0.0, 30.0);
        assert_eq!(drag.target_index, 0);

        // angle = 45° (mid slot 1) -> slot 1
        let a1 = 45.0_f32.to_radians();
        drag.update_position(200.0 + 100.0 * a1.cos(), 200.0 + 100.0 * a1.sin());
        drag.update_sub_target_slot(200.0, 200.0, 4, 5, 0.0, 30.0);
        assert_eq!(drag.target_index, 1);

        // angle = 75° (mid slot 2) -> slot 2
        let a2 = 75.0_f32.to_radians();
        drag.update_position(200.0 + 100.0 * a2.cos(), 200.0 + 100.0 * a2.sin());
        drag.update_sub_target_slot(200.0, 200.0, 4, 5, 0.0, 30.0);
        assert_eq!(drag.target_index, 2);

        // Over + Add button (slot 4, angle = 135°): clamps to valid_count - 1 = 3
        let a4 = 135.0_f32.to_radians();
        drag.update_position(200.0 + 100.0 * a4.cos(), 200.0 + 100.0 * a4.sin());
        drag.update_sub_target_slot(200.0, 200.0, 4, 5, 0.0, 30.0);
        assert_eq!(drag.target_index, 3);

        // Before slot 0 (angle = -10° = 350°): clamps to 0
        let aneg = (-10.0_f32).to_radians();
        drag.update_position(200.0 + 100.0 * aneg.cos(), 200.0 + 100.0 * aneg.sin());
        drag.update_sub_target_slot(200.0, 200.0, 4, 5, 0.0, 30.0);
        assert_eq!(drag.target_index, 0);
    }
}
