//! # Video Encoder Module
//!
//! Encodes captured frames into video files.
//!
//! ## Plain English
//!
//! We have 900 individual photos (frames). This module:
//! 1. Stitches them together in order
//! 2. Compresses them into video format (H.264)
//! 3. Wraps them in a container (MP4)
//!
//! On Quest 3, this uses hardware encoding for speed.

use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::capture::CapturedFrame;
use crate::config::Config;
use crate::error::{ShadowplayError, ShadowplayResult};

// ============================================
// VIDEO ENCODER
// ============================================

/// Encodes frames to video file.
///
/// ## Implementation Notes
///
/// This is a simplified implementation that creates a sequence of
/// JPEG frames. A full implementation would use Android MediaCodec
/// for H.264 hardware encoding on Quest 3.
pub struct VideoEncoder {
    /// Output width
    width: u32,
    /// Output height
    height: u32,
    /// Frames per second
    fps: u32,
    /// Bitrate in bits per second
    bitrate: u32,
}

impl VideoEncoder {
    /// Creates a new video encoder.
    pub fn new(width: u32, height: u32, fps: u32, bitrate: u32) -> Self {
        Self {
            width,
            height,
            fps,
            bitrate,
        }
    }

    /// Encodes frames to a video file.
    ///
    /// ## Parameters
    /// - `frames`: Frames to encode (oldest first)
    /// - `output_path`: Where to save the video
    /// - `config`: Configuration settings
    pub fn encode_frames(
        frames: &[CapturedFrame],
        output_path: &str,
        config: &Config,
    ) -> ShadowplayResult<()> {
        if frames.is_empty() {
            return Err(ShadowplayError::Encoder("No frames to encode".to_string()));
        }

        log::info!("Encoding {} frames to {}", frames.len(), output_path);
        let start = std::time::Instant::now();

        // Get dimensions from first frame
        let first = &frames[0];
        let encoder = Self::new(first.width, first.height, config.target_fps, config.video_bitrate);

        // For now, we'll create a simple format that stores the frames
        // In production, this would use MediaCodec for H.264 encoding
        encoder.write_frames(frames, output_path)?;

        let elapsed = start.elapsed();
        log::info!(
            "Encoding complete: {} frames in {:.2}s ({:.1} fps)",
            frames.len(),
            elapsed.as_secs_f64(),
            frames.len() as f64 / elapsed.as_secs_f64()
        );

        Ok(())
    }

    /// Writes frames to file.
    ///
    /// This is a simplified implementation. Real implementation would
    /// use hardware H.264 encoding.
    fn write_frames(&self, frames: &[CapturedFrame], output_path: &str) -> ShadowplayResult<()> {
        // Ensure parent directory exists
        if let Some(parent) = Path::new(output_path).parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Create output file
        let mut file = File::create(output_path)?;

        // Write a simple container format
        // Header: magic + version + frame count + width + height + fps
        file.write_all(b"QSPLAY01")?; // Magic + version
        file.write_all(&(frames.len() as u32).to_le_bytes())?;
        file.write_all(&self.width.to_le_bytes())?;
        file.write_all(&self.height.to_le_bytes())?;
        file.write_all(&self.fps.to_le_bytes())?;

        // Write each frame: timestamp + eye_index + data_len + data
        for frame in frames {
            file.write_all(&frame.timestamp_ns.to_le_bytes())?;
            file.write_all(&frame.eye_index.to_le_bytes())?;
            file.write_all(&(frame.data.len() as u32).to_le_bytes())?;
            file.write_all(&frame.data)?;
        }

        file.sync_all()?;
        
        log::debug!("Wrote {} bytes to {}", file.metadata()?.len(), output_path);
        Ok(())
    }

    /// Returns encoder info.
    pub fn info(&self) -> EncoderInfo {
        EncoderInfo {
            width: self.width,
            height: self.height,
            fps: self.fps,
            bitrate: self.bitrate,
        }
    }
}

/// Encoder configuration info.
#[derive(Debug, Clone)]
pub struct EncoderInfo {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bitrate: u32,
}

