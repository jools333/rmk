//! Inertial / Kinetic Scrolling for trackballs and pointing devices.
//!
//! Provides natural momentum-based scrolling with friction decay,
//! velocity smoothing, and instantaneous touch-braking.

use embassy_time::Instant;

/// Configuration for inertial / kinetic scrolling.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct InertialScrollConfig {
    /// Enable or disable inertial scrolling.
    pub enabled: bool,
    /// Friction decay factor per tick (e.g. 0.92 for natural glide).
    /// Lower values (e.g. 0.85) brake faster; higher values (e.g. 0.95) glide longer.
    pub friction: f32,
    /// Minimum velocity (in sensor counts/ms) to trigger coasting.
    /// Deliberate slow scrolling below this threshold stops immediately without coasting.
    pub min_velocity: f32,
    /// Maximum initial coasting velocity to prevent spinning out of control.
    pub max_velocity: f32,
    /// Stop velocity threshold below which coasting terminates.
    pub stop_velocity: f32,
    /// Idle duration in milliseconds after physical motion before coasting begins.
    pub release_timeout_ms: f32,
}

impl Default for InertialScrollConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            friction: 0.92,
            min_velocity: 0.8,
            max_velocity: 8.0,
            stop_velocity: 0.05,
            release_timeout_ms: 35.0,
        }
    }
}

/// Runtime state for inertial / kinetic scroller.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct InertialScroller {
    /// Active configuration.
    pub config: InertialScrollConfig,
    /// Current velocity along X axis (counts/ms).
    pub vel_x: f32,
    /// Current velocity along Y axis (counts/ms).
    pub vel_y: f32,
    /// Whether coasting / inertia is currently active.
    pub coasting: bool,
    /// Timestamp of most recent physical motion event.
    pub last_motion_time: Option<Instant>,
    /// Fallback dt in milliseconds between physical motion events.
    pub last_dt_ms: f32,
    /// Timestamp of previous periodic tick.
    pub last_tick_time: Option<Instant>,
    /// Sub-pixel carry remainder for X axis.
    pub carry_x: f32,
    /// Sub-pixel carry remainder for Y axis.
    pub carry_y: f32,
}

impl InertialScroller {
    /// Create a new inertial scroller with the given configuration.
    pub fn new(config: InertialScrollConfig) -> Self {
        Self {
            config,
            vel_x: 0.0,
            vel_y: 0.0,
            coasting: false,
            last_motion_time: None,
            last_dt_ms: 10.0,
            last_tick_time: None,
            carry_x: 0.0,
            carry_y: 0.0,
        }
    }

    /// Check if inertial coasting is currently active.
    pub fn is_coasting(&self) -> bool {
        self.coasting
    }

    /// Handle physical sensor motion event while in scroll mode.
    /// Updates velocity estimate and instantly brakes any ongoing coasting.
    pub fn on_motion(&mut self, dx: i16, dy: i16, now: Instant) {
        if !self.config.enabled {
            return;
        }

        // Physical touch acts as an immediate brake
        if self.coasting {
            self.cancel();
        }

        let dt_ms = match self.last_motion_time {
            Some(prev) => {
                let elapsed_us = now.duration_since(prev).as_micros();
                let ms = elapsed_us as f32 / 1000.0;
                if ms < 0.5 {
                    self.last_dt_ms
                } else if ms > 100.0 {
                    10.0
                } else {
                    ms
                }
            }
            None => 10.0,
        };
        self.last_motion_time = Some(now);
        self.last_dt_ms = dt_ms;

        let inst_vx = (dx as f32) / dt_ms;
        let inst_vy = (dy as f32) / dt_ms;

        // Exponential moving average filter for velocity (alpha = 0.6)
        const ALPHA: f32 = 0.6;
        self.vel_x = ALPHA * inst_vx + (1.0 - ALPHA) * self.vel_x;
        self.vel_y = ALPHA * inst_vy + (1.0 - ALPHA) * self.vel_y;
    }

