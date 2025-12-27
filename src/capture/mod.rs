//! # Frame Capture Module
//!
//! This module handles capturing frames from the VR eye buffer using OpenXR.
//!
//! ## Plain English Explanation
//!
//! Every time your Quest shows you an image (about 90 times per second), that
//! image travels from the VR game → through the OpenXR system → to your eyes.
//!
//! We insert ourselves in the middle of that journey:
//!
//! ```text
//! NORMAL:   Game → OpenXR → Your Eyes
//!
//! WITH US:  Game → [Our Layer] → OpenXR → Your Eyes
//!                      ↓
//!                 Copy frame
//!                      ↓
//!                 Our Buffer
//! ```
//!
//! We're like a photocopier attached to a mail conveyor - we copy every letter
//! but don't slow down the mail!

mod openxr_layer;

pub use openxr_layer::OpenXRLayer;

use std::time::{SystemTime, UNIX_EPOCH};

// ============================================
// CAPTURED FRAME
// Represents a single captured image
// ============================================

/// A single captured frame from the VR eye buffer
///
/// ## Plain English Explanation
///
/// This is one "photograph" from the VR headset. It contains:
/// - The actual image data (compressed to save memory)
/// - When it was taken (timestamp)
/// - Which eye it's for (left or right)
/// - How big the image is
///
/// Think of it as a labeled photo in a photo album.
#[derive(Clone, Debug)]
pub struct CapturedFrame {
    /// The image data, compressed as JPEG
    ///
    /// ## Why JPEG?
    /// Raw image data is HUGE. A single Quest 3 eye image at full resolution
    /// would be about 14MB uncompressed. JPEG shrinks it to ~100KB.
    /// That's a 140x reduction!
    pub data: Vec<u8>,

    /// When this frame was captured (nanoseconds since Unix epoch)
    ///
    /// ## Plain English
    /// A very precise timestamp. We use nanoseconds because at 90 FPS,
    /// frames are only 11 milliseconds apart - we need precision!
    pub timestamp_ns: u64,

    /// Which eye this frame is for (0 = left, 1 = right)
    ///
    /// ## Plain English
    /// VR shows different images to each eye (that's how 3D works).
    /// We track which eye this image belongs to.
    pub eye_index: u32,

    /// Image width in pixels
    pub width: u32,

    /// Image height in pixels
    pub height: u32,
}

impl CapturedFrame {
    /// Creates a new captured frame
    ///
    /// ## Parameters
    /// - `data`: The JPEG-compressed image bytes
    /// - `eye_index`: 0 for left eye, 1 for right eye
    /// - `width`: Image width in pixels
    /// - `height`: Image height in pixels
    pub fn new(data: Vec<u8>, eye_index: u32, width: u32, height: u32) -> Self {
        let timestamp_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        Self {
            data,
            timestamp_ns,
            eye_index,
            width,
            height,
        }
    }

    /// Returns the size of the compressed data in bytes
    pub fn compressed_size(&self) -> usize {
        self.data.len()
    }

    /// Returns an estimate of the uncompressed size
    ///
    /// ## Calculation
    /// width × height × 4 bytes (RGBA) = uncompressed size
    pub fn estimated_uncompressed_size(&self) -> usize {
        (self.width * self.height * 4) as usize
    }

    /// Returns the compression ratio achieved
    ///
    /// ## Plain English
    /// "How much did we shrink this image?"
    /// A ratio of 100 means the compressed version is 100x smaller.
    pub fn compression_ratio(&self) -> f32 {
        let uncompressed = self.estimated_uncompressed_size() as f32;
        let compressed = self.compressed_size() as f32;
        if compressed > 0.0 {
            uncompressed / compressed
        } else {
            0.0
        }
    }
}

// ============================================
// FRAME COMPRESSOR
// Handles JPEG compression of raw frames
// ============================================

/// Compresses raw frame data to JPEG format
///
/// ## Plain English Explanation
///
/// Raw images from the GPU are huge. This compressor is like a
/// vacuum-sealing machine for photos - it squishes them down
/// to a fraction of their original size while keeping them
/// looking good enough for video.
pub struct FrameCompressor {
    /// JPEG quality (0-100). Lower = smaller file, worse quality
    quality: u8,
}

