mod common;

use assert2::assert;
use borders_core::prelude::*;
use common::WaterPattern;
use rstest::rstest;
use std::collections::HashSet;

/// Helper to create terrain data for testing
fn create_terrain(water_tiles: &[U16Vec2], size: U16Vec2) -> TerrainData {
    let capacity = size.as_usizevec2().element_product();

    // Create two tile types: land (0) and water (1)
    let tile_types = vec![TileType { name: "Land".to_string(), color_base: "grass".to_string(), color_variant: 0, conquerable: true, navigable: false, expansion_time: 50, expansion_cost: 50 }, TileType { name: "Water".to_string(), color_base: "water".to_string(), color_variant: 0, conquerable: false, navigable: true, expansion_time: 50, expansion_cost: 50 }];

    // Create water tiles set for fast lookup
    let water_set: HashSet<U16Vec2> = water_tiles.iter().copied().collect();

    // Build terrain_data (legacy format) and tiles (new format)
    let mut terrain_data_raw = vec![0u8; capacity];
    let mut tiles = vec![0u8; capacity];

    for y in 0..size.y {
        for x in 0..size.x {
            let pos = U16Vec2::new(x, y);
            let idx = (y as usize * size.x as usize) + x as usize;

            if water_set.contains(&pos) {
                // Water tile: type index 1, no bit 7
                tiles[idx] = 1;
                terrain_data_raw[idx] = 0;
            } else {
                // Land tile: type index 0, bit 7 set
                tiles[idx] = 0;
                terrain_data_raw[idx] = 0x80;
            }
        }
    }

    let num_land_tiles = terrain_data_raw.iter().filter(|&&b| b & 0x80 != 0).count();

    TerrainData { _manifest: MapManifest { name: "Test".to_string(), map: MapMetadata { size, num_land_tiles }, nations: vec![] }, terrain_data: TileMap::from_vec(size, terrain_data_raw), tiles, tile_types }
}

/// Test empty state configurations where no coastal tiles should exist
///
/// This covers edge cases where either all tiles are land or all are water
#[rstest]
#[case::no_water(WaterPattern::no_water(), "no water tiles")]
#[case::all_water(WaterPattern::all_water(), "all water tiles")]
fn test_empty_coastal_states(#[case] pattern: WaterPattern, #[case] _description: &str) {
    let size = U16Vec2::new(pattern.map_size.0, pattern.map_size.1);
    let terrain = create_terrain(&pattern.water_tiles, size);

    let coastal = CoastalTiles::compute(&terrain, size);

    assert!(coastal.is_empty());
    assert!(coastal.len() == 0);
    assert!(coastal.tiles().is_empty());
}

/// Test single water tile configurations in different positions
///
/// Verifies coastal tile count and that water tiles themselves are not marked coastal
#[rstest]
#[case::center(WaterPattern::single_center())]
#[case::edge(WaterPattern::single_edge())]
#[case::corner(WaterPattern::single_corner())]
fn test_single_water_tile_positions(#[case] pattern: WaterPattern) {
    let size = U16Vec2::new(pattern.map_size.0, pattern.map_size.1);
    let terrain = create_terrain(&pattern.water_tiles, size);
    let coastal = CoastalTiles::compute(&terrain, size);

    assert!(!coastal.is_empty());
    assert!(coastal.len() == pattern.expected_coastal_count);

    // Water tiles themselves should never be coastal
    for water_tile in &pattern.water_tiles {
        assert!(!coastal.contains(*water_tile));
    }
}

/// Test complex water patterns generate correct coastal tile counts
///
/// Covers various geometric patterns: islands, L-shapes, lines, channels, checkerboards
#[rstest]
#[case::small_island(WaterPattern::small_island())]
#[case::l_shape(WaterPattern::l_shape())]
#[case::diagonal_line(WaterPattern::diagonal_line())]
#[case::horizontal_channel(WaterPattern::horizontal_channel())]
#[case::checkerboard(WaterPattern::checkerboard())]
fn test_complex_water_patterns(#[case] pattern: WaterPattern) {
    let size = U16Vec2::new(pattern.map_size.0, pattern.map_size.1);
    let terrain = create_terrain(&pattern.water_tiles, size);
    let coastal = CoastalTiles::compute(&terrain, size);

    assert!(!coastal.is_empty());
    assert!(coastal.len() == pattern.expected_coastal_count);

    // Water tiles themselves should never be coastal
    for water_tile in &pattern.water_tiles {
        assert!(!coastal.contains(*water_tile));
    }
}

#[test]
fn test_edge_water_creates_coastal_tiles() {
    // 5x5 grid with water along the top edge
    let size = U16Vec2::new(5, 5);
    let water_tiles: Vec<U16Vec2> = (0..5).map(|x| U16Vec2::new(x, 0)).collect();

    let terrain = create_terrain(&water_tiles, size);
    let coastal = CoastalTiles::compute(&terrain, size);

    assert!(!coastal.is_empty());
    assert!(coastal.len() == 5);

    // All tiles in row 1 should be coastal
    for x in 0..5 {
        assert!(coastal.contains(U16Vec2::new(x, 1)));
    }

    // Water tiles themselves should not be coastal
    for x in 0..5 {
        assert!(!coastal.contains(U16Vec2::new(x, 0)));
    }

    // Tiles further inland should not be coastal
    for x in 0..5 {
        assert!(!coastal.contains(U16Vec2::new(x, 2)));
    }
}

