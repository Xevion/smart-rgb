use std::collections::HashSet;

use glam::U16Vec2;

/// Lightweight change tracking buffer for tile mutations.
///
/// Stores only the indices of changed tiles, using a HashSet to automatically
/// deduplicate when the same tile changes multiple times per turn. This enables
/// efficient delta updates for GPU rendering and network synchronization.
///
/// # Design
/// - Records tile index changes as they occur
/// - Automatically deduplicates tile indices
/// - O(1) average insert, O(changes) iteration
/// - Optional: can be cleared/ignored when tracking not needed
#[derive(Debug, Clone)]
pub struct ChangeBuffer {
    changed_indices: HashSet<U16Vec2>,
}

impl ChangeBuffer {
    /// Creates a new empty ChangeBuffer.
    pub fn new() -> Self {
        Self { changed_indices: HashSet::new() }
    }

    /// Creates a new ChangeBuffer with pre-allocated capacity.
    ///
    /// Use this when you know the approximate number of changes to avoid reallocations.
    pub fn with_capacity(capacity: usize) -> Self {
        Self { changed_indices: HashSet::with_capacity(capacity) }
    }

    /// Records a tile index as changed.
    ///
    /// Automatically deduplicates - pushing the same index multiple times
    /// only records it once. This is O(1) average case.
    #[inline]
    pub fn push(&mut self, position: U16Vec2) {
        self.changed_indices.insert(position);
    }

    /// Returns an iterator over changed indices without consuming them.
    ///
    /// Use this when you need to read changes without clearing the buffer.
    /// The buffer will still contain all changes after iteration.
    pub fn iter(&self) -> impl Iterator<Item = U16Vec2> + '_ {
        self.changed_indices.iter().copied()
    }

    /// Drains all changed indices, returning an iterator and clearing the buffer.
    ///
    /// The buffer retains its capacity for reuse.
    pub fn drain(&mut self) -> impl Iterator<Item = U16Vec2> + '_ {
        self.changed_indices.drain()
    }

    /// Clears all tracked changes without returning them.
    ///
    /// The buffer retains its capacity for reuse.
    pub fn clear(&mut self) {
        self.changed_indices.clear();
    }

    /// Returns true if any changes have been recorded.
    #[inline]
    pub fn has_changes(&self) -> bool {
        !self.changed_indices.is_empty()
    }

    /// Returns the number of changes recorded.
    ///
    /// Note: This may include duplicate indices if the same tile was changed multiple times.
    #[inline]
    pub fn len(&self) -> usize {
        self.changed_indices.len()
    }

    /// Returns true if no changes have been recorded.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.changed_indices.is_empty()
    }

    /// Returns the current capacity of the internal buffer.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.changed_indices.capacity()
    }
}

