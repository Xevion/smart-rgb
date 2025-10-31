/// Time tracking resources for ECS
use bevy_ecs::prelude::Resource;
use quanta::{Clock as QuantaClock, Instant as QuantaInstant, Mock};
use std::sync::Arc;
use std::time::Duration;

/// Clock resource wrapping quanta::Clock for mockable time
///
/// This provides a unified time interface that works across all platforms
/// (including WebAssembly) and supports mocking for deterministic tests.
#[derive(Clone, Resource)]
pub struct Clock {
    inner: QuantaClock,
}

impl Clock {
    /// Create a new clock using the system time
    pub fn new() -> Self {
        Self { inner: QuantaClock::new() }
    }

    /// Get the current instant
    #[inline]
    pub fn now(&self) -> QuantaInstant {
        self.inner.now()
    }

    /// Get the recent (cached) instant for ultra-low-overhead timing
    #[inline]
    pub fn recent(&self) -> QuantaInstant {
        self.inner.recent()
    }

    /// Create a mock clock for testing
    pub fn mock() -> (Self, Arc<Mock>) {
        let (clock, mock) = QuantaClock::mock();
        (Self { inner: clock }, mock)
    }
}

impl Default for Clock {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Clock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Clock").finish_non_exhaustive()
    }
}

/// Time tracking resource for ECS with integrated clock source
///
/// This resource provides both frame timing (delta/elapsed) and the underlying
/// clock source for mockable time in tests.
#[derive(Clone, Resource)]
pub struct Time {
    clock: Clock,
    last_instant: QuantaInstant,
    creation_epoch_ms: u64, // Wall-clock time when Time was created (for epoch calculations)
    delta: Duration,
    elapsed: Duration,
}

impl Time {
    /// Create a new Time resource with system clock
    pub fn new() -> Self {
        let clock = Clock::new();
        let now = clock.now();
        let creation_epoch_ms = Self::get_wall_clock_epoch_ms();
        Self { clock, last_instant: now, creation_epoch_ms, delta: Duration::ZERO, elapsed: Duration::ZERO }
    }

    /// Create a Time resource with a custom clock (for testing with mock clocks)
    ///
    /// The epoch_offset_ms parameter allows tests to set a specific epoch time.
    /// Use 0 for tests that don't care about wall-clock time.
    pub fn with_clock(clock: Clock, epoch_offset_ms: u64) -> Self {
        let now = clock.now();
        Self { clock, last_instant: now, creation_epoch_ms: epoch_offset_ms, delta: Duration::ZERO, elapsed: Duration::ZERO }
    }

    /// Get current wall-clock time in milliseconds since UNIX epoch
    fn get_wall_clock_epoch_ms() -> u64 {
        #[cfg(not(target_arch = "wasm32"))]
        {
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64
        }

        #[cfg(target_arch = "wasm32")]
        {
            js_sys::Date::now() as u64
        }
    }

    /// Advance time by measuring delta since last tick
    ///
    /// This should be called once per frame by the game loop.
    pub fn tick(&mut self) {
        let now = self.clock.now();
        self.delta = now.duration_since(self.last_instant);
        self.elapsed += self.delta;
        self.last_instant = now;
    }

    /// Get the time elapsed since the last frame
    #[inline]
    pub fn delta(&self) -> Duration {
        self.delta
    }

    /// Get the time elapsed since the last frame in seconds
    #[inline]
    pub fn delta_secs(&self) -> f32 {
        self.delta.as_secs_f32()
    }

    /// Get the total time elapsed since Time was created
    #[inline]
    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }

    /// Get the total time elapsed since Time was created in seconds
    #[inline]
    pub fn elapsed_secs(&self) -> f32 {
        self.elapsed.as_secs_f32()
    }

    /// Get current time in milliseconds since UNIX epoch
    ///
    /// This is mockable in tests - it returns creation_epoch + elapsed time.
    /// For real usage, it gives actual wall-clock time.
    /// For tests with mock clocks, it gives predictable time based on mock advancement.
    #[inline]
    pub fn epoch_millis(&self) -> u64 {
        self.creation_epoch_ms + self.elapsed.as_millis() as u64
    }

    /// Legacy method for compatibility - prefer using tick() instead
    #[deprecated(note = "Use tick() instead - it measures delta automatically")]
    pub fn update(&mut self, delta: Duration) {
        self.delta = delta;
        self.elapsed += delta;
    }
}

impl Default for Time {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Time {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Time").field("delta", &self.delta).field("elapsed", &self.elapsed).finish_non_exhaustive()
    }
}

/// Fixed timestep time resource
#[derive(Debug, Clone, Resource)]
pub struct FixedTime {
    timestep: Duration,
}

impl FixedTime {
    pub fn from_seconds(seconds: f64) -> Self {
        Self { timestep: Duration::from_secs_f64(seconds) }
    }

    pub fn timestep(&self) -> Duration {
        self.timestep
    }
}
