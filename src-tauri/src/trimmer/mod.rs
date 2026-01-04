//! Video Trimmer Module
//!
//! Provides video analysis and trimming capabilities using native platform APIs.
//! On macOS, this uses AVFoundation via Swift FFI for high-performance operations.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};
use std::path::Path;

/// Video information structure (matches Swift VideoInfo)
#[repr(C)]
#[derive(Debug, Clone)]
pub struct VideoInfo {
    pub duration_secs: f32,
    pub width: u32,
    pub height: u32,
    pub frame_rate: f32,
    pub file_size_bytes: u64,
    pub has_audio: bool,
    pub codec: u32, // 0 = unknown, 1 = H.264, 2 = HEVC
}

impl Default for VideoInfo {
    fn default() -> Self {
        Self {
            duration_secs: 0.0,
            width: 0,
            height: 0,
            frame_rate: 0.0,
            file_size_bytes: 0,
            has_audio: false,
            codec: 0,
        }
    }
}

impl VideoInfo {
    /// Returns codec name as string
    pub fn codec_name(&self) -> &'static str {
        match self.codec {
            1 => "H.264",
            2 => "HEVC",
            _ => "Unknown",
        }
    }

    /// Returns human-readable duration
    pub fn duration_formatted(&self) -> String {
        let total_secs = self.duration_secs as u64;
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        let millis = ((self.duration_secs - total_secs as f32) * 10.0) as u32;
        format!("{}:{:02}.{}", mins, secs, millis)
    }

    /// Returns human-readable file size
    pub fn file_size_formatted(&self) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;

        if self.file_size_bytes >= MB {
            format!("{:.1} MB", self.file_size_bytes as f64 / MB as f64)
        } else if self.file_size_bytes >= KB {
            format!("{:.1} KB", self.file_size_bytes as f64 / KB as f64)
        } else {
            format!("{} B", self.file_size_bytes)
        }
    }
}

/// Thumbnail data with multiple images
pub struct TimelineThumbnails {
    /// JPEG data for each thumbnail
    pub thumbnails: Vec<Vec<u8>>,
}

impl TimelineThumbnails {
    /// Parse thumbnail data from Swift FFI format
    /// Format: [4-byte size (little-endian)][jpeg data]... repeated
    pub fn from_raw_data(data: &[u8]) -> Self {
        let mut thumbnails = Vec::new();
        let mut offset = 0;

        while offset + 4 <= data.len() {
            // Read size as little-endian u32
            let size = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]) as usize;

            offset += 4;

            if size == 0 {
                // Empty thumbnail (generation failed for this frame)
                thumbnails.push(Vec::new());
                continue;
            }

            if offset + size > data.len() {
                log::warn!("Truncated thumbnail data at offset {}", offset);
                break;
            }

            thumbnails.push(data[offset..offset + size].to_vec());
            offset += size;
        }

        Self { thumbnails }
    }

    /// Convert thumbnails to base64-encoded data URLs
    pub fn to_data_urls(&self) -> Vec<Option<String>> {
        self.thumbnails
            .iter()
            .map(|data| {
                if data.is_empty() {
                    None
                } else {
                    let base64 = base64::Engine::encode(
                        &base64::engine::general_purpose::STANDARD,
                        data,
                    );
                    Some(format!("data:image/jpeg;base64,{}", base64))
                }
            })
            .collect()
    }
}

/// Result of a trim operation
#[derive(Debug)]
pub struct TrimResult {
    pub success: bool,
    pub error: Option<String>,
    pub output_path: Option<String>,
    pub output_size_bytes: u64,
}

// ============================================
// macOS FFI Declarations
// ============================================

#[cfg(target_os = "macos")]
extern "C" {
    fn swift_get_video_info(
        path: *const c_char,
        out_duration: *mut f32,
        out_width: *mut u32,
        out_height: *mut u32,
        out_frame_rate: *mut f32,
        out_file_size: *mut u64,
        out_has_audio: *mut i32,
        out_codec: *mut u32,
    ) -> i32;

    fn swift_generate_thumbnails(
        path: *const c_char,
        count: u32,
        thumbnail_width: u32,
        out_size: *mut u64,
    ) -> *mut c_void;

    fn swift_free_data(ptr: *mut c_void, size: u64);

    fn swift_trim_video(
        input_path: *const c_char,
        output_path: *const c_char,
        start_time: f32,
        end_time: f32,
        error_out: *mut *const c_char,
    ) -> i32;

    fn swift_free_string(ptr: *const c_char);
}

// ============================================
// Public API
// ============================================

