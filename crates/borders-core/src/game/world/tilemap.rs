use glam::{U16Vec2, UVec2};
use std::ops::{Index, IndexMut};

/// A 2D grid-based map structure optimized for tile-based games.
///
/// Provides efficient access to tiles using 2D coordinates (U16Vec2) while maintaining
/// cache-friendly contiguous memory layout. Supports generic tile types that implement Copy.
///
/// Uses `u16` for dimensions, supporting maps up to 65,535x65,535 tiles.
///
/// # Type Parameters
/// * `T` - The tile value type. Must implement `Copy` for efficient access.
///
/// # Examples
/// ```
/// use glam::U16Vec2;
/// use borders_core::game::TileMap;
///
/// let mut map = TileMap::<u8>::new(U16Vec2::new(10, 10));
/// map[U16Vec2::new(5, 5)] = 42;
/// assert_eq!(map[U16Vec2::new(5, 5)], 42);
/// ```
#[derive(Clone, Debug)]
pub struct TileMap<T: Copy> {
    tiles: Box<[T]>,
    size: U16Vec2,
}

impl<T: Copy> TileMap<T> {
    /// Creates a new TileMap with the specified dimensions and default value.
    ///
    /// # Arguments
    /// * `size` - The size of the map (width, height) in tiles
    /// * `default` - The default value to initialize all tiles with
    pub fn with_default(size: U16Vec2, default: T) -> Self {
        let capacity = (size.x as usize) * (size.y as usize);
        let tiles = vec![default; capacity].into_boxed_slice();
        Self { tiles, size }
    }

    /// Creates a TileMap from an existing vector of tile data.
    ///
    /// # Arguments
    /// * `size` - The size of the map (width, height) in tiles
    /// * `data` - Vector containing tile data in row-major order
    ///
    /// # Panics
    /// Panics if `data.len() != size.x * size.y`
    pub fn from_vec(size: U16Vec2, data: Vec<T>) -> Self {
        assert_eq!(data.len(), (size.x as usize) * (size.y as usize), "Data length must match size.x * size.y");
        Self { tiles: data.into_boxed_slice(), size }
    }

    /// Converts the position to a flat array index.
    ///
    /// Accepts both U16Vec2 and UVec2 for backward compatibility.
    ///
    /// # Safety
    /// Debug builds will assert that the position is in bounds.
    /// Release builds skip the check for performance.
    #[inline]
    pub fn pos_to_index<P: Into<U16Vec2>>(&self, pos: P) -> u32 {
        let pos = pos.into();
        debug_assert!(pos.x < self.size.x && pos.y < self.size.y);
        (pos.y as u32) * (self.size.x as u32) + (pos.x as u32)
    }

    /// Converts a flat array index to a 2D position.
    #[inline]
    pub fn index_to_pos(&self, index: u32) -> U16Vec2 {
        debug_assert!(index < self.tiles.len() as u32);
        let width = self.size.x as u32;
        U16Vec2::new((index % width) as u16, (index / width) as u16)
    }

    /// Checks if a position is within the map bounds.
    ///
    /// Accepts both U16Vec2 and UVec2 for backward compatibility.
    #[inline]
    pub fn in_bounds<P: Into<U16Vec2>>(&self, pos: P) -> bool {
        let pos = pos.into();
        pos.x < self.size.x && pos.y < self.size.y
    }

    /// Gets the tile value at the specified position.
    ///
    /// Returns `None` if the position is out of bounds.
    pub fn get<P: Into<U16Vec2>>(&self, pos: P) -> Option<T> {
        let pos = pos.into();
        if self.in_bounds(pos) { Some(self.tiles[self.pos_to_index(pos) as usize]) } else { None }
    }

    /// Sets the tile value at the specified position.
    ///
    /// Returns `true` if the position was in bounds and the value was set,
    /// `false` otherwise.
    pub fn set<P: Into<U16Vec2>>(&mut self, pos: P, tile: T) -> bool {
        let pos = pos.into();
        if self.in_bounds(pos) {
            let idx = self.pos_to_index(pos) as usize;
            self.tiles[idx] = tile;
            true
        } else {
            false
        }
    }

    /// Returns the size of the map as U16Vec2.
    #[inline]
    pub fn size(&self) -> U16Vec2 {
        self.size
    }

    /// Returns the width of the map.
    #[inline]
    pub fn width(&self) -> u16 {
        self.size.x
    }

