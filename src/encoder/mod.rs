//! # Video Encoder Module
//!
//! This module handles encoding captured frames into video files.
//!
//! ## Plain English Explanation
//!
//! We have 900 individual photos (frames). Now we need to:
//! 1. Stitch them together in order
//! 2. Compress them into a video format (H.264)
//! 3. Wrap them in a container (MP4) that any video player can open
//!
//! It's like making a flipbook, then binding it into a book, then putting
//! it in a standardized case so any library can shelve it.
//!
//! ```text
//!     900 Frames            Video Encoder              MP4 File
//!    ┌──┬──┬──┬──┐         ┌───────────┐         ┌─────────────┐
//!    │1 │2 │3 │..│ ──────▶ │  H.264    │ ──────▶ │ clip.mp4    │
//!    └──┴──┴──┴──┘         │  Encoding │         │ (playable!) │
//!                          └───────────┘         └─────────────┘
//! ```

use std::path::Path;

use crate::capture::CapturedFrame;
use crate::config::Config;
use crate::error::{ShadowplayError, EncoderErrorKind};

// ============================================
// VIDEO ENCODER
// Main encoding functionality
// ============================================

/// Video encoder that converts frames to H.264 video
///
/// ## Plain English
///
/// This is the video-making machine. You feed it photos, it gives you a movie.
/// It uses "hardware encoding" when available - a special chip in the Quest 3
/// designed specifically for making videos quickly and efficiently.
pub struct VideoEncoder {
    /// Width of output video in pixels
    width: u32,
    
    /// Height of output video in pixels
    height: u32,
    
    /// Target frame rate
    fps: u32,
    
    /// Encoding bitrate in bits per second
    bitrate: u32,
    
    /// Frames encoded so far
    frames_encoded: u32,
}

impl VideoEncoder {
    /// Creates a new video encoder
    ///
    /// ## Parameters
    /// - `width`: Video width in pixels
    /// - `height`: Video height in pixels
    /// - `fps`: Frames per second
    /// - `bitrate`: Target bitrate in bits per second
    ///
    /// ## Plain English
    /// "Build me a video maker with these settings"
    pub fn new(width: u32, height: u32, fps: u32, bitrate: u32) -> Result<Self, ShadowplayError> {
        log::info!(
            "Creating video encoder: {}x{} @ {} FPS, {} Mbps",
            width, height, fps, bitrate / 1_000_000
        );

        // In real implementation, we'd initialize Android MediaCodec here
        // For now, we just store the parameters

        Ok(Self {
            width,
            height,
            fps,
            bitrate,
            frames_encoded: 0,
        })
    }

    /// Encodes a list of frames to a video file
    ///
    /// ## Parameters
    /// - `frames`: The frames to encode (in order, oldest first)
    /// - `output_path`: Where to save the video
    /// - `config`: Configuration settings
    ///
    /// ## What Happens (Plain English)
    ///
    /// 1. Open the output file
    /// 2. Set up the video encoder
    /// 3. For each frame:
    ///    a. Decompress from JPEG (we stored them compressed)
    ///    b. Convert color format if needed
    ///    c. Feed to hardware encoder
    ///    d. Write encoded data to file
    /// 4. Finalize the video file (write headers, etc.)
    ///
    /// This runs in a background thread so it doesn't block VR.
    pub fn encode_frames(
        frames: &[CapturedFrame],
        output_path: &str,
        config: &Config,
    ) -> Result<(), ShadowplayError> {
        if frames.is_empty() {
            log::warn!("No frames to encode");
            return Ok(());
        }

        log::info!("Encoding {} frames to {}", frames.len(), output_path);

        let start_time = std::time::Instant::now();

        // Get dimensions from first frame
        let first_frame = &frames[0];
        let width = first_frame.width;
        let height = first_frame.height;

        // Create encoder
        let mut encoder = Self::new(
            width,
            height,
            config.target_fps,
            config.video_bitrate,
        )?;

        // Create muxer (MP4 container writer)
        let mut muxer = Mp4Muxer::new(output_path, width, height, config.target_fps)?;

        // Calculate frame duration in microseconds
        let frame_duration_us = 1_000_000 / config.target_fps;

        // Encode each frame
        for (index, frame) in frames.iter().enumerate() {
            // Calculate presentation timestamp
            let pts_us = (index as u64) * (frame_duration_us as u64);

            // Decode JPEG to raw pixels
            let raw_pixels = Self::decode_jpeg(&frame.data)?;

            // Encode frame
            let encoded_data = encoder.encode_frame(&raw_pixels, pts_us)?;

            // Write to muxer
            muxer.add_frame(&encoded_data, pts_us)?;

            // Log progress periodically
            if index % 100 == 0 {
                log::debug!("Encoded {}/{} frames", index, frames.len());
            }
        }

        // Finalize
        muxer.finalize()?;

        let elapsed = start_time.elapsed();
        let fps = frames.len() as f64 / elapsed.as_secs_f64();
        log::info!(
            "Encoding complete! {} frames in {:.2}s ({:.1} fps)",
            frames.len(),
            elapsed.as_secs_f64(),
            fps
        );

        Ok(())
    }

