//! Animation engine for radial-dial.
//!
//! Provides easing functions, tweens, animation stages, sequential and
//! parallel animations matching the original QML radialMenu behavior.

/// Easing functions matching Qt's easing curves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Easing {
    /// Overshoot parameter: 1.25 for hover spring, 1.3 for hub pop
    OutBack(f32),
    /// Anticipation (pull-back) before shrink. Used for modal/hub close. s controls anticipation.
    InBack(f32),
    OutCubic,
    OutQuad,
    InQuad,
    InCubic,
}

impl Easing {
    /// Evaluates the easing curve for normalized progress `t` in [0.0, 1.0].
    /// Note: Output value may overshoot beyond [0.0, 1.0] for `OutBack`.
    pub fn value(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match *self {
            Easing::OutBack(s) => {
                let inv = t - 1.0;
                1.0 + (s + 1.0) * inv * inv * inv + s * inv * inv
            }
            Easing::InBack(s) => {
                // Anticipation: dips below 0 before shooting forward
                t * t * ((s + 1.0) * t - s)
            }
            Easing::OutCubic => {
                let inv = 1.0 - t;
                1.0 - inv * inv * inv
            }
            Easing::OutQuad => {
                let inv = 1.0 - t;
                1.0 - inv * inv
            }
            Easing::InQuad => t * t,
            Easing::InCubic => t * t * t,
        }
    }
}

/// A tween interpolating a float between `from` and `to`.
#[derive(Debug, Clone, PartialEq)]
pub struct Tween {
    pub from: f32,
    pub to: f32,
    pub duration_ms: u64,
    pub elapsed_ms: u64,
    pub easing: Easing,
    pub done: bool,
}

impl Tween {
    pub fn new(from: f32, to: f32, duration_ms: u64, easing: Easing) -> Self {
        Self {
            from,
            to,
            duration_ms,
            elapsed_ms: 0,
            easing,
            done: duration_ms == 0,
        }
    }

    /// Advance by dt_ms, return current value. Clamps elapsed to duration.
    /// Sets done=true when elapsed >= duration.
    /// Value may temporarily exceed [from, to] range due to OutBack overshoot —
    /// clamp to (-inf, +inf) NOT to [0, 1].
    pub fn step(&mut self, dt_ms: u64) -> f32 {
        self.elapsed_ms = (self.elapsed_ms + dt_ms).min(self.duration_ms);
        if self.elapsed_ms >= self.duration_ms {
            self.done = true;
        }
        self.current()
    }

    /// Returns current value without advancing.
    pub fn current(&self) -> f32 {
        if self.duration_ms == 0 {
            return self.to;
        }
        let t = (self.elapsed_ms as f32) / (self.duration_ms as f32);
        let progress = self.easing.value(t);
        self.from + (self.to - self.from) * progress
    }
}

/// All animated floats in one place.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimState {
    pub hub_scale: f32,           // 0.0..1.0  center hub pop-in/out
    pub reveal_progress: f32,     // 0.0..N    main ring staggered reveal (N = slice count)
    pub sub_reveal_progress: f32, // 0.0..M    sub-ring staggered reveal
    pub hover_factor: f32,        // 0.0..1.0  main ring hover spring
    pub outer_hover_factor: f32,  // 0.0..1.0  sub-ring hover spring
    pub overall_scale: f32,       // 0.88..1.0 global entrance scale
    pub overall_opacity: f32,     // 0.0..1.0  global entrance fade
    pub hub_hover_factor: f32,    // 0.0..1.0  center hub hover scale spring
    pub hub_pulse_elapsed: f32,   // ms elapsed for micro-pulse after open
    pub hub_pulse_active: bool,   // whether the post-open micro-pulse is running
    pub closing_opacity: f32,     // 1.0..0.0  full-menu fade during close
    pub sub_closing_elapsed: f32, // ms elapsed for sub-ring exit animation
    pub sub_closing_active: bool, // whether sub-ring is doing its close anim
    pub opening_elapsed: f32,
    pub hover_elapsed: f32,
    pub hover_fade_start: f32,
    pub outer_hover_elapsed: f32,
    pub outer_hover_fade_start: f32,
    pub closing_elapsed: f32,
    pub sub_elapsed: f32,
}

