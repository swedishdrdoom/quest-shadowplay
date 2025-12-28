//! # Platform-Agnostic Capture Interface
//!
//! Defines the common interface for frame capture across platforms.
//! Each platform implements this trait with its native capture API:
//! - macOS: ScreenCaptureKit + VideoToolbox (hardware accelerated)
//! - Android: MediaProjection (future)
//! - Other: Simulated test frames

use std::sync::Arc;
use quest_shadowplay::CapturedFrame;

// ============================================
// PLATFORM-SPECIFIC MODULES
// ============================================

mod simulated;
#[allow(unused_imports)]
pub use simulated::SimulatedCapture;

// Old CoreGraphics-based capture (deprecated, kept for fallback)
#[cfg(target_os = "macos")]
mod macos;

// New hardware-accelerated native capture
#[cfg(target_os = "macos")]
pub mod macos_native;

#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
pub use android::AndroidCapture;

// ============================================
// COMMON INTERFACE
// ============================================

/// Trait for platform-specific frame capture implementations.
///
/// This abstraction keeps platform code isolated:
/// - Core logic only sees this trait
/// - Platform-specific code is conditionally compiled
/// - No cross-platform contamination
pub trait FrameCapture: Send + Sync {
    /// Starts capturing frames.
    /// Frames are sent to the provided callback.
    fn start(&self, on_frame: Arc<dyn Fn(CapturedFrame) + Send + Sync>) -> Result<(), CaptureError>;

    /// Stops capturing frames.
    fn stop(&self);

    /// Returns whether capture is currently active.
    #[allow(dead_code)]
    fn is_active(&self) -> bool;

    /// Returns the name of this capture source (for logging/UI).
    fn source_name(&self) -> &'static str;
}

/// Errors that can occur during capture.
#[derive(Debug)]
#[allow(dead_code)]
pub enum CaptureError {
    /// Permission not granted
    PermissionDenied(String),
    /// Capture already running
    AlreadyRunning,
    /// Platform-specific error
    PlatformError(String),
    /// Initialization failed
    InitFailed(String),
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
            Self::AlreadyRunning => write!(f, "Capture already running"),
            Self::PlatformError(msg) => write!(f, "Platform error: {}", msg),
            Self::InitFailed(msg) => write!(f, "Initialization failed: {}", msg),
        }
    }
}

impl std::error::Error for CaptureError {}

// ============================================
// FACTORY FUNCTION
// ============================================

/// Creates the appropriate capture source for the current platform.
///
/// Platform selection:
/// - macOS: ScreenCaptureKit + VideoToolbox (hardware accelerated)
/// - Android: MediaProjection (when implemented)
/// - Other: Simulated test pattern frames
pub fn create_capture() -> Box<dyn FrameCapture> {
    #[cfg(target_os = "macos")]
    {
        // Use legacy CoreGraphics capture for now
        // Native capture writes directly to MP4, different interface
        log::info!("Platform: macOS - using Core Graphics screen capture");
        log::info!("Note: Use 'start_native_recording' for hardware-accelerated 60fps capture");
        Box::new(macos::MacOSCapture::new())
    }

    #[cfg(target_os = "android")]
    {
        log::info!("Platform: Android - using MediaProjection");
        Box::new(AndroidCapture::new())
    }

    #[cfg(not(any(target_os = "macos", target_os = "android")))]
    {
        log::info!("Platform: Unknown - using simulated capture");
        Box::new(SimulatedCapture::new())
    }
}
