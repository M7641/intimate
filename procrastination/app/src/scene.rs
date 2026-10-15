//! A short suburban street with one car. Kenney's CC0 kits, scaled up from
//! their toy proportions. See `docs/assets.md` for how models come in.

use std::f32::consts::{FRAC_PI_2, PI};

use avian3d::prelude::*;
use bevy::prelude::*;
use procrastination_sim::vehicle::{Vehicle, VehicleControls, WheelHub};
use procrastination_vehicle::VehicleSpec;

use crate::camera::ChaseCamera;

/// Marks the car the player drives.
#[derive(Component)]
pub struct Player;

/// Kenney's car models ship with their wheels as child nodes. The sim
/// drives its own wheels, so those nodes are hidden once the scene has
/// spawned under an entity carrying this marker. Hidden rather than
/// despawned: the scene spawner still has commands queued for them.
#[derive(Component)]
pub struct HideWheelNodes;

const GROUND_SIZE: f32 = 400.0;
/// Kenney's sedan has a 1.32 unit wheelbase; 1.6x makes it 2.1 m.
const CAR_SCALE: f32 = 1.6;
/// The suburban kit's houses are about 1.3 units wide; 8x makes them houses.
const PROP_SCALE: f32 = 8.0;
const BODY_SIZE: Vec3 = Vec3::new(2.4, 1.2, 4.0);

const CAR_KIT: &str = "models/kenney/car-kit";
const CITY_KIT: &str = "models/kenney/city-kit-suburban";

pub fn spawn(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Name::new("Ground"),
        Mesh3d(meshes.add(Plane3d::default().mesh().size(GROUND_SIZE, GROUND_SIZE))),
        MeshMaterial3d(materials.add(Color::srgb(0.3, 0.3, 0.32))),
        RigidBody::Static,
        Collider::cuboid(GROUND_SIZE, 1.0, GROUND_SIZE),
        // The collider is a slab whose top face sits at y = 0.
        Transform::from_xyz(0.0, -0.5, 0.0),
    ));

    spawn_street(&mut commands, &asset_server);
    spawn_car(&mut commands, &asset_server);

    commands.spawn((
        Name::new("Sun"),
        DirectionalLight {
            illuminance: 8000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::default().looking_at(Vec3::new(-1.0, -2.0, -1.0), Vec3::Y),
    ));

    commands.spawn((
        Name::new("Camera"),
        Camera3d::default(),
        ChaseCamera::default(),
        Transform::from_xyz(0.0, 4.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

/// The player's car: physics body from the spec, visuals from the car kit.
fn spawn_car(commands: &mut Commands, asset_server: &AssetServer) {
    let spec = VehicleSpec::sedan();
    let mut vehicle = Vehicle::new(spec.clone());
    vehicle.start_engine();
    let wheels: Vec<Vec3> = spec.wheels.iter().map(|w| w.attach).collect();
    let ground_offset = spec.ground_offset();

    let car = commands
        .spawn((
            Name::new("Car"),
            Player,
            Transform::from_xyz(0.0, 1.0, 0.0),
            Visibility::default(),
            RigidBody::Dynamic,
            Collider::cuboid(BODY_SIZE.x, BODY_SIZE.y, BODY_SIZE.z),
            Mass(spec.mass),
            vehicle,
            VehicleControls::default(),
        ))
        .id();

    commands.entity(car).with_children(|parent| {
        // The model's origin is at ground level and it faces +Z, so drop it
        // to where the ground sits in the body frame and turn it around.
        parent.spawn((
            Name::new("Body model"),
            HideWheelNodes,
            scene(asset_server, &format!("{CAR_KIT}/sedan.glb")),
            Transform::from_xyz(0.0, ground_offset, 0.0)
                .with_rotation(Quat::from_rotation_y(PI))
                .with_scale(Vec3::splat(CAR_SCALE)),
        ));

        for (index, attach) in wheels.into_iter().enumerate() {
            parent
                .spawn((
                    Name::new(format!("Wheel {index}")),
                    WheelHub {
                        vehicle: car,
                        index,
                    },
                    Transform::from_translation(attach),
                    Visibility::default(),
                ))
                .with_children(|hub| {
                    hub.spawn((
                        scene(asset_server, &format!("{CAR_KIT}/wheel-default.glb")),
                        Transform::from_scale(Vec3::splat(CAR_SCALE)),
                    ));
                });
        }
    });
}

/// Houses either side of the road, trees between them, cones down the middle.
fn spawn_street(commands: &mut Commands, asset_server: &AssetServer) {
    let houses = ["a", "b", "c", "d", "e", "f"];
    for (i, house) in houses.iter().enumerate() {
        let z = -30.0 - i as f32 * 22.0;
        for (x, yaw) in [(-18.0, FRAC_PI_2), (18.0, -FRAC_PI_2)] {
            static_prop(
                commands,
                asset_server,
                &format!("{CITY_KIT}/building-type-{house}.glb"),
                Vec3::new(x, 0.0, z),
                yaw,
            );
        }
        for x in [-10.0, 10.0] {
            let tree = if i % 2 == 0 {
                "tree-large"
            } else {
                "tree-small"
            };
            static_prop(
                commands,
                asset_server,
                &format!("{CITY_KIT}/{tree}.glb"),
                Vec3::new(x, 0.0, z - 11.0),
                0.0,
            );
        }
    }

    // A slalom of cones the car can knock over.
    for i in 0..8 {
        let x = if i % 2 == 0 { -1.5 } else { 1.5 };
        commands.spawn((
            Name::new(format!("Cone {i}")),
            scene(asset_server, &format!("{CAR_KIT}/cone.glb")),
            Transform::from_xyz(x, 0.0, -20.0 - i as f32 * 12.0).with_scale(Vec3::splat(CAR_SCALE)),
            RigidBody::Dynamic,
            ColliderConstructorHierarchy::new(ColliderConstructor::ConvexHullFromMesh),
            Mass(2.0),
        ));
    }
}

fn static_prop(
    commands: &mut Commands,
    asset_server: &AssetServer,
    path: &str,
    position: Vec3,
    yaw: f32,
) {
    commands.spawn((
        Name::new(path.rsplit('/').next().unwrap_or(path).to_string()),
        scene(asset_server, path),
        Transform::from_translation(position)
            .with_rotation(Quat::from_rotation_y(yaw))
            .with_scale(Vec3::splat(PROP_SCALE)),
        RigidBody::Static,
        ColliderConstructorHierarchy::new(ColliderConstructor::TrimeshFromMesh),
    ));
}

fn scene(asset_server: &AssetServer, path: &str) -> WorldAssetRoot {
    WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(path.to_string())))
}

/// Hide `wheel-*` nodes as they appear under a `HideWheelNodes` entity.
pub fn hide_wheel_nodes(
    mut commands: Commands,
    named: Query<(Entity, &Name), Added<Name>>,
    parents: Query<&ChildOf>,
    markers: Query<(), With<HideWheelNodes>>,
) {
    for (entity, name) in &named {
        if !name.as_str().starts_with("wheel-") {
            continue;
        }
        if parents
            .iter_ancestors(entity)
            .any(|ancestor| markers.contains(ancestor))
        {
            commands.entity(entity).insert(Visibility::Hidden);
        }
    }
}
