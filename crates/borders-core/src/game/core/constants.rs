/// Game constants organized by domain
///
/// This module centralizes all game balance constants that were previously
/// scattered across multiple files. Constants are grouped by gameplay domain
/// for easy discovery and tuning.
pub mod game {
    /// Game tick interval in milliseconds (10 TPS = 100ms per turn)
    pub const TICK_INTERVAL: u64 = 100;

    /// Number of bot nations
    pub const BOT_COUNT: usize = 500;
}

pub mod combat {
    /// Empire size balancing - prevents snowballing by large empires
    /// Defense effectiveness decreases as empire grows beyond this threshold
    pub const DEFENSE_DEBUFF_MIDPOINT: f32 = 150_000.0;

    /// Rate of defense effectiveness decay for large empires
    /// Uses natural log decay for smooth scaling
    pub const DEFENSE_DEBUFF_DECAY_RATE: f32 = std::f32::consts::LN_2 / 50_000.0;

    /// Base terrain magnitude cost for plains (baseline terrain)
    /// Determines troop losses when conquering a tile
    pub const BASE_MAG_PLAINS: f32 = 80.0;

    /// Base terrain speed for plains (baseline terrain)
    /// Affects how many tiles can be conquered per tick
    pub const BASE_SPEED_PLAINS: f32 = 16.5;

    /// Maximum random adjustment to border size when calculating expansion speed
    /// Introduces natural variation in attack progression (0-4 range)
    pub const BORDER_RANDOM_ADJUSTMENT_MAX: u32 = 5;

    /// Multiplier for tiles conquered per tick when attacking unclaimed territory
    pub const UNCLAIMED_TILES_PER_TICK_MULTIPLIER: f32 = 2.0;

    /// Multiplier for tiles conquered per tick when attacking claimed territory
    pub const CLAIMED_TILES_PER_TICK_MULTIPLIER: f32 = 3.0;

    /// Large empire threshold for attack penalties (>100k tiles)
    pub const LARGE_EMPIRE_THRESHOLD: u32 = 100_000;

    /// Random factor range for tile priority calculation (0-7)
    pub const TILE_PRIORITY_RANDOM_MAX: u32 = 8;

    /// Defense post magnitude multiplier (when implemented)
    pub const DEFENSE_POST_MAG_MULTIPLIER: f32 = 5.0;

    /// Defense post speed multiplier (when implemented)
    pub const DEFENSE_POST_SPEED_MULTIPLIER: f32 = 3.0;

    /// Base defense debuff for large defenders (70%)
    pub const LARGE_DEFENDER_BASE_DEBUFF: f32 = 0.7;

    /// Scaling factor for large defender sigmoid (30%)
    pub const LARGE_DEFENDER_SCALING: f32 = 0.3;

    /// Power exponent for large attacker bonus calculation
    pub const LARGE_ATTACKER_POWER_EXPONENT: f32 = 0.7;

    /// Speed exponent for large attacker penalty calculation
    pub const LARGE_ATTACKER_SPEED_EXPONENT: f32 = 0.6;

    /// Minimum troop ratio for combat calculations
    pub const TROOP_RATIO_MIN: f32 = 0.6;

    /// Maximum troop ratio for combat calculations
    pub const TROOP_RATIO_MAX: f32 = 2.0;

    /// Multiplier for attacker loss calculations
    pub const ATTACKER_LOSS_MULTIPLIER: f32 = 0.8;

    /// Divisor for tiles per tick calculation
    pub const TILES_PER_TICK_DIVISOR: f32 = 5.0;

    /// Minimum tiles per tick cost
    pub const TILES_PER_TICK_MIN: f32 = 0.2;

    /// Maximum tiles per tick cost
    pub const TILES_PER_TICK_MAX: f32 = 1.5;

    /// Divisor for unclaimed territory attack losses
    pub const UNCLAIMED_ATTACK_LOSS_DIVISOR: f32 = 5.0;

    /// Base multiplier for unclaimed territory conquest speed
    pub const UNCLAIMED_BASE_MULTIPLIER: f32 = 2_000.0;

    /// Minimum speed value for plains terrain
    pub const MIN_SPEED_PLAINS: f32 = 10.0;

    /// Minimum tiles per tick for unclaimed territory
    pub const UNCLAIMED_TILES_MIN: f32 = 5.0;

    /// Maximum tiles per tick for unclaimed territory
    pub const UNCLAIMED_TILES_MAX: f32 = 100.0;

    /// Multiplier for attack ratio calculation
    pub const ATTACK_RATIO_MULTIPLIER: f32 = 5.0;

    /// Scale factor for attack ratio
    pub const ATTACK_RATIO_SCALE: f32 = 2.0;

    /// Minimum attack ratio for dynamic calculation
    pub const ATTACK_RATIO_MIN: f32 = 0.01;

    /// Maximum attack ratio for dynamic calculation
    pub const ATTACK_RATIO_MAX: f32 = 0.5;

    /// Base priority value for tile conquest
    pub const TILE_PRIORITY_BASE: f32 = 1.0;

    /// Priority penalty per owned neighbor tile
    pub const TILE_PRIORITY_NEIGHBOR_PENALTY: f32 = 0.5;
}

