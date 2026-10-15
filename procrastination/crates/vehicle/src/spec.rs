//! Static description of a vehicle. Tuning data, not state.

use glam::Vec3;
use serde::{Deserialize, Serialize};

/// One wheel's attachment and geometry, in the body's local frame.
///
/// The body frame is right-handed with `+Y` up, `-Z` forward (the glTF and
/// Bevy convention), so a wheel on the front left sits at negative X and
/// negative Z.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WheelSpec {
    /// Where the suspension attaches to the body, in body space.
    pub attach: Vec3,
    /// Wheel radius.
    pub radius: f32,
    /// Whether this wheel receives drive torque.
    pub driven: bool,
    /// Whether this wheel turns with the steering input.
    pub steered: bool,
    /// Whether the handbrake acts on this wheel.
    pub handbraked: bool,
}

/// The complete tuning sheet for one vehicle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VehicleSpec {
    /// Total mass of the body (the wheels are massless).
    pub mass: f32,
    pub wheels: Vec<WheelSpec>,

    // Suspension
    /// Spring length with no load. The ray length is this plus the radius.
    pub suspension_rest: f32,
    /// Spring stiffness, N/m, per wheel.
    pub suspension_stiffness: f32,
    /// Damper coefficient, N·s/m, per wheel.
    pub suspension_damping: f32,

    // Tyres
    /// Peak friction coefficient. Forces saturate at `grip * load`.
    pub grip: f32,
    /// Lateral stiffness, N per (m/s) of sideslip per N of load.
    pub lateral_stiffness: f32,
    /// Maximum steering lock at the wheel, radians.
    pub max_steer: f32,
    /// Brake force at full pedal, split across all wheels.
    pub brake_force: f32,
    /// Handbrake force at full lever, split across the handbraked wheels.
    pub handbrake_force: f32,

    // Drivetrain
    /// Peak engine torque, N·m. The curve shape is fixed in `drivetrain`.
    pub engine_torque: f32,
    pub idle_rpm: f32,
    pub redline_rpm: f32,
    /// Below this the engine stalls when it is loaded.
    pub stall_rpm: f32,
    /// Engine rotational inertia, kg·m².
    pub engine_inertia: f32,
    /// Maximum torque the clutch can transmit when fully engaged.
    pub clutch_torque: f32,
    /// Forward gear ratios, first gear first.
    pub gear_ratios: Vec<f32>,
    pub reverse_ratio: f32,
    pub final_drive: f32,
}

impl VehicleSpec {
    /// A small saloon. The wheel geometry is read off the Kenney car-kit
    /// sedan at 1.6x scale (see `docs/assets.md`). The powertrain is tuned
    /// by feel to about three times a real supermini's acceleration: all
    /// four wheels driven, triple the torque, and stickier tyres so the
    /// torque reaches the road. A realistic supermini is 150 N·m, front
    /// wheel drive, grip 1.0, which does 0 to 30 mph in about 3.7 s.
    pub fn sedan() -> Self {
        let half_track = 0.48;
        let front = -1.056;
        let rear = 1.056;
        let attach_height = 0.0;
        let radius = 0.48;
        let wheel = |x: f32, z: f32, driven: bool, steered: bool| WheelSpec {
            attach: Vec3::new(x, attach_height, z),
            radius,
            driven,
            steered,
            handbraked: !steered,
        };
        Self {
            mass: 1200.0,
            wheels: vec![
                wheel(-half_track, front, true, true),
                wheel(half_track, front, true, true),
                wheel(-half_track, rear, true, false),
                wheel(half_track, rear, true, false),
            ],
            suspension_rest: 0.35,
            suspension_stiffness: 35_000.0,
            suspension_damping: 4_000.0,
            grip: 1.6,
            lateral_stiffness: 4.0,
            max_steer: 0.6,
            brake_force: 12_000.0,
            handbrake_force: 4_000.0,
            engine_torque: 450.0,
            idle_rpm: 850.0,
            redline_rpm: 6_500.0,
            stall_rpm: 400.0,
            engine_inertia: 0.2,
            clutch_torque: 900.0,
            gear_ratios: vec![3.6, 2.0, 1.4, 1.0, 0.8],
            reverse_ratio: 3.5,
            final_drive: 4.2,
        }
    }

    /// How far each spring compresses under the car's own weight, assuming
    /// the load is shared equally.
    pub fn static_sag(&self) -> f32 {
        self.mass * crate::GRAVITY / self.wheels.len() as f32 / self.suspension_stiffness
    }

    /// Where the ground sits in the body frame when the car is at rest:
    /// a negative `y`. Place a body model whose origin is at ground level
    /// here so it lines up with the physics body.
    pub fn ground_offset(&self) -> f32 {
        let wheel = &self.wheels[0];
        wheel.attach.y - self.suspension_rest + self.static_sag() - wheel.radius
    }

    /// Overall ratio from engine to wheels for a gear, `None` in neutral.
    pub fn ratio(&self, gear: i8) -> Option<f32> {
        let gearbox = match gear {
            0 => return None,
            -1 => self.reverse_ratio,
            g => *self.gear_ratios.get((g - 1) as usize)?,
        };
        let sign = if gear < 0 { -1.0 } else { 1.0 };
        Some(sign * gearbox * self.final_drive)
    }

    pub fn driven_wheel_count(&self) -> usize {
        self.wheels.iter().filter(|w| w.driven).count()
    }
}
