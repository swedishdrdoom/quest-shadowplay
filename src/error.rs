//! # Error Types Module
//!
//! This module defines all the error types used throughout Quest Shadowplay.
//!
//! ## Plain English Explanation
//!
//! When things go wrong (and they will), we need a way to describe WHAT
//! went wrong. These error types are like labels on problem reports:
//!
//! - "CaptureError: The camera broke"
//! - "StorageError: The disk is full"
//! - "EncoderError: The video compressor failed"
//!
//! Having specific error types helps us:
//! 1. Know exactly what went wrong
//! 2. Decide how to recover (or whether we can)
//! 3. Show helpful messages to the user

use std::fmt;
use std::io;

use crate::capture::CompressionError;
use crate::config::ConfigError;

// ============================================
// MAIN APPLICATION ERROR
// ============================================

/// The main error type for Quest Shadowplay
///
/// ## Plain English
///
/// This is the "parent" error that can contain any type of error
/// from any part of the application. It's like a filing cabinet
/// with a folder for each department's problems.
#[derive(Debug)]
pub enum ShadowplayError {
    /// Something went wrong with frame capture
    ///
    /// ## Examples
    /// - Couldn't access the GPU texture
    /// - OpenXR layer failed to load
    Capture(CaptureErrorKind),
    
    /// Something went wrong with video encoding
    ///
    /// ## Examples
    /// - Hardware encoder not available
    /// - Invalid frame data
    Encoder(EncoderErrorKind),
    
    /// Something went wrong with storage
    ///
    /// ## Examples
    /// - Disk full
    /// - Permission denied
    /// - File already exists
    Storage(StorageErrorKind),
    
    /// Something went wrong with configuration
    ///
    /// ## Examples
    /// - Invalid setting value
    /// - Config file corrupted
    Config(ConfigError),
    
    /// OpenXR or VR system error
    ///
    /// ## Examples
    /// - OpenXR runtime not installed
    /// - Session creation failed
    OpenXR(OpenXRErrorKind),
    
    /// Android system error
    ///
    /// ## Examples
    /// - Permission not granted
    /// - JNI call failed
    Android(AndroidErrorKind),
    
    /// Generic I/O error
    Io(io::Error),
    
    /// Something unexpected happened
    Internal(String),
}

// Allow converting from std::io::Error to our error type
impl From<io::Error> for ShadowplayError {
    fn from(err: io::Error) -> Self {
        ShadowplayError::Io(err)
    }
}

impl fmt::Display for ShadowplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Capture(e) => write!(f, "Capture error: {}", e),
            Self::Encoder(e) => write!(f, "Encoder error: {}", e),
            Self::Storage(e) => write!(f, "Storage error: {}", e),
            Self::Config(e) => write!(f, "Configuration error: {}", e),
            Self::OpenXR(e) => write!(f, "OpenXR error: {}", e),
            Self::Android(e) => write!(f, "Android error: {}", e),
            Self::Io(e) => write!(f, "I/O error: {}", e),
            Self::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for ShadowplayError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

// ============================================
// CAPTURE ERRORS
// ============================================

/// Errors that can occur during frame capture
///
/// ## Plain English
///
/// These are problems with getting pictures from the VR headset.
/// Like a camera that won't work.
#[derive(Debug)]
pub enum CaptureErrorKind {
    /// OpenXR layer couldn't be loaded
    ///
    /// ## What This Means
    /// Our "spy" couldn't be inserted between the game and OpenXR.
    LayerLoadFailed(String),
    
    /// GPU texture couldn't be accessed
    ///
    /// ## What This Means
    /// The image is locked in GPU memory and we can't reach it.
    TextureAccessDenied,
    
    /// Frame compression failed
    ///
    /// ## What This Means
    /// We got the image but couldn't shrink it down.
    CompressionFailed(String),
    
    /// Capture is disabled
    ///
    /// ## What This Means
    /// Someone told the capture system to stop, so we can't capture.
    CaptureDisabled,
    
