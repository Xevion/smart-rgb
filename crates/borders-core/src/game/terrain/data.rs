use bevy_ecs::prelude::Resource;
use glam::U16Vec2;
use image::GenericImageView;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::game::world::tilemap::TileMap;

/// Calculate terrain color using pastel theme formulas
fn calculate_theme_color(color_base: &str, color_variant: u8) -> [u8; 3] {
    let i = color_variant as i32;

    match color_base {
        "grass" => {
            // rgb(238 - 2 * i, 238 - 2 * i, 190 - i)
            [(238 - 2 * i).clamp(0, 255) as u8, (238 - 2 * i).clamp(0, 255) as u8, (190 - i).clamp(0, 255) as u8]
        }
        "mountain" => {
            // rgb(250 - 2 * i, 250 - 2 * i, 220 - i)
            [(250 - 2 * i).clamp(0, 255) as u8, (250 - 2 * i).clamp(0, 255) as u8, (220 - i).clamp(0, 255) as u8]
        }
        "water" => {
            // rgb(172 - 2 * i, 225 - 2 * i, 249 - 3 * i)
            [(172 - 2 * i).clamp(0, 255) as u8, (225 - 2 * i).clamp(0, 255) as u8, (249 - 3 * i).clamp(0, 255) as u8]
        }
        _ => {
            // Default fallback color (gray)
            [128, 128, 128]
        }
    }
}

/// Helper structs for loading World.json format
#[derive(Deserialize)]
struct WorldMapJson {
    tiles: Vec<WorldTileDef>,
}

#[derive(Deserialize)]
struct WorldTileDef {
    color: String,
    name: String,
    #[serde(default, rename = "colorBase")]
    color_base: Option<String>,
    #[serde(default, rename = "colorVariant")]
    color_variant: Option<u32>,
    conquerable: bool,
    navigable: bool,
    #[serde(default, rename = "expansionCost")]
    expansion_cost: Option<u32>,
    #[serde(default, rename = "expansionTime")]
    expansion_time: Option<u32>,
}

