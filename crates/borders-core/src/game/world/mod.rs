//! World and territory management module
//!
//! This module contains all spatial data structures and territory management.

pub mod changes;
pub mod coastal;
pub mod manager;
pub mod nation_id;
pub mod ownership;
pub mod tilemap;

pub use changes::*;
pub use coastal::*;
pub use manager::*;
pub use nation_id::*;
pub use ownership::*;
pub use tilemap::*;
