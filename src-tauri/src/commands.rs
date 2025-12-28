//! Tauri Commands
//!
//! These commands are callable from the frontend JavaScript.
//! They bridge the UI to the Rust backend.

use std::sync::Arc;
use tauri::State;

use crate::state::{AppState, ClipInfo};
use quest_shadowplay::encoder::VideoEncoder;
use quest_shadowplay::storage::StorageManager;

/// Status information sent to the frontend
#[derive(serde::Serialize)]
pub struct StatusInfo {
    pub is_recording: bool,
    pub buffer_fill_percent: f32,
    pub frame_count: usize,
    pub buffer_capacity: usize,
    pub clips_count: usize,
}

/// Result of a save operation
#[derive(serde::Serialize)]
pub struct SaveResult {
    pub success: bool,
    pub message: String,
    pub clip_id: Option<String>,
}

// ============================================
// RECORDING COMMANDS
// ============================================

/// Starts the recording/capture process
#[tauri::command]
pub async fn start_recording(state: State<'_, Arc<AppState>>) -> Result<bool, String> {
    if state.is_recording() {
        log::warn!("Recording already active");
        return Ok(false);
    }

    log::info!("Starting recording...");

    // Create callback to push frames to buffer
    let buffer = Arc::clone(&state.buffer);
    let callback = std::sync::Arc::new(move |frame| {
        buffer.push_frame(frame);
    });

    // Start platform-specific capture
    {
        let capture = state.capture.lock();
        if let Err(e) = capture.start(callback) {
            log::error!("Failed to start capture: {}", e);
            return Err(format!("Failed to start capture: {}", e));
        }
    }

    state.set_recording(true);
    log::info!("Recording started");
    Ok(true)
}

/// Stops the recording/capture process
#[tauri::command]
pub async fn stop_recording(state: State<'_, Arc<AppState>>) -> Result<bool, String> {
    if !state.is_recording() {
        log::warn!("Recording not active");
        return Ok(false);
    }

    log::info!("Stopping recording...");

    // Stop platform-specific capture
    {
        let capture = state.capture.lock();
        capture.stop();
    }

    state.set_recording(false);
    log::info!("Recording stopped");
    Ok(true)
}

// ============================================
// CLIP MANAGEMENT COMMANDS
// ============================================

/// Saves the current buffer as a clip
#[tauri::command]
pub async fn save_clip(state: State<'_, Arc<AppState>>) -> Result<SaveResult, String> {
    log::info!("Saving clip...");

    // Get frames from buffer
    let frames = state.snapshot_frames();

    if frames.is_empty() {
        return Ok(SaveResult {
            success: false,
            message: "No frames in buffer".to_string(),
            clip_id: None,
        });
    }

    let frame_count = frames.len();
    log::info!("Saving {} frames...", frame_count);

    // Generate output path
    let output_path = StorageManager::generate_filename(
        state.clips_directory.to_str().unwrap_or("")
    );

    // Ensure directory exists
    if let Err(e) = std::fs::create_dir_all(&state.clips_directory) {
        return Ok(SaveResult {
            success: false,
            message: format!("Failed to create directory: {}", e),
            clip_id: None,
        });
    }

    // Encode frames
    match VideoEncoder::encode_frames(&frames, &output_path, &state.config) {
        Ok(()) => {
            let clip_id = std::path::Path::new(&output_path)
                .file_name()
                .map(|s| s.to_string_lossy().to_string());

            log::info!("Clip saved: {}", output_path);

            // Clear the buffer after successful save
            state.buffer.clear();
            log::info!("Buffer cleared after save");

            Ok(SaveResult {
                success: true,
                message: format!("Saved {} frames", frame_count),
                clip_id,
            })
        }
        Err(e) => {
            log::error!("Failed to save clip: {}", e);
            Ok(SaveResult {
                success: false,
                message: format!("Encoding failed: {}", e),
                clip_id: None,
            })
        }
    }
}

/// Gets the current status
#[tauri::command]
pub async fn get_status(state: State<'_, Arc<AppState>>) -> Result<StatusInfo, String> {
    let clips_count = state.list_clips().map(|c| c.len()).unwrap_or(0);

    Ok(StatusInfo {
        is_recording: state.is_recording(),
        buffer_fill_percent: state.buffer_fill() * 100.0,
        frame_count: state.frame_count(),
        buffer_capacity: state.config.buffer_frame_count(),
        clips_count,
    })
}

/// Lists all saved clips
#[tauri::command]
pub async fn list_clips(state: State<'_, Arc<AppState>>) -> Result<Vec<ClipInfo>, String> {
    state.list_clips().map_err(|e| format!("Failed to list clips: {}", e))
}

/// Deletes a clip by ID
#[tauri::command]
pub async fn delete_clip(state: State<'_, Arc<AppState>>, id: String) -> Result<bool, String> {
    state.delete_clip(&id).map_err(|e| format!("Failed to delete: {}", e))?;
    Ok(true)
}

