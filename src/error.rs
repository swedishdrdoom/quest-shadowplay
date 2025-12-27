//! # Error Types
//!
//! All error types used throughout Quest Shadowplay.
//!
//! ## Plain English
//!
//! When things go wrong, we need to describe WHAT went wrong.
//! These error types are like labels on problem reports.

use std::fmt;
use std::io;

use crate::config::ConfigError;

// ============================================
// MAIN ERROR TYPE
// ============================================

/// The main error type for Quest Shadowplay.
#[derive(Debug)]
pub enum ShadowplayError {
    /// Configuration error
    Config(ConfigError),

    /// Frame capture error
    Capture(String),

    /// Video encoding error
    Encoder(String),

    /// Storage/file system error
    Storage(String),

    /// I/O error
    Io(io::Error),

    /// Internal error
    Internal(String),
}

impl fmt::Display for ShadowplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(e) => write!(f, "Configuration error: {}", e),
            Self::Capture(msg) => write!(f, "Capture error: {}", msg),
            Self::Encoder(msg) => write!(f, "Encoder error: {}", msg),
            Self::Storage(msg) => write!(f, "Storage error: {}", msg),
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

impl From<io::Error> for ShadowplayError {
    fn from(err: io::Error) -> Self {
        ShadowplayError::Io(err)
    }
}

impl From<ConfigError> for ShadowplayError {
    fn from(err: ConfigError) -> Self {
        ShadowplayError::Config(err)
    }
}

// ============================================
// RESULT TYPE ALIAS
// ============================================

/// A Result type using ShadowplayError.
pub type ShadowplayResult<T> = Result<T, ShadowplayError>;

// ============================================
// TESTS
// ============================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = ShadowplayError::Internal("test error".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("Internal error"));
        assert!(msg.contains("test error"));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "not found");
        let err: ShadowplayError = io_err.into();
        match err {
            ShadowplayError::Io(_) => {}
            _ => panic!("Expected Io variant"),
        }
    }
}
