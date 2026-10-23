//! Original launcher: allocate once, animate CSS-equivalent content, reclaim at 210ms.
#[derive(Debug)]
pub struct Motion {
    pub visual: f32,
    pub allocated: f32,
    pub target: f32,
    from: f32,
    started: Option<f64>,
}
impl Default for Motion {
    fn default() -> Self {
        Self {
            visual: 84.,
            allocated: 84.,
            target: 84.,
            from: 84.,
            started: None,
        }
    }
}
fn ease_out(x: f32) -> f32 {
    // CSS ease-out = cubic-bezier(0, 0, .58, 1), invert its x coordinate.
    let (mut lo, mut hi) = (0., 1.);
    for _ in 0..18 {
        let t = (lo + hi) * 0.5;
        let bx = 3. * (1. - t) * t * t * 0.58 + t * t * t;
        if bx < x { lo = t } else { hi = t }
    }
    let t = (lo + hi) * 0.5;
    3. * (1. - t) * t * t + t * t * t
}
impl Motion {
    pub fn step(&mut self, now: f64, target: f32, immediate: bool) -> bool {
        if let Some(start) = self.started {
            let t = ((now - start) / 0.180).clamp(0., 1.) as f32;
            self.visual = if t >= 1. {
                self.target
            } else {
                self.from + (self.target - self.from) * ease_out(t)
            };
            if now - start >= 0.210 {
                self.allocated = self.target;
                self.started = None;
            }
        }
        if (target - self.target).abs() >= 0.5 {
            self.from = self.visual;
            self.target = target;
            self.allocated = self.allocated.max(target);
            self.started = Some(now);
        }
        if immediate {
            self.visual = self.target;
            self.allocated = self.target;
            self.started = None;
        }
        self.started.is_some()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_allocation_changes_only_at_transition_boundaries() {
        let mut motion = Motion::default();
        motion.step(0., 642., false);
        assert_eq!(motion.allocated, 642.);
        for n in 1..180 {
            motion.step(n as f64 / 1000., 642., false);
            assert_eq!(motion.allocated, 642.);
        }
        assert!(motion.visual > 84. && motion.visual < 642.);
        motion.step(0.22, 642., false);
        assert_eq!(motion.visual, 642.);
        motion.step(1., 84., false);
        for n in 1..210 {
            motion.step(1. + n as f64 / 1000., 84., false);
            assert_eq!(motion.allocated, 642.);
        }
        motion.step(1.211, 84., false);
        assert_eq!(motion.allocated, 84.);
        assert_eq!(motion.visual, 84.);
    }
    #[test]
    fn retarget_is_continuous_and_reduced_motion_is_immediate() {
        let mut m = Motion::default();
        m.step(0., 642., false);
        m.step(0.09, 642., false);
        let visual = m.visual;
        m.step(0.09, 220., false);
        assert_eq!(m.visual, visual);
        m.step(0.2, 84., true);
        assert_eq!(m.visual, 84.);
        assert_eq!(m.allocated, 84.);
    }
}
