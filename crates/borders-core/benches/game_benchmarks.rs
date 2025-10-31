use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;

use borders_core::prelude::*;
use std::collections::{HashMap, HashSet};

/// Setup a game world with specified parameters
fn setup_game_world(map_size: u16, player_count: u16, tiles_per_player: usize) -> (World, Vec<NationId>) {
    let mut world = World::new();

    // Initialize Time resources (required by many systems)
    world.insert_resource(Time::default());
    world.insert_resource(FixedTime::from_seconds(0.1));

    // Generate terrain - all conquerable for simplicity
    let size = (map_size as usize) * (map_size as usize);
    let terrain_data = vec![0x80u8; size]; // bit 7 = land/conquerable
    let tiles = vec![1u8; size]; // all land tiles

    let tile_types = vec![TileType { name: "water".to_string(), color_base: "blue".to_string(), color_variant: 0, conquerable: false, navigable: true, expansion_time: 255, expansion_cost: 255 }, TileType { name: "land".to_string(), color_base: "green".to_string(), color_variant: 0, conquerable: true, navigable: false, expansion_time: 50, expansion_cost: 50 }];

    let map_size_vec = U16Vec2::new(map_size, map_size);
    let terrain_tile_map = TileMap::from_vec(map_size_vec, terrain_data);
    let terrain = TerrainData { _manifest: MapManifest { map: MapMetadata { size: map_size_vec, num_land_tiles: size }, name: "Benchmark Map".to_string(), nations: Vec::new() }, terrain_data: terrain_tile_map, tiles, tile_types };

    // Initialize TerritoryManager
    let mut territory_manager = TerritoryManager::new(map_size_vec);
    let conquerable_tiles = vec![true; size];
    territory_manager.reset(map_size_vec, &conquerable_tiles);

    // Create player entities and assign territories
    let mut entity_map = NationEntityMap::default();
    let mut player_ids = Vec::new();

    for i in 0..player_count {
        let nation_id = if i == 0 { NationId::ZERO } else { NationId::new(i).unwrap() };
        player_ids.push(nation_id);

        let color = HSLColor::new((i as f32 * 137.5) % 360.0, 0.6, 0.5);

        let entity = world.spawn((nation_id, NationName(format!("Player {}", i)), NationColor(color), BorderTiles::default(), Troops(100.0), TerritorySize(0), ShipCount::default())).id();

        entity_map.0.insert(nation_id, entity);

        // Assign tiles to this player
        let start_y = (i as usize * tiles_per_player) / (map_size as usize);
        let end_y = ((i as usize + 1) * tiles_per_player) / (map_size as usize);

        for y in start_y..end_y.min(map_size as usize) {
            for x in 0..(map_size as usize).min(tiles_per_player) {
                if y * (map_size as usize) + x < size {
                    territory_manager.conquer(U16Vec2::new(x as u16, y as u16), nation_id);
                }
            }
        }
    }

    // Insert core resources
    world.insert_resource(entity_map);
    world.insert_resource(territory_manager);
    world.insert_resource(ActiveAttacks::new());
    world.insert_resource(terrain);
    world.insert_resource(DeterministicRng::new(0xDEADBEEF));
    world.insert_resource(BorderCache::default());
    world.insert_resource(AttackControls::default());
    world.insert_resource(LocalPlayerContext::new(NationId::ZERO));
    world.insert_resource(SpawnPhase { active: false });

    // Compute coastal tiles
    let map_size_vec = U16Vec2::new(map_size, map_size);
    let coastal_tiles = CoastalTiles::compute(world.resource::<TerrainData>(), map_size_vec);
    world.insert_resource(coastal_tiles);

    (world, player_ids)
}

/// Get 4-directional neighbors (from game utils, inlined to avoid module issues)
fn neighbors(pos: U16Vec2, map_size: U16Vec2) -> impl Iterator<Item = U16Vec2> {
    let offsets = [
        glam::I16Vec2::new(0, 1),  // North
        glam::I16Vec2::new(1, 0),  // East
        glam::I16Vec2::new(0, -1), // South
        glam::I16Vec2::new(-1, 0), // West
    ];

    offsets.into_iter().filter_map(move |offset| pos.checked_add_signed(offset).filter(|&n| n.x < map_size.x && n.y < map_size.y))
}

