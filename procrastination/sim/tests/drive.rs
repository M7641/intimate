//! Headless end-to-end checks: a car on a slab, no window, fixed steps.

use avian3d::prelude::*;
use std::time::Duration;

use bevy::{prelude::*, time::TimeUpdateStrategy};
use procrastination_sim::SimPlugin;
use procrastination_sim::vehicle::{Vehicle, VehicleControls};
use procrastination_vehicle::VehicleSpec;

const STEP: f32 = 1.0 / 64.0;

fn app_with_car() -> (App, Entity) {
    let mut app = App::new();
    // The same plugin set Avian uses for its own headless tests: its mesh
    // collider systems expect the asset and scene message types to exist.
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        AssetPlugin::default(),
        bevy::scene::ScenePlugin,
        bevy::mesh::MeshPlugin,
        SimPlugin,
    ));
    app.insert_resource(Time::<Fixed>::from_seconds(STEP as f64));
    // Each update advances exactly one fixed step, so step counts are exact.
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
        STEP,
    )));

    app.world_mut().spawn((
        RigidBody::Static,
        // Long enough that ten seconds at full throttle stays on it.
        Collider::cuboid(2000.0, 1.0, 2000.0),
        Transform::from_xyz(0.0, -0.5, 0.0),
    ));
    let spec = VehicleSpec::sedan();
    let mut vehicle = Vehicle::new(spec.clone());
    vehicle.start_engine();
    let car = app
        .world_mut()
        .spawn((
            RigidBody::Dynamic,
            Collider::cuboid(2.4, 1.2, 4.0),
            Mass(spec.mass),
            Transform::from_xyz(0.0, 0.8, 0.0),
            vehicle,
            VehicleControls::default(),
        ))
        .id();
    app.finish();
    app.cleanup();
    (app, car)
}

/// Advance the app by whole fixed steps.
fn run(app: &mut App, steps: usize) {
    for _ in 0..steps {
        app.update();
    }
}

fn position(app: &App, car: Entity) -> Vec3 {
    app.world().get::<Position>(car).unwrap().0
}

#[test]
fn settles_on_its_suspension() {
    let (mut app, car) = app_with_car();
    run(&mut app, 256);
    let spec = VehicleSpec::sedan();
    let expected = -spec.ground_offset();
    let y = position(&app, car).y;
    assert!(
        (y - expected).abs() < 0.03,
        "rest height {y}, expected {expected}"
    );
}

#[test]
fn first_gear_pulls_away_and_brakes_stop_it() {
    let (mut app, car) = app_with_car();
    run(&mut app, 128);
    // Select first and let the clutch out over 1.5 s with some throttle.
    let release_steps = 96;
    for step in 0..=release_steps {
        let mut controls = app.world_mut().get_mut::<VehicleControls>(car).unwrap();
        controls.gear = 1;
        controls.throttle = 0.4;
        controls.clutch = 1.0 - step as f32 / release_steps as f32;
        run(&mut app, 1);
    }
    run(&mut app, 192);
    let moved = position(&app, car);
    assert!(moved.z < -5.0, "car should drive forward (-Z), got {moved}");
    assert!(app.world().get::<Vehicle>(car).unwrap().drivetrain.running);

    {
        let mut controls = app.world_mut().get_mut::<VehicleControls>(car).unwrap();
        controls.throttle = 0.0;
        controls.brake = 1.0;
        controls.clutch = 1.0;
    }
    run(&mut app, 256);
    let speed = app.world().get::<LinearVelocity>(car).unwrap().0.length();
    assert!(speed < 0.1, "car should stop, speed {speed}");
}

/// Prints speed against time at full throttle. Run with
/// `cargo test -p procrastination-sim -- --ignored --nocapture`.
#[test]
#[ignore]
fn acceleration_profile() {
    let (mut app, car) = app_with_car();
    run(&mut app, 128);
    let release_steps = 32;
    for step in 0..=release_steps {
        let mut controls = app.world_mut().get_mut::<VehicleControls>(car).unwrap();
        controls.gear = 1;
        controls.throttle = 1.0;
        controls.clutch = 1.0 - step as f32 / release_steps as f32;
        run(&mut app, 1);
    }
    for second in 1..=10 {
        run(&mut app, 64);
        let speed = app.world().get::<LinearVelocity>(car).unwrap().0.length();
        let vehicle = app.world().get::<Vehicle>(car).unwrap();
        println!(
            "t={second:>2}s  {:>5.1} m/s  {:>5.1} mph  {:>5.0} rpm  running={}",
            speed,
            speed * 2.237,
            vehicle.drivetrain.rpm(),
            vehicle.drivetrain.running
        );
    }
}
