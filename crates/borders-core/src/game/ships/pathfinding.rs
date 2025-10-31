use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

use glam::U16Vec2;
use tracing::debug;

use crate::game::terrain::data::TerrainData;
use crate::game::utils::neighbors;

/// A node in the pathfinding search
#[derive(Clone, Eq, PartialEq)]
struct PathNode {
    pos: U16Vec2,
    g_cost: u32, // Cost from start
    h_cost: u32, // Heuristic cost to goal
    f_cost: u32, // Total cost (g + h)
}

impl Ord for PathNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for min-heap
        other.f_cost.cmp(&self.f_cost)
    }
}

impl PartialOrd for PathNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Find a water path from start_tile to target_tile using A* algorithm
/// Returns None if no path exists
pub fn find_water_path(terrain: &TerrainData, start_tile: U16Vec2, target_tile: U16Vec2, max_path_length: usize) -> Option<Vec<U16Vec2>> {
    let size = terrain.size();

    // Check if target is reachable (must be coastal or water)
    if !is_valid_ship_destination(terrain, target_tile, size) {
        debug!("Pathfinding failed: target {:?} is not a valid ship destination", target_tile);
        return None;
    }

    // Find actual water start position (adjacent to coast)
    debug!("Pathfinding: looking for water launch tile adjacent to coastal tile {:?}", start_tile);
    let water_start = find_water_launch_tile(terrain, start_tile, size)?;
    debug!("Pathfinding: found water launch tile {:?}", water_start);

    // Find water tiles adjacent to target if target is land
    let water_targets = if terrain.is_navigable(target_tile) { vec![target_tile] } else { find_adjacent_water_tiles(terrain, target_tile, size) };

    if water_targets.is_empty() {
        return None;
    }

    // Run A* pathfinding
    let mut open_set = BinaryHeap::new();
    let mut closed_set = HashSet::new();
    let mut came_from: HashMap<U16Vec2, U16Vec2> = HashMap::new();
    let mut g_scores: HashMap<U16Vec2, u32> = HashMap::new();

    // Initialize with start node
    let start_h = water_start.manhattan_distance(water_targets[0]) as u32;
    open_set.push(PathNode { pos: water_start, g_cost: 0, h_cost: start_h, f_cost: start_h });
    g_scores.insert(water_start, 0);

    while let Some(current_node) = open_set.pop() {
        let current_pos = current_node.pos;

        // Check if we've reached any of the target tiles
        if water_targets.contains(&current_pos) {
            // Reconstruct path
            let mut path = vec![current_pos];
            let mut current_tile = current_pos;

            while let Some(&parent) = came_from.get(&current_tile) {
                path.push(parent);
                current_tile = parent;

                // Prevent infinite loops
                if path.len() > max_path_length {
                    return None;
                }
            }

            path.reverse();

            // If original target was land, add it to the end
            if !terrain.is_navigable(target_tile) {
                path.push(target_tile);
            }

            return Some(path);
        }

        // Skip if already processed
        if closed_set.contains(&current_pos) {
            continue;
        }
        closed_set.insert(current_pos);

        // Check if we've exceeded max path length
        if current_node.g_cost as usize > max_path_length {
            continue;
        }

        // Explore neighbors
        for neighbor in neighbors(current_pos, size) {
            if closed_set.contains(&neighbor) {
                continue;
            }

            if !terrain.is_navigable(neighbor) {
                continue;
            }

            let tentative_g = current_node.g_cost + 1;

            if tentative_g < *g_scores.get(&neighbor).unwrap_or(&u32::MAX) {
                came_from.insert(neighbor, current_pos);
                g_scores.insert(neighbor, tentative_g);

                // Find best heuristic to any target
                let h_cost = water_targets.iter().map(|&t| neighbor.manhattan_distance(t) as u32).min().unwrap_or(0);

                let f_cost = tentative_g + h_cost;

                open_set.push(PathNode { pos: neighbor, g_cost: tentative_g, h_cost, f_cost });
            }
        }
    }

    debug!("Pathfinding failed: no path found from {:?} to {:?}", start_tile, target_tile);
    None
}