pub mod nation {
    /// Multiplier for max troops calculation
    pub const MAX_TROOPS_MULTIPLIER: f32 = 2.0;

    /// Power exponent for max troops based on territory size
    pub const MAX_TROOPS_POWER: f32 = 0.6;

    /// Scale factor for max troops calculation
    pub const MAX_TROOPS_SCALE: f32 = 1000.0;

    /// Base max troops value
    pub const MAX_TROOPS_BASE: f32 = 50_000.0;

    /// Bots get 33% of human max troops
    pub const BOT_MAX_TROOPS_MULTIPLIER: f32 = 0.33;

    /// Base income per tick
    pub const BASE_INCOME: f32 = 10.0;

    /// Power exponent for income calculation
    pub const INCOME_POWER: f32 = 0.73;

    /// Divisor for income calculation
    pub const INCOME_DIVISOR: f32 = 4.0;

    /// Bots get 60% of human income
    pub const BOT_INCOME_MULTIPLIER: f32 = 0.6;

    /// Initial troops for all nations at spawn
    pub const INITIAL_TROOPS: f32 = 2500.0;
}

pub mod bot {
    /// Maximum initial cooldown for bot actions (0-9 ticks)
    pub const INITIAL_COOLDOWN_MAX: u64 = 10;

    /// Minimum cooldown between bot actions (ticks)
    pub const ACTION_COOLDOWN_MIN: u64 = 3;

    /// Maximum cooldown between bot actions (ticks)
    pub const ACTION_COOLDOWN_MAX: u64 = 15;

    /// Probability that bot chooses expansion over attack (60%)
    pub const EXPAND_PROBABILITY: f32 = 0.6;

    /// Minimum troop percentage for wilderness expansion (10%)
    pub const EXPAND_TROOPS_MIN: f32 = 0.1;

    /// Maximum troop percentage for wilderness expansion (30%)
    pub const EXPAND_TROOPS_MAX: f32 = 0.3;

    /// Minimum troop percentage for nation attacks (20%)
    pub const ATTACK_TROOPS_MIN: f32 = 0.2;

    /// Maximum troop percentage for nation attacks (50%)
    pub const ATTACK_TROOPS_MAX: f32 = 0.5;

    /// Minimum distance between spawn points (in tiles)
    pub const MIN_SPAWN_DISTANCE: f32 = 70.0;

    /// Absolute minimum spawn distance for fallback
    pub const ABSOLUTE_MIN_DISTANCE: f32 = 5.0;

    /// Distance reduction factor per adaptive wave (15% reduction)
    pub const DISTANCE_REDUCTION_FACTOR: f32 = 0.85;

    /// Number of random spawn placement attempts
    pub const SPAWN_RANDOM_ATTEMPTS: usize = 1000;

    /// Maximum attempts for grid-guided spawn placement
    pub const SPAWN_GRID_MAX_ATTEMPTS: usize = 200;

    /// Maximum attempts for fallback spawn placement
    pub const SPAWN_FALLBACK_ATTEMPTS: usize = 10_000;

    /// Stride factor for grid-guided spawn placement (80% of current distance)
    pub const SPAWN_GRID_STRIDE_FACTOR: f32 = 0.8;

    /// Maximum border tiles sampled for bot decision making
    pub const MAX_BORDER_SAMPLES: usize = 20;
}

pub mod colors {
    /// Golden angle for visually distinct color distribution (degrees)
    pub const GOLDEN_ANGLE: f32 = 137.5;

    /// Minimum saturation for nation colors
    pub const SATURATION_MIN: f32 = 0.75;

    /// Maximum saturation for nation colors
    pub const SATURATION_MAX: f32 = 0.95;

    /// Minimum lightness for nation colors
    pub const LIGHTNESS_MIN: f32 = 0.35;

    /// Maximum lightness for nation colors
    pub const LIGHTNESS_MAX: f32 = 0.65;
}

pub mod input {
    /// Default attack ratio when game starts (50%)
    pub const DEFAULT_ATTACK_RATIO: f32 = 0.5;

    /// Step size for attack ratio adjustment (10%)
    pub const ATTACK_RATIO_STEP: f32 = 0.1;

    /// Minimum attack ratio (10%)
    pub const ATTACK_RATIO_MIN: f32 = 0.1;

    /// Maximum attack ratio (100%)
    pub const ATTACK_RATIO_MAX: f32 = 1.0;
}

pub mod ships {
    /// Maximum ships per nation
    pub const MAX_SHIPS_PER_NATION: usize = 5;

    /// Ticks required to move one tile (1 = fast speed)
    pub const TICKS_PER_TILE: u32 = 1;

    /// Maximum path length for ship pathfinding
    pub const MAX_PATH_LENGTH: usize = 1_000_000;

    /// Percentage of troops carried by ship (20%)
    pub const TROOP_PERCENT: f32 = 0.20;
}

pub mod outcome {
    /// Win threshold - percentage of map needed to win (80%)
    pub const WIN_THRESHOLD: f32 = 0.80;
}

pub mod spawning {
    /// Radius of tiles claimed around spawn point (creates 5x5 square)
    pub const SPAWN_RADIUS: i16 = 2;

    /// Spawn timeout duration in seconds
    pub const SPAWN_TIMEOUT_SECS: f32 = 2.0;
}
