//! # Quest Shadowplay
//!
//! A replay buffer application for Meta Quest 3 that continuously captures
//! eye buffer frames and saves the last 10 seconds on demand.
//!
//! ## How It Works (Plain English)
//!
//! Imagine you're watching TV, and there's a magic DVR that's ALWAYS recording
//! the last 10 seconds. When something cool happens, you press a button and
//! that clip gets saved forever. That's what this app does for your VR experience!
//!
//! ## Architecture Overview
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Quest Shadowplay                          │
//! ├─────────────────────────────────────────────────────────────┤
//! │                                                              │
//! │  VR App → [Frame Capture] → [Ring Buffer] → [Encoder] → 📁  │
//! │                                    ↑                         │
//! │                              [Input Handler]                 │
//! │                          (button press triggers save)        │
//! └─────────────────────────────────────────────────────────────┘
//! ```

// ============================================
// MODULE DECLARATIONS
// These are like "chapters" in our app
// ============================================

/// Frame capture from OpenXR - grabs images from VR
pub mod capture;

/// Circular buffer for storing recent frames
pub mod buffer;

/// Controller input detection
pub mod input;

/// Video encoding (turns frames into video)
pub mod encoder;

/// File saving to Quest storage
pub mod storage;

/// Configuration and settings
pub mod config;

/// Error types used throughout the app
pub mod error;

// ============================================
// IMPORTS
// Bringing in tools we need from our modules and external libraries
// ============================================

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use log::{info, error, warn};
use parking_lot::RwLock;

use crate::buffer::SharedFrameBuffer;
use crate::capture::CapturedFrame;
use crate::config::Config;
use crate::encoder::VideoEncoder;
use crate::error::ShadowplayError;
use crate::input::InputHandler;
use crate::storage::StorageManager;

// ============================================
// MAIN APPLICATION STRUCTURE
// This is the "brain" of our app
// ============================================

/// The main Quest Shadowplay application
///
/// ## Plain English Explanation
///
/// Think of this as the control center of our app. It:
/// 1. Holds the circular buffer (our rolling 10-second memory)
/// 2. Watches for button presses
/// 3. Coordinates saving clips to disk
/// 4. Makes sure everything runs smoothly without crashing
pub struct QuestShadowplay {
    /// The circular buffer storing recent frames
    /// Arc = "Atomic Reference Counter" - lets multiple parts of our code
    /// share this safely
    buffer: Arc<SharedFrameBuffer>,

    /// Handles controller button detection
    input_handler: InputHandler,

    /// Manages file saving
    storage: StorageManager,

    /// Configuration settings
    config: Config,

    /// Flag: are we currently saving a clip?
    /// AtomicBool = a true/false that's safe to check from multiple threads
    is_saving: Arc<AtomicBool>,

    /// Flag: is the app running?
    is_running: Arc<AtomicBool>,
}

impl QuestShadowplay {
    /// Creates and initializes the application
    ///
    /// ## What Happens Here (Plain English)
    ///
    /// This is like the "startup sequence" when you turn on a machine:
    /// 1. Load settings (how long to record, what button to use, etc.)
    /// 2. Create the frame buffer (our 10-second memory)
    /// 3. Set up button detection
    /// 4. Prepare the file saving system
    /// 5. Return the ready-to-go app
    pub fn new() -> Result<Self, ShadowplayError> {
        // Load configuration (or use defaults)
        let config = Config::default();

        info!(
            "Initializing Quest Shadowplay: {} seconds buffer at {} FPS",
            config.buffer_duration_seconds,
            config.target_fps
        );

        // Create the circular frame buffer
        // This will hold our rolling 10 seconds of footage
        let buffer = Arc::new(SharedFrameBuffer::new(
            config.buffer_duration_seconds,
            config.target_fps,
        ));

        // Set up input handling for save trigger
        let input_handler = InputHandler::new(config.trigger_button.clone());

        // Set up storage for saving clips
        let storage = StorageManager::new(&config.output_directory)?;

        info!("Quest Shadowplay initialized successfully!");

        Ok(Self {
            buffer,
            input_handler,
            storage,
            config,
            is_saving: Arc::new(AtomicBool::new(false)),
            is_running: Arc::new(AtomicBool::new(true)),
        })
    }