impl FrameCompressor {
    /// Creates a new compressor with the given quality
    ///
    /// ## Quality Guidelines
    /// - 90-100: Visually lossless, larger files
    /// - 70-85: Good quality, reasonable size (recommended: 80)
    /// - 50-70: Noticeable artifacts, small files
    /// - Below 50: Visible degradation, very small
    pub fn new(quality: u8) -> Self {
        Self {
            quality: quality.min(100),
        }
    }

    /// Compresses raw RGBA pixel data to JPEG
    ///
    /// ## Parameters
    /// - `raw_data`: Raw pixel data in RGBA format (4 bytes per pixel)
    /// - `width`: Image width in pixels
    /// - `height`: Image height in pixels
    ///
    /// ## Returns
    /// JPEG-compressed data as bytes, or an error
    ///
    /// ## Plain English
    /// Takes a raw screenshot and compresses it. The input is like
    /// an uncompressed BMP, the output is a space-efficient JPEG.
    pub fn compress(
        &self,
        raw_data: &[u8],
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, CompressionError> {
        // Validate input
        let expected_size = (width * height * 4) as usize;
        if raw_data.len() != expected_size {
            return Err(CompressionError::InvalidInputSize {
                expected: expected_size,
                got: raw_data.len(),
            });
        }

        // For the real implementation, we'd use turbojpeg here
        // For now, we'll use the `image` crate as a fallback
        self.compress_with_image_crate(raw_data, width, height)
    }

    /// Fallback compression using the `image` crate
    fn compress_with_image_crate(
        &self,
        raw_data: &[u8],
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, CompressionError> {
        use image::{ImageBuffer, Rgba, ImageFormat};
        use std::io::Cursor;

        // Create image from raw data
        let img: ImageBuffer<Rgba<u8>, _> = ImageBuffer::from_raw(width, height, raw_data.to_vec())
            .ok_or(CompressionError::InvalidInputSize {
                expected: (width * height * 4) as usize,
                got: raw_data.len(),
            })?;

        // Encode to JPEG
        let mut output = Cursor::new(Vec::new());
        img.write_to(&mut output, ImageFormat::Jpeg)
            .map_err(|e| CompressionError::EncodingFailed(e.to_string()))?;

        Ok(output.into_inner())
    }
}

impl Default for FrameCompressor {
    fn default() -> Self {
        Self::new(80) // Good balance of quality and size
    }
}

// ============================================
// COMPRESSION ERRORS
// ============================================

/// Errors that can occur during frame compression
#[derive(Debug)]
pub enum CompressionError {
    /// Input data size doesn't match expected dimensions
    InvalidInputSize { expected: usize, got: usize },
    
    /// JPEG encoding failed
    EncodingFailed(String),
    
    /// Turbojpeg library error
    TurboJpegError(String),
}

impl std::fmt::Display for CompressionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInputSize { expected, got } => {
                write!(f, "Invalid input size: expected {} bytes, got {}", expected, got)
            }
            Self::EncodingFailed(msg) => {
                write!(f, "JPEG encoding failed: {}", msg)
            }
            Self::TurboJpegError(msg) => {
                write!(f, "TurboJPEG error: {}", msg)
            }
        }
    }
}

impl std::error::Error for CompressionError {}

// ============================================
// TESTS
// ============================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_creation() {
        let data = vec![0u8; 1000];
        let frame = CapturedFrame::new(data.clone(), 0, 100, 100);

        assert_eq!(frame.data, data);
        assert_eq!(frame.eye_index, 0);
        assert_eq!(frame.width, 100);
        assert_eq!(frame.height, 100);
        assert!(frame.timestamp_ns > 0);
    }

    #[test]
    fn test_compression_ratio() {
        let frame = CapturedFrame {
            data: vec![0u8; 1000], // 1KB compressed
            timestamp_ns: 0,
            eye_index: 0,
            width: 100,
            height: 100, // 100*100*4 = 40KB uncompressed
        };

        let ratio = frame.compression_ratio();
        assert!((ratio - 40.0).abs() < 0.1); // Should be about 40x
    }

    #[test]
    fn test_compressor_creation() {
        let compressor = FrameCompressor::new(80);
        assert_eq!(compressor.quality, 80);

        // Quality should be capped at 100
        let high_quality = FrameCompressor::new(150);
        assert_eq!(high_quality.quality, 100);
    }
}

