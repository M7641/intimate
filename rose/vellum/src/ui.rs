//! The only module that knows about entities. It turns the `Site` resource into
//! a tree of `bevy_ui` nodes and keeps that tree in sync with the resource.
//!
//! The whole layer obeys one rule: `UI = render(state)`. Nothing mutates nodes
//! in place; when the state changes we tear the tree down and build it again.

use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

use crate::content::{Block, Site};
use crate::theme;

/// Marks the single root node the whole UI lives under, so the render system
/// can clear and rebuild its children whenever the `Site` changes.
#[derive(Component)]
pub struct UiRoot;

/// The scrollable column holding the current page's body.
#[derive(Component)]
pub struct ContentPane;

/// A clickable nav entry carrying the page index it selects.
#[derive(Component)]
pub struct NavLink(pub usize);

/// Spawn the camera and an empty root shell. `render_on_change` fills it in on
/// the very first frame, because a freshly inserted resource counts as changed.
pub fn setup(mut commands: Commands) {
    // bevy_ui draws through a 2D camera; without one, nothing appears.
    commands.spawn(Camera2d);
    commands.spawn((
        UiRoot,
        Node {
            width: percent(100),
            height: percent(100),
            ..default()
        },
    ));
}

/// When the `Site` resource changes, wipe the root's children and rebuild the
/// sidebar and content pane from scratch. This is the engine of `UI = render`.
pub fn render_on_change(
    mut commands: Commands,
    site: Res<Site>,
    root: Single<(Entity, Option<&Children>), With<UiRoot>>,
) {
    if !site.is_changed() {
        return;
    }
    let (root, children) = root.into_inner();
    if let Some(children) = children {
        for &child in children {
            // Despawning a child despawns its whole subtree (and its observers).
            commands.entity(child).despawn();
        }
    }
    spawn_sidebar(&mut commands, root, &site);
    spawn_content(&mut commands, root, &site);
}

/// The left navigation column: a wordmark plus one button per page.
fn spawn_sidebar(commands: &mut Commands, root: Entity, site: &Site) {
    let sidebar = commands
        .spawn((
            ChildOf(root),
            Node {
                width: px(240),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(20)),
                row_gap: px(4),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(theme::SIDEBAR),
        ))
        .id();

    commands.spawn((
        ChildOf(sidebar),
        Text::new("vellum"),
        TextFont {
            font_size: theme::H2,
            ..default()
        },
        TextColor(theme::ACCENT),
        Node {
            margin: UiRect::bottom(px(16)),
            ..default()
        },
    ));

    for (i, page) in site.pages.iter().enumerate() {
        let active = i == site.current;
        let link = commands
            .spawn((
                ChildOf(sidebar),
                NavLink(i),
                Button,
                Node {
                    padding: UiRect::axes(px(10), px(8)),
                    border_radius: BorderRadius::all(px(6)),
                    ..default()
                },
                BackgroundColor(if active { theme::NAV_ACTIVE } else { Color::NONE }),
            ))
            // The whole point of the prototype: a click reads the index off the
            // entity it fired on and just mutates state. `render_on_change`
            // notices the changed resource and redraws everything.
            .observe(
                |click: On<Pointer<Click>>, links: Query<&NavLink>, mut site: ResMut<Site>| {
                    if let Ok(link) = links.get(click.event_target()) {
                        site.current = link.0;
                    }
                },
            )
            .id();

        commands.spawn((
            ChildOf(link),
            Text::new(page.title.clone()),
            TextFont {
                font_size: theme::NAV,
                ..default()
            },
            TextColor(if active { theme::TEXT } else { theme::MUTED }),
        ));
    }
}

/// The right reading column: the page title as an H1, then each block. The
/// column scrolls (`Overflow::scroll_y`) so long pages stay reachable.
fn spawn_content(commands: &mut Commands, root: Entity, site: &Site) {
    let pane = commands
        .spawn((
            ChildOf(root),
            ContentPane,
            Node {
                flex_grow: 1.0,
                height: percent(100),
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(px(48), px(40)),
                row_gap: px(14),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            // A translucent scrim so the GPU field behind the UI shows through
            // faintly without hurting text legibility.
            BackgroundColor(theme::CONTENT_SCRIM),
            ScrollPosition(Vec2::ZERO),
        ))
        .id();

    let page = &site.pages[site.current];
    commands.spawn((
        ChildOf(pane),
        Text::new(page.title.clone()),
        TextFont {
            font_size: theme::H1,
            ..default()
        },
        TextColor(theme::TEXT),
    ));

    for block in &page.blocks {
        spawn_block(commands, pane, block);
    }
}

/// Map one content block to its styled node(s) — the docs equivalent of a
/// Markdown renderer turning `##`, prose, and fenced code into HTML elements.
fn spawn_block(commands: &mut Commands, pane: Entity, block: &Block) {
    // A comfortable reading measure shared by every text block.
    const MEASURE: Val = Val::Px(720.0);

    match block {
        Block::Heading(text) => {
            commands.spawn((
                ChildOf(pane),
                Text::new(text.clone()),
                TextFont {
                    font_size: theme::H2,
                    ..default()
                },
                TextColor(theme::TEXT),
                Node {
                    margin: UiRect::top(px(12)),
                    ..default()
                },
            ));
        }
        Block::Paragraph(text) => {
            commands.spawn((
                ChildOf(pane),
                Text::new(text.clone()),
                TextFont {
                    font_size: theme::BODY,
                    ..default()
                },
                TextColor(theme::MUTED),
                Node {
                    max_width: MEASURE,
                    ..default()
                },
            ));
        }
        Block::Bullet(text) => {
            commands.spawn((
                ChildOf(pane),
                Text::new(format!("\u{2022}  {text}")),
                TextFont {
                    font_size: theme::BODY,
                    ..default()
                },
                TextColor(theme::TEXT),
                Node {
                    max_width: MEASURE,
                    ..default()
                },
            ));
        }
        Block::Code(text) => {
            let card = commands
                .spawn((
                    ChildOf(pane),
                    Node {
                        padding: UiRect::all(px(14)),
                        max_width: MEASURE,
                        border_radius: BorderRadius::all(px(8)),
                        ..default()
                    },
                    BackgroundColor(theme::CODE_BG),
                ))
                .id();
            commands.spawn((
                ChildOf(card),
                Text::new(text.clone()),
                TextFont {
                    font_size: theme::BODY,
                    ..default()
                },
                TextColor(theme::ACCENT),
            ));
        }
    }
}

/// Translate vertical mouse-wheel input into scrolling of the content pane.
/// In Bevy 0.18 buffered input arrives as messages, read via `MessageReader`.
/// `Option<Single<..>>` keeps this a no-op on the first frame, before the pane
/// has been spawned.
pub fn scroll_content(
    mut wheel: MessageReader<MouseWheel>,
    pane: Option<Single<&mut ScrollPosition, With<ContentPane>>>,
) {
    let Some(mut pane) = pane else {
        return;
    };
    const LINE_HEIGHT: f32 = 22.0;
    for ev in wheel.read() {
        let dy = match ev.unit {
            MouseScrollUnit::Line => ev.y * LINE_HEIGHT,
            MouseScrollUnit::Pixel => ev.y,
        };
        pane.0.y = (pane.0.y - dy).max(0.0);
    }
}
