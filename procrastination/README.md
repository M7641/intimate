# Procrastination

A driving simulator for learning to drive, built on Bevy.

## Layout

Dependencies point one way: `crates/*` know nothing about Bevy, `sim` adapts
them into the ECS without opening a window, and `app` is the only crate that
needs a GPU.

```
crates/vehicle   suspension, tyres, engine, clutch, gearbox. Pure Rust.
sim              Bevy plugins: casts the wheel rays, applies the forces.
app              window, scene, keyboard input, chase camera, HUD.
assets/          models, maps, scenarios, audio. Bevy's asset root.
docs/            how things work; start with assets.md.
```

Planned, not started: `crates/road` (lane graph from OpenDRIVE or OSM),
`crates/rules` (Highway Code checks), `crates/scenario` (lessons and scoring).

## Run

```
cargo run -p procrastination-app --features dev
```

The `dev` feature links Bevy dynamically for fast rebuilds. Drop it for a
release build.

## Keys

| Key              | Action                 |
| ---------------- | ---------------------- |
| Up / W           | throttle               |
| Down / S         | brake                  |
| Left, Right / A, D | steer                |
| C (hold)         | clutch                 |
| Left Shift       | gear up                |
| Left Ctrl        | gear down              |
| Space            | handbrake              |
| E                | restart a stalled engine |

The car starts in neutral with the engine running. Hold the clutch, select
first, then release the clutch gently while adding throttle. Dumping the
clutch at idle stalls it.

## Test

```
cargo nextest run
```

The vehicle crate carries the model's unit and property tests and builds in
seconds. The sim and app crates pull in Bevy.
