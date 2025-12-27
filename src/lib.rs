//! # Quest Shadowplay
//!
//! A replay buffer application for Meta Quest 3 that continuously captures
//! VR eye buffer frames and saves the last 10 seconds on demand.
//!
//! ## Architecture Overview
//!
//! The application is structured into independent modules:
//!
//! - `buffer`: Ring buffer for storing captured frames
//! - `capture`: Frame capture from VR eye buffer  
//! - `input`: Controller input handling
//! - `encoder`: Video encoding (H.264)
//! - `storage`: File system operations
//! - `config`: Application configuration
//! - `error`: Error types

// ============================================
// MODULE DECLARATIONS
// ============================================

pub mod buffer;
pub mod capture;
pub mod config;
pub mod encoder;
pub mod error;
pub mod input;
pub mod storage;

// ============================================
// RE-EXPORTS
// ============================================

pub use buffer::SharedFrameBuffer;
pub use capture::CapturedFrame;
pub use config::Config;
pub use error::{ShadowplayError, ShadowplayResult};
pub use input::InputHandler;

// ============================================
// IMPORTS
// ============================================

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use log::{error, info, warn};
use parking_lot::Mutex;

// ============================================
// APPLICATION STATE
// ============================================

/// The main Quest Shadowplay application.
///
/// ## Plain English
///
/// This is the "control center" that coordinates all parts of the app:
/// - Receives captured frames and stores them
/// - Watches for save button presses
/// - Triggers video encoding when saving
pub struct QuestShadowplay {
    /// The circular buffer storing recent frames
    buffer: Arc<SharedFrameBuffer>,

    /// Handles controller input
    input_handler: Arc<Mutex<InputHandler>>,

    /// Application configuration
    config: Config,

    /// Is a save currently in progress?
    is_saving: Arc<AtomicBool>,

    /// Is the application running?
    is_running: Arc<AtomicBool>,

    /// Statistics about operation
    stats: Arc<Mutex<AppStats>>,
}

/// Runtime statistics for monitoring
#[derive(Debug, Default, Clone)]
pub struct AppStats {
    /// Total frames received
    pub frames_received: u64,
    /// Total clips saved
    pub clips_saved: u64,
    /// Total save errors
    pub save_errors: u64,
}

impl QuestShadowplay {
    /// Creates a new application instance with default configuration.
    pub fn new() -> ShadowplayResult<Self> {
        Self::with_config(Config::default())
    }

    /// Creates a new application instance with custom configuration.
    ///
    /// ## Parameters
    /// - `config`: The configuration to use
    ///
    /// ## Returns
    /// A new `QuestShadowplay` instance or an error
    pub fn with_config(config: Config) -> ShadowplayResult<Self> {
        // Validate configuration
        let errors = config.validate();
        if !errors.is_empty() {
            return Err(ShadowplayError::Config(errors[0].clone()));
        }

        info!(
            "Initializing Quest Shadowplay: {}s buffer at {} FPS",
            config.buffer_duration_seconds, config.target_fps
        );

        // Create the frame buffer
        let buffer = Arc::new(SharedFrameBuffer::new(
            config.buffer_duration_seconds,
            config.target_fps,
        ));

        // Create input handler
        let input_handler = Arc::new(Mutex::new(InputHandler::new(
            config.trigger_button.clone(),
        )));

        info!("Quest Shadowplay initialized successfully");

        Ok(Self {
            buffer,
            input_handler,
            config,
            is_saving: Arc::new(AtomicBool::new(false)),
            is_running: Arc::new(AtomicBool::new(true)),
            stats: Arc::new(Mutex::new(AppStats::default())),
        })
    }

    /// Called when a new frame is captured.
    ///
    /// This should be called ~90 times per second (once per VR frame).
    /// The frame is added to the ring buffer, and we check if a save
    /// should be triggered.
    pub fn on_frame_captured(&self, frame: CapturedFrame) {
        // Update stats
        {
            let mut stats = self.stats.lock();
            stats.frames_received += 1;
        }

        // Add to buffer
        self.buffer.push_frame(frame);

        // Check for save trigger (only if not already saving)
        if !self.is_saving.load(Ordering::SeqCst) {
            let mut input = self.input_handler.lock();
            if input.check_save_triggered() {
                self.trigger_save();
            }
        }
    }

