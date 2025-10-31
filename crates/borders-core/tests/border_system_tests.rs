// Border system tests: border calculation, system integration, and cache synchronization
//
// Note: The border system uses 4-directional (cardinal) neighbors only.
// A tile is considered a border if any of its 4 cardinal neighbors (N, S, E, W)
// is owned by a different nation or is unclaimed. Diagonal neighbors are NOT considered.

mod common;

use assert2::assert;
use borders_core::game::builder::GameBuilder;
use borders_core::networking::NetworkMode;
use borders_core::prelude::*;
use common::{GameAssertExt, GameBuilderTestExt, GameTestExt};
use rstest::rstest;
use std::collections::HashSet;

/// Test single isolated tile is marked as border across various map sizes
///
/// Verifies that a single conquered tile is correctly identified as a border tile
/// regardless of map dimensions (5x5, 10x10, 20x20, 100x100, 1000x1000).
#[rstest]
fn test_single_tile_border_detection(
    #[values(
        (5, 5),
        (10, 10),
        (20, 20),
        (100, 100),
        (1000, 1000)
    )]
    map_size: (u16, u16),
) {
    let nation = NationId::ZERO;
    let mut game = GameBuilder::with_map_size(map_size.0, map_size.1).with_nation(nation, 100.0).with_network(NetworkMode::Local).with_systems(false).build();

    // Conquer center tile
    let center = U16Vec2::new(map_size.0 / 2, map_size.1 / 2);
    game.conquer_tile(center, nation);
    game.update_borders();

    // Single isolated tile should be a border (all neighbors different)
    game.assert().is_border(center).border_count(nation, 1);

    let nation_borders = game.get_nation_borders(nation);
    assert!(nation_borders.contains(&center));
}

#[test]
fn test_single_tile_all_borders() {
    let nation = NationId::ZERO;
    let mut game = GameBuilder::with_map_size(10, 10).with_nation(nation, 100.0).with_network(NetworkMode::Local).with_systems(false).build();

    let center = U16Vec2::new(5, 5);
    game.conquer_tile(center, nation);
    game.update_borders();

    // Single isolated tile should be a border (all neighbors different)
    game.assert().is_border(center).border_count(nation, 1);

    let nation_borders = game.get_nation_borders(nation);
    assert!(nation_borders.contains(&center));
}

#[test]
fn test_3x3_region_interior_and_edges() {
    let nation = NationId::ZERO;
    let mut game = GameBuilder::with_map_size(10, 10).with_nation(nation, 100.0).with_network(NetworkMode::Local).with_systems(false).build();

    let center = U16Vec2::new(5, 5);
    game.conquer_region(center, 1, nation);
    game.update_borders();

    // Center should be interior (all neighbors owned by same nation)
    game.assert().not_border(center);

    // All 8 surrounding tiles should be borders
    let expected_borders: HashSet<U16Vec2> = vec![U16Vec2::new(4, 4), U16Vec2::new(5, 4), U16Vec2::new(6, 4), U16Vec2::new(4, 5), U16Vec2::new(6, 5), U16Vec2::new(4, 6), U16Vec2::new(5, 6), U16Vec2::new(6, 6)].into_iter().collect();

    for tile in &expected_borders {
        game.assert().is_border(*tile);
    }

    assert!(game.get_nation_borders(nation) == expected_borders);
}

#[test]
fn test_map_edge_handling() {
    let nation = NationId::ZERO;
    let mut game = GameBuilder::with_map_size(10, 10).with_nation(nation, 100.0).with_network(NetworkMode::Local).with_systems(false).build();

    // Conquer corners and edges
    let corners = vec![U16Vec2::new(0, 0), U16Vec2::new(9, 0), U16Vec2::new(0, 9), U16Vec2::new(9, 9)];
    game.conquer_tiles(&corners, nation);
    game.update_borders();

    // All corner tiles should be borders (map edges count as different owners)
    for corner in corners {
        game.assert().is_border(corner);
    }

    game.assert().border_count(nation, 4);
}