#[test]
fn test_island_configuration() {
    // 5x5 grid with water around the edges and land in the middle
    let size = U16Vec2::new(5, 5);
    let mut water_tiles = Vec::new();

    // Top and bottom edges
    for x in 0..5 {
        water_tiles.push(U16Vec2::new(x, 0));
        water_tiles.push(U16Vec2::new(x, 4));
    }

    // Left and right edges
    for y in 1..4 {
        water_tiles.push(U16Vec2::new(0, y));
        water_tiles.push(U16Vec2::new(4, y));
    }

    let terrain = create_terrain(&water_tiles, size);
    let coastal = CoastalTiles::compute(&terrain, size);

    // The inner ring of land tiles (1,1), (2,1), (3,1), (1,2), (3,2), (1,3), (2,3), (3,3)
    // should be coastal, but (2,2) should not be
    assert!(!coastal.is_empty());
    assert!(coastal.len() == 8);

    // Check the outer ring of land is coastal
    assert!(coastal.contains(U16Vec2::new(1, 1)));
    assert!(coastal.contains(U16Vec2::new(2, 1)));
    assert!(coastal.contains(U16Vec2::new(3, 1)));
    assert!(coastal.contains(U16Vec2::new(1, 2)));
    assert!(coastal.contains(U16Vec2::new(3, 2)));
    assert!(coastal.contains(U16Vec2::new(1, 3)));
    assert!(coastal.contains(U16Vec2::new(2, 3)));
    assert!(coastal.contains(U16Vec2::new(3, 3)));

    // Center tile should not be coastal (no water neighbors)
    assert!(!coastal.contains(U16Vec2::new(2, 2)));
}

#[test]
fn test_tiles_returns_correct_set() {
    let size = U16Vec2::new(3, 3);
    let water_tiles = vec![U16Vec2::new(1, 1)];

    let terrain = create_terrain(&water_tiles, size);
    let coastal = CoastalTiles::compute(&terrain, size);

    let tiles = coastal.tiles();

    // Verify it's the correct type and contains the right tiles
    let expected: HashSet<U16Vec2> = vec![U16Vec2::new(1, 0), U16Vec2::new(0, 1), U16Vec2::new(2, 1), U16Vec2::new(1, 2)].into_iter().collect();

    assert!(tiles == &expected);
}

#[test]
fn test_len_with_various_sizes() {
    // Test that len() returns accurate counts

    // Empty case
    let size = U16Vec2::new(3, 3);
    let terrain = create_terrain(&[], size);
    let coastal = CoastalTiles::compute(&terrain, size);
    assert!(coastal.len() == 0);

    // Single water tile -> 4 coastal tiles
    let water_tiles = vec![U16Vec2::new(1, 1)];
    let terrain = create_terrain(&water_tiles, size);
    let coastal = CoastalTiles::compute(&terrain, size);
    assert!(coastal.len() == 4);

    // Two adjacent water tiles -> 6 coastal tiles
    // Water at (1,1) and (2,1) creates coastal tiles at (1,0), (0,1), (2,0), (3,1), (1,2), (2,2)
    let size = U16Vec2::new(4, 3);
    let water_tiles = vec![U16Vec2::new(1, 1), U16Vec2::new(2, 1)];
    let terrain = create_terrain(&water_tiles, size);
    let coastal = CoastalTiles::compute(&terrain, size);
    assert!(coastal.len() == 6);
}

#[test]
fn test_contains_with_out_of_bounds() {
    let size = U16Vec2::new(3, 3);
    let water_tiles = vec![U16Vec2::new(0, 0)];

    let terrain = create_terrain(&water_tiles, size);
    let coastal = CoastalTiles::compute(&terrain, size);

    // Valid coastal tile
    assert!(coastal.contains(U16Vec2::new(1, 0)));
    assert!(coastal.contains(U16Vec2::new(0, 1)));

    // Out of bounds tiles should not be in the set
    assert!(!coastal.contains(U16Vec2::new(100, 100)));
    assert!(!coastal.contains(U16Vec2::new(5, 5)));
}

#[test]
fn test_is_empty_true_and_false() {
    let size = U16Vec2::new(3, 3);

    // Empty case - no water
    let terrain = create_terrain(&[], size);
    let coastal = CoastalTiles::compute(&terrain, size);
    assert!(coastal.is_empty());

    // Non-empty case - has water
    let water_tiles = vec![U16Vec2::new(1, 1)];
    let terrain = create_terrain(&water_tiles, size);
    let coastal = CoastalTiles::compute(&terrain, size);
    assert!(!coastal.is_empty());
}

#[test]
fn test_multiple_disconnected_water_bodies() {
    // Test multiple separate water bodies
    let size = U16Vec2::new(5, 5);
    let water_tiles = vec![
        U16Vec2::new(1, 1), // First water body
        U16Vec2::new(3, 3), // Second water body
    ];

    let terrain = create_terrain(&water_tiles, size);
    let coastal = CoastalTiles::compute(&terrain, size);

    // Each water tile should create coastal tiles around it
    assert!(!coastal.is_empty());

    // Check coastal tiles around first water body
    assert!(coastal.contains(U16Vec2::new(1, 0)));
    assert!(coastal.contains(U16Vec2::new(0, 1)));
    assert!(coastal.contains(U16Vec2::new(2, 1)));
    assert!(coastal.contains(U16Vec2::new(1, 2)));

    // Check coastal tiles around second water body
    assert!(coastal.contains(U16Vec2::new(3, 2)));
    assert!(coastal.contains(U16Vec2::new(2, 3)));
    assert!(coastal.contains(U16Vec2::new(4, 3)));
    assert!(coastal.contains(U16Vec2::new(3, 4)));
}
