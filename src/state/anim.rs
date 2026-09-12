//! Animation engine for radial-dial.
//!
//! Provides easing functions, tweens, animation stages, sequential and
//! parallel animations matching the original QML radialMenu behavior.

/// Easing functions matching Qt's easing curves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Easing {
    /// Overshoot parameter: 1.25 for hover spring, 1.3 for hub pop
    OutBack(f32),
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
    pub overall_scale: f32,       // 0.85..1.0 global entrance scale
    pub overall_opacity: f32,     // 0.0..1.0  global entrance fade
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
            overall_scale: 0.85,
            overall_opacity: 0.0,
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
        (slice_count as u64 * 45).max(220)
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

/// Sub-ring open
pub fn sub_reveal_animation(sub_count: usize, is_low_end: bool) -> SequentialAnim {
    let duration = if is_low_end {
        80
    } else {
        (sub_count as u64 * 40).max(180)
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

/// Hover spring for main ring slice: 0.0 -> 1.0 in 160ms with OutBack(1.25)
pub fn hover_spring_tween() -> Tween {
    Tween::new(0.0, 1.0, 160, Easing::OutBack(1.25))
}

/// Hover fade for main ring slice: 1.0 -> 0.0 in 120ms with OutQuad
pub fn hover_fade_tween() -> Tween {
    Tween::new(1.0, 0.0, 120, Easing::OutQuad)
}

/// Hover spring for sub-ring slice: 0.0 -> 1.0 in 140ms with OutBack(1.2)
pub fn outer_hover_spring_tween() -> Tween {
    Tween::new(0.0, 1.0, 140, Easing::OutBack(1.2))
}

/// Hover fade for sub-ring slice: 1.0 -> 0.0 in 100ms with OutQuad
pub fn outer_hover_fade_tween() -> Tween {
    Tween::new(1.0, 0.0, 100, Easing::OutQuad)
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
        // Stage 0: hub, 150ms. Stage 1: reveal 8*45=360ms > 220, so 360ms
        assert_eq!(anim.stages[0].tween.duration_ms, 150);
        assert_eq!(anim.stages[1].tween.duration_ms, 360);
    }

    #[test]
    fn test_blossom_durations_low_end() {
        let anim = blossom_animation(4, true);
        assert_eq!(anim.stages[0].tween.duration_ms, 50);
        assert_eq!(anim.stages[1].tween.duration_ms, 70); // low-end override
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
        let sub_rev_normal = sub_reveal_animation(6, false);
        assert_eq!(sub_rev_normal.stages[0].tween.duration_ms, 240); // 6*40 = 240 > 180

        let sub_rev_min = sub_reveal_animation(2, false);
        assert_eq!(sub_rev_min.stages[0].tween.duration_ms, 180); // 2*40 = 80 < 180 -> 180

        let sub_rev_low = sub_reveal_animation(6, true);
        assert_eq!(sub_rev_low.stages[0].tween.duration_ms, 80);

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
            AnimStage::new(AnimTarget::OverallScale, Tween::new(0.85, 1.0, 100, Easing::OutQuad)),
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
        assert!(state.overall_scale > 0.85 && state.overall_scale < 1.0);

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
        assert_eq!(state.overall_scale, 0.85);
        assert_eq!(state.overall_opacity, 0.0);
    }

    #[test]
    fn test_hover_tweens() {
        let mut spring = hover_spring_tween();
        assert_eq!(spring.duration_ms, 160);
        assert_eq!(spring.from, 0.0);
        assert_eq!(spring.to, 1.0);
        spring.step(160);
        assert!(spring.done);
        assert!((spring.current() - 1.0).abs() < 0.001);

        let mut fade = hover_fade_tween();
        assert_eq!(fade.duration_ms, 120);
        assert_eq!(fade.from, 1.0);
        assert_eq!(fade.to, 0.0);
        fade.step(120);
        assert!(fade.done);
        assert!(fade.current().abs() < 0.001);

        let outer_spring = outer_hover_spring_tween();
        assert_eq!(outer_spring.duration_ms, 140);

        let outer_fade = outer_hover_fade_tween();
        assert_eq!(outer_fade.duration_ms, 100);
    }
}
