//! Instrument readout. One text block, updated every frame.

use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;
use procrastination_sim::vehicle::{Vehicle, VehicleControls};

use crate::scene::Player;

#[derive(Component)]
pub struct Readout;

pub fn update(
    mut commands: Commands,
    player: Query<(&Vehicle, &VehicleControls, &LinearVelocity), With<Player>>,
    mut readout: Query<&mut Text, With<Readout>>,
) {
    let Ok((vehicle, controls, velocity)) = player.single() else {
        return;
    };
    let gear = match controls.gear {
        -1 => "R".to_string(),
        0 => "N".to_string(),
        g => g.to_string(),
    };
    let engine = if vehicle.drivetrain.running {
        "running"
    } else {
        "STALLED (E to restart)"
    };
    let text = format!(
        "{:>3.0} mph   gear {gear}   {:>4.0} rpm   engine {engine}\nthrottle {:.2}  brake {:.2}  clutch {:.2}  steer {:+.2}",
        velocity.0.length() * 2.237,
        vehicle.drivetrain.rpm(),
        controls.throttle,
        controls.brake,
        controls.clutch,
        controls.steer,
    );
    match readout.single_mut() {
        Ok(mut existing) => existing.0 = text,
        Err(_) => {
            commands.spawn((
                Readout,
                Text::new(text),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(12.0),
                    top: Val::Px(12.0),
                    ..default()
                },
            ));
        }
    }
}