    /// Updates the input state from controller data.
    ///
    /// Call this every frame with the latest controller state.
    pub fn update_input(&self, state: input::InputState) {
        let mut handler = self.input_handler.lock();
        handler.update(state);
    }

    /// Manually triggers a save operation.
    ///
    /// Returns `true` if save was started, `false` if already saving.
    pub fn trigger_save(&self) -> bool {
        // Check if already saving
        if self.is_saving.swap(true, Ordering::SeqCst) {
            warn!("Save already in progress");
            return false;
        }

        info!("Save triggered - starting background encode");

        // Clone references for the background thread
        let buffer = Arc::clone(&self.buffer);
        let is_saving = Arc::clone(&self.is_saving);
        let config = self.config.clone();
        let stats = Arc::clone(&self.stats);

        // Spawn background thread for encoding
        thread::spawn(move || {
            let result = Self::do_save(&buffer, &config);

            // Update stats
            {
                let mut s = stats.lock();
                match &result {
                    Ok(_) => s.clips_saved += 1,
                    Err(_) => s.save_errors += 1,
                }
            }

            // Log result
            match result {
                Ok(path) => info!("Clip saved to: {}", path),
                Err(e) => error!("Failed to save clip: {}", e),
            }

            // Mark save as complete
            is_saving.store(false, Ordering::SeqCst);
        });

        true
    }

    /// Performs the actual save operation (runs in background thread).
    fn do_save(buffer: &SharedFrameBuffer, config: &Config) -> ShadowplayResult<String> {
        // Snapshot the buffer
        let frames = buffer.snapshot();
        let frame_count = frames.len();

        if frame_count == 0 {
            return Err(ShadowplayError::Internal("No frames to save".to_string()));
        }

        info!("Encoding {} frames...", frame_count);

        // Generate output path
        let output_path = storage::StorageManager::generate_filename(&config.output_directory);

        // Ensure output directory exists
        storage::ensure_directory(&config.output_directory)?;

        // Encode frames to video
        encoder::VideoEncoder::encode_frames(&frames, &output_path, config)?;

        Ok(output_path)
    }

    /// Returns whether a save is currently in progress.
    pub fn is_saving(&self) -> bool {
        self.is_saving.load(Ordering::SeqCst)
    }

    /// Returns the current buffer fill percentage (0.0 to 1.0).
    pub fn buffer_fill(&self) -> f32 {
        self.buffer.fill_percentage()
    }

    /// Returns the number of frames currently in the buffer.
    pub fn buffer_frame_count(&self) -> usize {
        self.buffer.frame_count()
    }

    /// Returns a copy of the current statistics.
    pub fn stats(&self) -> AppStats {
        self.stats.lock().clone()
    }

    /// Returns a reference to the configuration.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Shuts down the application gracefully.
    pub fn shutdown(&self) {
        info!("Shutting down Quest Shadowplay...");

        self.is_running.store(false, Ordering::SeqCst);

        // Wait for any in-progress save to complete
        while self.is_saving.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(50));
        }

        info!("Shutdown complete");
    }
}

impl Default for QuestShadowplay {
    fn default() -> Self {
        Self::new().expect("Failed to create default QuestShadowplay")
    }
}

// ============================================
// ANDROID ENTRY POINT
// ============================================

/// Initialize logging for the platform.
pub fn init_logging() {
    #[cfg(target_os = "android")]
    {
        android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(log::LevelFilter::Info)
                .with_tag("QuestShadowplay"),
        );
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Info)
            .try_init();
    }
}

// ============================================
// TESTS
// ============================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_creation() {
        init_logging();
        let app = QuestShadowplay::new();
        assert!(app.is_ok());
    }

    #[test]
    fn test_app_with_config() {
        let config = Config::default();
        let app = QuestShadowplay::with_config(config);
        assert!(app.is_ok());
    }

    #[test]
    fn test_buffer_starts_empty() {
        let app = QuestShadowplay::new().unwrap();
        assert_eq!(app.buffer_frame_count(), 0);
        assert_eq!(app.buffer_fill(), 0.0);
    }

    #[test]
    fn test_stats_default() {
        let app = QuestShadowplay::new().unwrap();
        let stats = app.stats();
        assert_eq!(stats.frames_received, 0);
        assert_eq!(stats.clips_saved, 0);
    }
}