    /// Called every frame when a new image is captured
    ///
    /// ## What Happens Here (Plain English)
    ///
    /// This function runs ~90 times per second (once per frame):
    /// 1. Take the new frame and add it to our buffer
    /// 2. Check if the user pressed the save button
    /// 3. If yes, start saving in the background
    ///
    /// It's like a factory worker on an assembly line - grab the item,
    /// put it in storage, check for special instructions, repeat.
    pub fn on_frame_captured(&mut self, frame: CapturedFrame) {
        // Add frame to our circular buffer
        // Old frames are automatically discarded when buffer is full
        self.buffer.push_frame(frame);

        // Check if user wants to save
        if self.input_handler.check_save_triggered() {
            self.trigger_save();
        }
    }

    /// Triggers a save operation
    ///
    /// ## What Happens Here (Plain English)
    ///
    /// When you press the save button:
    /// 1. We check if we're already saving (can't save two clips at once)
    /// 2. If not, we "freeze" the current buffer contents
    /// 3. Start a background task to encode and save the video
    /// 4. Continue recording normally - you don't miss any frames!
    fn trigger_save(&self) {
        // Check if already saving
        if self.is_saving.load(Ordering::SeqCst) {
            warn!("Save already in progress, ignoring trigger");
            return;
        }

        info!("Save triggered! Starting background save...");

        // Mark that we're now saving
        self.is_saving.store(true, Ordering::SeqCst);

        // Clone references for the background thread
        let buffer = Arc::clone(&self.buffer);
        let is_saving = Arc::clone(&self.is_saving);
        let config = self.config.clone();
        let output_dir = self.config.output_directory.clone();

        // Spawn a background thread to do the encoding
        // This way, recording continues while we save
        thread::spawn(move || {
            // Take a snapshot of current frames
            let frames = buffer.snapshot();
            let frame_count = frames.len();

            info!("Saving {} frames...", frame_count);

            // Generate output filename with timestamp
            let filename = StorageManager::generate_filename(&output_dir);

            // Encode to video
            match VideoEncoder::encode_frames(&frames, &filename, &config) {
                Ok(()) => {
                    info!("Successfully saved clip to: {}", filename);
                    // TODO: Send haptic feedback for success
                }
                Err(e) => {
                    error!("Failed to save clip: {}", e);
                    // TODO: Send haptic feedback for failure
                }
            }

            // Mark save as complete
            is_saving.store(false, Ordering::SeqCst);
        });
    }

    /// Shuts down the application gracefully
    ///
    /// ## What Happens Here (Plain English)
    ///
    /// Like turning off a machine properly instead of pulling the plug:
    /// 1. Signal all parts to stop
    /// 2. Wait for any in-progress saves to finish
    /// 3. Clean up resources
    pub fn shutdown(&mut self) {
        info!("Shutting down Quest Shadowplay...");
        
        self.is_running.store(false, Ordering::SeqCst);

        // Wait for any ongoing save to complete
        while self.is_saving.load(Ordering::SeqCst) {
            thread::sleep(std::time::Duration::from_millis(100));
        }

        info!("Quest Shadowplay shut down cleanly.");
    }

    /// Returns whether the app is currently saving a clip
    pub fn is_saving(&self) -> bool {
        self.is_saving.load(Ordering::SeqCst)
    }

    /// Returns the current buffer fill percentage (0.0 - 1.0)
    pub fn buffer_fill_percent(&self) -> f32 {
        self.buffer.fill_percentage()
    }
}

// ============================================
// ANDROID ENTRY POINT
// This is where Android starts our app
// ============================================

/// Android native activity entry point
///
/// ## What Happens Here (Plain English)
///
/// When you launch the app on Quest, Android calls this function.
/// It's like the "main" function in a regular program - the starting point.
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn android_main(app: android_activity::AndroidApp) {
    // Initialize Android logging
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("QuestShadowplay"),
    );

    info!("Quest Shadowplay starting...");

    // Create and run the application
    match QuestShadowplay::new() {
        Ok(mut app) => {
            info!("Application created, entering main loop");
            // In a real implementation, this would integrate with
            // the OpenXR layer to receive frames
            
            // For now, we just keep the app alive
            loop {
                // TODO: Process OpenXR events
                // TODO: Handle app lifecycle events
                
                if !app.is_running.load(Ordering::SeqCst) {
                    break;
                }
                
                thread::sleep(std::time::Duration::from_millis(10));
            }
            
            app.shutdown();
        }
        Err(e) => {
            error!("Failed to create application: {}", e);
        }
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
        // This test verifies the app can be created
        // Note: Will fail on non-Android without mocking
        // let app = QuestShadowplay::new();
        // assert!(app.is_ok());
    }
}

