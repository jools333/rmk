//! Mouse acceleration algorithms inspired by Leetmouse / RawAccel.
//!
//! Provides dynamic speed-based mouse acceleration with carry accumulation
//! for precise sub-pixel tracking at low speeds and swift flicking at high speeds.

use embassy_time::Instant;

/// Acceleration curve algorithms.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum AccelCurve {
    /// Linear acceleration:
    /// factor = 1.0 + accel * max(0.0, speed - offset)
    Linear,
    /// Classic (Power) acceleration:
    /// factor = (1.0 + accel * max(0.0, speed - offset))^exponent
    Classic,
    /// Natural / Sigmoid acceleration:
    /// factor = sens_cap / (1.0 + exp(midpoint - max(0.0, speed - offset)))
    Sigmoid,
}

impl Default for AccelCurve {
    fn default() -> Self {
        Self::Linear
    }
}

/// Configuration for mouse acceleration.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct AccelConfig {
    /// Base sensitivity applied at minimal speeds (e.g. 0.35 for sniper precision).
    pub base_sens: f32,
    /// Acceleration rate coefficient (e.g. 0.6).
    pub accel: f32,
    /// Maximum acceleration multiplier cap (e.g. 5.0 to clamp factor).
    /// If <= 1.0, capping is disabled.
    pub sens_cap: f32,
    /// Upper limit on input physical speed before applying acceleration (0.0 disables speed cap).
    pub speed_cap: f32,
    /// Speed deadzone threshold before acceleration begins in counts/ms (e.g. 0.0).
    pub offset: f32,
    /// Exponent for Classic acceleration mode (e.g. 1.5 or 2.0).
    pub exponent: f32,
    /// Midpoint for Sigmoid acceleration mode (e.g. 2.0).
    pub midpoint: f32,
    /// Selected acceleration curve.
    pub curve: AccelCurve,
}

impl Default for AccelConfig {
    fn default() -> Self {
        Self {
            base_sens: 0.35,
            accel: 0.6,
            sens_cap: 5.0,
            speed_cap: 0.0,
            offset: 0.0,
            exponent: 1.0,
            midpoint: 2.0,
            curve: AccelCurve::Linear,
        }
    }
}

/// State of mouse acceleration processor.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct MouseAccelerator {
    /// Active configuration.
    pub config: AccelConfig,
    /// Sub-pixel carry remainder for X axis.
    pub carry_x: f32,
    /// Sub-pixel carry remainder for Y axis.
    pub carry_y: f32,
    /// Timestamp of previous event.
    pub last_time: Option<Instant>,
    /// Fallback dt in milliseconds if timing is discontinuous (default 8.0 ms).
    pub last_dt_ms: f32,
}

impl MouseAccelerator {
    /// Create a new mouse accelerator with the given configuration.
    pub fn new(config: AccelConfig) -> Self {
        Self {
            config,
            carry_x: 0.0,
            carry_y: 0.0,
            last_time: None,
            last_dt_ms: 8.0,
        }
    }

    /// Reset sub-pixel accumulators and timing state.
    pub fn reset(&mut self) {
        self.carry_x = 0.0;
        self.carry_y = 0.0;
        self.last_time = None;
        self.last_dt_ms = 8.0;
    }