impl Default for AnimState {
    fn default() -> Self {
        AnimState {
            hub_scale: 0.0,
            reveal_progress: 0.0,
            sub_reveal_progress: 0.0,
            hover_factor: 0.0,
            outer_hover_factor: 0.0,
            overall_scale: 0.88,
            overall_opacity: 0.0,
            hub_hover_factor: 0.0,
            hub_pulse_elapsed: 0.0,
            hub_pulse_active: false,
            closing_opacity: 1.0,
            sub_closing_elapsed: 0.0,
            sub_closing_active: false,
            opening_elapsed: 0.0,
            hover_elapsed: 0.0,
            hover_fade_start: 1.0,
            outer_hover_elapsed: 0.0,
            outer_hover_fade_start: 1.0,
            closing_elapsed: 0.0,
            sub_elapsed: 0.0,
        }
    }
}

impl AnimState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies a new animated value to the targeted property.
    pub fn apply(&mut self, target: AnimTarget, value: f32) {
        match target {
            AnimTarget::HubScale => self.hub_scale = value,
            AnimTarget::RevealProgress => self.reveal_progress = value,
            AnimTarget::SubRevealProgress => self.sub_reveal_progress = value,
            AnimTarget::HoverFactor => self.hover_factor = value,
            AnimTarget::OuterHoverFactor => self.outer_hover_factor = value,
            AnimTarget::OverallScale => self.overall_scale = value,
            AnimTarget::OverallOpacity => self.overall_opacity = value,
        }
    }

    /// Reads current value for a target property.
    pub fn get(&self, target: AnimTarget) -> f32 {
        match target {
            AnimTarget::HubScale => self.hub_scale,
            AnimTarget::RevealProgress => self.reveal_progress,
            AnimTarget::SubRevealProgress => self.sub_reveal_progress,
            AnimTarget::HoverFactor => self.hover_factor,
            AnimTarget::OuterHoverFactor => self.outer_hover_factor,
            AnimTarget::OverallScale => self.overall_scale,
            AnimTarget::OverallOpacity => self.overall_opacity,
        }
    }
}

/// Which AnimState field to update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnimTarget {
    HubScale,
    RevealProgress,
    SubRevealProgress,
    HoverFactor,
    OuterHoverFactor,
    OverallScale,
    OverallOpacity,
}

/// A stage in a sequential or parallel animation.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimStage {
    pub tween: Tween,
    pub target: AnimTarget,
}

impl AnimStage {
    pub fn new(target: AnimTarget, tween: Tween) -> Self {
        Self { tween, target }
    }
}

/// Runs a list of stages sequentially (each starts when previous is done).
/// Returns true if all stages complete.
#[derive(Debug, Clone, PartialEq)]
pub struct SequentialAnim {
    pub stages: Vec<AnimStage>,
    pub current_stage: usize,
}

impl SequentialAnim {
    pub fn new(stages: Vec<AnimStage>) -> Self {
        Self {
            stages,
            current_stage: 0,
        }
    }

    /// Advance by dt_ms, update anim_state. Returns true when fully done.
    pub fn step(&mut self, mut dt_ms: u64, anim_state: &mut AnimState) -> bool {
        if self.is_done() {
            return true;
        }

        if dt_ms == 0 {
            if self.current_stage < self.stages.len() {
                let stage = &self.stages[self.current_stage];
                anim_state.apply(stage.target, stage.tween.current());
            }
            return self.is_done();
        }

        while dt_ms > 0 && self.current_stage < self.stages.len() {
            let stage = &mut self.stages[self.current_stage];
            let remaining = stage.tween.duration_ms.saturating_sub(stage.tween.elapsed_ms);

            if remaining == 0 {
                stage.tween.done = true;
                anim_state.apply(stage.target, stage.tween.current());
                self.current_stage += 1;
                continue;
            }

            if dt_ms >= remaining {
                let val = stage.tween.step(remaining);
                anim_state.apply(stage.target, val);
                dt_ms -= remaining;
                self.current_stage += 1;
            } else {
                let val = stage.tween.step(dt_ms);
                anim_state.apply(stage.target, val);
                dt_ms = 0;
            }
        }

        self.is_done()
    }

