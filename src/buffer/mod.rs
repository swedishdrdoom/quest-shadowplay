//! # Frame Buffer Module
//!
//! This module provides the circular (ring) buffer for storing recent frames.
//!
//! ## Plain English Explanation
//!
//! Imagine a circular conveyor belt at a sushi restaurant with exactly 900 spots.
//! Every time a new piece of sushi (frame) comes out of the kitchen:
//! 1. It goes on the belt at the next empty spot
//! 2. If the belt is full, the oldest sushi gets removed first
//! 3. Customers (the save function) can grab all current sushi at any time
//!
//! This lets us always have the last 10 seconds of footage without using
//! infinite memory.

mod ring_buffer;

pub use ring_buffer::RingBuffer;

use crate::capture::CapturedFrame;
use parking_lot::RwLock;
use std::sync::Arc;

// ============================================
// SHARED FRAME BUFFER
// Thread-safe wrapper for our ring buffer
// ============================================

/// A thread-safe frame buffer that can be shared across threads
///
/// ## Plain English Explanation
///
/// Multiple parts of our app need to access the buffer:
/// - The capture system WRITES new frames
/// - The save system READS frames when saving
/// - The UI might READ to show buffer status
///
/// This wrapper uses a "lock" system (like a bathroom door lock) to make
/// sure only one part is writing at a time, but multiple can read together.
pub struct SharedFrameBuffer {
    /// The actual ring buffer, protected by a read-write lock
    /// RwLock = "Read-Write Lock"
    /// - Many readers can access simultaneously
    /// - Only one writer can access at a time
    /// - Writers wait for all readers to finish
    inner: RwLock<RingBuffer<CapturedFrame>>,
    
    /// How many frames we can store (10 seconds * FPS)
    capacity: usize,
}

impl SharedFrameBuffer {
    /// Creates a new shared buffer
    ///
    /// ## Parameters
    /// - `duration_seconds`: How many seconds of footage to keep (e.g., 10.0)
    /// - `fps`: Frames per second (e.g., 90 for Quest 3)
    ///
    /// ## Example Calculation
    /// ```text
    /// 10 seconds × 90 FPS = 900 frames capacity
    /// ```
    pub fn new(duration_seconds: f32, fps: u32) -> Self {
        let capacity = (duration_seconds * fps as f32).ceil() as usize;
        
        log::info!(
            "Creating frame buffer: {} seconds at {} FPS = {} frame capacity",
            duration_seconds,
            fps,
            capacity
        );

        Self {
            inner: RwLock::new(RingBuffer::new(capacity)),
            capacity,
        }
    }

    /// Adds a new frame to the buffer
    ///
    /// ## What Happens (Plain English)
    ///
    /// 1. We acquire the "write lock" (like locking a door)
    /// 2. Add the frame to the buffer
    /// 3. If buffer was full, the oldest frame is automatically gone
    /// 4. Release the lock (unlock the door)
    ///
    /// This is called ~90 times per second, so it needs to be FAST!
    pub fn push_frame(&self, frame: CapturedFrame) {
        let mut buffer = self.inner.write();
        buffer.push(frame);
    }

    /// Takes a snapshot of all current frames
    ///
    /// ## What Happens (Plain English)
    ///
    /// When you want to save a clip:
    /// 1. We get a "read lock" (we can look but not modify)
    /// 2. Copy all frames out of the buffer
    /// 3. Return them in chronological order (oldest first)
    ///
    /// The original buffer is NOT modified - recording continues!
    pub fn snapshot(&self) -> Vec<CapturedFrame> {
        let buffer = self.inner.read();
        buffer.get_all()
            .into_iter()
            .cloned()
            .collect()
    }

    /// Returns how full the buffer is (0.0 = empty, 1.0 = full)
    ///
    /// ## Plain English
    ///
    /// This tells you: "How close are we to having a full 10 seconds?"
    /// - Just started: 0.0 (0%)
    /// - After 5 seconds: 0.5 (50%)
    /// - After 10+ seconds: 1.0 (100%, we have full 10 seconds)
    pub fn fill_percentage(&self) -> f32 {
        let buffer = self.inner.read();
        buffer.len() as f32 / self.capacity as f32
    }

    /// Returns the number of frames currently stored
    pub fn frame_count(&self) -> usize {
        self.inner.read().len()
    }

    /// Returns the maximum number of frames we can store
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Clears all frames from the buffer
    ///
    /// ## When Would You Use This?
    ///
    /// - After saving, if you want a "fresh start"
    /// - When switching VR apps
    /// - To free memory in low-memory situations
    pub fn clear(&self) {
        let mut buffer = self.inner.write();
        buffer.clear();
    }
}

// ============================================
// TESTS
// ============================================

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a dummy frame for testing
    fn dummy_frame(id: u64) -> CapturedFrame {
        CapturedFrame {
            data: vec![0u8; 100],
            timestamp_ns: id,
            eye_index: 0,
            width: 100,
            height: 100,
        }
    }

    #[test]
    fn test_buffer_creation() {
        let buffer = SharedFrameBuffer::new(10.0, 90);
        assert_eq!(buffer.capacity(), 900);
        assert_eq!(buffer.frame_count(), 0);
    }

    #[test]
    fn test_push_and_snapshot() {
        let buffer = SharedFrameBuffer::new(1.0, 10); // 10 frame capacity

        // Add 5 frames
        for i in 0..5 {
            buffer.push_frame(dummy_frame(i));
        }

        let snapshot = buffer.snapshot();
        assert_eq!(snapshot.len(), 5);
        
        // Verify order (oldest first)
        assert_eq!(snapshot[0].timestamp_ns, 0);
        assert_eq!(snapshot[4].timestamp_ns, 4);
    }

    #[test]
    fn test_buffer_overflow() {
        let buffer = SharedFrameBuffer::new(1.0, 5); // 5 frame capacity

        // Add 8 frames (3 more than capacity)
        for i in 0..8 {
            buffer.push_frame(dummy_frame(i));
        }

        let snapshot = buffer.snapshot();
        
        // Should only have 5 frames
        assert_eq!(snapshot.len(), 5);
        
        // Oldest should be frame 3 (0, 1, 2 were overwritten)
        assert_eq!(snapshot[0].timestamp_ns, 3);
        assert_eq!(snapshot[4].timestamp_ns, 7);
    }

    #[test]
    fn test_fill_percentage() {
        let buffer = SharedFrameBuffer::new(1.0, 10);

        assert_eq!(buffer.fill_percentage(), 0.0);

        for i in 0..5 {
            buffer.push_frame(dummy_frame(i));
        }
        assert!((buffer.fill_percentage() - 0.5).abs() < 0.01);

        for i in 5..10 {
            buffer.push_frame(dummy_frame(i));
        }
        assert!((buffer.fill_percentage() - 1.0).abs() < 0.01);
    }
}

