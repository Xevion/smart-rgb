use bevy_ecs::prelude::*;
use std::collections::{HashMap, HashSet};
use std::ops::{Deref, DerefMut};

use crate::game::core::constants::nation::*;
use crate::game::world::NationId;

/// Marker component to identify eliminated nations
/// Alive nations are identified by the ABSENCE of this component
/// Use Without<Dead> in queries to filter for alive nations
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Dead;

/// Nation name component
#[derive(Component, Debug, Clone)]
pub struct NationName(pub String);

/// Nation color component
#[derive(Component, Debug, Clone, Copy)]
pub struct NationColor(pub HSLColor);

/// Border tiles component - tiles at the edge of a nation's territory
#[derive(Component, Debug, Clone, Default)]
pub struct BorderTiles(pub HashSet<glam::U16Vec2>);

impl Deref for BorderTiles {
    type Target = HashSet<glam::U16Vec2>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for BorderTiles {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Troops component - current troop count
#[derive(Component, Debug, Clone, Copy)]
pub struct Troops(pub f32);

/// Territory size component - number of tiles owned
#[derive(Component, Debug, Clone, Copy)]
pub struct TerritorySize(pub u32);

/// Maps nation IDs to their ECS entities for O(1) lookup
///
/// This resource enables systems to quickly find a nation's entity
/// by their nation_id without iterating through all entities.
#[derive(Resource, Default)]
pub struct NationEntityMap(pub HashMap<NationId, Entity>);

impl NationEntityMap {
    /// Get the entity for a nation, panicking if not found
    ///
    /// # Panics
    /// Panics if the nation ID is not in the map
    #[inline]
    pub fn get_entity(&self, nation_id: NationId) -> Entity {
        *self.0.get(&nation_id).unwrap_or_else(|| panic!("Nation entity not found for nation {}", nation_id.get()))
    }

    /// Try to get the entity for a nation
    ///
    /// Returns None if the nation ID is not in the map
    #[inline]
    pub fn try_get_entity(&self, nation_id: NationId) -> Option<Entity> {
        self.0.get(&nation_id).copied()
    }
}

/// HSL Color representation
#[derive(Debug, Clone, Copy)]
pub struct HSLColor {
    pub h: f32, // Hue: 0-360
    pub s: f32, // Saturation: 0-1
    pub l: f32, // Lightness: 0-1
}

impl HSLColor {
    pub fn new(h: f32, s: f32, l: f32) -> Self {
        Self { h, s, l }
    }

    pub fn to_rgba(&self) -> [f32; 4] {
        let c = (1.0 - (2.0 * self.l - 1.0).abs()) * self.s;
        let h_prime = self.h / 60.0;
        let x = c * (1.0 - ((h_prime % 2.0) - 1.0).abs());

        let (r1, g1, b1) = if h_prime < 1.0 {
            (c, x, 0.0)
        } else if h_prime < 2.0 {
            (x, c, 0.0)
        } else if h_prime < 3.0 {
            (0.0, c, x)
        } else if h_prime < 4.0 {
            (0.0, x, c)
        } else if h_prime < 5.0 {
            (x, 0.0, c)
        } else {
            (c, 0.0, x)
        };

        let m = self.l - c / 2.0;
        [r1 + m, g1 + m, b1 + m, 1.0]
    }
}

/// Calculate maximum troop capacity based on territory size
#[inline]
pub fn calculate_max_troops(territory_size: u32, is_bot: bool) -> f32 {
    let base_max = MAX_TROOPS_MULTIPLIER * ((territory_size as f32).powf(MAX_TROOPS_POWER) * MAX_TROOPS_SCALE + MAX_TROOPS_BASE);

    if is_bot { base_max * BOT_MAX_TROOPS_MULTIPLIER } else { base_max }
}

/// Calculate income for this tick based on current troops and territory
#[inline]
pub fn calculate_income(troops: f32, territory_size: u32, is_bot: bool) -> f32 {
    let max_troops = calculate_max_troops(territory_size, is_bot);

    // Base income calculation
    let mut income = BASE_INCOME + (troops.powf(INCOME_POWER) / INCOME_DIVISOR);

    // Soft cap as approaching max troops
    let ratio = 1.0 - (troops / max_troops);
    income *= ratio;

    // Apply bot modifier
    if is_bot { income * BOT_INCOME_MULTIPLIER } else { income }
}

/// Add troops with max cap enforcement
#[inline]
pub fn add_troops_capped(current: f32, amount: f32, territory_size: u32, is_bot: bool) -> f32 {
    let max_troops = calculate_max_troops(territory_size, is_bot);
    (current + amount).min(max_troops)
}

/// Remove troops, ensuring non-negative result
#[inline]
pub fn remove_troops(current: f32, amount: f32) -> f32 {
    (current - amount).max(0.0)
}
