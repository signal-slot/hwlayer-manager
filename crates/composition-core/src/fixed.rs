//! Fixed-capacity containers used instead of heap collections.

use core::fmt;
use core::ops::Deref;

/// Error returned when a fixed-capacity container is full.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CapacityError;

/// A vector with inline storage for at most `N` elements.
#[derive(Clone, Copy)]
pub struct FixedVec<T: Copy + Default, const N: usize> {
    len: usize,
    items: [T; N],
}

impl<T: Copy + Default, const N: usize> FixedVec<T, N> {
    /// Creates an empty vector.
    pub fn new() -> Self {
        Self {
            len: 0,
            items: [T::default(); N],
        }
    }

    /// Creates a vector from a slice, failing if it does not fit.
    pub fn from_slice(items: &[T]) -> Result<Self, CapacityError> {
        let mut v = Self::new();
        for item in items {
            v.push(*item)?;
        }
        Ok(v)
    }

    /// Appends an element.
    pub fn push(&mut self, item: T) -> Result<(), CapacityError> {
        if self.len == N {
            return Err(CapacityError);
        }
        self.items[self.len] = item;
        self.len += 1;
        Ok(())
    }

    /// Removes and returns the last element.
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        Some(self.items[self.len])
    }

    /// Removes the element at `index`, replacing it with the last one.
    pub fn swap_remove(&mut self, index: usize) -> T {
        assert!(index < self.len);
        let item = self.items[index];
        self.len -= 1;
        self.items[index] = self.items[self.len];
        item
    }

    /// Removes all elements.
    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// Returns the elements as a slice.
    pub fn as_slice(&self) -> &[T] {
        &self.items[..self.len]
    }

    /// Returns the elements as a mutable slice.
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.items[..self.len]
    }

    /// Number of free slots.
    pub fn remaining(&self) -> usize {
        N - self.len
    }
}

impl<T: Copy + Default, const N: usize> Default for FixedVec<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Copy + Default, const N: usize> Deref for FixedVec<T, N> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T: Copy + Default + fmt::Debug, const N: usize> fmt::Debug for FixedVec<T, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.as_slice()).finish()
    }
}

impl<T: Copy + Default + PartialEq, const N: usize> PartialEq for FixedVec<T, N> {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<T: Copy + Default + Eq, const N: usize> Eq for FixedVec<T, N> {}

/// A FIFO queue with inline storage for at most `N` elements.
#[derive(Clone, Copy)]
pub struct FixedQueue<T: Copy + Default, const N: usize> {
    head: usize,
    len: usize,
    items: [T; N],
}

impl<T: Copy + Default, const N: usize> FixedQueue<T, N> {
    /// Creates an empty queue.
    pub fn new() -> Self {
        Self {
            head: 0,
            len: 0,
            items: [T::default(); N],
        }
    }

    /// Appends an element at the back.
    pub fn push(&mut self, item: T) -> Result<(), CapacityError> {
        if self.len == N {
            return Err(CapacityError);
        }
        self.items[(self.head + self.len) % N] = item;
        self.len += 1;
        Ok(())
    }

    /// Removes and returns the front element.
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        let item = self.items[self.head];
        self.head = (self.head + 1) % N;
        self.len -= 1;
        Some(item)
    }

    /// Number of queued elements.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if nothing is queued.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Number of free slots.
    pub fn remaining(&self) -> usize {
        N - self.len
    }
}

impl<T: Copy + Default + fmt::Debug, const N: usize> fmt::Debug for FixedQueue<T, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries((0..self.len).map(|i| &self.items[(self.head + i) % N]))
            .finish()
    }
}

impl<T: Copy + Default, const N: usize> Default for FixedQueue<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vec_respects_capacity() {
        let mut v: FixedVec<u8, 2> = FixedVec::new();
        v.push(1).unwrap();
        v.push(2).unwrap();
        assert_eq!(v.push(3), Err(CapacityError));
        assert_eq!(v.as_slice(), &[1, 2]);
        assert_eq!(v.swap_remove(0), 1);
        assert_eq!(v.as_slice(), &[2]);
    }

    #[test]
    fn queue_wraps_around() {
        let mut q: FixedQueue<u8, 3> = FixedQueue::new();
        for round in 0..5u8 {
            q.push(round).unwrap();
            q.push(round + 10).unwrap();
            assert_eq!(q.pop(), Some(round));
            assert_eq!(q.pop(), Some(round + 10));
        }
        assert!(q.is_empty());
        q.push(1).unwrap();
        q.push(2).unwrap();
        q.push(3).unwrap();
        assert_eq!(q.push(4), Err(CapacityError));
    }
}