    /// Encodes a single frame
    ///
    /// ## Parameters
    /// - `raw_pixels`: Raw RGBA pixel data
    /// - `pts_us`: Presentation timestamp in microseconds
    ///
    /// ## Returns
    /// Encoded H.264 NAL units (compressed video data)
    fn encode_frame(&mut self, raw_pixels: &[u8], pts_us: u64) -> Result<Vec<u8>, ShadowplayError> {
        // In real implementation:
        // 1. Convert RGBA to YUV420 (what H.264 uses)
        // 2. Queue input buffer to MediaCodec
        // 3. Dequeue output buffer with encoded data
        // 4. Return the encoded bytes

        // For now, simulate encoding
        self.frames_encoded += 1;

        // Placeholder: return dummy encoded data
        // Real encoded data would be much smaller than raw
        Ok(vec![0u8; 10000]) // ~10KB per frame (placeholder)
    }

    /// Decodes JPEG data back to raw pixels
    ///
    /// ## Plain English
    ///
    /// We stored frames as compressed JPEGs to save memory.
    /// Now we need to "unzip" them back to raw pixels so the
    /// video encoder can use them.
    fn decode_jpeg(jpeg_data: &[u8]) -> Result<Vec<u8>, ShadowplayError> {
        use image::io::Reader as ImageReader;
        use std::io::Cursor;

        // Decode JPEG
        let img = ImageReader::new(Cursor::new(jpeg_data))
            .with_guessed_format()
            .map_err(|e| ShadowplayError::Encoder(
                EncoderErrorKind::InvalidFrameData
            ))?
            .decode()
            .map_err(|e| ShadowplayError::Encoder(
                EncoderErrorKind::FrameEncodeFailed(e.to_string())
            ))?;

        // Convert to RGBA raw bytes
        Ok(img.into_rgba8().into_raw())
    }

    /// Returns encoding statistics
    pub fn stats(&self) -> EncoderStats {
        EncoderStats {
            frames_encoded: self.frames_encoded,
            width: self.width,
            height: self.height,
            fps: self.fps,
            bitrate: self.bitrate,
        }
    }
}

// ============================================
// MP4 MUXER
// Wraps encoded video in MP4 container
// ============================================

/// MP4 container muxer
///
/// ## Plain English
///
/// Raw H.264 data is like loose pages from a book. An MP4 "muxer"
/// (multiplexer) binds them into a proper book with:
/// - A table of contents (so you can skip around)
/// - Timing information (so it plays at the right speed)
/// - Metadata (title, creation date, etc.)
///
/// The result is a file that any video player knows how to read.
pub struct Mp4Muxer {
    /// Output file path
    output_path: String,
    
    /// Video width
    width: u32,
    
