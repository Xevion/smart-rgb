//! Pre-built test scenarios and fixtures for integration tests
//!
//! Provides:
//! - Static map fixtures (PLAINS_MAP, ISLAND_MAP, etc.)
//! - GameBuilderTestExt trait for common test configurations
//! - TestGameBuilder wrapper for test-specific nation spawning
use once_cell::sync::Lazy;
use std::sync::Arc;

use super::GameTestExt;
use super::builders::MapBuilder;
use borders_core::game::Game;
use borders_core::game::builder::GameBuilder;
use borders_core::game::terrain::TileType;
use borders_core::prelude::*;
use extension_traits::extension;

/// Standard 100x100 plains map (all conquerable)
pub static PLAINS_MAP: Lazy<Arc<TerrainData>> = Lazy::new(|| Arc::new(MapBuilder::new(100, 100).all_conquerable().build()));

/// Island archipelago map: 50x50 islands separated by water
pub static ISLAND_MAP: Lazy<Arc<TerrainData>> = Lazy::new(|| Arc::new(MapBuilder::new(100, 100).islands().build()));

/// Continental map: vertical strips of land and water
pub static CONTINENT_MAP: Lazy<Arc<TerrainData>> = Lazy::new(|| Arc::new(MapBuilder::new(100, 100).continents().build()));

/// Get a clone of the plains map
pub fn get_plains_map() -> TerrainData {
    (*PLAINS_MAP.clone()).clone()
}

/// Get a clone of the island map
pub fn get_island_map() -> TerrainData {
    (*ISLAND_MAP.clone()).clone()
}

/// Get a clone of the continent map
pub fn get_continent_map() -> TerrainData {
    (*CONTINENT_MAP.clone()).clone()
}

/// Create standard water tile type
///
/// Water tiles are non-conquerable but navigable by ships.
pub fn standard_water_type() -> TileType {
    TileType { name: "water".to_string(), color_base: "blue".to_string(), color_variant: 0, conquerable: false, navigable: true, expansion_time: 255, expansion_cost: 255 }
}

/// Create standard land tile type
///
/// Land tiles are conquerable with standard expansion costs.
pub fn standard_land_type() -> TileType {
    TileType { name: "land".to_string(), color_base: "green".to_string(), color_variant: 0, conquerable: true, navigable: false, expansion_time: 50, expansion_cost: 50 }
}

/// Create standard tile types (water and land)
///
/// Returns a vec containing water (index 0) and land (index 1) tile types.
pub fn standard_tile_types() -> Vec<TileType> {
    vec![standard_water_type(), standard_land_type()]
}

// ============================================================================
// rstest Fixtures
// ============================================================================

/// Map size fixture data: (width, height, description)
#[derive(Debug, Clone, Copy)]
pub struct MapSize {
    pub width: u16,
    pub height: u16,
    pub description: &'static str,
}

/// Standard map sizes for parameterized testing
pub const MAP_SIZES: &[MapSize] = &[MapSize { width: 5, height: 5, description: "5x5" }, MapSize { width: 10, height: 10, description: "10x10" }, MapSize { width: 20, height: 20, description: "20x20" }, MapSize { width: 100, height: 100, description: "100x100" }, MapSize { width: 1000, height: 1000, description: "1000x1000" }];

/// Terrain pattern for coastal/water testing
#[derive(Debug, Clone)]
pub struct WaterPattern {
    pub name: &'static str,
    pub map_size: (u16, u16),
    pub water_tiles: Vec<U16Vec2>,
    pub expected_coastal_count: usize,
}

impl WaterPattern {
    /// Create a new water pattern
    pub fn new(name: &'static str, map_size: (u16, u16), water_tiles: Vec<U16Vec2>, expected_coastal_count: usize) -> Self {
        Self { name, map_size, water_tiles, expected_coastal_count }
    }

    /// Single water tile in center of 10x10 map
    pub fn single_center() -> Self {
        Self::new(
            "single_center",
            (10, 10),
            vec![U16Vec2::new(5, 5)],
            4, // 4 orthogonal neighbors
        )
    }

