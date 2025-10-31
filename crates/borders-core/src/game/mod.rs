//! Game logic and state management
//!
//! This module contains all game-related functionality organized by domain.

// Core modules
pub mod ai;
pub mod builder;
pub mod combat;
pub mod core;
pub mod entities;
pub mod input;
pub mod queries;
pub mod ships;
pub mod systems;
pub mod terrain;
pub mod world;

// Re-exports from submodules
pub use builder::*;
pub use combat::*;
pub use core::*;
pub use entities::*;
pub use input::*;
pub use queries::*;
pub use ships::*;
pub use systems::*;
pub use terrain::*;
pub use world::*;

use bevy_ecs::message::{Message, Messages};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::{IntoScheduleConfigs, ScheduleLabel, Schedules};
use bevy_ecs::system::ScheduleSystem;
use std::fmt::Debug;
use std::sync::Arc;

#[derive(Debug, Hash, PartialEq, Eq, Clone, ScheduleLabel)]
pub struct Startup;

#[derive(Debug, Hash, PartialEq, Eq, Clone, ScheduleLabel)]
pub struct Update;

#[derive(Debug, Hash, PartialEq, Eq, Clone, ScheduleLabel)]
pub struct Last;

pub struct Game {
    world: World,
    input_queue: Option<Arc<input::InputQueue>>,
}

impl std::ops::Deref for Game {
    type Target = World;

    fn deref(&self) -> &Self::Target {
        &self.world
    }
}

impl std::ops::DerefMut for Game {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.world
    }
}

impl Game {
    /// Internal method for creating an uninitialized Game
    ///
    /// This is used internally by GameBuilder and for test setup.
    /// Most users should use `GameBuilder` instead.
    #[doc(hidden)]
    pub fn new_internal() -> Self {
        let mut world = World::new();

        // Initialize schedules with proper ordering
        let mut schedules = Schedules::new();
        schedules.insert(Schedule::new(Startup));
        schedules.insert(Schedule::new(Update));
        schedules.insert(Schedule::new(Last));

        world.insert_resource(schedules);

        Self { world, input_queue: None }
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    pub fn insert_resource<R: Resource>(&mut self, resource: R) -> &mut Self {
        self.world.insert_resource(resource);
        self
    }

    pub fn init_resource<R: Resource + FromWorld>(&mut self) -> &mut Self {
        self.world.init_resource::<R>();
        self
    }

    pub fn insert_non_send_resource<R: 'static>(&mut self, resource: R) -> &mut Self {
        self.world.insert_non_send_resource(resource);
        self
    }

    pub fn add_message<M: Message>(&mut self) -> &mut Self {
        if !self.world.contains_resource::<Messages<M>>() {
            self.world.init_resource::<Messages<M>>();

            // Add system to update this message type each frame
            self.add_systems(Last, |mut messages: ResMut<Messages<M>>| {
                messages.update();
            });
        }
        self
    }

    pub fn add_systems<M>(&mut self, schedule: impl ScheduleLabel, systems: impl IntoScheduleConfigs<ScheduleSystem, M>) -> &mut Self {
        let mut schedules = self.world.resource_mut::<Schedules>();
        if let Some(schedule_inst) = schedules.get_mut(schedule) {
            schedule_inst.add_systems(systems);
        }
        self
    }

    /// Process queued input events and send them as Bevy messages
    ///
    /// Called internally by update() at the start of each frame to drain the
    /// input queue and convert InputEvents into Bevy messages.
    fn process_input_queue(&mut self) {
        let Some(queue) = &self.input_queue else {
            return;
        };

        for event in queue.drain() {
            match event {
                input::InputEvent::MouseButton { button, state, tile, world_pos } => {
                    self.world.write_message(input::MouseButtonMessage { button, state, tile, world_pos });
                }
                input::InputEvent::MouseMotion { tile, world_pos } => {
                    self.world.write_message(input::MouseMotionMessage { tile, world_pos });
                }
                input::InputEvent::KeyEvent { key, state } => {
                    self.world.write_message(input::KeyEventMessage { key, state });
                }
            }
        }
    }

    pub fn update(&mut self) {
        let _guard = tracing::trace_span!("game_update").entered();

        // Process queued input at the start of each frame
        self.process_input_queue();

        // Remove schedules temporarily to avoid resource_scope conflicts
        let mut schedules = self.world.remove_resource::<Schedules>().unwrap();

        // Run Update schedule
        if let Some(schedule) = schedules.get_mut(Update) {
            let _guard = tracing::trace_span!("update_schedule").entered();
            schedule.run(&mut self.world);
        }

        // Run Last schedule (includes event updates)
        if let Some(schedule) = schedules.get_mut(Last) {
            let _guard = tracing::trace_span!("last_schedule").entered();
            schedule.run(&mut self.world);
        }

        // Re-insert schedules
        self.world.insert_resource(schedules);
    }

    pub fn run_startup(&mut self) {
        let _guard = tracing::trace_span!("run_startup_schedule").entered();

        // Remove schedules temporarily to avoid resource_scope conflicts
        let mut schedules = self.world.remove_resource::<Schedules>().unwrap();

        // Run Startup schedule
        if let Some(schedule) = schedules.get_mut(Startup) {
            schedule.run(&mut self.world);
        }

        // Re-insert schedules
        self.world.insert_resource(schedules);
    }

    pub fn finish(&mut self) {
        // Finalize schedules
        let mut schedules = self.world.remove_resource::<Schedules>().unwrap();

        let system_count: usize = schedules.iter().map(|(_, schedule)| schedule.systems().map(|iter| iter.count()).unwrap_or(0)).sum();

        let _guard = tracing::trace_span!("finish_schedules", system_count = system_count).entered();

        for (_, schedule) in schedules.iter_mut() {
            schedule.graph_mut().initialize(&mut self.world);
        }

        self.world.insert_resource(schedules);
    }
}