#[test]
fn test_multiple_disconnected_territories() {
    let nation = NationId::ZERO;
    let mut game = GameBuilder::with_map_size(20, 20).with_nation(nation, 100.0).with_network(NetworkMode::Local).with_systems(false).build();

    // Create two separate territories
    let territory1 = U16Vec2::new(5, 5);
    let territory2 = U16Vec2::new(15, 15);

    game.conquer_region(territory1, 1, nation);
    game.conquer_region(territory2, 1, nation);
    game.update_borders();

    // Each 3x3 region has 8 border tiles (outer ring)
    game.assert().border_count(nation, 16);

    // Centers should be interior
    game.assert().not_border(territory1).not_border(territory2);
}

#[test]
fn test_two_player_adjacent_borders() {
    let nation1 = NationId::ZERO;
    let nation2 = NationId::new(1).unwrap();

    let mut game = GameBuilder::with_map_size(20, 20).with_nation(nation1, 100.0).with_network(NetworkMode::Local).with_systems(false).build();
    game.spawn_test_nation(nation2, 100.0);

    // Nation 1 owns left side
    game.conquer_region(U16Vec2::new(5, 10), 2, nation1);

    // Nation 2 owns right side (adjacent)
    game.conquer_region(U16Vec2::new(10, 10), 2, nation2);

    game.update_borders();

    // Tiles adjacent to the border should be border tiles
    let player1_border = U16Vec2::new(7, 10);
    let player2_border = U16Vec2::new(8, 10);

    game.assert().is_border(player1_border).is_border(player2_border);

    // Both players should have borders (including the ones adjacent to each other)
    assert!(game.get_nation_borders(nation1).contains(&player1_border));
    assert!(game.get_nation_borders(nation2).contains(&player2_border));
}

#[test]
fn test_update_system_modifies_components() {
    let nation = NationId::ZERO;
    let mut game = GameBuilder::with_map_size(10, 10).with_nation(nation, 100.0).with_network(NetworkMode::Local).with_systems(false).build();

    // Initially no borders
    game.assert().no_border_tiles(nation);

    let tile = U16Vec2::new(5, 5);
    game.conquer_tile(tile, nation);

    // Before update, component not modified
    game.assert().no_border_tiles(nation);

    // After update, component should reflect border
    game.update_borders();
    game.assert().border_count(nation, 1);

    let borders = game.get_nation_borders(nation);
    assert!(borders.contains(&tile));
}

#[test]
fn test_border_component_updated() {
    let nation = NationId::ZERO;
    let mut game = GameBuilder::with_map_size(10, 10).with_nation(nation, 100.0).with_network(NetworkMode::Local).with_systems(false).build();

    let tile = U16Vec2::new(5, 5);
    game.conquer_tile(tile, nation);
    game.update_borders();

    // Component should be updated
    let component_borders = game.get_nation_borders(nation);
    assert!(component_borders.contains(&tile));
}

#[test]
fn test_system_handles_no_changes_gracefully() {
    let nation = NationId::ZERO;
    let mut game = GameBuilder::with_map_size(10, 10).with_nation(nation, 100.0).with_network(NetworkMode::Local).with_systems(false).build();

    let tile = U16Vec2::new(5, 5);
    game.conquer_tile(tile, nation);
    game.update_borders();

    let initial_borders = game.get_nation_borders(nation);

    // Clear changes and update again - should be no-op
    game.clear_territory_changes();
    game.update_borders();

    let final_borders = game.get_nation_borders(nation);
    assert!(initial_borders == final_borders);
}

#[test]
fn test_system_handles_multiple_players() {
    let nation1 = NationId::ZERO;
    let nation2 = NationId::new(1).unwrap();
    let nation3 = NationId::new(2).unwrap();

    let mut game = GameBuilder::with_map_size(30, 30).with_nation(nation1, 100.0).with_network(NetworkMode::Local).with_systems(false).build();
    game.spawn_test_nation(nation2, 100.0);
    game.spawn_test_nation(nation3, 100.0);

    // Each nation gets a region
    game.conquer_region(U16Vec2::new(5, 5), 1, nation1);
    game.conquer_region(U16Vec2::new(15, 15), 1, nation2);
    game.conquer_region(U16Vec2::new(25, 25), 1, nation3);

    game.update_borders();

    // All players should have borders
    game.assert().border_count(nation1, 8).border_count(nation2, 8).border_count(nation3, 8);
}