    /// Water tile at edge (not corner)
    pub fn single_edge() -> Self {
        Self::new(
            "single_edge",
            (10, 10),
            vec![U16Vec2::new(0, 5)],
            3, // 3 neighbors (edge blocks one direction)
        )
    }

    /// Water tile at corner
    pub fn single_corner() -> Self {
        Self::new(
            "single_corner",
            (10, 10),
            vec![U16Vec2::new(0, 0)],
            2, // 2 neighbors (corner blocks two directions)
        )
    }

    /// Small 2x2 island
    pub fn small_island() -> Self {
        Self::new(
            "small_island",
            (10, 10),
            vec![U16Vec2::new(4, 4), U16Vec2::new(5, 4), U16Vec2::new(4, 5), U16Vec2::new(5, 5)],
            8, // Perimeter of 2x2 square
        )
    }

    /// L-shaped water pattern
    pub fn l_shape() -> Self {
        Self::new(
            "l_shape",
            (10, 10),
            vec![U16Vec2::new(5, 5), U16Vec2::new(5, 6), U16Vec2::new(5, 7), U16Vec2::new(6, 5), U16Vec2::new(7, 5)],
            11, // Perimeter of L-shape
        )
    }

    /// Diagonal water line
    pub fn diagonal_line() -> Self {
        Self::new(
            "diagonal_line",
            (10, 10),
            vec![U16Vec2::new(2, 2), U16Vec2::new(3, 3), U16Vec2::new(4, 4), U16Vec2::new(5, 5)],
            10, // Coastal tiles around diagonal line
        )
    }

    /// Thin horizontal water channel
    pub fn horizontal_channel() -> Self {
        Self::new(
            "horizontal_channel",
            (10, 10),
            vec![U16Vec2::new(2, 5), U16Vec2::new(3, 5), U16Vec2::new(4, 5), U16Vec2::new(5, 5), U16Vec2::new(6, 5), U16Vec2::new(7, 5)],
            14, // Two land tiles above and below channel
        )
    }

    /// Checkerboard water pattern (alternating tiles)
    pub fn checkerboard() -> Self {
        let mut water_tiles = Vec::new();
        for y in 0..5 {
            for x in 0..5 {
                if (x + y) % 2 == 0 {
                    water_tiles.push(U16Vec2::new(x, y));
                }
            }
        }
        Self::new(
            "checkerboard",
            (10, 10),
            water_tiles,
            18, // Coastal tiles around checkerboard pattern
        )
    }

    /// All water (no coastal tiles)
    pub fn all_water() -> Self {
        let mut water_tiles = Vec::new();
        for y in 0..10 {
            for x in 0..10 {
                water_tiles.push(U16Vec2::new(x, y));
            }
        }
        Self::new(
            "all_water",
            (10, 10),
            water_tiles,
            0, // No land tiles to be coastal
        )
    }

    /// No water (no coastal tiles)
    pub fn no_water() -> Self {
        Self::new(
            "no_water",
            (10, 10),
            vec![],
            0, // No water means no coastal tiles
        )
    }
}

/// Wrapper for GameBuilder that allows test-specific nation spawning
///
/// Accumulates pending nations with custom troop counts, then spawns them
/// after the game is built. This allows tests to create nations with specific
/// IDs and initial troops without polluting the production GameBuilder API.
pub struct TestGameBuilder {
    builder: GameBuilder,
    pending_nations: Vec<(NationId, f32)>,
}

impl TestGameBuilder {
    /// Add a nation with custom initial troops
    ///
    /// Nations will be spawned in order after `.build()` is called.
    pub fn with_nation(mut self, id: NationId, troops: f32) -> Self {
        self.pending_nations.push((id, troops));
        self
    }

    // Delegate common GameBuilder methods to maintain fluent API

    pub fn with_network(mut self, mode: NetworkMode) -> Self {
        self.builder = self.builder.with_network(mode);
        self
    }