    pub fn is_done(&self) -> bool {
        self.current_stage >= self.stages.len()
    }
}

/// Runs multiple AnimStages in parallel.
#[derive(Debug, Clone, PartialEq)]
pub struct ParallelAnim {
    pub stages: Vec<AnimStage>,
}

impl ParallelAnim {
    pub fn new(stages: Vec<AnimStage>) -> Self {
        Self { stages }
    }

    /// Advance by dt_ms, update anim_state. Returns true when fully done.
    pub fn step(&mut self, dt_ms: u64, anim_state: &mut AnimState) -> bool {
        let mut all_done = true;
        for stage in &mut self.stages {
            let val = stage.tween.step(dt_ms);
            anim_state.apply(stage.target, val);
            if !stage.tween.done {
                all_done = false;
            }
        }
        all_done
    }

    pub fn is_done(&self) -> bool {
        self.stages.iter().all(|s| s.tween.done)
    }
}

// ──────────────── Convenience Constructors ────────────────

/// Blossom entrance: hub pop -> slices reveal
pub fn blossom_animation(slice_count: usize, is_low_end: bool) -> SequentialAnim {
    let hub_duration = if is_low_end { 50 } else { 150 };
    let reveal_duration = if is_low_end {
        70
    } else {
        // Phase 1: 28ms/slice (was 45ms), minimum 160ms (was 220ms)
        (slice_count as u64 * 28).max(160)
    };

    let hub_stage = AnimStage::new(
        AnimTarget::HubScale,
        Tween::new(0.0, 1.0, hub_duration, Easing::OutBack(1.3)),
    );

    let reveal_stage = AnimStage::new(
        AnimTarget::RevealProgress,
        Tween::new(0.0, slice_count as f32, reveal_duration, Easing::OutCubic),
    );

    SequentialAnim::new(vec![hub_stage, reveal_stage])
}

/// Collapse: slices -> hub
pub fn collapse_animation(slice_count: usize, is_low_end: bool) -> SequentialAnim {
    let collapse_duration = if is_low_end { 40 } else { 90 };
    let hub_duration = if is_low_end { 20 } else { 50 };

    let slices_stage = AnimStage::new(
        AnimTarget::RevealProgress,
        Tween::new(slice_count as f32, 0.0, collapse_duration, Easing::InCubic),
    );

    let hub_stage = AnimStage::new(
        AnimTarget::HubScale,
        Tween::new(1.0, 0.0, hub_duration, Easing::InQuad),
    );

    SequentialAnim::new(vec![slices_stage, hub_stage])
}

/// Sub-ring open — flat 150ms (Phase 3: no per-count scaling for snappier feel)
pub fn sub_reveal_animation(sub_count: usize, is_low_end: bool) -> SequentialAnim {
    let duration = if is_low_end {
        80
    } else {
        150 // was (sub_count * 40).max(180); flat 150ms feels snappier
    };

    let stage = AnimStage::new(
        AnimTarget::SubRevealProgress,
        Tween::new(0.0, sub_count as f32, duration, Easing::OutCubic),
    );

    SequentialAnim::new(vec![stage])
}

/// Sub-ring close
pub fn sub_collapse_animation(sub_count: usize, is_low_end: bool) -> SequentialAnim {
    let duration = if is_low_end {
        50
    } else {
        (sub_count as u64 * 26).max(130)
    };

    let stage = AnimStage::new(
        AnimTarget::SubRevealProgress,
        Tween::new(sub_count as f32, 0.0, duration, Easing::InCubic),
    );

    SequentialAnim::new(vec![stage])
}