    /// Returns the height of the map.
    #[inline]
    pub fn height(&self) -> u16 {
        self.size.y
    }

    /// Returns the total number of tiles in the map.
    #[inline]
    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    /// Returns `true` if the map contains no tiles.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    /// Returns an iterator over all valid cardinal neighbors of a position.
    ///
    /// Yields positions for up, down, left, and right neighbors that are within bounds.
    pub fn neighbors<P: Into<U16Vec2>>(&self, pos: P) -> impl Iterator<Item = U16Vec2> {
        crate::game::utils::neighbors(pos.into(), self.size)
    }

    /// Calls a closure for each neighbor using tile indices instead of positions.
    ///
    /// This is useful when working with systems that still use raw indices.
    pub fn on_neighbor_indices<F>(&self, index: u32, mut closure: F)
    where
        F: FnMut(u32),
    {
        let width = self.size.x as u32;
        let height = self.size.y as u32;
        let x = index % width;
        let y = index / width;

        if x > 0 {
            closure(index - 1);
        }
        if x < width - 1 {
            closure(index + 1);
        }
        if y > 0 {
            closure(index - width);
        }
        if y < height - 1 {
            closure(index + width);
        }
    }

    /// Returns an iterator over all positions and their tile values.
    pub fn iter(&self) -> impl Iterator<Item = (U16Vec2, T)> + '_ {
        (0..self.size.y).flat_map(move |y| {
            (0..self.size.x).map(move |x| {
                let pos = U16Vec2::new(x, y);
                (pos, self[pos])
            })
        })
    }

    /// Returns an iterator over just the tile values.
    pub fn iter_values(&self) -> impl Iterator<Item = T> + '_ {
        self.tiles.iter().copied()
    }

    /// Returns an iterator over all positions in the map.
    pub fn positions(&self) -> impl Iterator<Item = U16Vec2> + '_ {
        (0..self.size.y).flat_map(move |y| (0..self.size.x).map(move |x| U16Vec2::new(x, y)))
    }

    /// Returns an iterator over tile indices, positions, and values.
    pub fn enumerate(&self) -> impl Iterator<Item = (usize, U16Vec2, T)> + '_ {
        self.tiles.iter().enumerate().map(move |(idx, &value)| {
            let pos = self.index_to_pos(idx as u32);
            (idx, pos, value)
        })
    }

    /// Returns a reference to the underlying tile data as a slice.
    pub fn as_slice(&self) -> &[T] {
        &self.tiles
    }

    /// Returns a mutable reference to the underlying tile data as a slice.
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.tiles
    }
}

impl<T: Copy + Default> TileMap<T> {
    /// Creates a new TileMap with the specified dimensions, using T::default() for initialization.
    pub fn new(size: U16Vec2) -> Self {
        Self::with_default(size, T::default())
    }
}

impl<T: Copy> Index<U16Vec2> for TileMap<T> {
    type Output = T;

    #[inline]
    fn index(&self, pos: U16Vec2) -> &Self::Output {
        &self.tiles[self.pos_to_index(pos) as usize]
    }
}

impl<T: Copy> IndexMut<U16Vec2> for TileMap<T> {
    #[inline]
    fn index_mut(&mut self, pos: U16Vec2) -> &mut Self::Output {
        let idx = self.pos_to_index(pos) as usize;
        &mut self.tiles[idx]
    }
}

// Backward compatibility: allow indexing with UVec2
impl<T: Copy> Index<UVec2> for TileMap<T> {
    type Output = T;

    #[inline]
    fn index(&self, pos: UVec2) -> &Self::Output {
        let pos16 = U16Vec2::new(pos.x as u16, pos.y as u16);
        &self.tiles[self.pos_to_index(pos16) as usize]
    }
}

impl<T: Copy> IndexMut<UVec2> for TileMap<T> {
    #[inline]
    fn index_mut(&mut self, pos: UVec2) -> &mut Self::Output {
        let pos16 = U16Vec2::new(pos.x as u16, pos.y as u16);
        let idx = self.pos_to_index(pos16) as usize;
        &mut self.tiles[idx]
    }
}

impl<T: Copy> Index<usize> for TileMap<T> {
    type Output = T;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        &self.tiles[index]
    }
}

impl<T: Copy> IndexMut<usize> for TileMap<T> {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.tiles[index]
    }
}