    pub fn with_systems(mut self, enabled: bool) -> Self {
        self.builder = self.builder.with_systems(enabled);
        self
    }

    pub fn with_spawn_phase_enabled(mut self) -> Self {
        self.builder = self.builder.with_spawn_phase(Some(30));
        self
    }

    pub fn with_spawn_phase(mut self, timeout: Option<u32>) -> Self {
        self.builder = self.builder.with_spawn_phase(timeout);
        self
    }

    pub fn with_local_player(mut self, id: NationId) -> Self {
        self.builder = self.builder.with_local_player(id);
        self
    }

    pub fn with_bots(mut self, count: u32) -> Self {
        self.builder = self.builder.with_bots(count);
        self
    }

    pub fn with_map(mut self, terrain: Arc<TerrainData>) -> Self {
        self.builder = self.builder.with_map(terrain);
        self
    }

    /// Build the game and spawn all pending test nations
    pub fn build(self) -> Game {
        let mut game = self.builder.build();
        for (id, troops) in self.pending_nations {
            game.spawn_test_nation(id, troops);
        }
        game
    }
}

/// Extension trait providing test fixture methods for GameBuilder
///
/// Provides common test configurations without hiding critical settings.
/// Tests must still explicitly set:
/// - `.with_network(NetworkMode::Local)` for turn generation
/// - `.with_systems(false)` if system execution should be disabled
///
/// # Example
/// ```
/// use common::GameBuilderTestExt;
///
/// let mut game = GameBuilder::simple()
///     .with_nation(NationId::ZERO, 100.0)
///     .with_network(NetworkMode::Local)
///     .with_systems(false)
///     .build();
/// ```
#[extension(pub trait GameBuilderTestExt)]
impl GameBuilder {
    /// Create a simple test game with default 100x100 conquerable map
    ///
    /// Returns a partially-configured builder. Tests must still add:
    /// - `.with_network(NetworkMode::Local)`
    /// - `.with_systems(false)` if needed
    fn simple() -> Self {
        Self::new().with_map(Arc::new(MapBuilder::new(100, 100).all_conquerable().build()))
    }

    /// Create a test game with custom map size (all conquerable tiles)
    ///
    /// Returns a partially-configured builder. Tests must still add:
    /// - `.with_network(NetworkMode::Local)`
    /// - `.with_systems(false)` if needed
    fn with_map_size(width: u16, height: u16) -> Self {
        Self::new().with_map(Arc::new(MapBuilder::new(width, height).all_conquerable().build()))
    }

    /// Create a two-nation test game with default map
    ///
    /// Returns a partially-configured builder with:
    /// - Default 100x100 conquerable map
    /// - Local nation: NationId::ZERO
    /// - 1 bot nation
    ///
    /// Tests must still add:
    /// - `.with_network(NetworkMode::Local)`
    /// - `.with_systems(false)` if needed
    fn two_player() -> Self {
        Self::simple().with_local_player(NationId::ZERO).with_bots(1)
    }

    /// Enable spawn phase with default timeout
    ///
    /// Chainable after other fixture methods.
    fn with_spawn_phase_enabled(self) -> Self {
        self.with_spawn_phase(Some(30))
    }

    /// Add a nation with custom initial troops (switches to TestGameBuilder)
    ///
    /// This creates a TestGameBuilder wrapper that allows you to specify
    /// nation IDs and initial troop counts. The nations will be spawned
    /// after `.build()` is called.
    ///
    /// # Example
    /// ```
    /// let game = GameBuilder::simple()
    ///     .with_nation(NationId::ZERO, 100.0)
    ///     .with_nation(NationId::new(1).unwrap(), 150.0)
    ///     .with_network(NetworkMode::Local)
    ///     .build();
    /// ```
    fn with_nation(self, id: NationId, troops: f32) -> TestGameBuilder {
        TestGameBuilder { builder: self, pending_nations: vec![(id, troops)] }
    }
}
