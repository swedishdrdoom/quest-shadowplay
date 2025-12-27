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

    // On Android, this would start MediaProjection capture
    // For now, we just set the flag
    state.set_recording(true);

    #[cfg(target_os = "android")]
    {
        // Start MediaProjection capture
        if let Err(e) = crate::capture_android::start_capture(&state) {
            state.set_recording(false);
            return Err(format!("Failed to start capture: {}", e));
        }
    }

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

    #[cfg(target_os = "android")]
    {
        // Stop MediaProjection capture
        crate::capture_android::stop_capture(&state);
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

