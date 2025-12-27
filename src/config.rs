//! # Configuration
//!
//! All configurable settings for Quest Shadowplay.
//!
//! ## Plain English
//!
//! Like a car's settings panel - adjust buffer length, video quality,
//! which buttons trigger saves, and where to save files.

use std::fmt;

// ============================================
// TRIGGER BUTTON OPTIONS
// ============================================

/// Which button combination triggers saving a clip.
///
/// We use button combinations (not single buttons) to prevent
/// accidental saves during gameplay.
#[derive(Clone, Debug, PartialEq)]
pub enum TriggerButton {
    /// Hold left grip + left trigger together
    LeftGripAndTrigger,

    /// Hold right grip + right trigger together
    RightGripAndTrigger,

    /// Press both grip buttons simultaneously
    BothGrips,
}

impl Default for TriggerButton {
    fn default() -> Self {
        Self::LeftGripAndTrigger
    }
}

// ============================================
// MAIN CONFIGURATION
// ============================================

/// All configuration options for Quest Shadowplay.
#[derive(Clone, Debug)]
pub struct Config {
    // ----------------------------------------
    // BUFFER SETTINGS
    // ----------------------------------------
    /// How many seconds of footage to keep in memory (5-60)
    pub buffer_duration_seconds: f32,

    /// Target frames per second to capture (72, 90, or 120)
    pub target_fps: u32,

    // ----------------------------------------
    // INPUT SETTINGS
    // ----------------------------------------
    /// Which button(s) trigger a save
    pub trigger_button: TriggerButton,

    /// Minimum time between saves in milliseconds
    pub save_cooldown_ms: u32,

    // ----------------------------------------
    // OUTPUT SETTINGS
    // ----------------------------------------
    /// Directory where clips are saved
    pub output_directory: String,

    /// Video encoding bitrate in bits per second
    pub video_bitrate: u32,

    /// JPEG quality for buffered frames (0-100)
    pub jpeg_quality: u8,

    // ----------------------------------------
    // PERFORMANCE SETTINGS
    // ----------------------------------------
    /// Skip frames if processing takes too long
    pub skip_on_lag: bool,

    // ----------------------------------------
    // FEEDBACK SETTINGS
    // ----------------------------------------
    /// Enable haptic (vibration) feedback on save
    pub haptic_feedback: bool,
}

impl Config {
    /// Creates configuration with default values.
    pub fn default() -> Self {
        Self {
            // Buffer: 10 seconds at 90 FPS
            buffer_duration_seconds: 10.0,
            target_fps: 90,

            // Input: Left grip + trigger, 500ms cooldown
            trigger_button: TriggerButton::default(),
            save_cooldown_ms: 500,

            // Output: Standard location, 20 Mbps, 80% JPEG quality
            output_directory: default_output_directory(),
            video_bitrate: 20_000_000,
            jpeg_quality: 80,

            // Performance: Skip on lag
            skip_on_lag: true,

            // Feedback: Haptics enabled
            haptic_feedback: true,
        }
    }

    /// Validates configuration and returns any errors.
    pub fn validate(&self) -> Vec<ConfigError> {
        let mut errors = Vec::new();

        // Buffer duration
        if self.buffer_duration_seconds < 5.0 {
            errors.push(ConfigError::BufferTooShort(self.buffer_duration_seconds));
        }
        if self.buffer_duration_seconds > 60.0 {
            errors.push(ConfigError::BufferTooLong(self.buffer_duration_seconds));
        }

        // FPS
        if self.target_fps < 30 || self.target_fps > 144 {
            errors.push(ConfigError::InvalidFps(self.target_fps));
        }

        // Bitrate
        if self.video_bitrate < 1_000_000 {
            errors.push(ConfigError::BitrateTooLow(self.video_bitrate));
        }
        if self.video_bitrate > 100_000_000 {
            errors.push(ConfigError::BitrateTooHigh(self.video_bitrate));
        }

        errors
    }

    /// Calculates the number of frames the buffer will hold.
    pub fn buffer_frame_count(&self) -> usize {
        (self.buffer_duration_seconds * self.target_fps as f32).ceil() as usize
    }

    /// Estimates memory usage in megabytes.
    ///
    /// Assumes ~100KB per compressed frame at quality 80.
    pub fn estimated_memory_mb(&self) -> f32 {
        let frames = self.buffer_frame_count();
        let bytes_per_frame = 100_000.0 * (self.jpeg_quality as f32 / 80.0);
        (frames as f32 * bytes_per_frame) / (1024.0 * 1024.0)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::default()
    }
}

/// Returns the default output directory based on platform.
fn default_output_directory() -> String {
    #[cfg(target_os = "android")]
    {
        "/sdcard/QuestShadowplay/".to_string()
    }

    #[cfg(not(target_os = "android"))]
    {
        // Use temp directory for non-Android platforms
        let mut path = std::env::temp_dir();
        path.push("QuestShadowplay");
        path.to_string_lossy().to_string()
    }
}

// ============================================
// CONFIGURATION ERRORS
// ============================================

/// Errors that can occur with configuration values.
#[derive(Debug, Clone)]
pub enum ConfigError {
    /// Buffer duration too short
    BufferTooShort(f32),

    /// Buffer duration too long
    BufferTooLong(f32),

    /// FPS outside valid range
    InvalidFps(u32),

    /// Bitrate too low
    BitrateTooLow(u32),

    /// Bitrate too high
    BitrateTooHigh(u32),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferTooShort(val) => {
                write!(f, "Buffer {} seconds too short (min 5)", val)
            }
            Self::BufferTooLong(val) => {
                write!(f, "Buffer {} seconds too long (max 60)", val)
            }
            Self::InvalidFps(val) => {
                write!(f, "FPS {} outside valid range (30-144)", val)
            }
            Self::BitrateTooLow(val) => {
                write!(f, "Bitrate {} too low", val)
            }
            Self::BitrateTooHigh(val) => {
                write!(f, "Bitrate {} too high", val)
            }
        }
    }
}

impl std::error::Error for ConfigError {}

// ============================================
// TESTS
// ============================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.buffer_duration_seconds, 10.0);
        assert_eq!(config.target_fps, 90);
        assert!(config.validate().is_empty());
    }

    #[test]
    fn test_buffer_frame_count() {
        let config = Config::default();
        // 10 seconds at 90 FPS = 900 frames
        assert_eq!(config.buffer_frame_count(), 900);
    }

    #[test]
    fn test_validation_short_buffer() {
        let mut config = Config::default();
        config.buffer_duration_seconds = 2.0;
        let errors = config.validate();
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_memory_estimation() {
        let config = Config::default();
        let memory = config.estimated_memory_mb();
        // Should be around 90 MB for 900 frames at 100KB each
        assert!(memory > 50.0 && memory < 150.0);
    }
}
