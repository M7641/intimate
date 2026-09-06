//! The documentation content, modelled as plain data.
//!
//! A docs site is, at heart, a list of pages where each page is a sequence of
//! blocks. Keeping this independent of Bevy means the UI layer (`ui.rs`) is the
//! only thing that touches entities — this module is the "source content" you
//! would eventually load from Markdown, a CMS, or a build step.

use bevy::prelude::*;

/// One renderable element of a page. The `ui` module maps each variant to a
/// styled `Text` node, the way a Markdown renderer maps `#`, prose, and fenced
/// blocks to HTML elements.
pub enum Block {
    Heading(String),
    Paragraph(String),
    Bullet(String),
    Code(String),
}

/// A single documentation page: a title (also used as its nav label) and its
/// ordered blocks.
pub struct Page {
    pub title: String,
    pub blocks: Vec<Block>,
}

/// The whole site plus which page is currently open.
///
/// `current` is the only mutable state in the application. Changing it is what
/// drives a full UI rebuild — see [`crate::ui::render_on_change`].
#[derive(Resource)]
pub struct Site {
    pub pages: Vec<Page>,
    pub current: usize,
}

// Terse constructors so `sample()` below reads like the document it describes.
fn h(s: &str) -> Block {
    Block::Heading(s.into())
}
fn p(s: &str) -> Block {
    Block::Paragraph(s.into())
}
fn li(s: &str) -> Block {
    Block::Bullet(s.into())
}
fn code(s: &str) -> Block {
    Block::Code(s.into())
}

impl Site {
    /// Hard-coded sample documentation so the prototype has something real to
    /// render. Swap this for a loader later without touching the UI layer.
    pub fn sample() -> Self {
        Self {
            current: 0,
            pages: vec![
                Page {
                    title: "Overview".into(),
                    blocks: vec![
                        p("vellum is a documentation site rendered entirely inside the \
                           Bevy game engine. There is no HTML and no DOM: every heading, \
                           paragraph, and navigation link you see is an ECS entity."),
                        p("The same binary runs natively on the desktop and, compiled to \
                           WebAssembly, inside a browser tab — drawing onto a WebGL2 canvas."),
                        h("Why do this?"),
                        li("To explore UI as data: the page is a pure projection of a resource."),
                        li("To get game-engine capabilities — animation, particles, shaders — \
                            available to ordinary documentation."),
                        li("Because a website that is also a render loop is a fun place to live."),
                    ],
                },
                Page {
                    title: "Architecture".into(),
                    blocks: vec![
                        p("vellum follows a single rule: UI = render(state). All content lives \
                           in one `Site` resource, and the visible layout is rebuilt from it \
                           whenever it changes."),
                        h("The render cycle"),
                        li("`content.rs` holds the pages as plain data, free of any Bevy types."),
                        li("`ui::setup` spawns a 2D camera and one empty root node."),
                        li("`ui::render_on_change` detects a changed `Site` and rebuilds the tree."),
                        h("Switching pages"),
                        p("A nav link is a `Button` carrying a `NavLink(index)`. Clicking it runs \
                           an observer that sets `site.current`. That mutation flips the resource's \
                           change tick, the render system fires, and the page is rebuilt from scratch."),
                        code("observe(|_: On<Pointer<Click>>, mut site: ResMut<Site>| {\n    site.current = i;\n});"),
                    ],
                },
                Page {
                    title: "Running it".into(),
                    blocks: vec![
                        h("Native"),
                        code("cargo run"),
                        h("In the browser"),
                        p("Add the wasm target and a static-asset server, then serve:"),
                        code("rustup target add wasm32-unknown-unknown\ncargo install trunk\ntrunk serve --open"),
                        p("Trunk compiles the crate to wasm, wires it into `index.html`, and \
                           reloads on change. Bevy creates the canvas and fills the page."),
                    ],
                },
            ],
        }
    }
}
