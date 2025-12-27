//! # Captured Frame Types
//!
//! Structures for representing captured VR frames.

use std::io::Cursor;
use std::time::{SystemTime, UNIX_EPOCH};

use image::{ImageBuffer, ImageFormat, Rgba};

// ============================================
// CAPTURED FRAME
// ============================================

/// A single captured frame from the VR eye buffer.
///
/// ## Plain English
///
/// This is one "photograph" from the VR headset containing:
/// - The image data (compressed as JPEG)
/// - When it was taken
/// - Which eye it's for
/// - Image dimensions
#[derive(Clone, Debug)]
pub struct CapturedFrame {
    /// JPEG-compressed image data
    pub data: Vec<u8>,

    /// Timestamp in nanoseconds since Unix epoch
    pub timestamp_ns: u64,

    /// Which eye (0 = left, 1 = right)
    pub eye_index: u32,

    /// Image width in pixels
    pub width: u32,

    /// Image height in pixels
    pub height: u32,
}

impl CapturedFrame {
    /// Creates a new captured frame.
    ///
    /// Automatically sets the timestamp to now.
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

    /// Creates a frame with a specific timestamp.
    pub fn with_timestamp(
        data: Vec<u8>,
        eye_index: u32,
        width: u32,
        height: u32,
        timestamp_ns: u64,
    ) -> Self {
        Self {
            data,
            timestamp_ns,
            eye_index,
            width,
            height,
        }
    }

    /// Returns the compressed data size in bytes.
    pub fn compressed_size(&self) -> usize {
        self.data.len()
    }

    /// Estimates the uncompressed size (width × height × 4 bytes).
    pub fn uncompressed_size(&self) -> usize {
        (self.width * self.height * 4) as usize
    }

    /// Returns the compression ratio.
    pub fn compression_ratio(&self) -> f32 {
        if self.data.is_empty() {
            return 0.0;
        }
        self.uncompressed_size() as f32 / self.compressed_size() as f32
    }
}

// ============================================
// FRAME COMPRESSOR
// ============================================

/// Compresses raw frame data to JPEG format.
///
/// ## Plain English
///
/// Raw images are huge (~14MB for Quest 3 resolution).
/// This compressor shrinks them to ~100KB using JPEG compression.
pub struct FrameCompressor {
    /// JPEG quality (0-100)
    quality: u8,
}

impl FrameCompressor {
    /// Creates a new compressor.
    ///
    /// ## Quality Guidelines
    /// - 90-100: Visually lossless, larger files
    /// - 70-85: Good quality, reasonable size (80 recommended)
    /// - 50-70: Noticeable artifacts, small files
    pub fn new(quality: u8) -> Self {
        Self {
            quality: quality.min(100),
        }
    }

    /// Compresses raw RGBA pixel data to JPEG.
    ///
    /// ## Parameters
    /// - `raw_rgba`: Raw pixel data in RGBA format (4 bytes per pixel)
    /// - `width`: Image width in pixels
    /// - `height`: Image height in pixels
    ///
    /// ## Returns
    /// JPEG-compressed data or an error
    pub fn compress(
        &self,
        raw_rgba: &[u8],
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, CompressionError> {
        let expected_size = (width * height * 4) as usize;
        if raw_rgba.len() != expected_size {
            return Err(CompressionError::InvalidSize {
                expected: expected_size,
                got: raw_rgba.len(),
            });
        }

        // Create image from raw data
        let img: ImageBuffer<Rgba<u8>, _> =
            ImageBuffer::from_raw(width, height, raw_rgba.to_vec())
                .ok_or(CompressionError::InvalidData)?;

        // Encode to JPEG
        let mut output = Cursor::new(Vec::new());
        
        // Convert RGBA to RGB for JPEG (no alpha channel support)
        let rgb_img = image::DynamicImage::ImageRgba8(img).to_rgb8();
        
        rgb_img
            .write_to(&mut output, ImageFormat::Jpeg)
            .map_err(|e| CompressionError::EncodingFailed(e.to_string()))?;

        Ok(output.into_inner())
    }

    /// Returns the quality setting.
    pub fn quality(&self) -> u8 {
        self.quality
    }
}

impl Default for FrameCompressor {
    fn default() -> Self {
        Self::new(80)
    }
}

// ============================================
// COMPRESSION ERRORS
// ============================================

/// Errors during frame compression.
#[derive(Debug)]
pub enum CompressionError {
    /// Input data size doesn't match dimensions
    InvalidSize { expected: usize, got: usize },

    /// Invalid input data
    InvalidData,

    /// JPEG encoding failed
    EncodingFailed(String),
}

impl std::fmt::Display for CompressionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSize { expected, got } => {
                write!(f, "Invalid size: expected {} bytes, got {}", expected, got)
            }
            Self::InvalidData => write!(f, "Invalid image data"),
            Self::EncodingFailed(msg) => write!(f, "Encoding failed: {}", msg),
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
    fn test_uncompressed_size() {
        let frame = CapturedFrame::new(vec![0u8; 1000], 0, 100, 100);
        // 100 x 100 x 4 = 40000 bytes
        assert_eq!(frame.uncompressed_size(), 40000);
    }

    #[test]
    fn test_compression_ratio() {
        let frame = CapturedFrame::new(vec![0u8; 1000], 0, 100, 100);
        // 40000 / 1000 = 40
        assert_eq!(frame.compression_ratio(), 40.0);
    }

    #[test]
    fn test_compressor_quality() {
        let compressor = FrameCompressor::new(80);
        assert_eq!(compressor.quality(), 80);

        // Quality capped at 100
        let high = FrameCompressor::new(150);
        assert_eq!(high.quality(), 100);
    }

    #[test]
    fn test_compression() {
        let compressor = FrameCompressor::new(80);

        // Create a larger test image (100x100 with varied colors)
        // Small images don't compress well due to JPEG header overhead
        let mut raw_rgba = vec![0u8; 100 * 100 * 4];
        for i in 0..(100 * 100) {
            raw_rgba[i * 4] = (i % 256) as u8; // R - varied
            raw_rgba[i * 4 + 1] = ((i / 100) % 256) as u8; // G
            raw_rgba[i * 4 + 2] = 128; // B
            raw_rgba[i * 4 + 3] = 255; // A
        }

        let result = compressor.compress(&raw_rgba, 100, 100);
        assert!(result.is_ok());

        let compressed = result.unwrap();
        // Verify compression happened (should be significantly smaller)
        assert!(compressed.len() < raw_rgba.len() / 2);
    }

    #[test]
    fn test_compression_invalid_size() {
        let compressor = FrameCompressor::new(80);
        let result = compressor.compress(&[0u8; 100], 100, 100);
        assert!(result.is_err());
    }
}

