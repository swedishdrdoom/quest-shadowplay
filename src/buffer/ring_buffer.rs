//! # Ring Buffer Implementation
//!
//! A fixed-size circular buffer that overwrites old elements when full.
//!
//! ## Plain English
//!
//! Picture a circular track with numbered parking spots.
//! When all spots are full and a new car arrives,
//! the oldest car is towed away to make room.

use std::collections::VecDeque;

/// A fixed-capacity ring buffer.
///
/// ## Properties
/// - Fixed capacity (doesn't grow)
/// - O(1) push operation
/// - Automatically discards oldest when full
/// - Maintains insertion order
#[derive(Debug)]
pub struct RingBuffer<T> {
    /// The actual storage
    data: VecDeque<T>,

    /// Maximum number of items
    capacity: usize,
}

impl<T> RingBuffer<T> {
    /// Creates a new ring buffer with the given capacity.
    ///
    /// ## Example
    /// ```
    /// # use quest_shadowplay::buffer::RingBuffer;
    /// let buffer: RingBuffer<i32> = RingBuffer::new(100);
    /// assert_eq!(buffer.capacity(), 100);
    /// ```
    pub fn new(capacity: usize) -> Self {
        Self {
            data: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Adds an item to the buffer.
    ///
    /// If the buffer is full, the oldest item is removed first.
    pub fn push(&mut self, item: T) {
        if self.data.len() >= self.capacity {
            self.data.pop_front();
        }
        self.data.push_back(item);
    }

    /// Returns the number of items currently stored.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns true if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns true if the buffer is at capacity.
    pub fn is_full(&self) -> bool {
        self.data.len() >= self.capacity
    }

    /// Returns the maximum capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Clears all items from the buffer.
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Returns the oldest item without removing it.
    pub fn peek_oldest(&self) -> Option<&T> {
        self.data.front()
    }

    /// Returns the newest item without removing it.
    pub fn peek_newest(&self) -> Option<&T> {
        self.data.back()
    }

    /// Returns an iterator over all items (oldest to newest).
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.data.iter()
    }

    /// Returns all items as references (oldest first).
    pub fn get_all(&self) -> Vec<&T> {
        self.data.iter().collect()
    }
}

impl<T: Clone> RingBuffer<T> {
    /// Returns cloned copies of all items (oldest first).
    pub fn get_all_cloned(&self) -> Vec<T> {
        self.data.iter().cloned().collect()
    }
}

// ============================================
// TESTS
// ============================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_buffer() {
        let buffer: RingBuffer<i32> = RingBuffer::new(5);
        assert!(buffer.is_empty());
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.capacity(), 5);
    }

    #[test]
    fn test_push_single() {
        let mut buffer = RingBuffer::new(5);
        buffer.push(42);

        assert!(!buffer.is_empty());
        assert_eq!(buffer.len(), 1);
        assert_eq!(buffer.peek_newest(), Some(&42));
        assert_eq!(buffer.peek_oldest(), Some(&42));
    }

    #[test]
    fn test_push_multiple() {
        let mut buffer = RingBuffer::new(5);

        for i in 1..=3 {
            buffer.push(i);
        }

        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.peek_oldest(), Some(&1));
        assert_eq!(buffer.peek_newest(), Some(&3));

        let all: Vec<_> = buffer.get_all_cloned();
        assert_eq!(all, vec![1, 2, 3]);
    }

    #[test]
    fn test_overflow() {
        let mut buffer = RingBuffer::new(3);

        // Add 5 items to capacity-3 buffer
        for i in 1..=5 {
            buffer.push(i);
        }

        // Should only have 3, 4, 5
        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.peek_oldest(), Some(&3));
        assert_eq!(buffer.peek_newest(), Some(&5));

        let all: Vec<_> = buffer.get_all_cloned();
        assert_eq!(all, vec![3, 4, 5]);
    }

    #[test]
    fn test_clear() {
        let mut buffer = RingBuffer::new(5);

        for i in 1..=3 {
            buffer.push(i);
        }

        buffer.clear();
        assert!(buffer.is_empty());
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn test_is_full() {
        let mut buffer = RingBuffer::new(3);

        assert!(!buffer.is_full());
        buffer.push(1);
        buffer.push(2);
        assert!(!buffer.is_full());
        buffer.push(3);
        assert!(buffer.is_full());
        buffer.push(4);
        assert!(buffer.is_full());
    }

    #[test]
    fn test_iterator() {
        let mut buffer = RingBuffer::new(5);

        for i in 1..=3 {
            buffer.push(i);
        }

        let collected: Vec<_> = buffer.iter().copied().collect();
        assert_eq!(collected, vec![1, 2, 3]);
    }
}
