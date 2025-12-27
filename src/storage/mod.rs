//! # Storage Module
//!
//! This module handles saving video files to Quest 3's storage.
//!
//! ## Plain English Explanation
//!
//! After we've made a video, we need to save it somewhere. This module:
//! 1. Creates the output folder if it doesn't exist
//! 2. Generates unique filenames (with date and time)
//! 3. Writes files to storage
//! 4. Manages storage space (deletes old clips if needed)
//!
//! Files are saved to `/sdcard/QuestShadowplay/` which is visible when
//! you connect your Quest to a computer via USB.
//!
//! ```text
//!     Quest Storage
//!     └── sdcard/
//!         └── QuestShadowplay/
//!             ├── clip_20241227_143052.mp4
//!             ├── clip_20241227_144521.mp4
//!             └── clip_20241227_151033.mp4
//! ```

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};

use crate::error::{ShadowplayError, StorageErrorKind};

// ============================================
// STORAGE MANAGER
// Main storage functionality
// ============================================

/// Manages file storage for saved clips
///
/// ## Plain English
///
/// This is like a librarian for your video clips:
/// - Creates and organizes the folder structure
/// - Names files with dates and times
/// - Keeps track of how much space is used
/// - Can delete old clips to make room for new ones
pub struct StorageManager {
    /// Root directory for saving clips
    output_directory: PathBuf,
    
    /// Maximum storage to use in bytes (0 = unlimited)
    max_storage_bytes: u64,
    
    /// Whether to auto-delete old clips when storage is full
    auto_cleanup: bool,
}

impl StorageManager {
    /// Creates a new storage manager
    ///
    /// ## Parameters
    /// - `output_directory`: Where to save clips (e.g., "/sdcard/QuestShadowplay/")
    ///
    /// ## What Happens
    /// 1. Validates the directory path
    /// 2. Creates the directory if it doesn't exist
    /// 3. Returns a ready-to-use storage manager
    pub fn new(output_directory: &str) -> Result<Self, ShadowplayError> {
        let path = PathBuf::from(output_directory);

        // Create directory if needed
        if !path.exists() {
            log::info!("Creating output directory: {:?}", path);
            fs::create_dir_all(&path).map_err(|e| {
                ShadowplayError::Storage(StorageErrorKind::DirectoryCreationFailed(
                    path.display().to_string()
                ))
            })?;
        }

        // Verify it's a directory
        if !path.is_dir() {
            return Err(ShadowplayError::Storage(
                StorageErrorKind::DirectoryCreationFailed(
                    format!("{} exists but is not a directory", path.display())
                )
            ));
        }

        Ok(Self {
            output_directory: path,
            max_storage_bytes: 0, // Unlimited by default
            auto_cleanup: true,
        })
    }

    /// Generates a unique filename for a new clip
    ///
    /// ## Format
    /// `clip_YYYYMMDD_HHMMSS.mp4`
    ///
    /// ## Example
    /// If you save a clip on December 27, 2024 at 2:30:52 PM:
    /// `clip_20241227_143052.mp4`
    ///
    /// ## Plain English
    /// "Give me a filename for a new clip, based on the current date and time"
    pub fn generate_filename(output_directory: &str) -> String {
        let now = Local::now();
        let filename = format!(
            "clip_{}.mp4",
            now.format("%Y%m%d_%H%M%S")
        );
        
        let path = PathBuf::from(output_directory).join(&filename);
        path.to_string_lossy().to_string()
    }

    /// Gets the full path for a new clip
    pub fn get_new_clip_path(&self) -> String {
        Self::generate_filename(self.output_directory.to_str().unwrap_or(""))
    }

    /// Returns all saved clips, sorted by modification time (newest first)
    ///
    /// ## Plain English
    /// "Show me all the clips I've saved, with the newest ones first"
    pub fn list_clips(&self) -> Result<Vec<ClipInfo>, ShadowplayError> {
        let mut clips = Vec::new();

        for entry in fs::read_dir(&self.output_directory)? {
            let entry = entry?;
            let path = entry.path();

            // Only include MP4 files
            if path.extension().map(|e| e == "mp4").unwrap_or(false) {
                if let Ok(metadata) = entry.metadata() {
                    let modified = metadata.modified().ok();
                    let size = metadata.len();
                    
                    clips.push(ClipInfo {
                        path: path.clone(),
                        filename: path.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string(),
                        size_bytes: size,
                        modified_time: modified.map(|t| DateTime::from(t)),
                    });
                }
            }
        }

        // Sort by modification time (newest first)
        clips.sort_by(|a, b| {
            b.modified_time.cmp(&a.modified_time)
        });

        Ok(clips)
    }

