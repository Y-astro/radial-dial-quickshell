// src/state/drag.rs

#[derive(Debug, Clone, PartialEq)]
pub struct DragState {
    pub is_dragging: bool,
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
}
