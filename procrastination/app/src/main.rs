//! The driving simulator binary. Opens a window, builds a test scene, and
//! wires the keyboard to the car. Everything that simulates lives in `sim`.

mod camera;
mod hud;
mod input;
mod scene;

use bevy::prelude::*;
use procrastination_sim::SimPlugin;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Procrastination".into(),
                    ..default()
                }),
                ..default()
            }),
            SimPlugin,
        ))
        .insert_resource(ClearColor(Color::srgb(0.55, 0.7, 0.9)))
        .add_systems(Startup, scene::spawn)
        .add_systems(
            Update,
            (
                input::keyboard,
                hud::update,
                camera::follow,
                scene::hide_wheel_nodes,
            ),
        )
        .run();
}