    /// Buffer is full and dropping frames
    ///
    /// ## What This Means
    /// We're capturing faster than we can process.
    BufferOverflow,
}

impl fmt::Display for CaptureErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LayerLoadFailed(reason) => {
                write!(f, "Failed to load OpenXR layer: {}", reason)
            }
            Self::TextureAccessDenied => {
                write!(f, "Cannot access GPU texture for frame capture")
            }
            Self::CompressionFailed(reason) => {
                write!(f, "Frame compression failed: {}", reason)
            }
            Self::CaptureDisabled => {
                write!(f, "Frame capture is currently disabled")
            }
            Self::BufferOverflow => {
                write!(f, "Frame buffer overflow - capture too fast")
            }
        }
    }
}

// ============================================
// ENCODER ERRORS
// ============================================

/// Errors that can occur during video encoding
///
/// ## Plain English
///
/// These are problems with turning captured frames into a video file.
/// Like a video editor that crashes.
#[derive(Debug)]
pub enum EncoderErrorKind {
    /// No hardware encoder available
    ///
    /// ## What This Means
    /// Quest 3 should always have a hardware encoder, but if it
    /// doesn't respond, we can't make videos efficiently.
    HardwareEncoderUnavailable,
    
    /// Encoder initialization failed
    ///
    /// ## What This Means
    /// The encoder exists but won't start up properly.
    InitializationFailed(String),
    
    /// Frame encoding failed
    ///
    /// ## What This Means
    /// One frame couldn't be added to the video.
    FrameEncodeFailed(String),
    
    /// Invalid frame data
    ///
    /// ## What This Means
    /// The frame we tried to encode was corrupted or wrong format.
    InvalidFrameData,
    
    /// Encoder ran out of buffer space
    OutputBufferFull,
    
    /// Finalization failed
    ///
    /// ## What This Means
    /// Everything encoded fine, but we couldn't properly close the video.
    FinalizationFailed(String),
}

impl fmt::Display for EncoderErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HardwareEncoderUnavailable => {
                write!(f, "Hardware video encoder not available")
            }
            Self::InitializationFailed(reason) => {
                write!(f, "Encoder initialization failed: {}", reason)
            }
            Self::FrameEncodeFailed(reason) => {
                write!(f, "Failed to encode frame: {}", reason)
            }
            Self::InvalidFrameData => {
                write!(f, "Invalid frame data for encoding")
            }
            Self::OutputBufferFull => {
                write!(f, "Encoder output buffer is full")
            }
            Self::FinalizationFailed(reason) => {
                write!(f, "Failed to finalize video: {}", reason)
            }
        }
    }
}

// ============================================
// STORAGE ERRORS
// ============================================

/// Errors that can occur during file storage
///
/// ## Plain English
///
/// These are problems with saving files to the Quest's storage.
/// Like trying to save a file but the USB drive is full.
#[derive(Debug)]
pub enum StorageErrorKind {
    /// Not enough disk space
    ///
    /// ## What This Means
    /// The Quest's internal storage is full. Delete some apps or videos.
    DiskFull,
    
    /// Permission denied
    ///
    /// ## What This Means
    /// The app doesn't have permission to write to storage.
    /// User may need to grant storage permission.
    PermissionDenied,
    
    /// Directory creation failed
    ///
    /// ## What This Means
    /// Couldn't create the folder for saving clips.
    DirectoryCreationFailed(String),
    
    /// File write failed
    ///
    /// ## What This Means
    /// Something went wrong while writing the video file.
    WriteFailed(String),
    
    /// File already exists
    ///
    /// ## What This Means
    /// Tried to save with a filename that already exists.
    FileExists(String),
    
    /// Storage not mounted
    ///
    /// ## What This Means
    /// The storage device isn't accessible (very rare on Quest).
    StorageNotMounted,
}