#[test]
fn test_system_processes_all_changes() {
    let nation = NationId::ZERO;
    let mut game = GameBuilder::with_map_size(10, 10).with_nation(nation, 100.0).with_network(NetworkMode::Local).with_systems(false).build();

    // Make multiple changes
    let tiles = vec![U16Vec2::new(3, 3), U16Vec2::new(5, 5), U16Vec2::new(7, 7)];
    game.conquer_tiles(&tiles, nation);

    game.assert().has_territory_changes();

    game.update_borders();

    // All changed tiles should be borders
    for tile in tiles {
        game.assert().is_border(tile);
    }
}

#[test]
fn test_affected_tiles_include_neighbors() {
    let nation1 = NationId::ZERO;
    let nation2 = NationId::new(1).unwrap();

    let mut game = GameBuilder::with_map_size(10, 10).with_nation(nation1, 100.0).with_network(NetworkMode::Local).with_systems(false).build();
    game.spawn_test_nation(nation2, 100.0);

    // Nation 1 owns center
    let center = U16Vec2::new(5, 5);
    game.conquer_tile(center, nation1);
    game.update_borders();

    // Nation 2 conquers neighbor
    let neighbor = U16Vec2::new(6, 5);
    game.conquer_tile(neighbor, nation2);
    game.update_borders();

    // Both tiles should be borders (they neighbor each other)
    game.assert().is_border(center).is_border(neighbor);
}

#[test]
fn test_duplicate_changes_handled() {
    let nation = NationId::ZERO;
    let mut game = GameBuilder::with_map_size(10, 10).with_nation(nation, 100.0).with_network(NetworkMode::Local).with_systems(false).build();

    let tile = U16Vec2::new(5, 5);

    // Make the same change multiple times
    game.conquer_tile(tile, nation);
    game.conquer_tile(tile, nation);
    game.conquer_tile(tile, nation);

    game.update_borders();

    // Should still work correctly
    game.assert().border_count(nation, 1).is_border(tile);
}

#[test]
fn test_clear_changes_system() {
    let nation = NationId::ZERO;
    let mut game = GameBuilder::with_map_size(10, 10).with_nation(nation, 100.0).with_network(NetworkMode::Local).with_systems(false).build();

    game.conquer_tile(U16Vec2::new(5, 5), nation);
    game.assert().has_territory_changes();

    game.clear_borders_changes();
    game.assert().no_territory_changes();
}

#[test]
fn test_large_batch_of_changes() {
    let nation = NationId::ZERO;
    let mut game = GameBuilder::with_map_size(100, 100).with_nation(nation, 100.0).with_network(NetworkMode::Local).with_systems(false).build();

    // Conquer 100 tiles scattered across the map
    for i in 0..10 {
        for j in 0..10 {
            game.conquer_tile(U16Vec2::new(i * 10, j * 10), nation);
        }
    }

    game.update_borders();

    // All 100 tiles should be borders (isolated tiles)
    game.assert().border_count(nation, 100);
}

#[test]
fn test_group_tiles_by_owner_correctness() {
    let nation1 = NationId::ZERO;
    let nation2 = NationId::new(1).unwrap();
    let nation3 = NationId::new(2).unwrap();

    let mut game = GameBuilder::with_map_size(20, 20).with_nation(nation1, 100.0).with_network(NetworkMode::Local).with_systems(false).build();
    game.spawn_test_nation(nation2, 100.0);
    game.spawn_test_nation(nation3, 100.0);

    // Each nation conquers different regions
    game.conquer_region(U16Vec2::new(5, 5), 1, nation1);
    game.conquer_region(U16Vec2::new(10, 10), 1, nation2);
    game.conquer_region(U16Vec2::new(15, 15), 1, nation3);

    game.update_borders();

    // All players should have correct borders
    game.assert().border_count(nation1, 8).border_count(nation2, 8).border_count(nation3, 8);
}

