//! # Configuration Module
//!
//! This module handles all configurable settings for Quest Shadowplay.
//!
//! ## Plain English Explanation
//!
//! Just like a car has settings (seat position, mirror angles, radio presets),
//! our app has settings too. This module defines what those settings are
//! and what their default values should be.
//!
//! Settings include:
//! - How many seconds to keep in the buffer
//! - What button triggers a save
//! - Where to save video files
//! - Video quality settings

use std::path::PathBuf;

// ============================================
// TRIGGER BUTTON OPTIONS
// ============================================

/// Which button combination triggers saving a clip
///
/// ## Plain English
///
/// We need a way for you to tell the app "save that!"
/// Different people prefer different buttons, so we offer options.
#[derive(Clone, Debug, PartialEq)]
pub enum TriggerButton {
    /// Hold left grip + left trigger together
    ///
    /// ## Why This Default?
    /// It's hard to press accidentally, but easy to do intentionally.
    /// Both buttons are on the same hand, so you don't need to
    /// coordinate two hands.
    LeftGripAndTrigger,
    
    /// Hold right grip + right trigger together
    RightGripAndTrigger,
    
    /// Press both grip buttons simultaneously
    BothGrips,
    
    /// Use a custom button combination (advanced)
    Custom {
        /// Description of the custom binding
        description: String,
    },
}

impl Default for TriggerButton {
    fn default() -> Self {
        Self::LeftGripAndTrigger
    }
}

// ============================================
// MAIN CONFIGURATION
// ============================================

/// All configuration options for Quest Shadowplay
///
/// ## Plain English
///
/// This is the "settings menu" of our app. Each field is one setting
/// you can adjust to customize how the app behaves.
#[derive(Clone, Debug)]
pub struct Config {
    // ----------------------------------------
    // BUFFER SETTINGS
    // "How much to remember"
    // ----------------------------------------
    
    /// How many seconds of footage to keep in memory
    ///
    /// ## Plain English
    /// If set to 10.0, pressing save will give you the last 10 seconds.
    /// Higher = more footage, but uses more memory.
    ///
    /// ## Limits
    /// - Minimum: 5 seconds (short but useful)
    /// - Maximum: 60 seconds (one minute uses ~500MB RAM)
    /// - Default: 10 seconds (good balance)
    pub buffer_duration_seconds: f32,
    
    /// Target frames per second to capture
    ///
    /// ## Plain English
    /// Quest 3 can run at 72, 90, or 120 FPS. We try to match this.
    /// Higher FPS = smoother video, but more memory and CPU usage.
    pub target_fps: u32,
    
    // ----------------------------------------
    // INPUT SETTINGS
    // "How to trigger saves"
    // ----------------------------------------
    
    /// Which button(s) trigger a save
    pub trigger_button: TriggerButton,
    
    /// Minimum time between saves (prevents accidental double-saves)
    ///
    /// ## Plain English
    /// If you press the button and it bounces, we don't want to start
    /// two saves. This is the "cooldown" period.
    pub save_cooldown_ms: u32,
    
    // ----------------------------------------
    // OUTPUT SETTINGS
    // "Where and how to save"
    // ----------------------------------------
    
    /// Directory where clips are saved
    ///
    /// ## Default Location
    /// `/sdcard/QuestShadowplay/` - visible when Quest is connected to PC
    pub output_directory: String,
    
    /// Video encoding bitrate in bits per second
    ///
    /// ## Plain English
    /// Higher bitrate = better quality, bigger files.
    /// - 10 Mbps: Good quality, ~75 MB for 60 seconds
    /// - 20 Mbps: Great quality, ~150 MB for 60 seconds
    /// - 50 Mbps: Excellent quality, ~375 MB for 60 seconds
    pub video_bitrate: u32,
    
    /// JPEG compression quality for buffered frames (0-100)
    ///
    /// ## Plain English
    /// While frames sit in memory, they're JPEG compressed.
    /// This doesn't affect final video quality much (re-encoded anyway).
    /// - 80: Good balance (default)
    /// - 90+: Higher quality, more memory usage
    pub jpeg_quality: u8,
    
    // ----------------------------------------
    // PERFORMANCE SETTINGS
    // "How to balance quality vs. performance"
    // ----------------------------------------
    
    /// Maximum percentage of frames to capture (1-100)
    ///
    /// ## Plain English
    /// If the system is struggling, we can capture fewer frames.
    /// At 50%, we capture every other frame (still 45 FPS at 90 FPS target).
    pub max_capture_percentage: u8,
    
    /// Skip frames if processing takes too long
    ///
    /// ## Plain English
    /// If capturing a frame would cause VR stuttering, skip it.
    /// Better to have a slightly choppy recording than a choppy VR experience.
    pub skip_on_lag: bool,
    
    // ----------------------------------------
    // FEEDBACK SETTINGS
    // "How to tell the user what's happening"
    // ----------------------------------------
    
    /// Enable haptic (vibration) feedback on save
    pub haptic_feedback: bool,
    
    /// Enable audio feedback on save
    pub audio_feedback: bool,
}

