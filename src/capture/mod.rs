//! # Frame Capture Module
//!
//! Handles capturing frames from the VR eye buffer.
//!
//! ## Plain English
//!
//! Every time your Quest shows you an image (~90 times per second),
//! we intercept it, make a copy, compress it, and store it.
//! It's like a photocopier attached to a movie projector.

mod frame;

pub use frame::{CapturedFrame, FrameCompressor};

use crate::buffer::SharedFrameBuffer;
use crate::error::ShadowplayResult;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

// ============================================
// FRAME CAPTURER
// ============================================

/// Captures frames from the VR eye buffer.
///
/// ## Plain English
///
/// This is the "camera" that photographs every frame of VR content.
/// In a real implementation, this would hook into OpenXR to intercept
/// frames before they're displayed.
pub struct FrameCapturer {
    /// Where to store captured frames
    buffer: Arc<SharedFrameBuffer>,

    /// Compresses frames to save memory
    compressor: FrameCompressor,

    /// Is capture enabled?
    enabled: AtomicBool,

    /// Frames captured successfully
    frames_captured: AtomicU64,

    /// Frames skipped (errors or performance)
    frames_skipped: AtomicU64,
}

impl FrameCapturer {
    /// Creates a new frame capturer.
    ///
    /// ## Parameters
    /// - `buffer`: Where to store captured frames
    /// - `jpeg_quality`: Compression quality (0-100)
    pub fn new(buffer: Arc<SharedFrameBuffer>, jpeg_quality: u8) -> Self {
        Self {
            buffer,
            compressor: FrameCompressor::new(jpeg_quality),
            enabled: AtomicBool::new(true),
            frames_captured: AtomicU64::new(0),
            frames_skipped: AtomicU64::new(0),
        }
    }

    /// Processes a raw frame from the eye buffer.
    ///
    /// ## Parameters
    /// - `raw_rgba`: Raw RGBA pixel data
    /// - `width`: Image width in pixels
    /// - `height`: Image height in pixels
    /// - `eye_index`: Which eye (0 = left, 1 = right)
    ///
    /// ## Returns
    /// Ok(()) if frame was captured, Err if there was a problem
    pub fn capture_frame(
        &self,
        raw_rgba: &[u8],
        width: u32,
        height: u32,
        eye_index: u32,
    ) -> ShadowplayResult<()> {
        if !self.enabled.load(Ordering::Relaxed) {
            return Ok(());
        }

        // Compress the frame
        match self.compressor.compress(raw_rgba, width, height) {
            Ok(compressed) => {
                let frame = CapturedFrame::new(compressed, eye_index, width, height);
                self.buffer.push_frame(frame);
                self.frames_captured.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
            Err(e) => {
                self.frames_skipped.fetch_add(1, Ordering::Relaxed);
                log::warn!("Frame compression failed: {}", e);
                Err(crate::error::ShadowplayError::Capture(e.to_string()))
            }
        }
    }

    /// Enables frame capture.
    pub fn enable(&self) {
        self.enabled.store(true, Ordering::Relaxed);
        log::info!("Frame capture enabled");
    }

    /// Disables frame capture.
    pub fn disable(&self) {
        self.enabled.store(false, Ordering::Relaxed);
        log::info!("Frame capture disabled");
    }

    /// Returns whether capture is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Returns the number of frames captured.
    pub fn frames_captured(&self) -> u64 {
        self.frames_captured.load(Ordering::Relaxed)
    }

    /// Returns the number of frames skipped.
    pub fn frames_skipped(&self) -> u64 {
        self.frames_skipped.load(Ordering::Relaxed)
    }

    /// Returns capture statistics.
    pub fn stats(&self) -> CaptureStats {
        CaptureStats {
            frames_captured: self.frames_captured(),
            frames_skipped: self.frames_skipped(),
            is_enabled: self.is_enabled(),
            buffer_fill: self.buffer.fill_percentage(),
        }
    }
}

/// Statistics about the capture process.
#[derive(Debug, Clone)]
pub struct CaptureStats {
    pub frames_captured: u64,
    pub frames_skipped: u64,
    pub is_enabled: bool,
    pub buffer_fill: f32,
}

// ============================================
// TESTS
// ============================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capturer_creation() {
        let buffer = Arc::new(SharedFrameBuffer::new(1.0, 10));
        let capturer = FrameCapturer::new(buffer, 80);

        assert!(capturer.is_enabled());
        assert_eq!(capturer.frames_captured(), 0);
        assert_eq!(capturer.frames_skipped(), 0);
    }

    #[test]
    fn test_enable_disable() {
        let buffer = Arc::new(SharedFrameBuffer::new(1.0, 10));
        let capturer = FrameCapturer::new(buffer, 80);

        assert!(capturer.is_enabled());
        capturer.disable();
        assert!(!capturer.is_enabled());
        capturer.enable();
        assert!(capturer.is_enabled());
    }
}