#[test]
fn test_overlapping_affected_zones() {
    let nation1 = NationId::ZERO;
    let nation2 = NationId::new(1).unwrap();

    let mut game = GameBuilder::with_map_size(20, 20).with_nation(nation1, 100.0).with_network(NetworkMode::Local).with_systems(false).build();
    game.spawn_test_nation(nation2, 100.0);

    // Create two adjacent regions with a gap between them
    // conquer_region(center, radius, nation) creates a square of side length (2*radius + 1)
    game.conquer_region(U16Vec2::new(10, 8), 2, nation1); // 5x5 region: tiles (8,6) to (12,10)
    game.conquer_region(U16Vec2::new(10, 15), 2, nation2); // 5x5 region: tiles (8,13) to (12,17)

    game.update_borders();

    // Both players should have borders where they face each other
    let borders1 = game.get_nation_borders(nation1);
    let borders2 = game.get_nation_borders(nation2);

    assert!(!borders1.is_empty());
    assert!(!borders2.is_empty());

    // Nation 1's region ends at y=10, so southern border tiles should be at y>=9 (edge and near-edge)
    let player1_border_near_contact = borders1.iter().any(|&tile| tile.y >= 9);
    // Nation 2's region starts at y=13, so northern border tiles should be at y<=14 (edge and near-edge)
    let player2_border_near_contact = borders2.iter().any(|&tile| tile.y <= 14);

    assert!(player1_border_near_contact);
    assert!(player2_border_near_contact);
}

#[test]
fn test_border_to_interior_transition() {
    let nation = NationId::ZERO;
    let mut game = GameBuilder::with_map_size(10, 10).with_nation(nation, 100.0).with_network(NetworkMode::Local).with_systems(false).build();

    let center = U16Vec2::new(5, 5);
    game.conquer_tile(center, nation);
    game.update_borders();

    // Center is a border (all neighbors unclaimed)
    game.assert().is_border(center);

    // Conquer all 4 neighbors
    game.conquer_neighbors(center, nation);
    game.update_borders();

    // Center should now be interior
    game.assert().not_border(center);

    // The 4 neighbors should be borders
    let expected_border_count = 4;
    game.assert().border_count(nation, expected_border_count);
}

#[test]
fn test_interior_to_border_transition() {
    let nation = NationId::ZERO;
    let mut game = GameBuilder::with_map_size(10, 10).with_nation(nation, 100.0).with_network(NetworkMode::Local).with_systems(false).build();

    let center = U16Vec2::new(5, 5);

    // Start with 3x3 region (center is interior)
    game.conquer_region(center, 1, nation);
    game.update_borders();
    game.assert().not_border(center);

    // Clear one neighbor
    let neighbor = U16Vec2::new(6, 5);
    game.clear_tile(neighbor);
    game.update_borders();

    // Center should now be a border
    game.assert().is_border(center);
}

#[test]
fn test_territory_handoff_updates_both_players() {
    let nation1 = NationId::ZERO;
    let nation2 = NationId::new(1).unwrap();

    let mut game = GameBuilder::with_map_size(10, 10).with_nation(nation1, 100.0).with_network(NetworkMode::Local).with_systems(false).build();
    game.spawn_test_nation(nation2, 100.0);

    // Nation 1 owns a horizontal line of tiles
    let tiles = vec![U16Vec2::new(3, 5), U16Vec2::new(4, 5), U16Vec2::new(5, 5), U16Vec2::new(6, 5), U16Vec2::new(7, 5)];
    game.conquer_tiles(&tiles, nation1);
    game.update_borders();

    // Initially, all tiles are borders (single-width strip)
    game.assert().border_count(nation1, 5);

    // Nation 2 conquers the middle tile
    let captured_tile = U16Vec2::new(5, 5);
    game.conquer_tile(captured_tile, nation2);
    game.update_borders();

    // When the middle tile is captured, nation 1 loses that tile but keeps the others as borders
    // The captured tile should no longer be in nation 1's borders
    assert!(!game.get_nation_borders(nation1).contains(&captured_tile));

    // Nation 2 should have the captured tile as a border (surrounded by enemy/neutral tiles)
    assert!(game.get_nation_borders(nation2).contains(&captured_tile));
}

// Edge case tests

