//! # Native macOS Replay Buffer
//!
//! Hardware-accelerated screen capture with replay buffer:
//! - ScreenCaptureKit: 1080p60 display capture
//! - VideoToolbox: Hardware H.264 encoding
//! - Ring buffer: Stores last N seconds of encoded H.264
//! - On-demand save: Muxes buffer to MP4 via AVAssetWriter
//!
//! This module provides Rust bindings to a Swift implementation
//! that handles the actual Apple framework calls.

use std::ffi::{c_void, CString};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// Configuration for the replay buffer
#[derive(Debug, Clone)]
pub struct ReplayConfig {
    /// Output width (default: 1920)
    pub width: u32,
    /// Output height (default: 1080)
    pub height: u32,
    /// Target frame rate (default: 60)
    pub fps: u32,
    /// H.264 bitrate in bits/sec (default: 8_000_000 = 8 Mbps)
    pub bitrate: u32,
    /// Keyframe interval in seconds (default: 1.0)
    pub keyframe_interval: f32,
    /// Buffer duration in seconds (default: 10.0)
    pub buffer_duration_secs: f32,
}

impl Default for ReplayConfig {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: 60,
            bitrate: 8_000_000,
            keyframe_interval: 1.0,
            buffer_duration_secs: 10.0,
        }
    }
}

/// Statistics from the replay buffer
#[derive(Debug, Default)]
pub struct ReplayStats {
    /// Total frames captured from screen
    pub frames_captured: AtomicU64,
    /// Frames dropped due to backpressure
    pub frames_dropped: AtomicU64,
    /// Frames successfully encoded to H.264
    pub frames_encoded: AtomicU64,
    /// Buffer fill percentage (0-100)
    pub buffer_fill_percent: std::sync::atomic::AtomicU32,
    /// Buffer memory usage in bytes
    pub buffer_memory_bytes: AtomicU64,
}

/// Handle to the native replay buffer
#[allow(dead_code)]
pub struct ReplayBufferHandle {
    /// Pointer to Swift CaptureController instance
    handle: *mut c_void,
    /// Is capture currently active
    is_active: Arc<AtomicBool>,
    /// Capture statistics
    pub stats: Arc<ReplayStats>,
    /// Configuration used
    config: ReplayConfig,
}

// Swift functions we'll link against (linked via build.rs)
#[cfg(target_os = "macos")]
extern "C" {
    fn swift_replay_create(
        width: u32,
        height: u32,
        fps: u32,
        bitrate: u32,
        keyframe_interval: f32,
        buffer_duration_secs: f32,
    ) -> *mut c_void;
    
    fn swift_replay_start(handle: *mut c_void) -> bool;
    fn swift_replay_stop(handle: *mut c_void);
    fn swift_replay_save(handle: *mut c_void, output_path: *const i8) -> bool;
    fn swift_replay_destroy(handle: *mut c_void);
    
    fn swift_replay_is_active(handle: *mut c_void) -> bool;
    fn swift_replay_get_frames_captured(handle: *mut c_void) -> u64;
    fn swift_replay_get_frames_dropped(handle: *mut c_void) -> u64;
    fn swift_replay_get_frames_encoded(handle: *mut c_void) -> u64;
    fn swift_replay_get_buffer_fill(handle: *mut c_void) -> f32;
    fn swift_replay_get_buffer_memory(handle: *mut c_void) -> u64;
}

#[cfg(target_os = "macos")]
impl ReplayBufferHandle {
    /// Creates a new replay buffer with the given configuration
    pub fn new(config: ReplayConfig) -> Result<Self, String> {
        let handle = unsafe {
            swift_replay_create(
                config.width,
                config.height,
                config.fps,
                config.bitrate,
                config.keyframe_interval,
                config.buffer_duration_secs,
            )
        };

        if handle.is_null() {
            return Err("Failed to create replay buffer".to_string());
        }

        log::info!(
            "Replay buffer created: {}x{} @ {}fps, {}Mbps, {}s buffer",
            config.width,
            config.height,
            config.fps,
            config.bitrate / 1_000_000,
            config.buffer_duration_secs
        );

        Ok(Self {
            handle,
            is_active: Arc::new(AtomicBool::new(false)),
            stats: Arc::new(ReplayStats::default()),
            config,
        })
    }

