//! Tile ownership representation

use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use serde::{Deserialize, Serialize};

use super::NationId;

/// Represents the ownership state of a single tile.
///
/// Terrain type (water, land, mountain, etc.) is stored separately in TerrainData.
/// This enum only tracks whether a tile is owned by a nation or unclaimed.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize, Default, Archive, RkyvSerialize, RkyvDeserialize)]
#[rkyv(derive(Debug))]
pub enum TileOwnership {
    /// Owned by a specific nation
    Owned(NationId),
    /// Unclaimed but potentially conquerable land
    #[default]
    Unclaimed,
}

impl TileOwnership {
    /// Check if this tile is owned by any nation
    #[inline]
    pub fn is_owned(self) -> bool {
        matches!(self, TileOwnership::Owned(_))
    }

    /// Check if this tile is unclaimed land
    #[inline]
    pub fn is_unclaimed(self) -> bool {
        matches!(self, TileOwnership::Unclaimed)
    }

    /// Get the nation ID if this tile is owned, otherwise None
    #[inline]
    pub fn nation_id(self) -> Option<NationId> {
        match self {
            TileOwnership::Owned(id) => Some(id),
            TileOwnership::Unclaimed => None,
        }
    }

    /// Check if this tile is owned by a specific nation
    #[inline]
    pub fn is_owned_by(self, nation_id: NationId) -> bool {
        matches!(self, TileOwnership::Owned(id) if id == nation_id)
    }
}

impl From<u16> for TileOwnership {
    fn from(value: u16) -> Self {
        if value == 65535 { TileOwnership::Unclaimed } else { NationId::new(value).map(TileOwnership::Owned).unwrap_or(TileOwnership::Unclaimed) }
    }
}

impl From<TileOwnership> for u16 {
    fn from(ownership: TileOwnership) -> Self {
        match ownership {
            TileOwnership::Owned(id) => id.get(),
            TileOwnership::Unclaimed => 65535,
        }
    }
}