    /// Video height
    height: u32,
    
    /// Frames per second
    fps: u32,
    
    /// Encoded frame data (stored until finalize)
    frame_data: Vec<(Vec<u8>, u64)>, // (data, timestamp)
}

impl Mp4Muxer {
    /// Creates a new MP4 muxer
    ///
    /// ## Parameters
    /// - `output_path`: Where to save the MP4 file
    /// - `width`: Video width
    /// - `height`: Video height
    /// - `fps`: Frames per second
    pub fn new(
        output_path: &str,
        width: u32,
        height: u32,
        fps: u32,
    ) -> Result<Self, ShadowplayError> {
        log::debug!("Creating MP4 muxer for {}", output_path);

        // Ensure parent directory exists
        if let Some(parent) = Path::new(output_path).parent() {
            std::fs::create_dir_all(parent)?;
        }

        Ok(Self {
            output_path: output_path.to_string(),
            width,
            height,
            fps,
            frame_data: Vec::new(),
        })
    }

    /// Adds an encoded frame to the MP4
    ///
    /// ## Parameters
    /// - `data`: Encoded H.264 data for this frame
    /// - `pts_us`: Presentation timestamp in microseconds
    pub fn add_frame(&mut self, data: &[u8], pts_us: u64) -> Result<(), ShadowplayError> {
        self.frame_data.push((data.to_vec(), pts_us));
        Ok(())
    }

    /// Finalizes the MP4 file
    ///
    /// ## What Happens (Plain English)
    ///
    /// 1. Write the video data
    /// 2. Write the "moov" atom (table of contents)
    /// 3. Close the file
    ///
    /// After this, the file is a valid, playable MP4!
    pub fn finalize(self) -> Result<(), ShadowplayError> {
        log::info!("Finalizing MP4: {}", self.output_path);

        // In real implementation, we'd use an MP4 muxing library
        // (like mp4 crate or Android's MediaMuxer)

        // For now, simulate by writing a placeholder file
        use std::fs::File;
        use std::io::Write;

        let mut file = File::create(&self.output_path)?;
        
        // Write placeholder MP4 header
        // Real implementation would write proper ftyp, moov, mdat atoms
        file.write_all(b"MP4_PLACEHOLDER")?;

        // Write frame count for verification
        let frame_count = self.frame_data.len();
        file.write_all(&(frame_count as u32).to_le_bytes())?;

        log::info!(
            "MP4 finalized with {} frames: {}",
            frame_count,
            self.output_path
        );

        Ok(())
    }
}

// ============================================
// ENCODER STATISTICS
// ============================================

/// Statistics about the encoding process
#[derive(Debug, Clone)]
pub struct EncoderStats {
    /// Number of frames encoded
    pub frames_encoded: u32,
    
    /// Video width
    pub width: u32,
    
    /// Video height
    pub height: u32,
    
    /// Target frame rate
    pub fps: u32,
    
    /// Target bitrate
    pub bitrate: u32,
}

impl EncoderStats {
    /// Returns estimated file size in bytes for a given duration
    ///
    /// ## Plain English
    /// "How big will a 10-second clip be?"
    pub fn estimated_file_size(&self, duration_seconds: f32) -> u64 {
        // File size ≈ bitrate × duration / 8
        // (Divide by 8 to convert bits to bytes)
        ((self.bitrate as f64 * duration_seconds as f64) / 8.0) as u64
    }

    /// Returns estimated encoding time based on hardware capability
    ///
    /// ## Plain English
    /// "How long will encoding take?"
    /// Quest 3 hardware can usually encode at 2-4x real-time speed.
    pub fn estimated_encoding_time(&self, frame_count: u32) -> std::time::Duration {
        // Assume 3x real-time encoding speed
        let real_time_seconds = frame_count as f64 / self.fps as f64;
        let encoding_seconds = real_time_seconds / 3.0;
        std::time::Duration::from_secs_f64(encoding_seconds)
    }
}

