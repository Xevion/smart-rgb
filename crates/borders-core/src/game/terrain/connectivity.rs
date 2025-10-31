use crate::game::NationId;
use crate::game::TileOwnership;
use crate::game::terrain::data::TerrainData;
use crate::game::utils::neighbors;
use glam::U16Vec2;
use std::collections::VecDeque;

/// Check if a target tile's region connects to any of the nation's tiles
/// Uses flood-fill through tiles matching the target's ownership
/// Returns true if connected (normal attack), false if disconnected (ship needed)
pub fn is_connected_to_player(territory: &[TileOwnership], terrain: &TerrainData, target_tile: U16Vec2, nation_id: NationId, size: U16Vec2) -> bool {
    let target_idx = (target_tile.y as usize) * (size.x as usize) + (target_tile.x as usize);
    let target_ownership = territory[target_idx];

    // Can't connect to water
    if terrain.is_navigable(target_tile) {
        return false;
    }

    // If target is owned by nation, it's already connected
    if target_ownership.is_owned_by(nation_id) {
        return true;
    }

    // Flood-fill from target through tiles with same ownership
    let mut queue = VecDeque::new();
    let mut visited = vec![false; (size.x as usize) * (size.y as usize)];

    queue.push_back(target_tile);
    visited[target_idx] = true;

    while let Some(current_pos) = queue.pop_front() {
        for neighbor_pos in neighbors(current_pos, size) {
            let neighbor_idx = (neighbor_pos.y as usize) * (size.x as usize) + (neighbor_pos.x as usize);

            if visited[neighbor_idx] {
                continue;
            }

            // Don't cross water - only flood-fill through land on the same landmass
            if terrain.is_navigable(neighbor_pos) {
                continue;
            }

            let neighbor_ownership = territory[neighbor_idx];

            // Check if we found a nation tile - SUCCESS!
            if neighbor_ownership.is_owned_by(nation_id) {
                return true;
            }

            // Only continue through tiles matching target's ownership
            if neighbor_ownership == target_ownership {
                visited[neighbor_idx] = true;
                queue.push_back(neighbor_pos);
            }
        }
    }

    // Exhausted search without finding nation tile
    false
}

/// Find the nearest coastal tile in a region by flood-filling from target
/// Only expands through tiles matching the target's ownership
/// Returns coastal tile position if found
pub fn find_coastal_tile_in_region(territory: &[TileOwnership], terrain: &TerrainData, target_tile: U16Vec2, size: U16Vec2) -> Option<U16Vec2> {
    let target_idx = (target_tile.y as usize) * (size.x as usize) + (target_tile.x as usize);
    let target_ownership = territory[target_idx];

    // Can't find coastal tile in water
    if terrain.is_navigable(target_tile) {
        return None;
    }

    // Check if target itself is coastal
    if is_coastal_tile(terrain, target_tile, size) {
        return Some(target_tile);
    }

    // BFS from target through same-ownership tiles
    let mut queue = VecDeque::new();
    let mut visited = vec![false; (size.x as usize) * (size.y as usize)];

    queue.push_back(target_tile);
    visited[target_idx] = true;

    while let Some(current_pos) = queue.pop_front() {
        for neighbor_pos in neighbors(current_pos, size) {
            let neighbor_idx = (neighbor_pos.y as usize) * (size.x as usize) + (neighbor_pos.x as usize);

            if visited[neighbor_idx] {
                continue;
            }

            let neighbor_ownership = territory[neighbor_idx];

            // Only expand through matching ownership
            if neighbor_ownership == target_ownership {
                visited[neighbor_idx] = true;

                // Check if this tile is coastal
                if is_coastal_tile(terrain, neighbor_pos, size) {
                    return Some(neighbor_pos);
                }

                queue.push_back(neighbor_pos);
            }
        }
    }

    None
}

/// Check if a tile is coastal (land tile adjacent to water)
pub fn is_coastal_tile(terrain: &TerrainData, tile: U16Vec2, size: U16Vec2) -> bool {
    // Must be land tile
    if terrain.is_navigable(tile) {
        return false;
    }

    // Check if any neighbor is water (4-directional)
    neighbors(tile, size).any(|neighbor| terrain.is_navigable(neighbor))
}
