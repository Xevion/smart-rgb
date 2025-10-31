use bevy_ecs::prelude::Component;
use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Unique identifier for a nation/player in the game.
///
/// This is a validated newtype wrapper around u16 that prevents invalid nation IDs
/// from being constructed. The maximum valid nation ID is 65534, as 65535 is reserved
/// for encoding unclaimed tiles in the ownership serialization format.
#[derive(Component, Debug, Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Archive, RkyvSerialize, RkyvDeserialize)]
#[rkyv(derive(Debug, Hash, PartialEq, Eq))]
pub struct NationId(u16);

impl NationId {
    /// Maximum valid nation ID (65534). Value 65535 is reserved for unclaimed tiles.
    pub const MAX: u16 = u16::MAX - 1;

    /// Constant for nation ID 0 (commonly used for default/first player)
    pub const ZERO: Self = Self(0);

    /// Creates a new NationId if the value is valid (<= MAX).
    ///
    /// Returns None if id > MAX (i.e., id == 65535).
    #[inline]
    pub fn new(id: u16) -> Option<Self> {
        (id <= Self::MAX).then_some(Self(id))
    }

    /// Creates a NationId without validation.
    ///
    /// # Safety
    /// Caller must ensure id <= MAX. This is primarily for const contexts.
    #[inline]
    pub const fn new_unchecked(id: u16) -> Self {
        Self(id)
    }

    /// Extracts the inner u16 value.
    #[inline]
    pub fn get(self) -> u16 {
        self.0
    }

    /// Converts to little-endian bytes.
    #[inline]
    pub fn to_le_bytes(self) -> [u8; 2] {
        self.0.to_le_bytes()
    }
}

impl TryFrom<u16> for NationId {
    type Error = InvalidNationId;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::new(value).ok_or(InvalidNationId(value))
    }
}

impl From<NationId> for u16 {
    fn from(id: NationId) -> Self {
        id.0
    }
}

impl std::fmt::Display for NationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Error type for invalid nation ID values
#[derive(Debug, Clone, Copy)]
pub struct InvalidNationId(pub u16);

impl std::fmt::Display for InvalidNationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Invalid nation ID: {} (must be <= {})", self.0, NationId::MAX)
    }
}

impl std::error::Error for InvalidNationId {}

impl Serialize for NationId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Serialize::serialize(&self.0, serializer)
    }
}

impl<'de> Deserialize<'de> for NationId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u16::deserialize(deserializer)?;
        NationId::new(value).ok_or_else(|| serde::de::Error::custom(format!("Invalid nation ID: {} (must be <= {})", value, NationId::MAX)))
    }
}
