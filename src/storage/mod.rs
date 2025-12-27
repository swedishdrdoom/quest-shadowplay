//! # Storage Module
//!
//! Saves video files to Quest 3's storage.
//!
//! ## Plain English
//!
//! After making a video, we need to save it. This module:
//! 1. Creates the output folder
//! 2. Generates unique filenames with date/time
//! 3. Writes files to storage
//! 4. Manages storage space

use std::fs;
use std::path::{Path, PathBuf};

use chrono::Local;

use crate::error::{ShadowplayError, ShadowplayResult};

// ============================================
// STORAGE MANAGER
// ============================================

/// Manages file storage for saved clips.
pub struct StorageManager {
    /// Root directory for clips
    output_directory: PathBuf,
}

impl StorageManager {
    /// Creates a new storage manager.
    pub fn new(output_directory: &str) -> ShadowplayResult<Self> {
        let path = PathBuf::from(output_directory);

        // Create directory if needed
        if !path.exists() {
            log::info!("Creating output directory: {:?}", path);
            fs::create_dir_all(&path)?;
        }

        Ok(Self {
            output_directory: path,
        })
    }

    /// Generates a unique filename for a new clip.
    ///
    /// Format: `clip_YYYYMMDD_HHMMSS.qsp`
    pub fn generate_filename(output_directory: &str) -> String {
        let now = Local::now();
        let filename = format!("clip_{}.qsp", now.format("%Y%m%d_%H%M%S"));
        let path = PathBuf::from(output_directory).join(&filename);
        path.to_string_lossy().to_string()
    }

    /// Returns all saved clips, newest first.
    pub fn list_clips(&self) -> ShadowplayResult<Vec<ClipInfo>> {
        let mut clips = Vec::new();

        for entry in fs::read_dir(&self.output_directory)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map(|e| e == "qsp").unwrap_or(false) {
                if let Ok(metadata) = entry.metadata() {
                    clips.push(ClipInfo {
                        path: path.clone(),
                        filename: path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string(),
                        size_bytes: metadata.len(),
                        modified: metadata.modified().ok(),
                    });
                }
            }
        }

        // Sort newest first
        clips.sort_by(|a, b| b.modified.cmp(&a.modified));
        Ok(clips)
    }

    /// Returns total storage used in bytes.
    pub fn total_storage_used(&self) -> ShadowplayResult<u64> {
        Ok(self.list_clips()?.iter().map(|c| c.size_bytes).sum())
    }

    /// Deletes a clip.
    pub fn delete_clip(&self, path: &Path) -> ShadowplayResult<()> {
        if !path.starts_with(&self.output_directory) {
            return Err(ShadowplayError::Storage("Path outside directory".to_string()));
        }
        fs::remove_file(path)?;
        log::info!("Deleted clip: {:?}", path);
        Ok(())
    }

    /// Returns the output directory.
    pub fn output_directory(&self) -> &Path {
        &self.output_directory
    }
}

/// Information about a saved clip.
#[derive(Debug, Clone)]
pub struct ClipInfo {
    pub path: PathBuf,
    pub filename: String,
    pub size_bytes: u64,
    pub modified: Option<std::time::SystemTime>,
}

impl ClipInfo {
    /// Returns human-readable size.
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
}

// ============================================
// UTILITY FUNCTIONS
// ============================================

/// Ensures a directory exists.
pub fn ensure_directory(path: &str) -> ShadowplayResult<()> {
    let path = Path::new(path);
    if !path.exists() {
        fs::create_dir_all(path)?;
    }
    Ok(())
}

/// Returns available storage in bytes (placeholder).
pub fn available_storage() -> u64 {
    // In real implementation, use statvfs or Android API
    10 * 1024 * 1024 * 1024 // 10 GB placeholder
}

/// Checks if storage permission is granted (placeholder).
pub fn check_storage_permission() -> bool {
    true
}

// ============================================
// TESTS
// ============================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_filename_generation() {
        let filename = StorageManager::generate_filename("/test/");
        assert!(filename.contains("clip_"));
        assert!(filename.ends_with(".qsp"));
    }

    #[test]
    fn test_storage_manager_creation() {
        let dir = tempdir().unwrap();
        let manager = StorageManager::new(dir.path().to_str().unwrap());
        assert!(manager.is_ok());
    }

    #[test]
    fn test_clip_info_size_human() {
        let clip = ClipInfo {
            path: PathBuf::from("/test.qsp"),
            filename: "test.qsp".to_string(),
            size_bytes: 5 * 1024 * 1024,
            modified: None,
        };
        assert!(clip.size_human().contains("5"));
        assert!(clip.size_human().contains("MB"));
    }

    #[test]
    fn test_ensure_directory() {
        let dir = tempdir().unwrap();
        let subdir = dir.path().join("subdir");
        ensure_directory(subdir.to_str().unwrap()).unwrap();
        assert!(subdir.exists());
    }
}