    /// Returns total size of all saved clips in bytes
    ///
    /// ## Plain English
    /// "How much storage space are my clips using?"
    pub fn total_storage_used(&self) -> Result<u64, ShadowplayError> {
        let clips = self.list_clips()?;
        Ok(clips.iter().map(|c| c.size_bytes).sum())
    }

    /// Returns the number of saved clips
    pub fn clip_count(&self) -> Result<usize, ShadowplayError> {
        Ok(self.list_clips()?.len())
    }

    /// Deletes a clip by path
    ///
    /// ## Plain English
    /// "Delete this specific clip"
    pub fn delete_clip(&self, path: &Path) -> Result<(), ShadowplayError> {
        if !path.starts_with(&self.output_directory) {
            return Err(ShadowplayError::Storage(
                StorageErrorKind::PermissionDenied
            ));
        }

        fs::remove_file(path)?;
        log::info!("Deleted clip: {:?}", path);
        Ok(())
    }

    /// Deletes the oldest clips until storage is below the limit
    ///
    /// ## Parameters
    /// - `target_bytes`: Target maximum storage usage
    ///
    /// ## Returns
    /// Number of clips deleted
    ///
    /// ## Plain English
    /// "Delete old clips until we're using less than X bytes"
    pub fn cleanup_to_limit(&self, target_bytes: u64) -> Result<usize, ShadowplayError> {
        let mut deleted = 0;
        
        loop {
            let used = self.total_storage_used()?;
            if used <= target_bytes {
                break;
            }

            // Get oldest clip
            let clips = self.list_clips()?;
            if let Some(oldest) = clips.last() {
                log::info!("Deleting old clip to free space: {}", oldest.filename);
                self.delete_clip(&oldest.path)?;
                deleted += 1;
            } else {
                break; // No more clips to delete
            }
        }

        Ok(deleted)
    }

    /// Checks if there's enough space for a new clip
    ///
    /// ## Parameters
    /// - `estimated_size`: Expected size of the new clip in bytes
    ///
    /// ## Returns
    /// `true` if there's enough space, `false` otherwise
    pub fn has_space_for(&self, estimated_size: u64) -> Result<bool, ShadowplayError> {
        // Check Quest storage
        let available = self.available_storage()?;
        
        // Need at least the estimated size plus some buffer (10%)
        let required = estimated_size + (estimated_size / 10);
        
        Ok(available >= required)
    }

    /// Returns available storage space in bytes
    ///
    /// ## Plain English
    /// "How much free space is on the Quest?"
    pub fn available_storage(&self) -> Result<u64, ShadowplayError> {
        // In real implementation, we'd use statvfs or similar
        // For now, return a placeholder
        
        // This would be replaced with actual disk space check:
        // let stat = nix::sys::statvfs::statvfs(&self.output_directory)?;
        // Ok(stat.blocks_available() * stat.block_size())
        
        Ok(10 * 1024 * 1024 * 1024) // Placeholder: 10 GB available
    }

    /// Sets maximum storage to use (0 for unlimited)
    pub fn set_max_storage(&mut self, max_bytes: u64) {
        self.max_storage_bytes = max_bytes;
    }

    /// Sets whether to auto-delete old clips when storage is full
    pub fn set_auto_cleanup(&mut self, enabled: bool) {
        self.auto_cleanup = enabled;
    }

    /// Returns the output directory path
    pub fn output_directory(&self) -> &Path {
        &self.output_directory
    }
}

// ============================================
// CLIP INFO
// Information about a saved clip
// ============================================

/// Information about a saved video clip
///
/// ## Plain English
///
/// This is like a library card for a clip:
/// - Where it's stored
/// - Its name
/// - How big it is
/// - When it was saved
#[derive(Debug, Clone)]
pub struct ClipInfo {
    /// Full path to the clip file
    pub path: PathBuf,
    
