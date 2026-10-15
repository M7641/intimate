//! Runs the raycast-car model against Avian. Each fixed step: cast one ray
//! per wheel, hand the hit and the body motion to the vehicle crate, and
//! apply the force it returns at the wheel's attachment point.

use avian3d::prelude::*;
use bevy::{ecs::query::QueryData, prelude::*};
use procrastination_vehicle::{Controls, Drivetrain, VehicleSpec, WheelInput, WheelState};

pub struct VehiclePlugin;

impl Plugin for VehiclePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, step_vehicles)
            .add_systems(Update, place_wheel_hubs);
    }
}

/// A drivable vehicle. Attach to an entity that also has a dynamic
/// `RigidBody`, a `Collider` and a `Mass`.
#[derive(Component)]
pub struct Vehicle {
    pub spec: VehicleSpec,
    pub drivetrain: Drivetrain,
    pub wheels: Vec<WheelState>,
}

impl Vehicle {
    pub fn new(spec: VehicleSpec) -> Self {
        let wheels = vec![WheelState::default(); spec.wheels.len()];
        Self {
            spec,
            drivetrain: Drivetrain::default(),
            wheels,
        }
    }

    pub fn start_engine(&mut self) {
        self.drivetrain.start(&self.spec);
    }
}

/// The driver's inputs for this frame. Written by whatever owns the input
/// device: keyboard, wheel, or an AI.
#[derive(Component, Default, Deref, DerefMut)]
pub struct VehicleControls(pub Controls);

/// Marks a child entity as the hub of wheel `index` on `vehicle`. The sim
/// positions it each frame; attach a mesh under it to see the wheel.
#[derive(Component)]
pub struct WheelHub {
    pub vehicle: Entity,
    pub index: usize,
}

/// Everything one vehicle's step reads and writes on the body.
#[derive(QueryData)]
#[query_data(mutable)]
struct VehicleBody {
    entity: Entity,
    vehicle: &'static mut Vehicle,
    controls: &'static VehicleControls,
    centre_of_mass: &'static ComputedCenterOfMass,
    /// Also the only route to the body's pose and velocity: the helper
    /// writes velocity, so reading the components alongside it conflicts.
    forces: Forces,
}

fn step_vehicles(time: Res<Time>, spatial: SpatialQuery, mut vehicles: Query<VehicleBody>) {
    let dt = time.delta_secs();
    for body in &mut vehicles {
        let VehicleBodyItem {
            entity,
            mut vehicle,
            controls,
            centre_of_mass,
            mut forces,
        } = body;
        let position = forces.position().0;
        let rotation = forces.rotation().0;
        let linear = forces.linear_velocity();
        let angular = forces.angular_velocity();
        let drive_force = axle_force(&mut vehicle, controls, dt);
        let Vehicle { spec, wheels, .. } = &mut *vehicle;
        let up = rotation * Vec3::Y;
        let forward = rotation * Vec3::NEG_Z;
        let right = rotation * Vec3::X;
        let centre = position + rotation * centre_of_mass.0;
        let filter = SpatialQueryFilter::from_excluded_entities([entity]);

        for (index, wheel) in spec.wheels.iter().enumerate() {
            let attach = position + rotation * wheel.attach;
            let ray_length = spec.suspension_rest + wheel.radius;
            let hit_distance = Dir3::new(-up)
                .ok()
                .and_then(|dir| spatial.cast_ray(attach, dir, ray_length, true, &filter))
                .map(|hit| hit.distance);
            let velocity = linear + angular.cross(attach - centre);

            let input = WheelInput {
                hit_distance,
                velocity,
                up,
                forward,
                right,
                drive_force: if wheel.driven { drive_force } else { 0.0 },
            };
            let output = wheels[index].step(spec, wheel, &controls.0, input, dt);
            forces.apply_force_at_point(output.force, attach);
        }
    }
}

/// Step the drivetrain and return the drive force per driven wheel.
fn axle_force(vehicle: &mut Vehicle, controls: &VehicleControls, dt: f32) -> f32 {
    let spec = &vehicle.spec;
    let driven: Vec<usize> = (0..spec.wheels.len())
        .filter(|&i| spec.wheels[i].driven)
        .collect();
    if driven.is_empty() {
        return 0.0;
    }
    let wheel_speed =
        driven.iter().map(|&i| vehicle.wheels[i].spin).sum::<f32>() / driven.len() as f32;
    let axle_torque = vehicle.drivetrain.step(spec, &controls.0, wheel_speed, dt);
    let radius = spec.wheels[driven[0]].radius;
    axle_torque / radius / driven.len() as f32
}

/// Move each wheel hub to where its wheel sits: dropped by the suspension
/// travel, turned by the steering, and spun by the rolling speed.
fn place_wheel_hubs(vehicles: Query<&Vehicle>, mut hubs: Query<(&WheelHub, &mut Transform)>) {
    for (hub, mut transform) in &mut hubs {
        let Ok(vehicle) = vehicles.get(hub.vehicle) else {
            continue;
        };
        let (Some(spec), Some(state)) = (
            vehicle.spec.wheels.get(hub.index),
            vehicle.wheels.get(hub.index),
        ) else {
            continue;
        };
        transform.translation = spec.attach - Vec3::Y * state.drop;
        transform.rotation =
            Quat::from_rotation_y(-state.steer_angle) * Quat::from_rotation_x(-state.angle);
    }
}
