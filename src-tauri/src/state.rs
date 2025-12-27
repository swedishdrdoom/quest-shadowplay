//! Application State Management
//!
//! Manages the shared state between the UI and the capture system.

#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use quest_shadowplay::{Config, SharedFrameBuffer, CapturedFrame};

/// Shared application state
pub struct AppState {
    /// Frame buffer for storing captured frames
    pub buffer: Arc<SharedFrameBuffer>,

    /// Application configuration
    pub config: Config,

    /// Is recording currently active?
    pub is_recording: AtomicBool,

    /// Directory for saved clips
    pub clips_directory: PathBuf,

    /// Capture handle (platform-specific)
    #[cfg(target_os = "android")]
    pub capture_handle: parking_lot::Mutex<Option<crate::capture_android::CaptureHandle>>,
}

impl AppState {
    /// Creates a new application state
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let config = Config::default();

        // Determine clips directory based on platform
        let clips_directory = Self::get_clips_directory();

        // Create clips directory if it doesn't exist
        if !clips_directory.exists() {
            std::fs::create_dir_all(&clips_directory)?;
        }

        log::info!("Clips directory: {:?}", clips_directory);

        let buffer = Arc::new(SharedFrameBuffer::new(
            config.buffer_duration_seconds,
            config.target_fps,
        ));

        Ok(Self {
            buffer,
            config,
            is_recording: AtomicBool::new(false),
            clips_directory,
            #[cfg(target_os = "android")]
            capture_handle: Mutex::new(None),
        })
    }

    /// Gets the clips directory for the current platform
    fn get_clips_directory() -> PathBuf {
        #[cfg(target_os = "android")]
        {
            PathBuf::from("/sdcard/QuestShadowplay")
        }

        #[cfg(not(target_os = "android"))]
        {
            let mut path = std::env::temp_dir();
            path.push("QuestShadowplay");
            path
        }
    }

    /// Returns whether recording is active
    pub fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::SeqCst)
    }

    /// Sets the recording state
    pub fn set_recording(&self, recording: bool) {
        self.is_recording.store(recording, Ordering::SeqCst);
    }

    /// Returns the buffer fill percentage
    pub fn buffer_fill(&self) -> f32 {
        self.buffer.fill_percentage()
    }

    /// Returns the number of frames in the buffer
    pub fn frame_count(&self) -> usize {
        self.buffer.frame_count()
    }

    /// Adds a frame to the buffer
    pub fn push_frame(&self, frame: CapturedFrame) {
        self.buffer.push_frame(frame);
    }

    /// Gets a snapshot of all frames for saving
    pub fn snapshot_frames(&self) -> Vec<CapturedFrame> {
        self.buffer.snapshot()
    }

    /// Lists all saved clips
    pub fn list_clips(&self) -> Result<Vec<ClipInfo>, std::io::Error> {
        let mut clips = Vec::new();

        if !self.clips_directory.exists() {
            return Ok(clips);
        }

        for entry in std::fs::read_dir(&self.clips_directory)? {
            let entry = entry?;
            let path = entry.path();

            // Check for our clip format
            if path.extension().map(|e| e == "qsp").unwrap_or(false) {
                if let Ok(metadata) = entry.metadata() {
                    let filename = path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();

                    // Parse timestamp from filename (clip_YYYYMMDD_HHMMSS.qsp)
                    let timestamp = Self::parse_clip_timestamp(&filename);

                    clips.push(ClipInfo {
                        id: filename.clone(),
                        path: path.clone(),
                        filename,
                        size_bytes: metadata.len(),
                        timestamp,
                    });
                }
            }
        }

        // Sort newest first
        clips.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(clips)
    }

    /// Parses timestamp from clip filename
    fn parse_clip_timestamp(filename: &str) -> Option<chrono::DateTime<chrono::Local>> {
        // Format: clip_YYYYMMDD_HHMMSS.qsp
        if filename.starts_with("clip_") && filename.len() >= 20 {
            let date_part = &filename[5..13]; // YYYYMMDD
            let time_part = &filename[14..20]; // HHMMSS

            if let (Ok(year), Ok(month), Ok(day), Ok(hour), Ok(min), Ok(sec)) = (
                date_part[0..4].parse::<i32>(),
                date_part[4..6].parse::<u32>(),
                date_part[6..8].parse::<u32>(),
                time_part[0..2].parse::<u32>(),
                time_part[2..4].parse::<u32>(),
                time_part[4..6].parse::<u32>(),
            ) {
                use chrono::{Local, TimeZone};
                return Local.with_ymd_and_hms(year, month, day, hour, min, sec).single();
            }
        }
        None
    }

    /// Deletes a clip by ID
    pub fn delete_clip(&self, id: &str) -> Result<(), std::io::Error> {
        let path = self.clips_directory.join(id);
        if path.exists() {
            std::fs::remove_file(&path)?;
            log::info!("Deleted clip: {}", id);
        }
        Ok(())
    }
}

/// Information about a saved clip
#[derive(Debug, Clone, serde::Serialize)]
pub struct ClipInfo {
    pub id: String,
    #[serde(skip)]
    pub path: PathBuf,
    pub filename: String,
    pub size_bytes: u64,
    #[serde(serialize_with = "serialize_datetime_option")]
    pub timestamp: Option<chrono::DateTime<chrono::Local>>,
}

fn serialize_datetime_option<S>(
    dt: &Option<chrono::DateTime<chrono::Local>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match dt {
        Some(dt) => serializer.serialize_str(&dt.format("%Y-%m-%d %H:%M:%S").to_string()),
        None => serializer.serialize_none(),
    }
}

impl ClipInfo {
    /// Returns human-readable size
    pub fn size_human(&self) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;

        if self.size_bytes >= MB {
            format!("{:.1} MB", self.size_bytes as f64 / MB as f64)
        } else if self.size_bytes >= KB {
            format!("{:.1} KB", self.size_bytes as f64 / KB as f64)
        } else {
            format!("{} B", self.size_bytes)
        }
    }
}