    /// Just the filename (e.g., "clip_20241227_143052.mp4")
    pub filename: String,
    
    /// File size in bytes
    pub size_bytes: u64,
    
    /// When the clip was last modified (usually when it was saved)
    pub modified_time: Option<DateTime<Local>>,
}

impl ClipInfo {
    /// Returns the file size in a human-readable format
    ///
    /// ## Examples
    /// - 1,234 bytes → "1.2 KB"
    /// - 5,242,880 bytes → "5.0 MB"
    /// - 1,073,741,824 bytes → "1.0 GB"
    pub fn size_human(&self) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;

        if self.size_bytes >= GB {
            format!("{:.1} GB", self.size_bytes as f64 / GB as f64)
        } else if self.size_bytes >= MB {
            format!("{:.1} MB", self.size_bytes as f64 / MB as f64)
        } else if self.size_bytes >= KB {
            format!("{:.1} KB", self.size_bytes as f64 / KB as f64)
        } else {
            format!("{} bytes", self.size_bytes)
        }
    }

    /// Returns the age of the clip (how long ago it was saved)
    pub fn age(&self) -> Option<chrono::Duration> {
        self.modified_time.map(|t| Local::now() - t)
    }

    /// Returns a human-readable description of when the clip was saved
    ///
    /// ## Examples
    /// - "just now"
    /// - "5 minutes ago"
    /// - "2 hours ago"
    /// - "yesterday"
    pub fn age_human(&self) -> String {
        match self.age() {
            Some(age) => {
                let minutes = age.num_minutes();
                let hours = age.num_hours();
                let days = age.num_days();

                if minutes < 1 {
                    "just now".to_string()
                } else if minutes < 60 {
                    format!("{} minute{} ago", minutes, if minutes == 1 { "" } else { "s" })
                } else if hours < 24 {
                    format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" })
                } else if days == 1 {
                    "yesterday".to_string()
                } else {
                    format!("{} days ago", days)
                }
            }
            None => "unknown".to_string(),
        }
    }
}

// ============================================
// STORAGE UTILITIES
// ============================================

/// Checks if the Quest has storage permission
///
/// ## Plain English
/// 
/// Android requires apps to ask for permission before writing files.
/// This checks if we have that permission.
pub fn check_storage_permission() -> bool {
    // In real implementation, we'd check Android permissions
    // For now, assume we have permission
    true
}

/// Requests storage permission from the user
///
/// ## Plain English
///
/// Shows a dialog asking the user to allow file access.
/// They need to tap "Allow" for us to save clips.
pub fn request_storage_permission() -> Result<bool, ShadowplayError> {
    // In real implementation, we'd use Android's permission API
    log::info!("Requesting storage permission...");
    Ok(true) // Placeholder: assume granted
}

// ============================================
// TESTS
// ============================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_filename_generation() {
        let filename = StorageManager::generate_filename("/test/");
        
        // Should contain "clip_" prefix
        assert!(filename.contains("clip_"));
        
        // Should have .mp4 extension
        assert!(filename.ends_with(".mp4"));
        
        // Should have date pattern
        assert!(filename.contains("_202")); // 2020s
    }

    #[test]
    fn test_clip_info_size_human() {
        let clip = ClipInfo {
            path: PathBuf::from("/test.mp4"),
            filename: "test.mp4".to_string(),
            size_bytes: 5 * 1024 * 1024, // 5 MB
            modified_time: None,
        };

        let size = clip.size_human();
        assert!(size.contains("5.0 MB") || size.contains("5 MB"));
    }

    #[test]
    fn test_clip_info_size_bytes() {
        let clip = ClipInfo {
            path: PathBuf::from("/test.mp4"),
            filename: "test.mp4".to_string(),
            size_bytes: 500,
            modified_time: None,
        };

        assert!(clip.size_human().contains("bytes"));
    }

    #[test]
    fn test_storage_manager_creation() {
        let temp_dir = env::temp_dir().join("quest_shadowplay_test");
        let _ = fs::remove_dir_all(&temp_dir); // Clean up from previous runs
        
        let manager = StorageManager::new(temp_dir.to_str().unwrap());
        assert!(manager.is_ok());
        
        // Directory should exist now
        assert!(temp_dir.exists());
        
        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }
}