/// Update player borders (simplified version of the border system)
fn update_borders(world: &mut World) {
    let (changed_tiles, map_size, tiles_by_owner) = {
        let territory_manager = world.resource::<TerritoryManager>();

        let changed_tiles: HashSet<U16Vec2> = territory_manager.iter_changes().collect();
        if changed_tiles.is_empty() {
            return;
        }

        let map_size = U16Vec2::new(territory_manager.width(), territory_manager.height());

        let mut affected_tiles = HashSet::with_capacity(changed_tiles.len() * 5);
        for &tile in &changed_tiles {
            affected_tiles.insert(tile);
            affected_tiles.extend(neighbors(tile, map_size));
        }

        let mut tiles_by_owner: HashMap<NationId, HashSet<U16Vec2>> = HashMap::new();
        for &tile in &affected_tiles {
            if let Some(nation_id) = territory_manager.get_nation_id(tile) {
                tiles_by_owner.entry(nation_id).or_default().insert(tile);
            }
        }

        (changed_tiles, map_size, tiles_by_owner)
    };

    let ownership_snapshot: HashMap<U16Vec2, Option<NationId>> = {
        let territory_manager = world.resource::<TerritoryManager>();
        let mut snapshot = HashMap::new();
        for &tile in changed_tiles.iter() {
            for neighbor in neighbors(tile, map_size) {
                snapshot.entry(neighbor).or_insert_with(|| territory_manager.get_nation_id(neighbor));
            }
            snapshot.insert(tile, territory_manager.get_nation_id(tile));
        }
        snapshot
    };

    let mut nations_query = world.query::<(&NationId, &mut BorderTiles)>();
    for (nation_id, mut component_borders) in nations_query.iter_mut(world) {
        let empty_set = HashSet::new();
        let player_tiles = tiles_by_owner.get(nation_id).unwrap_or(&empty_set);

        for &tile in player_tiles {
            let is_border = neighbors(tile, map_size).any(|neighbor| ownership_snapshot.get(&neighbor).and_then(|&owner| owner) != Some(*nation_id));

            if is_border {
                component_borders.0.insert(tile);
            } else {
                component_borders.0.remove(&tile);
            }
        }

        for &tile in changed_tiles.iter() {
            if ownership_snapshot.get(&tile).and_then(|&owner| owner) != Some(*nation_id) {
                component_borders.0.remove(&tile);
            }
        }
    }
}

fn bench_border_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("border_updates");

    // Parameter: territory sizes (small, medium, large)
    let configs = [
        ("small_10_tiles", 20, 2, 10),     // 2 players, 10 tiles each
        ("medium_100_tiles", 50, 5, 100),  // 5 players, 100 tiles each
        ("large_500_tiles", 100, 10, 500), // 10 players, 500 tiles each
    ];

    for (name, map_size, player_count, tiles_per_player) in configs {
        group.bench_with_input(BenchmarkId::from_parameter(name), &name, |b, _| {
            let (mut world, player_ids) = setup_game_world(map_size, player_count, tiles_per_player);

            // Simulate some territory changes
            b.iter(|| {
                // Change ownership of a few tiles to trigger border updates
                let tiles_to_change = 5;
                for i in 0..tiles_to_change {
                    let tile = U16Vec2::new(i as u16, i as u16);
                    let new_owner = player_ids[i % player_ids.len()];
                    world.resource_mut::<TerritoryManager>().conquer(tile, new_owner);
                }

                update_borders(black_box(&mut world));

                // Clear changes for next iteration
                world.resource_mut::<TerritoryManager>().clear_changes();
            });
        });
    }

    group.finish();
}

fn bench_turn_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("turn_execution");

    // Parameter: player counts
    let player_counts = [10, 50, 101];

    for player_count in player_counts {
        group.bench_with_input(BenchmarkId::from_parameter(player_count), &player_count, |b, &count| {
            b.iter(|| {
                // Setup game state for each iteration
                let (mut world, _player_ids) = setup_game_world(100, count, 50);

                // Simulate a turn with territory changes
                let changes = 10;
                for i in 0..changes {
                    let tile = U16Vec2::new((i * 5) as u16, (i * 5) as u16);
                    let owner_idx = i % (count as usize);
                    let owner = if owner_idx == 0 { NationId::ZERO } else { NationId::new(owner_idx as u16).unwrap() };
                    world.resource_mut::<TerritoryManager>().conquer(tile, owner);
                }

                update_borders(black_box(&mut world));

                // TODO: Add actual turn execution logic here when ready
                // This currently only benchmarks territory changes + border updates
            });
        });
    }

    group.finish();
}

// TODO: Add benchmark for territory expansion
// fn bench_territory_expansion(c: &mut Criterion) {
//     // Benchmark territory growth/conquest operations
//     // Parameters: different expansion patterns (linear, radial, etc.)
// }

// TODO: Add benchmark for bot AI decision-making
// fn bench_bot_decisions(c: &mut Criterion) {
//     // Benchmark AI decision performance with different territory sizes
//     // Parameters: number of border tiles, number of viable targets
// }

// TODO: Add benchmark for attack processing
// fn bench_attack_processing(c: &mut Criterion) {
//     // Benchmark combat calculation and resolution
//     // Parameters: number of simultaneous attacks
// }

// TODO: Add benchmark for ship pathfinding
// fn bench_ship_pathfinding(c: &mut Criterion) {
//     // Benchmark naval pathfinding algorithms
//     // Parameters: map complexity, path length
// }

criterion_group!(benches, bench_border_updates, bench_turn_execution);
criterion_main!(benches);