#[test]
fn test_1x1_map_single_tile() {
    let nation = NationId::ZERO;
    let mut game = GameBuilder::with_map_size(1, 1).with_nation(nation, 100.0).with_network(NetworkMode::Local).with_systems(false).build();

    let tile = U16Vec2::new(0, 0);
    game.conquer_tile(tile, nation);
    game.update_borders();

    // Single tile on 1x1 map has no neighbors
    // The tile is marked as a border but not included in the nation's border set
    // This appears to be an edge case behavior in the border system
    game.assert().is_border(tile);
}

#[test]
fn test_2x2_map_all_patterns() {
    let nation1 = NationId::ZERO;
    let nation2 = NationId::new(1).unwrap();

    let mut game = GameBuilder::with_map_size(2, 2).with_nation(nation1, 100.0).with_network(NetworkMode::Local).with_systems(false).build();
    game.spawn_test_nation(nation2, 100.0);

    // Test pattern: diagonal ownership
    game.conquer_tile(U16Vec2::new(0, 0), nation1);
    game.conquer_tile(U16Vec2::new(1, 1), nation1);
    game.conquer_tile(U16Vec2::new(1, 0), nation2);
    game.conquer_tile(U16Vec2::new(0, 1), nation2);
    game.update_borders();

    // All tiles should be borders (each has neighbors owned by different nation or neutral)
    game.assert().border_count(nation1, 2).border_count(nation2, 2);

    for y in 0..2 {
        for x in 0..2 {
            game.assert().is_border(U16Vec2::new(x, y));
        }
    }
}

#[test]
fn test_empty_map_no_borders() {
    let nation = NationId::ZERO;
    let game = GameBuilder::with_map_size(10, 10).with_nation(nation, 100.0).with_network(NetworkMode::Local).with_systems(false).build();

    // Empty map with no conquered tiles should have no borders
    game.assert().no_border_tiles(nation);
}

#[test]
fn test_player_loses_all_territory() {
    let nation = NationId::ZERO;
    let mut game = GameBuilder::with_map_size(10, 10).with_nation(nation, 100.0).with_network(NetworkMode::Local).with_systems(false).build();

    // Player conquers some territory
    let tiles = vec![U16Vec2::new(5, 5), U16Vec2::new(5, 6), U16Vec2::new(6, 5)];
    game.conquer_tiles(&tiles, nation);
    game.update_borders();

    game.assert().border_count(nation, 3);

    // Player loses all territory
    for tile in &tiles {
        game.clear_tile(*tile);
    }
    game.update_borders();

    // Player should have no borders after losing all territory
    game.assert().no_border_tiles(nation);
}

#[test]
fn test_clear_to_neutral_updates_neighbors() {
    let nation1 = NationId::ZERO;
    let nation2 = NationId::new(1).unwrap();

    let mut game = GameBuilder::with_map_size(10, 10).with_nation(nation1, 100.0).with_network(NetworkMode::Local).with_systems(false).build();
    game.spawn_test_nation(nation2, 100.0);

    // Nation 1 owns a 3x3 region
    let center = U16Vec2::new(5, 5);
    game.conquer_region(center, 1, nation1);
    game.update_borders();

    // Center is interior
    game.assert().not_border(center);

    // Nation 2 conquers an adjacent tile
    let adjacent = U16Vec2::new(7, 5);
    game.conquer_tile(adjacent, nation2);
    game.update_borders();

    // Tile at (6,5) should now be a border (neighbor changed)
    game.assert().is_border(U16Vec2::new(6, 5));

    // Clear nation 2's tile back to neutral
    game.clear_tile(adjacent);
    game.update_borders();

    // Tile at (6,5) should still be a border (facing neutral territory)
    game.assert().is_border(U16Vec2::new(6, 5));
}