    /// Starts capturing to the replay buffer
    pub fn start(&self) -> Result<(), String> {
        if self.is_active.load(Ordering::SeqCst) {
            return Err("Replay buffer already active".to_string());
        }

        let success = unsafe { swift_replay_start(self.handle) };

        if success {
            self.is_active.store(true, Ordering::SeqCst);
            log::info!("Replay buffer started");
            Ok(())
        } else {
            Err("Failed to start replay buffer - check screen recording permissions".to_string())
        }
    }

    /// Stops capturing
    pub fn stop(&self) {
        if self.is_active.swap(false, Ordering::SeqCst) {
            unsafe { swift_replay_stop(self.handle) };
            log::info!("Replay buffer stopped");
        }
    }

    /// Saves the current replay buffer contents to an MP4 file
    pub fn save(&self, output_path: &Path) -> Result<(), String> {
        let path_str = output_path
            .to_str()
            .ok_or("Invalid path")?;
        let c_path = CString::new(path_str)
            .map_err(|_| "Invalid path string")?;

        log::info!("Saving replay buffer to: {:?}", output_path);

        let success = unsafe { swift_replay_save(self.handle, c_path.as_ptr()) };

        if success {
            log::info!("Replay saved successfully");
            Ok(())
        } else {
            Err("Failed to save replay buffer".to_string())
        }
    }

    /// Returns whether capture is currently active
    pub fn is_active(&self) -> bool {
        unsafe { swift_replay_is_active(self.handle) }
    }

    /// Updates statistics from Swift side
    pub fn update_stats(&self) {
        if !self.handle.is_null() {
            unsafe {
                self.stats.frames_captured.store(
                    swift_replay_get_frames_captured(self.handle),
                    Ordering::Relaxed,
                );
                self.stats.frames_dropped.store(
                    swift_replay_get_frames_dropped(self.handle),
                    Ordering::Relaxed,
                );
                self.stats.frames_encoded.store(
                    swift_replay_get_frames_encoded(self.handle),
                    Ordering::Relaxed,
                );
                let fill = swift_replay_get_buffer_fill(self.handle);
                self.stats.buffer_fill_percent.store(
                    (fill * 100.0) as u32,
                    Ordering::Relaxed,
                );
                self.stats.buffer_memory_bytes.store(
                    swift_replay_get_buffer_memory(self.handle),
                    Ordering::Relaxed,
                );
            }
        }
    }

    /// Returns the configuration
    pub fn config(&self) -> &ReplayConfig {
        &self.config
    }
}

#[cfg(target_os = "macos")]
impl Drop for ReplayBufferHandle {
    fn drop(&mut self) {
        self.stop();
        if !self.handle.is_null() {
            unsafe { swift_replay_destroy(self.handle) };
        }
    }
}

// Safety: The Swift handle is thread-safe (uses dispatch queues internally)
#[cfg(target_os = "macos")]
unsafe impl Send for ReplayBufferHandle {}
#[cfg(target_os = "macos")]
unsafe impl Sync for ReplayBufferHandle {}

// Stub for non-macOS platforms
#[cfg(not(target_os = "macos"))]
impl ReplayBufferHandle {
    pub fn new(_config: ReplayConfig) -> Result<Self, String> {
        Err("Replay buffer only available on macOS".to_string())
    }

    pub fn start(&self) -> Result<(), String> {
        Err("Replay buffer only available on macOS".to_string())
    }

    pub fn stop(&self) {}

    pub fn save(&self, _output_path: &Path) -> Result<(), String> {
        Err("Replay buffer only available on macOS".to_string())
    }

    pub fn is_active(&self) -> bool {
        false
    }

    pub fn update_stats(&self) {}

    pub fn config(&self) -> &ReplayConfig {
        &self.config
    }
}

#[cfg(not(target_os = "macos"))]
unsafe impl Send for ReplayBufferHandle {}
#[cfg(not(target_os = "macos"))]
unsafe impl Sync for ReplayBufferHandle {}