/// Find a water tile adjacent to a coastal land tile for ship launch
fn find_water_launch_tile(terrain: &TerrainData, coast_tile: U16Vec2, size: U16Vec2) -> Option<U16Vec2> {
    debug!("find_water_launch_tile: checking coastal tile {:?}", coast_tile);

    let water_tile = neighbors(coast_tile, size)
        .inspect(|&neighbor| {
            if terrain.is_navigable(neighbor) {
                debug!("  Checking neighbor {:?}: is_water=true", neighbor);
            }
        })
        .find(|&neighbor| terrain.is_navigable(neighbor));

    if let Some(tile) = water_tile {
        debug!("  Found water launch tile {:?}", tile);
    } else {
        debug!("  No water launch tile found for coastal tile {:?}", coast_tile);
    }

    water_tile
}

/// Find all water tiles adjacent to a land tile
fn find_adjacent_water_tiles(terrain: &TerrainData, tile: U16Vec2, size: U16Vec2) -> Vec<U16Vec2> {
    neighbors(tile, size).filter(|&neighbor| terrain.is_navigable(neighbor)).collect()
}

/// Check if a tile is a valid ship destination (water or coastal land)
fn is_valid_ship_destination(terrain: &TerrainData, tile: U16Vec2, size: U16Vec2) -> bool {
    // If it's water, it's valid
    if terrain.is_navigable(tile) {
        return true;
    }

    // If it's land, check if it's coastal
    neighbors(tile, size).any(|neighbor| terrain.is_navigable(neighbor))
}

/// Simplify a path by removing unnecessary waypoints (path smoothing)
/// This maintains determinism as it's purely geometric
pub fn smooth_path(path: Vec<U16Vec2>, terrain: &TerrainData) -> Vec<U16Vec2> {
    if path.len() <= 2 {
        return path;
    }

    let mut smoothed = vec![path[0]];
    let mut current_idx = 0;

    while current_idx < path.len() - 1 {
        let mut farthest = current_idx + 1;

        // Find the farthest point we can see directly
        for i in (current_idx + 2)..path.len() {
            if has_clear_water_line(terrain, path[current_idx], path[i]) {
                farthest = i;
            } else {
                break;
            }
        }

        smoothed.push(path[farthest]);
        current_idx = farthest;
    }

    smoothed
}

/// Check if there's a clear water line between two tiles
/// Uses Bresenham-like algorithm for deterministic line checking
fn has_clear_water_line(terrain: &TerrainData, from: U16Vec2, to: U16Vec2) -> bool {
    let x0 = from.x as i32;
    let y0 = from.y as i32;
    let x1 = to.x as i32;
    let y1 = to.y as i32;

    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx - dy;

    let mut x = x0;
    let mut y = y0;

    loop {
        if !terrain.is_navigable(U16Vec2::new(x as u16, y as u16)) {
            return false; // Hit land
        }

        if x == x1 && y == y1 {
            return true; // Reached target
        }

        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x += sx;
        }
        if e2 < dx {
            err += dx;
            y += sy;
        }
    }
}

/// Find the nearest coastal tile owned by a player to a target tile
/// Returns None if no valid coastal tile found
pub fn find_nearest_player_coastal_tile(coastal_tiles: &HashSet<U16Vec2>, player_border_tiles: &HashSet<U16Vec2>, target_tile: U16Vec2) -> Option<U16Vec2> {
    let best_tile = player_border_tiles.iter().filter(|&tile| coastal_tiles.contains(tile)).min_by_key(|&tile| tile.manhattan_distance(target_tile));

    debug!("Finding coastal tile: coastal_tiles.len={}, player_border_tiles.len={}, target_tile={:?}, best_tile={:?}", coastal_tiles.len(), player_border_tiles.len(), target_tile, best_tile);

    best_tile.copied()
}