    /// Periodic update tick (called from processor polling loop).
    /// Detects release of the trackball, applies friction decay, and emits virtual scroll deltas.
    pub fn tick(&mut self, now: Instant) -> Option<(i16, i16)> {
        if !self.config.enabled {
            return None;
        }

        // If not yet coasting, check if user has released the trackball after a fast flick
        if !self.coasting {
            if let Some(last_time) = self.last_motion_time {
                let idle_ms = now.duration_since(last_time).as_micros() as f32 / 1000.0;
                if idle_ms >= self.config.release_timeout_ms {
                    let speed = libm::sqrtf(self.vel_x * self.vel_x + self.vel_y * self.vel_y);
                    if speed >= self.config.min_velocity && idle_ms <= 150.0 {
                        // Fast flick detected! Enter coasting phase.
                        self.coasting = true;
                        if self.config.max_velocity > 0.0 && speed > self.config.max_velocity {
                            let scale = self.config.max_velocity / speed;
                            self.vel_x *= scale;
                            self.vel_y *= scale;
                        }
                        self.last_tick_time = Some(now);
                        self.carry_x = 0.0;
                        self.carry_y = 0.0;
                    } else {
                        // Deliberate slow stop or idle timeout: clear velocity
                        self.cancel();
                        return None;
                    }
                }
            }
        }

        // Coasting phase: decay velocity and produce displacement
        if self.coasting {
            let dt_ms = match self.last_tick_time {
                Some(prev) => {
                    let elapsed_us = now.duration_since(prev).as_micros();
                    let ms = elapsed_us as f32 / 1000.0;
                    if ms < 1.0 { 12.0 } else if ms > 50.0 { 12.0 } else { ms }
                }
                None => 12.0,
            };
            self.last_tick_time = Some(now);

            // Apply friction decay
            self.vel_x *= self.config.friction;
            self.vel_y *= self.config.friction;

            let speed = libm::sqrtf(self.vel_x * self.vel_x + self.vel_y * self.vel_y);
            if speed < self.config.stop_velocity {
                // Coasting finished
                self.cancel();
                return None;
            }

            // Displacement for this tick
            let step_x_f = self.vel_x * dt_ms + self.carry_x;
            let step_y_f = self.vel_y * dt_ms + self.carry_y;

            let step_x_rounded = libm::roundf(step_x_f);
            let step_y_rounded = libm::roundf(step_y_f);

            let step_x = step_x_rounded as i16;
            let step_y = step_y_rounded as i16;

            self.carry_x = step_x_f - (step_x as f32);
            self.carry_y = step_y_f - (step_y as f32);

            if step_x == 0 && step_y == 0 {
                return None;
            }

            return Some((step_x, step_y));
        }

        None
    }

    /// Cancel any ongoing coasting and reset velocity/carry state (braking).
    pub fn cancel(&mut self) {
        self.coasting = false;
        self.vel_x = 0.0;
        self.vel_y = 0.0;
        self.last_motion_time = None;
        self.last_tick_time = None;
        self.carry_x = 0.0;
        self.carry_y = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embassy_time::Duration;

    #[test]
    fn test_slow_scroll_no_inertia() {
        let config = InertialScrollConfig {
            enabled: true,
            friction: 0.92,
            min_velocity: 1.0,
            max_velocity: 8.0,
            stop_velocity: 0.05,
            release_timeout_ms: 35.0,
        };
        let mut scroller = InertialScroller::new(config);
        let mut now = Instant::from_ticks(0);

        // Slow motion: 2 counts over 20ms = 0.1 counts/ms (well below min_velocity 1.0)
        scroller.on_motion(0, 2, now);
        now += Duration::from_millis(20);
        scroller.on_motion(0, 2, now);

        // Advance beyond release timeout
        now += Duration::from_millis(40);
        let step = scroller.tick(now);
        assert_eq!(step, None);
        assert!(!scroller.is_coasting());
    }

    #[test]
    fn test_fast_flick_triggers_inertia_and_decays() {
        let config = InertialScrollConfig {
            enabled: true,
            friction: 0.85,
            min_velocity: 1.0,
            max_velocity: 8.0,
            stop_velocity: 0.1,
            release_timeout_ms: 35.0,
        };
        let mut scroller = InertialScroller::new(config);
        let mut now = Instant::from_ticks(0);

        // Fast flick: 30 counts over 10ms = 3.0 counts/ms
        scroller.on_motion(0, 30, now);
        now += Duration::from_millis(10);
        scroller.on_motion(0, 30, now);

        // Release timeout reached (40ms later)
        now += Duration::from_millis(40);
        let step1 = scroller.tick(now);
        assert!(step1.is_some());
        assert!(scroller.is_coasting());

        // Subsequent ticks continue decaying
        let mut tick_count = 0;
        while scroller.is_coasting() && tick_count < 50 {
            now += Duration::from_millis(12);
            let _ = scroller.tick(now);
            tick_count += 1;
        }

        // Coasting eventually stops
        assert!(!scroller.is_coasting());
        assert!(tick_count > 3 && tick_count < 30);
    }

    #[test]
    fn test_touch_brakes_inertia() {
        let config = InertialScrollConfig::default();
        let mut scroller = InertialScroller::new(config);
        let mut now = Instant::from_ticks(0);

        // Fast flick
        scroller.on_motion(0, 40, now);
        now += Duration::from_millis(10);
        scroller.on_motion(0, 40, now);

        now += Duration::from_millis(40);
        let _ = scroller.tick(now);
        assert!(scroller.is_coasting());

        // New physical touch arrives
        now += Duration::from_millis(10);
        scroller.on_motion(0, 1, now);

        // Coasting must be immediately canceled
        assert!(!scroller.is_coasting());
    }
}
