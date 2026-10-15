//! Keyboard driving, for development before a wheel is plugged in.
//!
//! Up/W throttle, Down/S brake, Left/Right or A/D steer, C clutch,
//! Shift gear up, Ctrl gear down, Space handbrake, E restart the engine.
//! Steering ramps rather than snapping, so a tap is a small input.

use bevy::prelude::*;
use procrastination_sim::vehicle::{Vehicle, VehicleControls};

use crate::scene::Player;

const STEER_RATE: f32 = 2.5;
const STEER_RETURN_RATE: f32 = 4.0;
const PEDAL_RATE: f32 = 4.0;

pub fn keyboard(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&mut VehicleControls, &mut Vehicle), With<Player>>,
) {
    let dt = time.delta_secs();
    for (mut controls, mut vehicle) in &mut query {
        let held = |a, b| keys.pressed(a) || keys.pressed(b);

        let steer_target = match (
            held(KeyCode::ArrowLeft, KeyCode::KeyA),
            held(KeyCode::ArrowRight, KeyCode::KeyD),
        ) {
            (true, false) => -1.0,
            (false, true) => 1.0,
            _ => 0.0,
        };
        let rate = if steer_target == 0.0 {
            STEER_RETURN_RATE
        } else {
            STEER_RATE
        };
        controls.steer = approach(controls.steer, steer_target, rate * dt);

        let throttle_target = if held(KeyCode::ArrowUp, KeyCode::KeyW) {
            1.0
        } else {
            0.0
        };
        let brake_target = if held(KeyCode::ArrowDown, KeyCode::KeyS) {
            1.0
        } else {
            0.0
        };
        controls.throttle = approach(controls.throttle, throttle_target, PEDAL_RATE * dt);
        controls.brake = approach(controls.brake, brake_target, PEDAL_RATE * dt);

        let clutch_target = if keys.pressed(KeyCode::KeyC) {
            1.0
        } else {
            0.0
        };
        controls.clutch = approach(controls.clutch, clutch_target, PEDAL_RATE * dt);
        controls.handbrake = if keys.pressed(KeyCode::Space) {
            1.0
        } else {
            0.0
        };

        let top_gear = vehicle.spec.gear_ratios.len() as i8;
        if keys.just_pressed(KeyCode::ShiftLeft) {
            controls.gear = (controls.gear + 1).min(top_gear);
        }
        if keys.just_pressed(KeyCode::ControlLeft) {
            controls.gear = (controls.gear - 1).max(-1);
        }

        if keys.just_pressed(KeyCode::KeyE) && !vehicle.drivetrain.running {
            vehicle.start_engine();
        }
    }
}

fn approach(current: f32, target: f32, max_step: f32) -> f32 {
    current + (target - current).clamp(-max_step, max_step)
}