// ============================================
// COLOR CONVERSION
// ============================================

/// Converts RGBA pixels to YUV420 format
///
/// ## Plain English
///
/// H.264 doesn't use RGB colors like your screen. It uses "YUV" which
/// separates brightness (Y) from color (U and V). This is more efficient
/// for compression because our eyes are more sensitive to brightness
/// than color detail.
///
/// YUV420 means the color information is at quarter resolution - we
/// have full-resolution brightness, but only quarter-resolution color.
/// You won't notice the difference!
pub fn rgba_to_yuv420(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    let pixel_count = (width * height) as usize;
    
    // YUV420 size: Y at full res + U at quarter + V at quarter
    // = width×height + (width×height/4) + (width×height/4)
    // = width×height × 1.5
    let yuv_size = pixel_count + pixel_count / 2;
    let mut yuv = vec![0u8; yuv_size];

    // Y plane (full resolution brightness)
    for i in 0..pixel_count {
        let r = rgba[i * 4] as f32;
        let g = rgba[i * 4 + 1] as f32;
        let b = rgba[i * 4 + 2] as f32;
        
        // Standard BT.601 conversion
        let y = (0.299 * r + 0.587 * g + 0.114 * b) as u8;
        yuv[i] = y;
    }

    // U and V planes (quarter resolution color)
    let uv_width = width / 2;
    let uv_height = height / 2;
    let u_offset = pixel_count;
    let v_offset = pixel_count + (uv_width * uv_height) as usize;

    for y in 0..uv_height {
        for x in 0..uv_width {
            // Average 2x2 block of pixels
            let base_x = (x * 2) as usize;
            let base_y = (y * 2) as usize;
            
            let mut sum_r = 0u32;
            let mut sum_g = 0u32;
            let mut sum_b = 0u32;

            for dy in 0..2 {
                for dx in 0..2 {
                    let idx = ((base_y + dy) * width as usize + base_x + dx) * 4;
                    sum_r += rgba[idx] as u32;
                    sum_g += rgba[idx + 1] as u32;
                    sum_b += rgba[idx + 2] as u32;
                }
            }

            let r = (sum_r / 4) as f32;
            let g = (sum_g / 4) as f32;
            let b = (sum_b / 4) as f32;

            // U (Cb) and V (Cr) components
            let u = ((-0.169 * r - 0.331 * g + 0.500 * b) + 128.0) as u8;
            let v = ((0.500 * r - 0.419 * g - 0.081 * b) + 128.0) as u8;

            let uv_idx = (y * uv_width + x) as usize;
            yuv[u_offset + uv_idx] = u;
            yuv[v_offset + uv_idx] = v;
        }
    }

    yuv
}

// ============================================
// TESTS
// ============================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoder_creation() {
        let encoder = VideoEncoder::new(1920, 1080, 90, 20_000_000);
        assert!(encoder.is_ok());
        
        let encoder = encoder.unwrap();
        let stats = encoder.stats();
        assert_eq!(stats.width, 1920);
        assert_eq!(stats.height, 1080);
        assert_eq!(stats.fps, 90);
    }

    #[test]
    fn test_estimated_file_size() {
        let stats = EncoderStats {
            frames_encoded: 0,
            width: 1920,
            height: 1080,
            fps: 90,
            bitrate: 20_000_000, // 20 Mbps
        };

        // 10 seconds at 20 Mbps = 200 Mb = 25 MB
        let size = stats.estimated_file_size(10.0);
        assert_eq!(size, 25_000_000);
    }

    #[test]
    fn test_yuv_conversion_size() {
        let width = 100;
        let height = 100;
        let rgba = vec![128u8; (width * height * 4) as usize];
        
        let yuv = rgba_to_yuv420(&rgba, width, height);
        
        // YUV420 should be 1.5x the pixel count
        let expected = (width * height) as usize + (width * height / 2) as usize;
        assert_eq!(yuv.len(), expected);
    }
}