/// Gets a thumbnail for a clip (base64 encoded)
#[tauri::command]
pub async fn get_clip_thumbnail(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<Option<String>, String> {
    let path = state.clips_directory.join(&id);

    if !path.exists() {
        return Ok(None);
    }

    // Try to read the clip and extract first frame
    match quest_shadowplay::encoder::FrameReader::open(path.to_str().unwrap_or("")) {
        Ok(reader) => {
            if let Some(frame) = reader.frames().first() {
                // Frame data is already JPEG, just base64 encode it
                let base64_data = base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    &frame.data
                );
                return Ok(Some(format!("data:image/jpeg;base64,{}", base64_data)));
            }
        }
        Err(e) => {
            log::warn!("Failed to read clip for thumbnail: {}", e);
        }
    }

    Ok(None)
}

/// Result of MP4 export
#[derive(serde::Serialize)]
pub struct ExportResult {
    pub success: bool,
    pub message: String,
    pub mp4_path: Option<String>,
}

/// Exports a clip to MP4 using ffmpeg
#[tauri::command]
pub async fn export_to_mp4(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<ExportResult, String> {
    let qsp_path = state.clips_directory.join(&id);
    
    if !qsp_path.exists() {
        return Ok(ExportResult {
            success: false,
            message: format!("Clip not found: {}", id),
            mp4_path: None,
        });
    }

    log::info!("Exporting {} to MP4...", id);

    // Read the clip
    let reader = match quest_shadowplay::encoder::FrameReader::open(qsp_path.to_str().unwrap_or("")) {
        Ok(r) => r,
        Err(e) => {
            return Ok(ExportResult {
                success: false,
                message: format!("Failed to read clip: {}", e),
                mp4_path: None,
            });
        }
    };

    let frames = reader.frames();
    if frames.is_empty() {
        return Ok(ExportResult {
            success: false,
            message: "Clip has no frames".to_string(),
            mp4_path: None,
        });
    }

    log::info!("Exporting {} frames to MP4...", frames.len());

    // Create temp directory for frames
    let temp_dir = std::env::temp_dir().join("quest_shadowplay_export");
    if let Err(e) = std::fs::create_dir_all(&temp_dir) {
        return Ok(ExportResult {
            success: false,
            message: format!("Failed to create temp dir: {}", e),
            mp4_path: None,
        });
    }

    // Write frames as JPEG files
    for (i, frame) in frames.iter().enumerate() {
        let frame_path = temp_dir.join(format!("frame_{:05}.jpg", i));
        if let Err(e) = std::fs::write(&frame_path, &frame.data) {
            return Ok(ExportResult {
                success: false,
                message: format!("Failed to write frame {}: {}", i, e),
                mp4_path: None,
            });
        }
    }

    // Calculate FPS from timestamps
    let fps = if frames.len() > 1 {
        let first_ts = frames.first().unwrap().timestamp_ns;
        let last_ts = frames.last().unwrap().timestamp_ns;
        let duration_ns = last_ts.saturating_sub(first_ts);
        if duration_ns > 0 {
            let duration_secs = duration_ns as f64 / 1_000_000_000.0;
            (frames.len() as f64 / duration_secs).round() as u32
        } else {
            30
        }
    } else {
        30
    };

    log::info!("Detected FPS: {}", fps);

    // Output MP4 path
    let mp4_name = id.replace(".qsp", ".mp4");
    let mp4_path = state.clips_directory.join(&mp4_name);

    // Prepare paths for ffmpeg
    let input_pattern = temp_dir.join("frame_%05d.jpg");
    let input_pattern_str = input_pattern.to_str().unwrap().to_string();
    let output_path_str = mp4_path.to_str().unwrap().to_string();
    let fps_str = fps.to_string();

    log::info!("Running ffmpeg: input={}, output={}, fps={}", input_pattern_str, output_path_str, fps_str);

    let output = std::process::Command::new("ffmpeg")
        .args([
            "-y",  // Overwrite
            "-framerate", &fps_str,
            "-i", &input_pattern_str,
            "-c:v", "libx264",
            "-preset", "fast",
            "-crf", "23",
            "-pix_fmt", "yuv420p",
            &output_path_str,
        ])
        .output();

    // Cleanup temp files
    let _ = std::fs::remove_dir_all(&temp_dir);

    match output {
        Ok(result) => {
            if result.status.success() {
                log::info!("MP4 exported successfully: {:?}", mp4_path);
                Ok(ExportResult {
                    success: true,
                    message: format!("Exported {} frames at {} FPS", frames.len(), fps),
                    mp4_path: Some(mp4_path.to_string_lossy().to_string()),
                })
            } else {
                let stderr = String::from_utf8_lossy(&result.stderr);
                log::error!("ffmpeg failed: {}", stderr);
                Ok(ExportResult {
                    success: false,
                    message: format!("ffmpeg failed: {}", stderr.chars().take(200).collect::<String>()),
                    mp4_path: None,
                })
            }
        }
        Err(e) => {
            log::error!("Failed to run ffmpeg: {}", e);
            Ok(ExportResult {
                success: false,
                message: format!("Failed to run ffmpeg: {}. Is ffmpeg installed?", e),
                mp4_path: None,
            })
        }
    }
}