/// Get information about a video file
pub fn get_video_info(path: &Path) -> Result<VideoInfo, String> {
    #[cfg(target_os = "macos")]
    {
        let path_str = path
            .to_str()
            .ok_or_else(|| "Invalid path".to_string())?;
        let c_path = CString::new(path_str).map_err(|_| "Invalid path string")?;

        let mut duration: f32 = 0.0;
        let mut width: u32 = 0;
        let mut height: u32 = 0;
        let mut frame_rate: f32 = 0.0;
        let mut file_size: u64 = 0;
        let mut has_audio: i32 = 0;
        let mut codec: u32 = 0;

        let result = unsafe {
            swift_get_video_info(
                c_path.as_ptr(),
                &mut duration,
                &mut width,
                &mut height,
                &mut frame_rate,
                &mut file_size,
                &mut has_audio,
                &mut codec,
            )
        };

        if result == 0 || duration <= 0.0 {
            return Err(format!("Could not read video info from {:?}", path));
        }

        Ok(VideoInfo {
            duration_secs: duration,
            width,
            height,
            frame_rate,
            file_size_bytes: file_size,
            has_audio: has_audio != 0,
            codec,
        })
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        Err("Video trimming only available on macOS".to_string())
    }
}

/// Generate timeline thumbnails for a video
pub fn generate_thumbnails(
    path: &Path,
    count: usize,
    thumbnail_width: u32,
) -> Result<TimelineThumbnails, String> {
    #[cfg(target_os = "macos")]
    {
        let path_str = path
            .to_str()
            .ok_or_else(|| "Invalid path".to_string())?;
        let c_path = CString::new(path_str).map_err(|_| "Invalid path string")?;

        let mut out_size: u64 = 0;
        let ptr = unsafe {
            swift_generate_thumbnails(
                c_path.as_ptr(),
                count as u32,
                thumbnail_width,
                &mut out_size,
            )
        };

        if ptr.is_null() || out_size == 0 {
            return Err("Failed to generate thumbnails".to_string());
        }

        // Copy data to Rust Vec
        let data = unsafe {
            std::slice::from_raw_parts(ptr as *const u8, out_size as usize).to_vec()
        };

        // Free Swift-allocated memory
        unsafe { swift_free_data(ptr, out_size) };

        Ok(TimelineThumbnails::from_raw_data(&data))
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (path, count, thumbnail_width);
        Err("Video trimming only available on macOS".to_string())
    }
}

/// Trim a video to the specified time range
pub fn trim_video(
    input_path: &Path,
    output_path: &Path,
    start_time: f32,
    end_time: f32,
) -> TrimResult {
    #[cfg(target_os = "macos")]
    {
        let input_str = match input_path.to_str() {
            Some(s) => s,
            None => {
                return TrimResult {
                    success: false,
                    error: Some("Invalid input path".to_string()),
                    output_path: None,
                    output_size_bytes: 0,
                }
            }
        };

        let output_str = match output_path.to_str() {
            Some(s) => s,
            None => {
                return TrimResult {
                    success: false,
                    error: Some("Invalid output path".to_string()),
                    output_path: None,
                    output_size_bytes: 0,
                }
            }
        };

        let c_input = match CString::new(input_str) {
            Ok(s) => s,
            Err(_) => {
                return TrimResult {
                    success: false,
                    error: Some("Invalid input path string".to_string()),
                    output_path: None,
                    output_size_bytes: 0,
                }
            }
        };

        let c_output = match CString::new(output_str) {
            Ok(s) => s,
            Err(_) => {
                return TrimResult {
                    success: false,
                    error: Some("Invalid output path string".to_string()),
                    output_path: None,
                    output_size_bytes: 0,
                }
            }
        };

        let mut error_ptr: *const c_char = std::ptr::null();

        let result = unsafe {
            swift_trim_video(
                c_input.as_ptr(),
                c_output.as_ptr(),
                start_time,
                end_time,
                &mut error_ptr,
            )
        };

        let success = result != 0;

        let error = if !error_ptr.is_null() {
            let err = unsafe { CStr::from_ptr(error_ptr) }
                .to_string_lossy()
                .to_string();
            unsafe { swift_free_string(error_ptr) };
            Some(err)
        } else {
            None
        };

        // Get output file size
        let output_size = if success {
            std::fs::metadata(output_path)
                .map(|m| m.len())
                .unwrap_or(0)
        } else {
            0
        };

        TrimResult {
            success,
            error,
            output_path: if success {
                Some(output_str.to_string())
            } else {
                None
            },
            output_size_bytes: output_size,
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (input_path, output_path, start_time, end_time);
        TrimResult {
            success: false,
            error: Some("Video trimming only available on macOS".to_string()),
            output_path: None,
            output_size_bytes: 0,
        }
    }
}