    /// Process raw integer deltas and return accelerated integer output deltas.
    pub fn accelerate(&mut self, x: i16, y: i16, now: Instant) -> (i16, i16) {
        if x == 0 && y == 0 {
            return (0, 0);
        }

        let dt_ms = match self.last_time {
            Some(prev) => {
                let elapsed_us = now.duration_since(prev).as_micros();
                let ms = elapsed_us as f32 / 1000.0;
                // If dt is too small (< 0.5ms) due to burst/interrupt bunching, fallback to last valid dt.
                // If dt is too large (> 100ms), user paused motion or lifted hand: reset to default poll dt.
                if ms < 0.5 {
                    self.last_dt_ms
                } else if ms > 100.0 {
                    8.0
                } else {
                    ms
                }
            }
            None => 8.0,
        };
        self.last_time = Some(now);
        self.last_dt_ms = dt_ms;

        let dx_f = x as f32;
        let dy_f = y as f32;

        // Calculate Euclidean distance traveled
        let mut distance = libm::sqrtf(dx_f * dx_f + dy_f * dy_f);

        // Apply speed cap on raw input distance if configured
        if self.config.speed_cap > 0.0 && distance > self.config.speed_cap {
            distance = self.config.speed_cap;
        }

        // Speed in counts per millisecond
        let speed = distance / dt_ms;
        let effective_speed = if speed > self.config.offset {
            speed - self.config.offset
        } else {
            0.0
        };

        // Calculate acceleration factor
        let mut factor = match self.config.curve {
            AccelCurve::Linear => {
                if effective_speed > 0.0 {
                    1.0 + self.config.accel * effective_speed
                } else {
                    1.0
                }
            }
            AccelCurve::Classic => {
                if effective_speed > 0.0 {
                    let base = 1.0 + self.config.accel * effective_speed;
                    libm::powf(base, self.config.exponent)
                } else {
                    1.0
                }
            }
            AccelCurve::Sigmoid => {
                let diff = self.config.midpoint - effective_speed;
                let exp_val = libm::expf(diff);
                self.config.sens_cap / (1.0 + exp_val)
            }
        };

        // Clamp factor with sens_cap if enabled
        if self.config.sens_cap > 1.0 && factor > self.config.sens_cap {
            factor = self.config.sens_cap;
        }

        // Final sensitivity multiplier (Leetmouse / RawAccel model)
        let total_mult = factor * self.config.base_sens;

        // Apply multiplier and accumulate carry
        let out_x_f = dx_f * total_mult + self.carry_x;
        let out_y_f = dy_f * total_mult + self.carry_y;

        let out_x_rounded = libm::roundf(out_x_f);
        let out_y_rounded = libm::roundf(out_y_f);

        // Clamp to i16 bounds
        let out_x = if out_x_rounded > i16::MAX as f32 {
            i16::MAX
        } else if out_x_rounded < i16::MIN as f32 {
            i16::MIN
        } else {
            out_x_rounded as i16
        };

        let out_y = if out_y_rounded > i16::MAX as f32 {
            i16::MAX
        } else if out_y_rounded < i16::MIN as f32 {
            i16::MIN
        } else {
            out_y_rounded as i16
        };

        // Update carry buffer
        self.carry_x = out_x_f - (out_x as f32);
        self.carry_y = out_y_f - (out_y as f32);

        (out_x, out_y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embassy_time::Duration;

    #[test]
    fn test_slow_movement_subpixel_carry() {
        let config = AccelConfig {
            base_sens: 0.3333,
            accel: 0.5,
            sens_cap: 5.0,
            speed_cap: 0.0,
            offset: 0.0,
            exponent: 1.0,
            midpoint: 2.0,
            curve: AccelCurve::Linear,
        };
        let mut accel = MouseAccelerator::new(config);
        let mut now = Instant::from_ticks(0);

        // First tick: dx = 1 at 10ms -> low speed -> mult ~0.333 -> round is 0, carry ~0.333
        let (x1, y1) = accel.accelerate(1, 0, now);
        assert_eq!(x1, 0);
        assert_eq!(y1, 0);
        assert!(accel.carry_x > 0.3 && accel.carry_x < 0.4);

        // Second tick: dx = 1 at +10ms -> total ~0.666 -> round is 1, carry ~-0.333
        now += Duration::from_millis(10);
        let (x2, y2) = accel.accelerate(1, 0, now);
        assert_eq!(x2, 1);
        assert_eq!(y2, 0);

        // Third tick: dx = 1 at +10ms -> total 0.333 - 0.333 ~ 0.0 -> round is 0
        now += Duration::from_millis(10);
        let (x3, _y3) = accel.accelerate(1, 0, now);
        assert_eq!(x3, 0);

        // Fourth tick: dx = 1 at +10ms -> total ~0.333 -> round is 0
        now += Duration::from_millis(10);
        let (x4, _y4) = accel.accelerate(1, 0, now);
        assert_eq!(x4, 0);

        // Fifth tick: dx = 1 at +10ms -> total ~0.666 -> round is 1
        now += Duration::from_millis(10);
        let (x5, _y5) = accel.accelerate(1, 0, now);
        assert_eq!(x5, 1);
    }

    #[test]
    fn test_fast_flick_acceleration_and_cap() {
        let config = AccelConfig {
            base_sens: 0.5,
            accel: 1.0,
            sens_cap: 4.0,
            speed_cap: 0.0,
            offset: 0.0,
            exponent: 1.0,
            midpoint: 2.0,
            curve: AccelCurve::Linear,
        };
        let mut accel = MouseAccelerator::new(config);
        let now = Instant::from_ticks(0);

        // Very high speed movement (dx = 100 in 8ms -> speed = 12.5 counts/ms)
        // factor = 1 + 1.0 * 12.5 = 13.5 -> clamped to sens_cap = 4.0
        // total_mult = 4.0 * 0.5 = 2.0
        // out_x should be 100 * 2.0 = 200
        let (out_x, out_y) = accel.accelerate(100, 0, now);
        assert_eq!(out_x, 200);
        assert_eq!(out_y, 0);
    }

    #[test]
    fn test_reset_clears_carry() {
        let config = AccelConfig::default();
        let mut accel = MouseAccelerator::new(config);
        let now = Instant::from_ticks(0);

        let _ = accel.accelerate(1, 0, now);
        assert_ne!(accel.carry_x, 0.0);

        accel.reset();
        assert_eq!(accel.carry_x, 0.0);
        assert_eq!(accel.carry_y, 0.0);
        assert!(accel.last_time.is_none());
    }
}
