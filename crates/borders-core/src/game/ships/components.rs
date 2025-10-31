use bevy_ecs::prelude::*;
use glam::U16Vec2;

/// Ship component containing all ship state
#[derive(Component, Debug, Clone)]
pub struct Ship {
    pub id: u32,
    pub troops: u32,
    pub path: Vec<U16Vec2>,
    pub current_path_index: usize,
    pub ticks_per_tile: u32,
    pub ticks_since_move: u32,
    pub launch_tick: u64,
    pub target_tile: U16Vec2,
}

impl Ship {
    /// Create a new ship
    pub fn new(id: u32, troops: u32, path: Vec<U16Vec2>, ticks_per_tile: u32, launch_tick: u64) -> Self {
        let target_tile = *path.last().unwrap_or(&path[0]);

        Self { id, troops, path, current_path_index: 0, ticks_per_tile, ticks_since_move: 0, launch_tick, target_tile }
    }

    /// Update the ship's position based on the current tick
    /// Returns true if the ship has reached its destination
    pub fn update(&mut self) -> bool {
        if self.has_arrived() {
            return true;
        }

        self.ticks_since_move += 1;

        if self.ticks_since_move >= self.ticks_per_tile {
            self.ticks_since_move = 0;
            self.current_path_index += 1;

            if self.has_arrived() {
                return true;
            }
        }

        false
    }

    /// Get the current tile the ship is on
    #[inline]
    pub fn get_current_tile(&self) -> U16Vec2 {
        if self.current_path_index < self.path.len() { self.path[self.current_path_index] } else { self.target_tile }
    }

    /// Check if the ship has reached its destination
    #[inline]
    pub fn has_arrived(&self) -> bool {
        self.current_path_index >= self.path.len() - 1
    }

    /// Get interpolation factor for smooth rendering (0.0 to 1.0)
    #[inline]
    pub fn get_visual_interpolation(&self) -> f32 {
        if self.ticks_per_tile == 0 {
            return 1.0;
        }
        self.ticks_since_move as f32 / self.ticks_per_tile as f32
    }

    /// Get the next tile in the path (for interpolation)
    #[inline]
    pub fn get_next_tile(&self) -> Option<U16Vec2> {
        if self.current_path_index + 1 < self.path.len() { Some(self.path[self.current_path_index + 1]) } else { None }
    }
}

/// Component tracking number of ships owned by a player
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct ShipCount(pub usize);

/// Resource for generating unique ship IDs
#[derive(Resource)]
pub struct ShipIdCounter {
    next_id: u32,
}

impl ShipIdCounter {
    pub fn new() -> Self {
        Self { next_id: 1 }
    }

    pub fn generate_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

impl Default for ShipIdCounter {
    fn default() -> Self {
        Self::new()
    }
}
