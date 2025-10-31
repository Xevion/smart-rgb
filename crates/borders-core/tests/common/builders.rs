/// Map builder for programmatic terrain generation in tests
///
/// Use this to create custom terrain layouts for test scenarios.
use borders_core::prelude::*;

use super::fixtures::standard_tile_types;

/// Builder for programmatic terrain generation
///
/// # Example
/// ```
/// let terrain = MapBuilder::new(100, 100)
///     .all_conquerable()
///     .build();
/// ```
pub struct MapBuilder {
    width: u16,
    height: u16,
    terrain_data: Vec<u8>,
    tile_types: Vec<TileType>,
}

impl MapBuilder {
    /// Create a new map builder
    pub fn new(width: u16, height: u16) -> Self {
        let size = (width as usize) * (height as usize);
        Self { width, height, terrain_data: vec![0; size], tile_types: Vec::new() }
    }

    /// Make all tiles land and conquerable
    pub fn all_conquerable(mut self) -> Self {
        let size = (self.width as usize) * (self.height as usize);
        self.terrain_data = vec![0x80; size]; // bit 7 = land/conquerable
        self.tile_types = standard_tile_types();
        self
    }

    /// Create islands: center 50x50 land, rest water
    pub fn islands(mut self) -> Self {
        self.tile_types = standard_tile_types();

        let center_x = self.width / 2;
        let center_y = self.height / 2;
        let island_size = 25u16;

        for y in 0..self.height {
            for x in 0..self.width {
                let idx = (y as usize) * (self.width as usize) + (x as usize);
                let in_island = x.abs_diff(center_x) < island_size && y.abs_diff(center_y) < island_size;
                self.terrain_data[idx] = if in_island { 0x80 } else { 0x00 };
            }
        }

        self
    }

    /// Create continents: alternating vertical strips of land/water
    pub fn continents(mut self) -> Self {
        self.tile_types = standard_tile_types();

        for y in 0..self.height {
            for x in 0..self.width {
                let idx = (y as usize) * (self.width as usize) + (x as usize);
                // 30-tile wide strips
                let is_land = (x / 30) % 2 == 0;
                self.terrain_data[idx] = if is_land { 0x80 } else { 0x00 };
            }
        }

        self
    }

    /// Set specific tiles as land
    pub fn with_land(mut self, tiles: &[U16Vec2]) -> Self {
        for &tile in tiles {
            let idx = (tile.y as usize) * (self.width as usize) + (tile.x as usize);
            if idx < self.terrain_data.len() {
                self.terrain_data[idx] = 0x80;
            }
        }
        self
    }

    /// Build the TerrainData
    pub fn build(self) -> TerrainData {
        let tiles: Vec<u8> = self.terrain_data.iter().map(|&byte| if byte & 0x80 != 0 { 1 } else { 0 }).collect();

        let num_land_tiles = tiles.iter().filter(|&&t| t == 1).count();

        let map_size = U16Vec2::new(self.width, self.height);
        let terrain_tile_map = TileMap::from_vec(map_size, self.terrain_data);

        TerrainData { _manifest: MapManifest { map: MapMetadata { size: map_size, num_land_tiles }, name: "Test Map".to_string(), nations: Vec::new() }, terrain_data: terrain_tile_map, tiles, tile_types: self.tile_types }
    }
}
