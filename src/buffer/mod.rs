//! # Frame Buffer Module
//!
//! Circular (ring) buffer for storing recent VR frames.
//!
//! ## Plain English
//!
//! Like a circular sushi conveyor belt with exactly 900 spots.
//! When spot #901 arrives, spot #1 is removed to make room.
//! This keeps exactly 10 seconds of footage without growing forever.

mod ring_buffer;

pub use ring_buffer::RingBuffer;

use crate::capture::CapturedFrame;
use parking_lot::RwLock;

// ============================================
// SHARED FRAME BUFFER
// ============================================

/// Thread-safe frame buffer that can be shared across threads.
///
/// Multiple parts of the app access this:
/// - Capture system WRITES new frames
/// - Save system READS frames when saving
/// - UI might READ to show buffer status
pub struct SharedFrameBuffer {
    /// The ring buffer, protected by a read-write lock
    inner: RwLock<RingBuffer<CapturedFrame>>,

    /// Maximum frames this buffer can hold
    capacity: usize,
}

impl SharedFrameBuffer {
    /// Creates a new buffer for the given duration and frame rate.
    ///
    /// ## Example
    /// ```
    /// # use quest_shadowplay::buffer::SharedFrameBuffer;
    /// let buffer = SharedFrameBuffer::new(10.0, 90);
    /// // Creates buffer for 10 seconds at 90 FPS = 900 frames
    /// assert_eq!(buffer.capacity(), 900);
    /// ```
    pub fn new(duration_seconds: f32, fps: u32) -> Self {
        let capacity = (duration_seconds * fps as f32).ceil() as usize;

        log::info!(
            "Creating frame buffer: {:.1}s at {} FPS = {} frames",
            duration_seconds,
            fps,
            capacity
        );

        Self {
            inner: RwLock::new(RingBuffer::new(capacity)),
            capacity,
        }
    }

    /// Adds a new frame to the buffer.
    ///
    /// If the buffer is full, the oldest frame is automatically removed.
    /// This is called ~90 times per second, so it must be fast.
    pub fn push_frame(&self, frame: CapturedFrame) {
        self.inner.write().push(frame);
    }

    /// Takes a snapshot of all current frames.
    ///
    /// Returns frames in chronological order (oldest first).
    /// The original buffer is NOT modified - recording continues.
    pub fn snapshot(&self) -> Vec<CapturedFrame> {
        self.inner.read().get_all_cloned()
    }

    /// Returns how full the buffer is (0.0 = empty, 1.0 = full).
    pub fn fill_percentage(&self) -> f32 {
        let len = self.inner.read().len();
        len as f32 / self.capacity as f32
    }

    /// Returns the number of frames currently stored.
    pub fn frame_count(&self) -> usize {
        self.inner.read().len()
    }

    /// Returns the maximum number of frames.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Clears all frames from the buffer.
    pub fn clear(&self) {
        self.inner.write().clear();
    }
}

// ============================================
// TESTS
// ============================================

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_frame(_id: u64) -> CapturedFrame {
        CapturedFrame::new(vec![0u8; 100], 0, 100, 100)
    }

    #[test]
    fn test_buffer_creation() {
        let buffer = SharedFrameBuffer::new(10.0, 90);
        assert_eq!(buffer.capacity(), 900);
        assert_eq!(buffer.frame_count(), 0);
        assert_eq!(buffer.fill_percentage(), 0.0);
    }

    #[test]
    fn test_push_and_snapshot() {
        let buffer = SharedFrameBuffer::new(1.0, 10);

        for i in 0..5 {
            buffer.push_frame(dummy_frame(i));
        }

        assert_eq!(buffer.frame_count(), 5);
        let snapshot = buffer.snapshot();
        assert_eq!(snapshot.len(), 5);
    }

    #[test]
    fn test_buffer_overflow() {
        let buffer = SharedFrameBuffer::new(1.0, 5);

        // Add 8 frames to a 5-capacity buffer
        for i in 0..8 {
            buffer.push_frame(dummy_frame(i));
        }

        // Should only have 5 frames
        assert_eq!(buffer.frame_count(), 5);
    }

    #[test]
    fn test_clear() {
        let buffer = SharedFrameBuffer::new(1.0, 10);

        for i in 0..5 {
            buffer.push_frame(dummy_frame(i));
        }

        buffer.clear();
        assert_eq!(buffer.frame_count(), 0);
    }
}
