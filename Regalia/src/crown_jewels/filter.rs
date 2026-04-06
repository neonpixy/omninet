use serde::{Deserialize, Serialize};

/// Configuration for the One Euro Filter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OneEuroFilterConfig {
    /// Base cutoff frequency in Hz. Lower = smoother.
    pub min_cutoff: f64,
    /// Speed coefficient. Higher = opens faster when moving quickly.
    pub beta: f64,
    /// Derivative filter cutoff in Hz.
    pub d_cutoff: f64,
}

impl Default for OneEuroFilterConfig {
    fn default() -> Self {
        Self {
            min_cutoff: 1.0,
            beta: 0.007,
            d_cutoff: 1.0,
        }
    }
}

/// Adaptive low-pass filter for signal smoothing.
/// Reduces jitter at rest while staying responsive during fast motion.
///
/// Used by CrystalKit's Gleam system for smooth light tracking.
#[derive(Debug, Clone)]
pub struct OneEuroFilter {
    config: OneEuroFilterConfig,
    prev_raw: f64,
    prev_filtered: f64,
    prev_dx_filtered: f64,
    initialized: bool,
}

impl OneEuroFilter {
    /// Create a new filter with the given configuration. Not yet initialized.
    pub fn new(config: OneEuroFilterConfig) -> Self {
        Self {
            config,
            prev_raw: 0.0,
            prev_filtered: 0.0,
            prev_dx_filtered: 0.0,
            initialized: false,
        }
    }

    /// Filter a single sample. `dt` is time since last sample in seconds.
    pub fn filter(&mut self, value: f64, dt: f64) -> f64 {
        if !self.initialized {
            self.prev_raw = value;
            self.prev_filtered = value;
            self.prev_dx_filtered = 0.0;
            self.initialized = true;
            return value;
        }

        if dt < 1e-10 {
            return self.prev_filtered;
        }

        // Estimate derivative.
        let dx = (value - self.prev_raw) / dt;

        // Filter the derivative.
        let alpha_d = Self::alpha(self.config.d_cutoff, dt);
        self.prev_dx_filtered = alpha_d * dx + (1.0 - alpha_d) * self.prev_dx_filtered;

        // Adaptive cutoff based on speed.
        let cutoff = self.config.min_cutoff + self.config.beta * self.prev_dx_filtered.abs();

        // Filter the value.
        let alpha = Self::alpha(cutoff, dt);
        self.prev_raw = value;
        self.prev_filtered = alpha * value + (1.0 - alpha) * self.prev_filtered;

        self.prev_filtered
    }

    /// Reset the filter state.
    pub fn reset(&mut self) {
        self.initialized = false;
        self.prev_raw = 0.0;
        self.prev_filtered = 0.0;
        self.prev_dx_filtered = 0.0;
    }

    /// Low-pass filter coefficient.
    fn alpha(cutoff: f64, dt: f64) -> f64 {
        let tau = 1.0 / (2.0 * std::f64::consts::PI * cutoff);
        1.0 / (1.0 + tau / dt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_sample_passthrough() {
        let mut f = OneEuroFilter::new(OneEuroFilterConfig::default());
        let result = f.filter(5.0, 1.0 / 60.0);
        assert!((result - 5.0).abs() < 1e-10);
    }

    #[test]
    fn low_speed_smoothing() {
        let mut f = OneEuroFilter::new(OneEuroFilterConfig {
            min_cutoff: 1.0,
            beta: 0.0, // no speed adaptation
            d_cutoff: 1.0,
        });
        let dt = 1.0 / 60.0;
        f.filter(0.0, dt);

        // Small step — should be heavily smoothed.
        let result = f.filter(0.01, dt);
        assert!(result < 0.01); // Filtered value lags behind input.
        assert!(result > 0.0);
    }

    #[test]
    fn high_speed_responsive() {
        let mut f = OneEuroFilter::new(OneEuroFilterConfig {
            min_cutoff: 1.0,
            beta: 1.0, // high speed adaptation
            d_cutoff: 1.0,
        });
        let dt = 1.0 / 60.0;
        f.filter(0.0, dt);

        // Large step — speed coefficient opens the filter.
        let result = f.filter(100.0, dt);
        // Should track closer to the input than the low-speed case.
        assert!(result > 10.0);
    }

    #[test]
    fn reset_reinitializes() {
        let mut f = OneEuroFilter::new(OneEuroFilterConfig::default());
        f.filter(5.0, 1.0 / 60.0);
        f.filter(10.0, 1.0 / 60.0);
        f.reset();

        // After reset, next sample should pass through.
        let result = f.filter(42.0, 1.0 / 60.0);
        assert!((result - 42.0).abs() < 1e-10);
    }

    #[test]
    fn config_serde_roundtrip() {
        let config = OneEuroFilterConfig {
            min_cutoff: 1.2,
            beta: 0.05,
            d_cutoff: 1.0,
        };
        let json = serde_json::to_string(&config).unwrap();
        let decoded: OneEuroFilterConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, decoded);
    }
}