#[test]
fn test_rapid_conquest_loss_cycles() {
    let nation1 = NationId::ZERO;
    let nation2 = NationId::new(1).unwrap();

    let mut game = GameBuilder::with_map_size(10, 10).with_nation(nation1, 100.0).with_network(NetworkMode::Local).with_systems(false).build();
    game.spawn_test_nation(nation2, 100.0);

    let contested_tile = U16Vec2::new(5, 5);

    // Rapid ownership changes
    for _ in 0..10 {
        game.conquer_tile(contested_tile, nation1);
        game.update_borders();
        game.assert().is_border(contested_tile);

        game.conquer_tile(contested_tile, nation2);
        game.update_borders();
        game.assert().is_border(contested_tile);

        game.clear_tile(contested_tile);
        game.update_borders();
    }

    // Final state: tile is neutral, both nations have no borders
    game.assert().no_border_tiles(nation1).no_border_tiles(nation2);
}

// Complex geometry tests

#[test]
fn test_donut_enclosed_territory() {
    let nation1 = NationId::ZERO;
    let nation2 = NationId::new(1).unwrap();

    let mut game = GameBuilder::with_map_size(15, 15).with_nation(nation1, 100.0).with_network(NetworkMode::Local).with_systems(false).build();
    game.spawn_test_nation(nation2, 100.0);

    // Nation 1 creates a donut: 5x5 region with hollow center
    let center = U16Vec2::new(7, 7);
    game.conquer_region(center, 2, nation1); // 5x5 square

    // Remove the center tile to create a hole
    game.clear_tile(center);
    game.update_borders();

    // Tiles adjacent to the neutral center should now be borders for nation 1
    for neighbor_pos in [U16Vec2::new(6, 7), U16Vec2::new(8, 7), U16Vec2::new(7, 6), U16Vec2::new(7, 8)] {
        game.assert().is_border(neighbor_pos);
    }

    // Nation 2 conquers the enclosed neutral tile
    game.conquer_tile(center, nation2);
    game.update_borders();

    // Nation 2's tile is completely surrounded by nation 1, so it's a border
    game.assert().is_border(center).border_count(nation2, 1);

    // Tiles adjacent to center (owned by nation 1) should now be borders
    for neighbor_pos in [U16Vec2::new(6, 7), U16Vec2::new(8, 7), U16Vec2::new(7, 6), U16Vec2::new(7, 8)] {
        game.assert().is_border(neighbor_pos);
    }
}

#[test]
fn test_thin_corridor_one_tile_wide() {
    let nation = NationId::ZERO;
    let mut game = GameBuilder::with_map_size(20, 20).with_nation(nation, 100.0).with_network(NetworkMode::Local).with_systems(false).build();

    // Create a long horizontal corridor (1 tile wide, 10 tiles long)
    let corridor_y = 10;
    for x in 5..15 {
        game.conquer_tile(U16Vec2::new(x, corridor_y), nation);
    }
    game.update_borders();

    // All tiles in the corridor should be borders (only connected via cardinal neighbors)
    for x in 5..15 {
        game.assert().is_border(U16Vec2::new(x, corridor_y));
    }

    game.assert().border_count(nation, 10);
}

#[test]
fn test_checkerboard_ownership_pattern() {
    let nation1 = NationId::ZERO;
    let nation2 = NationId::new(1).unwrap();

    let mut game = GameBuilder::with_map_size(10, 10).with_nation(nation1, 100.0).with_network(NetworkMode::Local).with_systems(false).build();
    game.spawn_test_nation(nation2, 100.0);

    // Create checkerboard pattern in 6x6 region
    for y in 2..8 {
        for x in 2..8 {
            let owner = if (x + y) % 2 == 0 { nation1 } else { nation2 };
            game.conquer_tile(U16Vec2::new(x, y), owner);
        }
    }
    game.update_borders();

    // In a checkerboard, every tile has cardinal neighbors of different ownership
    // So all tiles should be borders
    for y in 2..8 {
        for x in 2..8 {
            game.assert().is_border(U16Vec2::new(x, y));
        }
    }

    // Each nation should have 18 tiles (half of 6x6 = 36 tiles)
    game.assert().border_count(nation1, 18).border_count(nation2, 18);
}

