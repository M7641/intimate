//! The "stylesheet": every colour and font size in one place.
//!
//! In a browser these would be CSS custom properties; here they are just
//! constants the `ui` module reaches for when spawning nodes.

use bevy::prelude::Color;

// — palette (a calm dark docs theme) —
pub const BACKGROUND: Color = Color::srgb(0.055, 0.059, 0.075);
pub const SIDEBAR: Color = Color::srgb(0.086, 0.094, 0.118);

/// A translucent wash drawn over the reading column. The GPU compute field
/// (`field.rs`) renders behind the whole UI; this lets it glow faintly through
/// while keeping body text legible. Same hue as `BACKGROUND`, partly see-through.
pub const CONTENT_SCRIM: Color = Color::srgba(0.055, 0.059, 0.075, 0.82);
pub const NAV_ACTIVE: Color = Color::srgb(0.18, 0.22, 0.33);
pub const CODE_BG: Color = Color::srgb(0.12, 0.13, 0.16);

pub const TEXT: Color = Color::srgb(0.86, 0.88, 0.92);
pub const MUTED: Color = Color::srgb(0.55, 0.58, 0.66);
pub const ACCENT: Color = Color::srgb(0.55, 0.78, 1.0);

// — type scale (logical pixels) —
pub const H1: f32 = 30.0;
pub const H2: f32 = 21.0;
pub const BODY: f32 = 16.0;
pub const NAV: f32 = 15.0;