/// Hover spring for main ring slice: 0.0 -> 1.0 in 110ms with OutBack(1.35)
/// Phase 1: was 160ms OutBack(1.25) — faster + slightly more dramatic
pub fn hover_spring_tween() -> Tween {
    Tween::new(0.0, 1.0, 110, Easing::OutBack(1.35))
}

/// Hover fade for main ring slice: 1.0 -> 0.0 in 80ms with OutCubic
/// Phase 1: was 120ms OutQuad — crisper release
pub fn hover_fade_tween() -> Tween {
    Tween::new(1.0, 0.0, 80, Easing::OutCubic)
}

/// Hover spring for sub-ring slice: 0.0 -> 1.0 in 95ms with OutBack(1.3)
/// Phase 1: was 140ms OutBack(1.2) — snappier
pub fn outer_hover_spring_tween() -> Tween {
    Tween::new(0.0, 1.0, 95, Easing::OutBack(1.3))
}

/// Hover fade for sub-ring slice: 1.0 -> 0.0 in 65ms with OutCubic
/// Phase 1: was 100ms OutQuad — faster release
pub fn outer_hover_fade_tween() -> Tween {
    Tween::new(1.0, 0.0, 65, Easing::OutCubic)
}

/// Hub hover scale spring: 0.0 -> 1.0 in 80ms with OutBack(1.4)
/// Phase 2: hub now smoothly scales to 1.06 instead of instant jump
pub fn hub_hover_spring_tween() -> Tween {
    Tween::new(0.0, 1.0, 80, Easing::OutBack(1.4))
}

/// Hub hover scale fade: 1.0 -> 0.0 in 60ms with OutQuad
pub fn hub_hover_fade_tween() -> Tween {
    Tween::new(1.0, 0.0, 60, Easing::OutQuad)
}

/// Hub micro-pulse after open: 0.0 -> 1.0 in 80ms with OutBack(2.0) — big spring
pub fn hub_pulse_tween() -> Tween {
    Tween::new(0.0, 1.0, 80, Easing::OutBack(2.0))
}

/// Modal open spring: 0.0 -> 1.0 in 160ms with OutBack(1.1)
/// Phase 4: was 180ms OutCubic
pub fn modal_open_tween() -> Tween {
    Tween::new(0.0, 1.0, 160, Easing::OutBack(1.1))
}

