# Putting 3D assets into the world

How a model goes from a file on disk to an object the car can drive past,
or into. Written for Bevy 0.19 and avian3d 0.7, the versions this project
pins.

## The format is glTF

Bevy loads `.glb` (one binary file) and `.gltf` (JSON plus side files).
Nothing else is worth using. Every asset you buy or download either ships
as glTF or exports to it from Blender in one step.

Files live under `assets/`, Bevy's asset root. Paths in code are relative
to it: `models/car.glb` means `assets/models/car.glb`.

Bevy would normally look for `assets/` beside the crate being run, which
is `app/assets`. The workspace's `.cargo/config.toml` sets
`BEVY_ASSET_ROOT` to the workspace root so the one folder serves every
crate. A shipped binary expects `assets/` beside the executable.

## A visual is one component

A glTF file holds one or more scenes. Load a scene as an asset, attach it
to an entity, and Bevy spawns the node tree beneath it: meshes, materials,
textures and the transforms from the file.

```rust
commands.spawn((
    Name::new("Traffic light"),
    WorldAssetRoot(asset_server.load(
        GltfAssetLabel::Scene(0).from_asset("models/traffic_light.glb"),
    )),
    Transform::from_xyz(12.0, 0.0, -30.0),
));
```

That is a complete static prop. Loading is asynchronous, so it appears a
frame or two later. `Scene(0)` is the first scene in the file; Kenney's
kits have one scene per file.

## Collision is a separate decision

A mesh does not collide on its own. Two options:

**A hand-placed primitive.** `Collider::cuboid`, `Collider::cylinder`,
and so on, sized by eye. Cheap, predictable, and what real driving sims
use for vehicles and most props. The car uses one.

**Derived from the mesh.** `ColliderConstructorHierarchy` walks the
spawned node tree and builds a collider per mesh:

```rust
commands.spawn((
    WorldAssetRoot(asset_server.load(
        GltfAssetLabel::Scene(0).from_asset("models/road_bend.glb"),
    )),
    RigidBody::Static,
    ColliderConstructorHierarchy::new(ColliderConstructor::TrimeshFromMesh),
));
```

Pick the shape class by how the object moves:

| Constructor                    | Use for                   | Trade-off                     |
| ------------------------------ | ------------------------- | ----------------------------- |
| `TrimeshFromMesh`              | static scenery, roads     | exact, but must never move    |
| `ConvexHullFromMesh`           | dynamic props             | fast, bulges over concavities |
| `ConvexDecompositionFromMesh`  | dynamic concave objects   | accurate, slow to build       |

## The car

Keep the physics as it is: the cuboid collider, the mass, the `Vehicle`
component. Replace only the visual. Spawn the body glTF as a child of the
car entity, and a wheel glTF under each `WheelHub` instead of the cylinder.
The sim already poses the hubs, so the wheels drop, steer and spin for
free.

The attachment points in `VehicleSpec` must match where the model's wheels
sit. Read them off the model once and write them into the spec. For the
Kenney sedan at 1.6x: wheel nodes at `x = ±0.3, z = ±0.66` in the file
become `±0.48, ±1.056` in the spec, and the 0.3 wheel radius becomes 0.48.
`VehicleSpec::ground_offset` gives where to put a model whose origin is
at ground level so it lines up with the physics body.

Kenney's car files also contain the wheels as child nodes. The scene
hides any node named `wheel-*` under the body model, since the sim poses
its own wheels. Reading a `.glb`'s node names and translations is a few
lines of Python over its JSON chunk; see `scene.rs` for what was read.

## Scale

Kenney's kits are toy-scaled, and the kits are not consistent with each
other: a car is about 2.5 units long, a suburban house about 1.3 units
wide. The scene scales cars by 1.6 and props by 8. Colliders derived from
meshes follow the entity's scale.

## Export conventions

Every mismatch shows up as a car floating above its wheels or driving
sideways. The vehicle crate assumes the glTF frame: metres, `+Y` up,
`-Z` forward, `+X` right.

From Blender:

- Work in metres, scene unit scale 1.0.
- Apply scale and rotation before export (`Ctrl+A`).
- Keep the exporter's default `+Y Up`. It converts Blender's Z-up.
- Put the body's origin at the centre of the wheelbase, at axle height.
- Put each wheel's origin at its hub, axle along `X`.

## Where assets come from

- **Kenney** (kenney.nl): car kit, city kit, road kit. Free, CC0, already
  glTF. Low-poly and consistent. Enough to build a town.
- **Poly Haven**: materials, textures, HDRI skies. CC0.
- **Sketchfab**, **Synty**: higher-fidelity cars and props, mostly paid.
- **Blender**: anything you adjust or make.

Keep files under 5 MB each; the pre-commit hook rejects larger ones and
the repo has no LFS. Kenney's models are tens of kilobytes.

## Later: Blender as the level editor

Placing props by editing coordinates in `scene.rs` does not scale. Lay out
an area in Blender, export one glTF, spawn it as a single scene, and let
`ColliderConstructorHierarchy` give every mesh a collider. Tools such as
Blenvy go further and let a Blender object carry Bevy components, so a
traffic light placed in Blender spawns as a working traffic light. Worth
it once the road crate exists and there is something to place things
along.