impl Config {
    /// Creates a configuration with all default values
    pub fn default() -> Self {
        Self {
            // Buffer: 10 seconds at 90 FPS
            buffer_duration_seconds: 10.0,
            target_fps: 90,
            
            // Input: Left grip + trigger, 500ms cooldown
            trigger_button: TriggerButton::default(),
            save_cooldown_ms: 500,
            
            // Output: Standard location, 20 Mbps
            output_directory: "/sdcard/QuestShadowplay/".to_string(),
            video_bitrate: 20_000_000, // 20 Mbps
            jpeg_quality: 80,
            
            // Performance: Capture all frames, skip on lag
            max_capture_percentage: 100,
            skip_on_lag: true,
            
            // Feedback: Both haptic and audio
            haptic_feedback: true,
            audio_feedback: true,
        }
    }

    /// Creates a configuration optimized for low memory usage
    ///
    /// ## When to Use
    /// If other apps are using lots of memory, or you want a longer
    /// buffer without running out of RAM.
    pub fn low_memory() -> Self {
        Self {
            buffer_duration_seconds: 10.0,
            target_fps: 72,  // Lower FPS = fewer frames
            jpeg_quality: 70, // More compression
            max_capture_percentage: 50, // Skip half the frames
            ..Self::default()
        }
    }

    /// Creates a configuration optimized for maximum quality
    ///
    /// ## When to Use
    /// If you want the best possible recording quality and have
    /// a powerful system with plenty of memory.
    pub fn high_quality() -> Self {
        Self {
            target_fps: 120, // Max Quest 3 refresh rate
            video_bitrate: 50_000_000, // 50 Mbps
            jpeg_quality: 95,
            max_capture_percentage: 100,
            skip_on_lag: false, // Never skip (may cause stuttering)
            ..Self::default()
        }
    }

    /// Validates the configuration and returns errors if invalid
    ///
    /// ## Plain English
    /// Makes sure all settings are within reasonable bounds.
    /// Returns a list of problems, or empty if all is well.
    pub fn validate(&self) -> Vec<ConfigError> {
        let mut errors = Vec::new();

        // Check buffer duration
        if self.buffer_duration_seconds < 5.0 {
            errors.push(ConfigError::BufferTooShort(self.buffer_duration_seconds));
        }
        if self.buffer_duration_seconds > 60.0 {
            errors.push(ConfigError::BufferTooLong(self.buffer_duration_seconds));
        }

        // Check FPS
        if self.target_fps < 30 || self.target_fps > 144 {
            errors.push(ConfigError::InvalidFps(self.target_fps));
        }

        // Check bitrate
        if self.video_bitrate < 1_000_000 {
            errors.push(ConfigError::BitrateTooLow(self.video_bitrate));
        }
        if self.video_bitrate > 100_000_000 {
            errors.push(ConfigError::BitrateTooHigh(self.video_bitrate));
        }

        errors
    }

    /// Calculates estimated memory usage for the buffer
    ///
    /// ## Returns
    /// Estimated memory usage in megabytes
    pub fn estimated_memory_mb(&self) -> f32 {
        let frame_count = self.buffer_duration_seconds * self.target_fps as f32;
        
        // Estimate ~100KB per compressed frame at quality 80
        let bytes_per_frame = 100_000.0 * (self.jpeg_quality as f32 / 80.0);
        
        (frame_count * bytes_per_frame) / (1024.0 * 1024.0)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::default()
    }
}

// ============================================
// CONFIGURATION ERRORS
// ============================================

/// Errors that can occur with configuration values
#[derive(Debug)]
pub enum ConfigError {
    /// Buffer duration is too short
    BufferTooShort(f32),
    
    /// Buffer duration is too long (would use too much memory)
    BufferTooLong(f32),
    
    /// FPS value is outside valid range
    InvalidFps(u32),
    
    /// Video bitrate is too low for acceptable quality
    BitrateTooLow(u32),
    
    /// Video bitrate is unreasonably high
    BitrateTooHigh(u32),
    
    /// Output directory doesn't exist or isn't writable
    InvalidOutputDirectory(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BufferTooShort(val) => {
                write!(f, "Buffer duration {} seconds is too short (minimum 5)", val)
            }
            Self::BufferTooLong(val) => {
                write!(f, "Buffer duration {} seconds is too long (maximum 60)", val)
            }
            Self::InvalidFps(val) => {
                write!(f, "FPS {} is outside valid range (30-144)", val)
            }
            Self::BitrateTooLow(val) => {
                write!(f, "Bitrate {} bps is too low for acceptable quality", val)
            }
            Self::BitrateTooHigh(val) => {
                write!(f, "Bitrate {} bps is unreasonably high", val)
            }
            Self::InvalidOutputDirectory(path) => {
                write!(f, "Output directory '{}' is invalid or not writable", path)
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
    fn test_memory_estimation() {
        let config = Config::default();
        let memory = config.estimated_memory_mb();
        
        // 10 seconds at 90 FPS, ~100KB each = ~90MB
        assert!(memory > 50.0 && memory < 150.0);
    }

    #[test]
    fn test_validation_errors() {
        let mut config = Config::default();
        
        // Too short buffer
        config.buffer_duration_seconds = 2.0;
        assert!(!config.validate().is_empty());
        
        // Fix it
        config.buffer_duration_seconds = 10.0;
        assert!(config.validate().is_empty());
        
        // Invalid FPS
        config.target_fps = 200;
        assert!(!config.validate().is_empty());
    }
}