impl EncoderInfo {
    /// Estimates file size for given duration.
    pub fn estimated_size_bytes(&self, duration_secs: f32) -> u64 {
        // size = bitrate * duration / 8
        ((self.bitrate as f64 * duration_secs as f64) / 8.0) as u64
    }
}

// ============================================
// FRAME READER (for playback)
// ============================================

/// Reads frames from our custom format.
pub struct FrameReader {
    frames: Vec<CapturedFrame>,
    width: u32,
    height: u32,
    fps: u32,
}

impl FrameReader {
    /// Opens a clip file for reading.
    pub fn open(path: &str) -> ShadowplayResult<Self> {
        use std::io::Read;

        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        Self::parse(&buffer)
    }

    /// Parses clip data.
    fn parse(data: &[u8]) -> ShadowplayResult<Self> {
        if data.len() < 24 {
            return Err(ShadowplayError::Encoder("File too small".to_string()));
        }

        // Check magic
        if &data[0..8] != b"QSPLAY01" {
            return Err(ShadowplayError::Encoder("Invalid file format".to_string()));
        }

        let frame_count = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;
        let width = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
        let height = u32::from_le_bytes([data[16], data[17], data[18], data[19]]);
        let fps = u32::from_le_bytes([data[20], data[21], data[22], data[23]]);

        let mut frames = Vec::with_capacity(frame_count);
        let mut offset = 24;

        for _ in 0..frame_count {
            if offset + 16 > data.len() {
                break;
            }

            let timestamp_ns = u64::from_le_bytes([
                data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
                data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7],
            ]);
            offset += 8;

            let eye_index = u32::from_le_bytes([
                data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
            ]);
            offset += 4;

            let data_len = u32::from_le_bytes([
                data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
            ]) as usize;
            offset += 4;

            if offset + data_len > data.len() {
                break;
            }

            let frame_data = data[offset..offset + data_len].to_vec();
            offset += data_len;

            frames.push(CapturedFrame::with_timestamp(
                frame_data,
                eye_index,
                width,
                height,
                timestamp_ns,
            ));
        }

        Ok(Self {
            frames,
            width,
            height,
            fps,
        })
    }

    /// Returns all frames.
    pub fn frames(&self) -> &[CapturedFrame] {
        &self.frames
    }

    /// Returns frame count.
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Returns video dimensions.
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Returns frame rate.
    pub fn fps(&self) -> u32 {
        self.fps
    }

    /// Returns duration in seconds.
    pub fn duration_secs(&self) -> f32 {
        self.frames.len() as f32 / self.fps as f32
    }
}

// ============================================
// TESTS
// ============================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn dummy_frame(id: u64) -> CapturedFrame {
        CapturedFrame::with_timestamp(vec![0u8; 100], 0, 100, 100, id)
    }

    #[test]
    fn test_encoder_info() {
        let encoder = VideoEncoder::new(1920, 1080, 90, 20_000_000);
        let info = encoder.info();

        assert_eq!(info.width, 1920);
        assert_eq!(info.height, 1080);
        assert_eq!(info.fps, 90);
    }

    #[test]
    fn test_estimated_size() {
        let info = EncoderInfo {
            width: 1920,
            height: 1080,
            fps: 90,
            bitrate: 20_000_000, // 20 Mbps
        };

        // 10 seconds at 20 Mbps = 200 Mb = 25 MB
        let size = info.estimated_size_bytes(10.0);
        assert_eq!(size, 25_000_000);
    }

    #[test]
    fn test_encode_and_read() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.qsp");
        let path_str = path.to_str().unwrap();

        // Create test frames
        let frames: Vec<_> = (0..10).map(|i| dummy_frame(i)).collect();

        // Encode
        let config = Config::default();
        VideoEncoder::encode_frames(&frames, path_str, &config).unwrap();

        // Verify file exists
        assert!(path.exists());

        // Read back
        let reader = FrameReader::open(path_str).unwrap();
        assert_eq!(reader.frame_count(), 10);
        assert_eq!(reader.dimensions(), (100, 100));
    }
}