/// Modal close: 1.0 -> 0.0 in 120ms with InBack(0.9) — scale+fade exit
pub fn modal_close_tween() -> Tween {
    Tween::new(1.0, 0.0, 120, Easing::InBack(0.9))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_out_back_overshoot() {
        let mut t = Tween::new(0.0, 1.0, 150, Easing::OutBack(1.3));
        // At t=0.5 (halfway), OutBack should exceed 1.0 (overshoot)
        t.step(75);
        let v = t.current();
        // OutBack overshoots, so at midpoint value can be >1
        assert!(v >= 0.0); // just check it's positive and progressing
        assert!(v > 1.0, "Expected OutBack(1.3) to overshoot 1.0 at t=0.5, got {}", v);
    }

    #[test]
    fn test_in_back_anticipation() {
        // InBack should dip below 0 before reaching 1
        let t = Easing::InBack(1.0);
        // At t=0 => 0, at t=1 => 1
        assert!((t.value(0.0) - 0.0).abs() < 1e-6);
        assert!((t.value(1.0) - 1.0).abs() < 1e-6);
        // In the middle it should be lower than a linear curve (anticipation)
        // specifically at t=0.3, InBack(1.0) dips negative
        assert!(t.value(0.25) < 0.0, "InBack should dip below 0 for anticipation");
    }

    #[test]
    fn test_tween_done_at_end() {
        let mut t = Tween::new(0.0, 1.0, 100, Easing::OutCubic);
        t.step(100);
        assert!(t.done);
        // Value should be at or very near 1.0
        assert!((t.current() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_blossom_durations_normal() {
        let anim = blossom_animation(8, false);
        // Stage 0: hub, 150ms. Stage 1: reveal 8*28=224ms > 160 minimum
        assert_eq!(anim.stages[0].tween.duration_ms, 150);
        assert_eq!(anim.stages[1].tween.duration_ms, 224); // 8*28=224
    }

    #[test]
    fn test_blossom_durations_low_end() {
        let anim = blossom_animation(4, true);
        assert_eq!(anim.stages[0].tween.duration_ms, 50);
        assert_eq!(anim.stages[1].tween.duration_ms, 70); // low-end override unchanged
    }

    #[test]
    fn test_blossom_durations_minimum() {
        // 3 slices: 3*28=84 < 160 minimum => should clamp to 160
        let anim = blossom_animation(3, false);
        assert_eq!(anim.stages[0].tween.duration_ms, 150);
        assert_eq!(anim.stages[1].tween.duration_ms, 160); // minimum enforced
    }

    #[test]
    fn test_collapse_durations_normal_and_low_end() {
        let anim_normal = collapse_animation(8, false);
        assert_eq!(anim_normal.stages[0].tween.duration_ms, 90);
        assert_eq!(anim_normal.stages[1].tween.duration_ms, 50);

        let anim_low = collapse_animation(8, true);
        assert_eq!(anim_low.stages[0].tween.duration_ms, 40);
        assert_eq!(anim_low.stages[1].tween.duration_ms, 20);
    }

    #[test]
    fn test_sub_reveal_and_collapse_durations() {
        // sub_reveal is now flat 150ms for all counts (non-low-end)
        let sub_rev_normal = sub_reveal_animation(6, false);
        assert_eq!(sub_rev_normal.stages[0].tween.duration_ms, 150); // flat 150ms

        let sub_rev_min = sub_reveal_animation(2, false);
        assert_eq!(sub_rev_min.stages[0].tween.duration_ms, 150); // flat 150ms

        let sub_rev_low = sub_reveal_animation(6, true);
        assert_eq!(sub_rev_low.stages[0].tween.duration_ms, 80); // low-end unchanged

        let sub_col_normal = sub_collapse_animation(6, false);
        assert_eq!(sub_col_normal.stages[0].tween.duration_ms, 156); // 6*26 = 156 > 130

        let sub_col_min = sub_collapse_animation(2, false);
        assert_eq!(sub_col_min.stages[0].tween.duration_ms, 130); // 2*26 = 52 < 130 -> 130

        let sub_col_low = sub_collapse_animation(6, true);
        assert_eq!(sub_col_low.stages[0].tween.duration_ms, 50);
    }

    #[test]
    fn test_sequential_anim_execution() {
        let mut anim = blossom_animation(4, true);
        let mut state = AnimState::default();

        assert_eq!(anim.stages.len(), 2);
        assert_eq!(anim.current_stage, 0);
        assert!(!anim.is_done());

        // Step through stage 0 (hub pop: 50ms)
        let done1 = anim.step(25, &mut state);
        assert!(!done1);
        assert_eq!(anim.current_stage, 0);
        assert!(state.hub_scale > 0.0);

        // Step by 35ms: finishes stage 0 (25ms remaining) and advances into stage 1 by 10ms
        let done2 = anim.step(35, &mut state);
        assert!(!done2);
        assert_eq!(anim.current_stage, 1);
        assert_eq!(state.hub_scale, 1.0);
        assert!(state.reveal_progress > 0.0);

        // Step to the end (stage 1 has 70ms total, 10ms consumed, 60ms remaining)
        let done3 = anim.step(60, &mut state);
        assert!(done3);
        assert!(anim.is_done());
        assert_eq!(state.hub_scale, 1.0);
        assert_eq!(state.reveal_progress, 4.0);
    }

    #[test]
    fn test_parallel_anim_execution() {
        let stages = vec![
            AnimStage::new(AnimTarget::OverallScale, Tween::new(0.88, 1.0, 100, Easing::OutQuad)),
            AnimStage::new(AnimTarget::OverallOpacity, Tween::new(0.0, 1.0, 50, Easing::OutQuad)),
        ];
        let mut anim = ParallelAnim::new(stages);
        let mut state = AnimState::default();

        assert!(!anim.is_done());

        // Step 50ms: stage 1 (opacity) completes, stage 0 (scale) reaches halfway
        let done1 = anim.step(50, &mut state);
        assert!(!done1);
        assert!(!anim.is_done());
        assert!((state.overall_opacity - 1.0).abs() < 0.001);
        assert!(state.overall_scale > 0.88 && state.overall_scale < 1.0);

        // Step another 50ms: stage 0 finishes
        let done2 = anim.step(50, &mut state);
        assert!(done2);
        assert!(anim.is_done());
        assert!((state.overall_scale - 1.0).abs() < 0.001);
        assert!((state.overall_opacity - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_all_easing_curves() {
        let curves = vec![
            Easing::OutBack(1.3),
            Easing::InBack(1.0),
            Easing::OutCubic,
            Easing::OutQuad,
            Easing::InQuad,
            Easing::InCubic,
        ];

        for easing in curves {
            assert_eq!(easing.value(0.0), 0.0, "Easing {:?} at t=0", easing);
            assert!((easing.value(1.0) - 1.0).abs() < 1e-6, "Easing {:?} at t=1", easing);
        }

        // Test in vs out curvature at t=0.5
        assert_eq!(Easing::InQuad.value(0.5), 0.25);
        assert_eq!(Easing::InCubic.value(0.5), 0.125);
        assert_eq!(Easing::OutQuad.value(0.5), 0.75);
        assert_eq!(Easing::OutCubic.value(0.5), 0.875);
    }

    #[test]
    fn test_anim_state_default() {
        let state = AnimState::default();
        assert_eq!(state.hub_scale, 0.0);
        assert_eq!(state.reveal_progress, 0.0);
        assert_eq!(state.sub_reveal_progress, 0.0);
        assert_eq!(state.hover_factor, 0.0);
        assert_eq!(state.outer_hover_factor, 0.0);
        assert_eq!(state.overall_scale, 0.88); // updated from 0.85
        assert_eq!(state.overall_opacity, 0.0);
        assert_eq!(state.hub_hover_factor, 0.0);
        assert!(!state.hub_pulse_active);
        assert_eq!(state.closing_opacity, 1.0);
        assert!(!state.sub_closing_active);
    }

    #[test]
    fn test_hover_tweens() {
        let mut spring = hover_spring_tween();
        assert_eq!(spring.duration_ms, 110); // was 160
        assert_eq!(spring.from, 0.0);
        assert_eq!(spring.to, 1.0);
        spring.step(110);
        assert!(spring.done);
        assert!((spring.current() - 1.0).abs() < 0.001);

        let mut fade = hover_fade_tween();
        assert_eq!(fade.duration_ms, 80); // was 120
        assert_eq!(fade.from, 1.0);
        assert_eq!(fade.to, 0.0);
        fade.step(80);
        assert!(fade.done);
        assert!(fade.current().abs() < 0.001);

        let outer_spring = outer_hover_spring_tween();
        assert_eq!(outer_spring.duration_ms, 95); // was 140

        let outer_fade = outer_hover_fade_tween();
        assert_eq!(outer_fade.duration_ms, 65); // was 100
    }

    #[test]
    fn test_new_tweens() {
        let hub_spring = hub_hover_spring_tween();
        assert_eq!(hub_spring.duration_ms, 80);
        assert_eq!(hub_spring.from, 0.0);
        assert_eq!(hub_spring.to, 1.0);

        let hub_fade = hub_hover_fade_tween();
        assert_eq!(hub_fade.duration_ms, 60);
        assert_eq!(hub_fade.from, 1.0);
        assert_eq!(hub_fade.to, 0.0);

        let pulse = hub_pulse_tween();
        assert_eq!(pulse.duration_ms, 80);

        let modal_open = modal_open_tween();
        assert_eq!(modal_open.duration_ms, 160); // was 180

        let modal_close = modal_close_tween();
        assert_eq!(modal_close.duration_ms, 120);
        assert_eq!(modal_close.from, 1.0);
        assert_eq!(modal_close.to, 0.0);
    }
}
