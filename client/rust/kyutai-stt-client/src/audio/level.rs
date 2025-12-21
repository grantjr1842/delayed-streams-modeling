//! Audio level metering for real-time input visualization.

/// Audio level measurements in decibels.
#[derive(Clone, Copy, Debug, Default)]
pub struct AudioLevel {
    /// RMS (root mean square) level in dBFS.
    pub rms_db: f32,
    /// Peak level in dBFS.
    pub peak_db: f32,
}

/// Minimum dB floor to avoid -infinity for silence.
const DB_FLOOR: f32 = -60.0;

impl AudioLevel {
    /// Compute audio levels from f32 samples (expected range: -1.0 to 1.0).
    pub fn compute(samples: &[f32]) -> Self {
        if samples.is_empty() {
            return Self {
                rms_db: DB_FLOOR,
                peak_db: DB_FLOOR,
            };
        }

        // Single pass to compute RMS and peak.
        let mut sum_sq = 0.0f32;
        let mut peak = 0.0f32;
        for &sample in samples {
            let abs = sample.abs();
            sum_sq += sample * sample;
            if abs > peak {
                peak = abs;
            }
        }
        let rms = (sum_sq / samples.len() as f32).sqrt();

        // Convert to dB with floor
        let rms_db = linear_to_db(rms);
        let peak_db = linear_to_db(peak);

        Self { rms_db, peak_db }
    }

    /// Returns true if the audio is essentially silent.
    pub fn is_silent(&self) -> bool {
        self.rms_db <= DB_FLOOR + 1.0
    }
}

/// Convert linear amplitude to decibels, with floor.
fn linear_to_db(linear: f32) -> f32 {
    if linear <= 0.0 {
        DB_FLOOR
    } else {
        (20.0 * linear.log10()).max(DB_FLOOR)
    }
}

/// A level meter that tracks audio levels with optional smoothing.
#[derive(Clone, Debug)]
pub struct LevelMeter {
    /// Smoothing factor for level display (0.0 = no smoothing, 1.0 = infinite smoothing).
    smoothing: f32,
    /// Current smoothed RMS level.
    smoothed_rms_db: f32,
    /// Current smoothed peak level (with slower decay).
    smoothed_peak_db: f32,
}

impl Default for LevelMeter {
    fn default() -> Self {
        Self::new(0.7)
    }
}

impl LevelMeter {
    /// Create a new level meter with the given smoothing factor.
    ///
    /// Smoothing should be between 0.0 and 1.0:
    /// - 0.0: No smoothing, instant response
    /// - 0.7: Recommended for visual display
    /// - 0.9: Very smooth, slow response
    pub fn new(smoothing: f32) -> Self {
        Self {
            smoothing: smoothing.clamp(0.0, 0.99),
            smoothed_rms_db: DB_FLOOR,
            smoothed_peak_db: DB_FLOOR,
        }
    }

    /// Process a buffer of samples and update the meter.
    pub fn process(&mut self, samples: &[f32]) -> AudioLevel {
        let level = AudioLevel::compute(samples);

        // Apply exponential smoothing
        self.smoothed_rms_db =
            self.smoothing * self.smoothed_rms_db + (1.0 - self.smoothing) * level.rms_db;

        // Peak has asymmetric attack/release: fast attack, slow release
        if level.peak_db > self.smoothed_peak_db {
            self.smoothed_peak_db = level.peak_db; // Instant attack
        } else {
            // Slow release
            self.smoothed_peak_db =
                self.smoothing * self.smoothed_peak_db + (1.0 - self.smoothing) * level.peak_db;
        }

        AudioLevel {
            rms_db: self.smoothed_rms_db,
            peak_db: self.smoothed_peak_db,
        }
    }

    /// Reset the meter to initial state.
    pub fn reset(&mut self) {
        self.smoothed_rms_db = DB_FLOOR;
        self.smoothed_peak_db = DB_FLOOR;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_level_silence() {
        let samples = vec![0.0; 1920];
        let level = AudioLevel::compute(&samples);
        assert!(level.rms_db <= DB_FLOOR + 0.1);
        assert!(level.peak_db <= DB_FLOOR + 0.1);
        assert!(level.is_silent());
    }

    #[test]
    fn test_level_full_scale() {
        // Full-scale sine wave approximation (samples at +1 and -1)
        let samples: Vec<f32> = (0..1920)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect();
        let level = AudioLevel::compute(&samples);
        // RMS of alternating +1/-1 is 1.0, which is 0 dB
        assert!((level.rms_db - 0.0).abs() < 0.1);
        assert!((level.peak_db - 0.0).abs() < 0.1);
    }

    #[test]
    fn test_level_half_amplitude() {
        // Half amplitude: samples at +0.5 and -0.5
        let samples: Vec<f32> = (0..1920)
            .map(|i| if i % 2 == 0 { 0.5 } else { -0.5 })
            .collect();
        let level = AudioLevel::compute(&samples);
        // RMS of alternating +0.5/-0.5 is 0.5, which is ~-6.02 dB
        assert!((level.rms_db - (-6.02)).abs() < 0.1);
        assert!((level.peak_db - (-6.02)).abs() < 0.1);
    }

    #[test]
    fn test_level_meter_smoothing() {
        let mut meter = LevelMeter::new(0.5);

        // Process silence
        let level1 = meter.process(&vec![0.0; 1920]);
        assert!(level1.rms_db <= DB_FLOOR + 1.0);

        // Process loud signal
        let loud_samples: Vec<f32> = (0..1920)
            .map(|i| if i % 2 == 0 { 0.8 } else { -0.8 })
            .collect();
        let level2 = meter.process(&loud_samples);

        // Should be between silence and full level due to smoothing
        assert!(level2.rms_db > DB_FLOOR);
        assert!(level2.rms_db < 0.0);
    }

    #[test]
    fn test_empty_samples() {
        let level = AudioLevel::compute(&[]);
        assert_eq!(level.rms_db, DB_FLOOR);
        assert_eq!(level.peak_db, DB_FLOOR);
    }
}
