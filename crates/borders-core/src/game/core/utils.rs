use glam::{I16Vec2, U16Vec2};

/// Serde helper for U16Vec2 serialization
pub mod u16vec2_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(vec: &glam::U16Vec2, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        (vec.x, vec.y).serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<glam::U16Vec2, D::Error>
    where
        D: Deserializer<'de>,
    {
        let (x, y) = <(u16, u16)>::deserialize(deserializer)?;
        Ok(glam::U16Vec2::new(x, y))
    }
}

/// Returns an iterator over all valid cardinal neighbors of a tile position.
///
/// Yields positions for left, right, up, and down neighbors that are within bounds.
/// Handles boundary checks for the 4-connected grid.
///
/// # Examples
/// ```
/// use glam::U16Vec2;
/// use borders_core::game::utils::neighbors;
///
/// let size = U16Vec2::new(10, 10);
/// let tile = U16Vec2::new(5, 5);
/// let neighbor_count = neighbors(tile, size).count();
/// assert_eq!(neighbor_count, 4);
/// ```
pub fn neighbors(tile: U16Vec2, size: U16Vec2) -> impl Iterator<Item = U16Vec2> {
    const CARDINAL_DIRECTIONS: [I16Vec2; 4] = [I16Vec2::new(-1, 0), I16Vec2::new(1, 0), I16Vec2::new(0, -1), I16Vec2::new(0, 1)];

    CARDINAL_DIRECTIONS.into_iter().filter_map(move |offset| {
        let neighbor = tile.checked_add_signed(offset)?;
        if neighbor.x < size.x && neighbor.y < size.y { Some(neighbor) } else { None }
    })
}
