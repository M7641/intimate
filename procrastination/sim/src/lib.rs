//! The simulation layer. Bevy plugins that adapt the pure crates into the
//! ECS and drive them from the physics step. Nothing here opens a window,
//! so the whole crate runs under `MinimalPlugins` for headless tests.

pub mod vehicle;

use avian3d::prelude::*;
use bevy::prelude::*;

/// Everything the simulation needs: physics plus the vehicle systems.
pub struct SimPlugin;

impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        // PhysicsPlugins is a group, so it cannot be checked for; add it
        // here and nowhere else.
        app.add_plugins((PhysicsPlugins::default(), vehicle::VehiclePlugin));
    }
}