/// Parse hex color string (#RRGGBB) to RGB bytes
fn parse_hex_rgb(s: &str) -> Option<[u8; 3]> {
    let s = s.trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some([r, g, b])
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TileType {
    pub name: String,
    pub color_base: String,
    pub color_variant: u8,
    pub conquerable: bool,
    pub navigable: bool,
    pub expansion_time: u8,
    pub expansion_cost: u8,
}

/// Map manifest structure
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MapManifest {
    pub map: MapMetadata,
    pub name: String,
    pub nations: Vec<NationSpawn>,
}

/// Map size metadata
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MapMetadata {
    pub size: glam::U16Vec2,
    pub num_land_tiles: usize,
}

impl MapMetadata {
    /// Get the width of the map
    #[inline]
    pub fn width(&self) -> u16 {
        self.size.x
    }

    /// Get the height of the map
    #[inline]
    pub fn height(&self) -> u16 {
        self.size.y
    }
}

/// Nation spawn point
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NationSpawn {
    pub coordinates: [usize; 2],
    pub flag: String,
    pub name: String,
    pub strength: u32,
}

/// Loaded map data
#[derive(Debug, Clone, Resource)]
pub struct TerrainData {
    pub _manifest: MapManifest,
    /// Legacy terrain data (for backward compatibility)
    pub terrain_data: TileMap<u8>,
    /// Tile type indices (new format)
    pub tiles: Vec<u8>,
    /// Tile type definitions
    pub tile_types: Vec<TileType>,
}

impl TerrainData {
    /// Load the World map from embedded assets
    pub fn load_world_map() -> Result<Self, Box<dyn std::error::Error>> {
        let _guard = tracing::debug_span!("load_world_map").entered();

        const MAP_JSON: &[u8] = include_bytes!("../../../assets/maps/World.json");
        const MAP_PNG: &[u8] = include_bytes!("../../../assets/maps/World.png");

        // Parse JSON tile definitions
        let map_json: WorldMapJson = {
            let _guard = tracing::trace_span!("parse_json").entered();
            serde_json::from_slice(MAP_JSON)?
        };

        // Load PNG image
        let (png, width, height) = {
            let _guard = tracing::trace_span!("load_png").entered();
            let png = image::load_from_memory(MAP_PNG)?;
            let (width, height) = png.dimensions();
            (png, width, height)
        };

        info!("Loading World map: {}x{}", width, height);

        // Build color-to-index lookup table
        let color_to_index: Vec<([u8; 3], usize)> = map_json.tiles.iter().enumerate().filter_map(|(idx, t)| parse_hex_rgb(&t.color).map(|rgb| (rgb, idx))).collect();

        let pixel_count = (width as usize) * (height as usize);
        let mut tiles = vec![0u8; pixel_count];
        let mut terrain_data_raw = vec![0u8; pixel_count];

        // Match each pixel to nearest tile type by color
        {
            let _guard = tracing::trace_span!("pixel_processing", pixel_count = pixel_count).entered();

            for y in 0..height {
                for x in 0..width {
                    let pixel = png.get_pixel(x, y).0;
                    let rgb = [pixel[0], pixel[1], pixel[2]];

                    // Find nearest tile by RGB distance
                    let (tile_idx, _) = color_to_index
                        .iter()
                        .map(|(c, idx)| {
                            let dr = rgb[0] as i32 - c[0] as i32;
                            let dg = rgb[1] as i32 - c[1] as i32;
                            let db = rgb[2] as i32 - c[2] as i32;
                            let dist = (dr * dr + dg * dg + db * db) as u32;
                            (idx, dist)
                        })
                        .min_by_key(|(_, d)| *d)
                        .unwrap();

                    let i = (y * width + x) as usize;
                    tiles[i] = *tile_idx as u8;

                    // Set bit 7 if conquerable (land)
                    if map_json.tiles[*tile_idx].conquerable {
                        terrain_data_raw[i] |= 0x80;
                    }
                    // Lower 5 bits for terrain magnitude (unused for World map)
                }
            }
        }

        // Convert to TileType format
        let tile_types = {
            let _guard = tracing::trace_span!("tile_type_conversion").entered();
            map_json.tiles.into_iter().map(|t| TileType { name: t.name, color_base: t.color_base.unwrap_or_default(), color_variant: t.color_variant.unwrap_or(0) as u8, conquerable: t.conquerable, navigable: t.navigable, expansion_cost: t.expansion_cost.unwrap_or(50) as u8, expansion_time: t.expansion_time.unwrap_or(50) as u8 }).collect()
        };

        let num_land_tiles = terrain_data_raw.iter().filter(|&&b| b & 0x80 != 0).count();

        info!("World map loaded: {} land tiles", num_land_tiles);

        Ok(Self { _manifest: MapManifest { name: "World".to_string(), map: MapMetadata { size: glam::U16Vec2::new(width as u16, height as u16), num_land_tiles }, nations: vec![] }, terrain_data: TileMap::from_vec(glam::U16Vec2::new(width as u16, height as u16), terrain_data_raw), tiles, tile_types })
    }

    /// Get the size of the map
    #[inline]
    pub fn size(&self) -> U16Vec2 {
        self.terrain_data.size()
    }

    #[inline]
    pub fn get_value(&self, pos: U16Vec2) -> u8 {
        self.terrain_data[pos]
    }

    /// Check if a tile is land (bit 7 set)
    #[inline]
    pub fn is_land(&self, pos: U16Vec2) -> bool {
        self.get_value(pos) & 0x80 != 0
    }

    /// Get tile type at position
    pub fn get_tile_type(&self, pos: U16Vec2) -> &TileType {
        let idx = self.terrain_data.pos_to_index(pos) as usize;
        &self.tile_types[self.tiles[idx] as usize]
    }

    /// Check if a tile is conquerable
    pub fn is_conquerable(&self, pos: U16Vec2) -> bool {
        self.get_tile_type(pos).conquerable
    }

    /// Check if a tile is navigable (water)
    pub fn is_navigable(&self, pos: U16Vec2) -> bool {
        self.get_tile_type(pos).navigable
    }

    /// Get tile type IDs for rendering (each position maps to a tile type)
    pub fn get_tile_ids(&self) -> &[u8] {
        &self.tiles
    }

    /// Get terrain palette colors from tile types (for rendering)
    /// Returns a vec where index = tile type ID, value = RGB color
    /// Colors are calculated using theme formulas based on colorBase and colorVariant
    pub fn get_terrain_palette_colors(&self) -> Vec<[u8; 3]> {
        self.tile_types.iter().map(|tile_type| calculate_theme_color(&tile_type.color_base, tile_type.color_variant)).collect()
    }
}
