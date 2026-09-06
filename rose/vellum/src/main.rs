//! vellum — a documentation site rendered entirely inside the Bevy game engine.
//!
//! There is no HTML and no DOM. Every heading, paragraph, and nav link is an
//! ECS entity, laid out by Bevy's flexbox UI. The content lives in one `Site`
//! resource and the UI is a pure projection of it (see `ui.rs`). The same binary
//! runs natively (`cargo run`) and, compiled to wasm, in a browser tab
//! (`trunk serve`), drawing through WebGPU — including a compute-shader
//! background (`field.rs`) that WebGL2 could not run.

mod content;
mod field;
mod theme;
mod ui;

use bevy::prelude::*;

use content::Site;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "vellum — docs in a game engine".into(),
                // On the web, size the canvas to its parent element.
                fit_canvas_to_parent: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(field::FieldComputePlugin)
        .insert_resource(ClearColor(theme::BACKGROUND))
        .insert_resource(Site::sample())
        .add_systems(Startup, ui::setup)
        .add_systems(Update, (ui::render_on_change, ui::scroll_content))
        .run();
}
