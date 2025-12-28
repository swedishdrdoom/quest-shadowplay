//! # Native macOS Capture Pipeline
//!
//! Hardware-accelerated screen capture using Apple frameworks:
//! - ScreenCaptureKit: 1080p60 display capture
//! - CoreMedia/CoreVideo: Zero-copy buffer handling  
//! - VideoToolbox: Hardware H.264 encoding
//! - AVFoundation: MP4 muxing
//!
//! This module provides Rust bindings to a Swift implementation
//! that handles the actual Apple framework calls.

use std::ffi::c_void;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// Configuration for the capture pipeline
#[repr(C)]
#[derive(Debug, Clone)]
pub struct CaptureConfig {
    /// Output width (default: 1920)
    pub width: u32,
    /// Output height (default: 1080)
    pub height: u32,
    /// Target frame rate (default: 60)
    pub fps: u32,
    /// H.264 bitrate in bits/sec (default: 8_000_000 = 8 Mbps)
    pub bitrate: u32,
    /// Keyframe interval in seconds (default: 2)
    pub keyframe_interval: f32,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: 60,
            bitrate: 8_000_000,
            keyframe_interval: 2.0,
        }
    }
}

/// Statistics from the capture pipeline
#[derive(Debug, Default)]
pub struct CaptureStats {
    /// Total frames captured
    pub frames_captured: AtomicU64,
    /// Frames dropped due to backpressure
    pub frames_dropped: AtomicU64,
    /// Frames encoded
    pub frames_encoded: AtomicU64,
    /// Current capture FPS (measured)
    pub current_fps: AtomicU64, // Stored as fps * 100 for precision
}

impl CaptureStats {
    pub fn get_fps(&self) -> f64 {
        self.current_fps.load(Ordering::Relaxed) as f64 / 100.0
    }
}

/// Handle to the native capture pipeline
pub struct NativeCaptureHandle {
    /// Pointer to Swift CaptureController instance
    handle: *mut c_void,
    /// Is capture currently active
    is_active: Arc<AtomicBool>,
    /// Capture statistics
    pub stats: Arc<CaptureStats>,
    /// Configuration used
    pub config: CaptureConfig,
}

// Swift functions we'll link against
#[cfg(target_os = "macos")]
#[link(name = "CaptureKit")]
extern "C" {
    fn swift_capture_create(
        width: u32,
        height: u32,
        fps: u32,
        bitrate: u32,
        keyframe_interval: f32,
    ) -> *mut c_void;
    fn swift_capture_start(handle: *mut c_void, output_path: *const i8) -> bool;
    fn swift_capture_stop(handle: *mut c_void);
    fn swift_capture_destroy(handle: *mut c_void);
    fn swift_capture_get_frames_captured(handle: *mut c_void) -> u64;
    fn swift_capture_get_frames_dropped(handle: *mut c_void) -> u64;
    fn swift_capture_get_frames_encoded(handle: *mut c_void) -> u64;
    fn swift_capture_is_active(handle: *mut c_void) -> bool;
}

#[cfg(target_os = "macos")]
impl NativeCaptureHandle {
    /// Creates a new capture pipeline with the given configuration
    pub fn new(config: CaptureConfig) -> Result<Self, String> {
        let handle = unsafe {
            swift_capture_create(
                config.width,
                config.height,
                config.fps,
                config.bitrate,
                config.keyframe_interval,
            )
        };
        
        if handle.is_null() {
            return Err("Failed to create capture pipeline".to_string());
        }

        Ok(Self {
            handle,
            is_active: Arc::new(AtomicBool::new(false)),
            stats: Arc::new(CaptureStats::default()),
            config,
        })
    }

    /// Starts capturing to the specified output file
    pub fn start(&self, output_path: &Path) -> Result<(), String> {
        if self.is_active.load(Ordering::SeqCst) {
            return Err("Capture already active".to_string());
        }

        let path_str = output_path
            .to_str()
            .ok_or("Invalid path")?;
        let c_path = std::ffi::CString::new(path_str)
            .map_err(|_| "Invalid path string")?;

        let success = unsafe { swift_capture_start(self.handle, c_path.as_ptr()) };

        if success {
            self.is_active.store(true, Ordering::SeqCst);
            log::info!("Native capture started: {:?}", output_path);
            Ok(())
        } else {
            Err("Failed to start capture - check screen recording permissions".to_string())
        }
    }

    /// Stops capturing and finalizes the output file
    pub fn stop(&self) {
        if self.is_active.swap(false, Ordering::SeqCst) {
            unsafe { swift_capture_stop(self.handle) };
            log::info!("Native capture stopped");
        }
    }

    /// Returns whether capture is currently active
    pub fn is_active(&self) -> bool {
        unsafe { swift_capture_is_active(self.handle) }
    }

    /// Updates and returns current statistics
    pub fn update_stats(&self) {
        if !self.handle.is_null() {
            unsafe {
                self.stats.frames_captured.store(
                    swift_capture_get_frames_captured(self.handle),
                    Ordering::Relaxed,
                );
                self.stats.frames_dropped.store(
                    swift_capture_get_frames_dropped(self.handle),
                    Ordering::Relaxed,
                );
                self.stats.frames_encoded.store(
                    swift_capture_get_frames_encoded(self.handle),
                    Ordering::Relaxed,
                );
            }
        }
    }
}

#[cfg(target_os = "macos")]
impl Drop for NativeCaptureHandle {
    fn drop(&mut self) {
        self.stop();
        if !self.handle.is_null() {
            unsafe { swift_capture_destroy(self.handle) };
        }
    }
}

// Safety: The Swift handle is thread-safe (uses dispatch queues internally)
#[cfg(target_os = "macos")]
unsafe impl Send for NativeCaptureHandle {}
#[cfg(target_os = "macos")]
unsafe impl Sync for NativeCaptureHandle {}

// Stub for non-macOS platforms
#[cfg(not(target_os = "macos"))]
impl NativeCaptureHandle {
    pub fn new(_config: CaptureConfig) -> Result<Self, String> {
        Err("Native capture only available on macOS".to_string())
    }

    pub fn start(&self, _output_path: &Path) -> Result<(), String> {
        Err("Native capture only available on macOS".to_string())
    }

    pub fn stop(&self) {}
    
    pub fn is_active(&self) -> bool {
        false
    }

    pub fn update_stats(&self) {}
}

#[cfg(not(target_os = "macos"))]
unsafe impl Send for NativeCaptureHandle {}
#[cfg(not(target_os = "macos"))]
unsafe impl Sync for NativeCaptureHandle {}

