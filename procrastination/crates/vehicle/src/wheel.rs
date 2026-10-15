//! One wheel's step: from a ray hit and the body's motion to a force on the
//! body. This is the seam between the pure model and whatever physics engine
//! casts the rays.

use glam::Vec3;

use crate::{Controls, VehicleSpec, WheelSpec, suspension, tyre};

/// What the physics engine measured for this wheel this step.
#[derive(Debug, Clone, Copy)]
pub struct WheelInput {
    /// Distance from the attachment point to the ground along the ray, if
    /// the ray hit within `suspension_rest + radius`.
    pub hit_distance: Option<f32>,
    /// Body velocity at the attachment point, world space.
    pub velocity: Vec3,
    /// Body axes in world space.
    pub up: Vec3,
    pub forward: Vec3,
    pub right: Vec3,
    /// Drive force this wheel should push with, newtons, from the axle.
    pub drive_force: f32,
}

/// What to apply to the body, and the wheel's pose for rendering.
#[derive(Debug, Clone, Copy, Default)]
pub struct WheelOutput {
    /// Force to apply at `attach` in world space.
    pub force: Vec3,
    /// Vertical load, for the drivetrain and for audio.
    pub load: f32,
}

/// Per-wheel state carried between steps.
#[derive(Debug, Clone, Copy, Default)]
pub struct WheelState {
    /// Spring compression at the end of the last step, metres.
    pub compression: f32,
    /// Where the wheel centre sits below the attachment point, metres.
    pub drop: f32,
    /// Rolling angular speed, rad/s, positive forward.
    pub spin: f32,
    /// Accumulated rotation about the axle, radians, for rendering.
    pub angle: f32,
    /// Steering angle applied this step, radians.
    pub steer_angle: f32,
}

impl WheelState {
    /// Compute this wheel's force and update its state.
    pub fn step(
        &mut self,
        spec: &VehicleSpec,
        wheel: &WheelSpec,
        controls: &Controls,
        input: WheelInput,
        dt: f32,
    ) -> WheelOutput {
        self.steer_angle = if wheel.steered {
            controls.steer.clamp(-1.0, 1.0) * spec.max_steer
        } else {
            0.0
        };
        let Some(distance) = input.hit_distance else {
            self.compression = 0.0;
            self.drop = spec.suspension_rest;
            self.spin *= 0.99;
            self.angle += self.spin * dt;
            return WheelOutput::default();
        };

        let compression =
            (spec.suspension_rest + wheel.radius - distance).clamp(0.0, spec.suspension_rest);
        let closing_speed = if dt > 0.0 {
            (compression - self.compression) / dt
        } else {
            0.0
        };
        self.compression = compression;
        self.drop = spec.suspension_rest - compression;

        let load = suspension::force(
            spec.suspension_stiffness,
            spec.suspension_damping,
            compression,
            closing_speed,
        );

        let (rolling, lateral) =
            steered_axes(input.forward, input.right, input.up, self.steer_angle);
        let forward_speed = input.velocity.dot(rolling);
        let side_speed = input.velocity.dot(lateral);
        self.spin = forward_speed / wheel.radius;
        self.angle += self.spin * dt;

        let brake = brake_share(spec, wheel, controls);
        let patch = tyre::force(
            tyre::Contact {
                load,
                grip: spec.grip,
                lateral_stiffness: spec.lateral_stiffness,
                forward_speed,
                side_speed,
            },
            input.drive_force,
            brake,
            dt,
        );

        WheelOutput {
            force: input.up * load + rolling * patch.x + lateral * patch.y,
            load,
        }
    }
}

/// Rotate the body's forward and right axes about its up axis by the steer
/// angle. Positive steer turns to the right.
fn steered_axes(forward: Vec3, right: Vec3, up: Vec3, angle: f32) -> (Vec3, Vec3) {
    let (sin, cos) = angle.sin_cos();
    let rolling = (forward * cos + right * sin).normalize();
    let lateral = rolling.cross(up).normalize();
    (rolling, lateral)
}

fn brake_share(spec: &VehicleSpec, wheel: &WheelSpec, controls: &Controls) -> f32 {
    let foot = controls.brake.clamp(0.0, 1.0) * spec.brake_force / spec.wheels.len() as f32;
    let hand = if wheel.handbraked {
        let handbraked = spec.wheels.iter().filter(|w| w.handbraked).count().max(1);
        controls.handbrake.clamp(0.0, 1.0) * spec.handbrake_force / handbraked as f32
    } else {
        0.0
    };
    foot + hand
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resting_input(spec: &VehicleSpec, wheel: &WheelSpec) -> WheelInput {
        WheelInput {
            hit_distance: Some(spec.suspension_rest + wheel.radius - 0.08),
            velocity: Vec3::ZERO,
            up: Vec3::Y,
            forward: Vec3::NEG_Z,
            right: Vec3::X,
            drive_force: 0.0,
        }
    }

    #[test]
    fn airborne_wheel_applies_nothing() {
        let spec = VehicleSpec::sedan();
        let wheel = spec.wheels[0];
        let mut state = WheelState::default();
        let input = WheelInput {
            hit_distance: None,
            ..resting_input(&spec, &wheel)
        };
        let out = state.step(&spec, &wheel, &Controls::default(), input, 1.0 / 60.0);
        assert_eq!(out.force, Vec3::ZERO);
        assert_eq!(state.drop, spec.suspension_rest);
    }

    #[test]
    fn resting_wheel_pushes_up() {
        let spec = VehicleSpec::sedan();
        let wheel = spec.wheels[0];
        let mut state = WheelState {
            compression: 0.08,
            ..Default::default()
        };
        let out = state.step(
            &spec,
            &wheel,
            &Controls::default(),
            resting_input(&spec, &wheel),
            1.0 / 60.0,
        );
        assert!((out.force.y - 0.08 * spec.suspension_stiffness).abs() < 1e-2);
        assert!(out.force.x.abs() < 1e-3 && out.force.z.abs() < 1e-3);
    }

    #[test]
    fn steering_right_rotates_rolling_axis_right() {
        let (rolling, lateral) = steered_axes(Vec3::NEG_Z, Vec3::X, Vec3::Y, 0.5);
        assert!(rolling.x > 0.0);
        assert!(rolling.z < 0.0);
        assert!(lateral.dot(rolling).abs() < 1e-6);
        assert!(lateral.x > 0.0);
    }
}