impl fmt::Display for StorageErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DiskFull => {
                write!(f, "Not enough disk space to save clip")
            }
            Self::PermissionDenied => {
                write!(f, "Storage permission denied")
            }
            Self::DirectoryCreationFailed(path) => {
                write!(f, "Failed to create directory: {}", path)
            }
            Self::WriteFailed(reason) => {
                write!(f, "File write failed: {}", reason)
            }
            Self::FileExists(path) => {
                write!(f, "File already exists: {}", path)
            }
            Self::StorageNotMounted => {
                write!(f, "Storage is not mounted")
            }
        }
    }
}

// ============================================
// OPENXR ERRORS
// ============================================

/// Errors related to OpenXR / VR system
///
/// ## Plain English
///
/// These are problems with the VR system itself.
/// Like if the VR headset's software has issues.
#[derive(Debug)]
pub enum OpenXRErrorKind {
    /// OpenXR runtime not found
    ///
    /// ## What This Means
    /// The Quest's OpenXR system isn't responding.
    RuntimeNotFound,
    
    /// Session creation failed
    SessionCreationFailed(String),
    
    /// Function not available
    FunctionNotAvailable(String),
    
    /// Instance creation failed
    InstanceCreationFailed(String),
    
    /// Generic OpenXR error with code
    RuntimeError { code: i32, message: String },
}

impl fmt::Display for OpenXRErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RuntimeNotFound => {
                write!(f, "OpenXR runtime not found")
            }
            Self::SessionCreationFailed(reason) => {
                write!(f, "Failed to create OpenXR session: {}", reason)
            }
            Self::FunctionNotAvailable(name) => {
                write!(f, "OpenXR function not available: {}", name)
            }
            Self::InstanceCreationFailed(reason) => {
                write!(f, "Failed to create OpenXR instance: {}", reason)
            }
            Self::RuntimeError { code, message } => {
                write!(f, "OpenXR error (code {}): {}", code, message)
            }
        }
    }
}

// ============================================
// ANDROID ERRORS
// ============================================

/// Errors related to Android system
///
/// ## Plain English
///
/// These are problems with the Android operating system
/// that Quest runs on.
#[derive(Debug)]
pub enum AndroidErrorKind {
    /// JNI (Java Native Interface) error
    ///
    /// ## What This Means
    /// Communication between our Rust code and Android's Java system failed.
    JniError(String),
    
    /// Required permission not granted
    PermissionNotGranted(String),
    
    /// Android service not available
    ServiceUnavailable(String),
    
    /// Activity lifecycle error
    ActivityError(String),
}

impl fmt::Display for AndroidErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::JniError(reason) => {
                write!(f, "JNI error: {}", reason)
            }
            Self::PermissionNotGranted(permission) => {
                write!(f, "Permission not granted: {}", permission)
            }
            Self::ServiceUnavailable(service) => {
                write!(f, "Android service unavailable: {}", service)
            }
            Self::ActivityError(reason) => {
                write!(f, "Activity error: {}", reason)
            }
        }
    }
}

// ============================================
// RESULT TYPE ALIAS
// ============================================

/// A Result type that uses ShadowplayError
///
/// ## Plain English
///
/// This is a shorthand. Instead of writing:
/// ```
/// fn do_something() -> Result<Value, ShadowplayError>
/// ```
/// We can write:
/// ```
/// fn do_something() -> ShadowplayResult<Value>
/// ```
pub type ShadowplayResult<T> = Result<T, ShadowplayError>;

// ============================================
// TESTS
// ============================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = ShadowplayError::Capture(CaptureErrorKind::CaptureDisabled);
        let message = format!("{}", err);
        assert!(message.contains("Capture"));
        assert!(message.contains("disabled"));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let app_err: ShadowplayError = io_err.into();
        
        match app_err {
            ShadowplayError::Io(_) => {} // Expected
            _ => panic!("Expected Io error variant"),
        }
    }
}

