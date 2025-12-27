//! # Ring Buffer Implementation
//!
//! A fixed-size circular buffer that overwrites old elements when full.
//!
//! ## Plain English Explanation
//!
//! Picture a circular track with numbered parking spots (0, 1, 2, ... N-1).
//! A parking attendant (write_index) walks around the track:
//!
//! ```text
//! Empty buffer (capacity 5):
//!
//!        [0]
//!       /   \
//!    [4]     [1]
//!       \   /
//!     [3]-[2]
//!
//! After adding A, B, C (write_index at 3):
//!
//!        [A]
//!       /   \
//!    [ ]     [B]
//!       \   /
//!     [ ]-[C]
//!          ↑
//!       write here next
//!
//! After buffer is full + adding F (overwrites A):
//!
//!        [F] ← newest (just wrote here)
//!       /   \
//!    [E]     [B] ← oldest (will be overwritten next)
//!       \   /
//!     [D]-[C]
//! ```

use std::collections::VecDeque;

/// A fixed-capacity ring buffer
///
/// ## Type Parameter
/// - `T`: The type of items stored (for us, it's `CapturedFrame`)
///
/// ## Properties
/// - Fixed capacity (doesn't grow)
/// - O(1) push operation (constant time, always fast)
/// - Automatically discards oldest when full
/// - Maintains insertion order
#[derive(Debug)]
pub struct RingBuffer<T> {
    /// The actual storage for items
    /// VecDeque = "Vector Double-Ended Queue"
    /// It's efficient for adding/removing from both ends
    data: VecDeque<T>,
    
    /// Maximum number of items we can hold
    capacity: usize,
}

impl<T: Clone> RingBuffer<T> {
    /// Creates a new ring buffer with the given capacity
    ///
    /// ## Plain English
    ///
    /// "Build me a circular track with this many parking spots"
    ///
    /// ## Example
    /// ```
    /// let buffer: RingBuffer<i32> = RingBuffer::new(100);
    /// // Can now hold up to 100 integers
    /// ```
    pub fn new(capacity: usize) -> Self {
        Self {
            data: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Adds an item to the buffer
    ///
    /// ## Plain English
    ///
    /// "Here's a new item. Put it in the next spot. If we're full,
    /// throw away the oldest item first to make room."
    ///
    /// ## Behavior
    /// - If buffer is not full: item is simply added
    /// - If buffer is full: oldest item is removed, new item is added
    ///
    /// ## Performance
    /// - Time: O(1) - always the same speed regardless of buffer size
    /// - Memory: No new allocations (we pre-allocated in `new`)
    pub fn push(&mut self, item: T) {
        // If we're at capacity, remove the oldest item
        if self.data.len() >= self.capacity {
            self.data.pop_front(); // Remove from front (oldest)
        }
        
        // Add new item at the back (newest)
        self.data.push_back(item);
    }

    /// Returns all items in order (oldest first)
    ///
    /// ## Plain English
    ///
    /// "Give me a list of everything in the buffer, starting with
    /// the oldest item and ending with the newest."
    ///
    /// ## Returns
    /// A vector of references to all items, in chronological order
    pub fn get_all(&self) -> Vec<&T> {
        self.data.iter().collect()
    }

    /// Returns the number of items currently stored
    ///
    /// ## Plain English
    ///
    /// "How many parking spots are currently filled?"
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns true if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns true if the buffer is at capacity
    ///
    /// ## Plain English
    ///
    /// "Is every parking spot filled?"
    pub fn is_full(&self) -> bool {
        self.data.len() >= self.capacity
    }

    /// Returns the maximum capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Clears all items from the buffer
    ///
    /// ## Plain English
    ///
    /// "Empty all the parking spots"
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Returns the oldest item without removing it
    ///
    /// ## Plain English
    ///
    /// "Show me the oldest item, but don't remove it"
    pub fn peek_oldest(&self) -> Option<&T> {
        self.data.front()
    }

    /// Returns the newest item without removing it
    ///
    /// ## Plain English
    ///
    /// "Show me the most recently added item"
    pub fn peek_newest(&self) -> Option<&T> {
        self.data.back()
    }

    /// Returns an iterator over all items (oldest to newest)
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.data.iter()
    }
}

// ============================================
// TESTS
// ============================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_buffer_is_empty() {
        let buffer: RingBuffer<i32> = RingBuffer::new(5);
        assert!(buffer.is_empty());
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.capacity(), 5);
    }

    #[test]
    fn test_push_single_item() {
        let mut buffer = RingBuffer::new(5);
        buffer.push(42);
        
        assert!(!buffer.is_empty());
        assert_eq!(buffer.len(), 1);
        assert_eq!(buffer.peek_newest(), Some(&42));
        assert_eq!(buffer.peek_oldest(), Some(&42));
    }

    #[test]
    fn test_push_multiple_items() {
        let mut buffer = RingBuffer::new(5);
        
        for i in 1..=3 {
            buffer.push(i);
        }
        
        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.peek_oldest(), Some(&1));
        assert_eq!(buffer.peek_newest(), Some(&3));
        
        let all: Vec<_> = buffer.get_all().into_iter().copied().collect();
        assert_eq!(all, vec![1, 2, 3]);
    }

    #[test]
    fn test_overflow_removes_oldest() {
        let mut buffer = RingBuffer::new(3);
        
        // Add 5 items to a buffer of capacity 3
        for i in 1..=5 {
            buffer.push(i);
        }
        
        // Should only contain 3, 4, 5
        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.peek_oldest(), Some(&3));
        assert_eq!(buffer.peek_newest(), Some(&5));
        
        let all: Vec<_> = buffer.get_all().into_iter().copied().collect();
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
        
        // Still full after overflow
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