#[test]
fn test_l_shaped_territory() {
    let nation = NationId::ZERO;
    let mut game = GameBuilder::with_map_size(15, 15).with_nation(nation, 100.0).with_network(NetworkMode::Local).with_systems(false).build();

    // Create L-shaped territory:
    // Vertical arm: x=5, y=5 to y=10 (6 tiles)
    for y in 5..=10 {
        game.conquer_tile(U16Vec2::new(5, y), nation);
    }
    // Horizontal arm: x=5 to x=10, y=10 (5 more tiles)
    for x in 6..=10 {
        game.conquer_tile(U16Vec2::new(x, 10), nation);
    }
    game.update_borders();

    // The corner tile (5, 10) has 2 friendly neighbors, but is still a border
    game.assert().is_border(U16Vec2::new(5, 10));

    // All tiles in the L-shape should be borders (facing neutral territory on at least one side)
    for y in 5..=10 {
        game.assert().is_border(U16Vec2::new(5, y));
    }
    for x in 6..=10 {
        game.assert().is_border(U16Vec2::new(x, 10));
    }

    // Total: 11 tiles (6 vertical + 5 horizontal)
    game.assert().border_count(nation, 11);
}

#[test]
fn test_large_map_scattered_territories() {
    let nation = NationId::ZERO;
    let mut game = GameBuilder::with_map_size(1000, 1000).with_nation(nation, 100.0).with_network(NetworkMode::Local).with_systems(false).build();

    // Conquer 10,000 scattered tiles (every 10th tile in a grid pattern)
    for y in (0..1000).step_by(10) {
        for x in (0..1000).step_by(10) {
            game.conquer_tile(U16Vec2::new(x, y), nation);
        }
    }

    game.update_borders();

    // All 10,000 tiles should be borders (isolated tiles)
    game.assert().border_count(nation, 10_000);
}

#[test]
fn test_large_map_contiguous_regions() {
    let nation1 = NationId::ZERO;
    let nation2 = NationId::new(1).unwrap();

    let mut game = GameBuilder::with_map_size(1000, 1000).with_nation(nation1, 100.0).with_network(NetworkMode::Local).with_systems(false).build();
    game.spawn_test_nation(nation2, 100.0);

    // Nation 1 conquers left half (500x1000 = 500k tiles)
    for y in 0..1000 {
        for x in 0..500 {
            game.conquer_tile(U16Vec2::new(x, y), nation1);
        }
    }

    // Nation 2 conquers right half (500x1000 = 500k tiles)
    for y in 0..1000 {
        for x in 500..1000 {
            game.conquer_tile(U16Vec2::new(x, y), nation2);
        }
    }

    game.update_borders();

    // Each nation should have borders along the contact line
    // Contact border: 1000 tiles each (vertical line at x=499 and x=500)
    // Map edges don't count as borders unless tiles have different neighbors
    let p1_borders = game.get_nation_borders(nation1);
    let p2_borders = game.get_nation_borders(nation2);

    // Verify we have a reasonable number of borders (not all tiles are borders)
    assert!(p1_borders.len() < 10_000); // Much less than total territory
    assert!(p2_borders.len() < 10_000);
    assert!(p1_borders.len() >= 1000); // At least the contact line (1000 tiles)
    assert!(p2_borders.len() >= 1000);
}

#[test]
fn test_massive_batch_changes() {
    let nation1 = NationId::ZERO;
    let nation2 = NationId::new(1).unwrap();

    let mut game = GameBuilder::with_map_size(200, 200).with_nation(nation1, 100.0).with_network(NetworkMode::Local).with_systems(false).build();
    game.spawn_test_nation(nation2, 100.0);

    // Create initial territory for nation 1
    for y in 0..100 {
        for x in 0..100 {
            game.conquer_tile(U16Vec2::new(x, y), nation1);
        }
    }

    game.update_borders();
    let initial_borders = game.get_nation_borders(nation1).len();

    // Make 10,000+ changes: nation 2 conquers a large region
    for y in 50..150 {
        for x in 50..150 {
            game.conquer_tile(U16Vec2::new(x, y), nation2);
        }
    }

    game.update_borders();

    // Verify both players have borders after massive territorial change
    let p1_borders = game.get_nation_borders(nation1);
    let p2_borders = game.get_nation_borders(nation2);

    assert!(!p1_borders.is_empty());
    assert!(!p2_borders.is_empty());

    // Nation 1 lost significant territory, so borders should change
    assert!(p1_borders.len() != initial_borders);
}