impl Default for ChangeBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use assert2::assert;
    use glam::U16Vec2;

    use crate::game::world::changes::ChangeBuffer;

    #[test]
    fn test_len_empty() {
        let buffer = ChangeBuffer::new();
        assert!(buffer.len() == 0);
    }

    #[test]
    fn test_len_single_item() {
        let mut buffer = ChangeBuffer::new();
        buffer.push(U16Vec2::new(10, 20));
        assert!(buffer.len() == 1);
    }

    #[test]
    fn test_len_multiple_items() {
        let mut buffer = ChangeBuffer::new();
        buffer.push(U16Vec2::new(10, 20));
        buffer.push(U16Vec2::new(30, 40));
        buffer.push(U16Vec2::new(50, 60));
        assert!(buffer.len() == 3);
    }

    #[test]
    fn test_len_with_duplicates() {
        let mut buffer = ChangeBuffer::new();
        let pos = U16Vec2::new(10, 20);

        buffer.push(pos);
        buffer.push(pos);
        buffer.push(pos);

        // HashSet deduplicates, so len should be 1
        assert!(buffer.len() == 1);
    }

    #[test]
    fn test_len_after_drain() {
        let mut buffer = ChangeBuffer::new();
        buffer.push(U16Vec2::new(10, 20));
        buffer.push(U16Vec2::new(30, 40));

        // Drain all items
        let _drained: Vec<_> = buffer.drain().collect();

        assert!(buffer.len() == 0);
    }

    #[test]
    fn test_is_empty_new_buffer() {
        let buffer = ChangeBuffer::new();
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_is_empty_after_push() {
        let mut buffer = ChangeBuffer::new();
        buffer.push(U16Vec2::new(10, 20));
        assert!(!buffer.is_empty());
    }

    #[test]
    fn test_is_empty_after_clear() {
        let mut buffer = ChangeBuffer::new();
        buffer.push(U16Vec2::new(10, 20));
        buffer.push(U16Vec2::new(30, 40));

        buffer.clear();

        assert!(buffer.is_empty());
    }

    #[test]
    fn test_is_empty_after_drain() {
        let mut buffer = ChangeBuffer::new();
        buffer.push(U16Vec2::new(10, 20));

        let _drained: Vec<_> = buffer.drain().collect();

        assert!(buffer.is_empty());
    }

    #[test]
    fn test_with_capacity_allocates_space() {
        let buffer = ChangeBuffer::with_capacity(100);

        // Capacity should be at least what we requested
        assert!(buffer.capacity() >= 100, "Expected capacity >= 100, but got {}", buffer.capacity());
    }

    #[test]
    fn test_with_capacity_differs_from_new() {
        let buffer_new = ChangeBuffer::new();
        let buffer_with_cap = ChangeBuffer::with_capacity(100);

        // with_capacity should allocate more space than new
        assert!(buffer_with_cap.capacity() > buffer_new.capacity(), "with_capacity(100) should have larger capacity than new(), but got {} vs {}", buffer_with_cap.capacity(), buffer_new.capacity());
    }

    #[test]
    fn test_with_capacity_differs_from_default() {
        let buffer_default = ChangeBuffer::default();
        let buffer_with_cap = ChangeBuffer::with_capacity(100);

        // with_capacity should allocate more space than default
        assert!(buffer_with_cap.capacity() > buffer_default.capacity(), "with_capacity(100) should have larger capacity than default(), but got {} vs {}", buffer_with_cap.capacity(), buffer_default.capacity());
    }

    #[test]
    fn test_capacity_persists_after_clear() {
        let mut buffer = ChangeBuffer::with_capacity(100);
        let initial_capacity = buffer.capacity();

        buffer.push(U16Vec2::new(10, 20));
        buffer.clear();

        // Capacity should remain after clear
        assert!(buffer.capacity() == initial_capacity, "Capacity should persist after clear, but changed from {} to {}", initial_capacity, buffer.capacity());
    }

    #[test]
    fn test_capacity_persists_after_drain() {
        let mut buffer = ChangeBuffer::with_capacity(100);
        let initial_capacity = buffer.capacity();

        buffer.push(U16Vec2::new(10, 20));
        let _drained: Vec<_> = buffer.drain().collect();

        // Capacity should remain after drain
        assert!(buffer.capacity() == initial_capacity, "Capacity should persist after drain, but changed from {} to {}", initial_capacity, buffer.capacity());
    }

    #[test]
    fn test_len_is_empty_consistency() {
        let mut buffer = ChangeBuffer::new();

        // Empty: len == 0 and is_empty() == true
        assert!(buffer.len() == 0);
        assert!(buffer.is_empty());

        // Non-empty: len > 0 and is_empty() == false
        buffer.push(U16Vec2::new(10, 20));
        assert!(buffer.len() > 0);
        assert!(!buffer.is_empty());

        // Add more items
        buffer.push(U16Vec2::new(30, 40));
        assert!(buffer.len() == 2);
        assert!(!buffer.is_empty());
    }
}
