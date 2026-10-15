//! Vehicle dynamics for a learner-driver simulator.
//!
//! The model is a "raycast car": the body is one rigid body, and each wheel
//! is a ray cast downward from its attachment point. The crate computes the
//! force each wheel applies to the body given what the ray hit and how the
//! body is moving. It never touches a physics engine or an ECS; the caller
//! casts the rays, supplies the velocities, and applies the forces.
//!
//! Units are SI throughout: metres, seconds, kilograms, newtons, radians.
//! Rotational speeds are rad/s unless a name says `rpm`.

pub mod drivetrain;
pub mod spec;
pub mod suspension;
pub mod tyre;
pub mod wheel;

pub use drivetrain::Drivetrain;
pub use spec::{VehicleSpec, WheelSpec};
pub use wheel::{WheelInput, WheelOutput, WheelState};

/// What the driver is doing. Every axis is normalised to `0..=1` or `-1..=1`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Controls {
    /// Accelerator pedal, `0` released to `1` floored.
    pub throttle: f32,
    /// Brake pedal, `0` released to `1` floored.
    pub brake: f32,
    /// Clutch pedal, `0` released (engaged) to `1` pressed (disengaged).
    pub clutch: f32,
    /// Steering, `-1` full left to `1` full right.
    pub steer: f32,
    /// Selected gear: `-1` reverse, `0` neutral, `1..` forward gears.
    pub gear: i8,
    /// Handbrake, `0` released to `1` fully applied.
    pub handbrake: f32,
}

/// Standard gravity, used to turn a wheel load back into a supported mass.
pub const GRAVITY: f32 = 9.81;
