//! Palette and a couple of shared element builders. Catppuccin Mocha, because
//! it is easy on the eyes and everyone already knows what it looks like.

use gpui::{App, ClickEvent, Div, ElementId, Rgba, Stateful, Window, div, prelude::*};

/// `gpui::rgb` is not const, so build the struct directly.
const fn hex(v: u32) -> Rgba {
    Rgba {
        r: ((v >> 16) & 0xff) as f32 / 255.,
        g: ((v >> 8) & 0xff) as f32 / 255.,
        b: (v & 0xff) as f32 / 255.,
        a: 1.,
    }
}

pub const BASE: Rgba = hex(0x1e1e2e);
pub const MANTLE: Rgba = hex(0x181825);
pub const SURFACE: Rgba = hex(0x313244);
pub const SURFACE_HI: Rgba = hex(0x45475a);
pub const TEXT: Rgba = hex(0xcdd6f4);
pub const SUBTEXT: Rgba = hex(0x9399b2);
pub const BLUE: Rgba = hex(0x89b4fa);
pub const LAVENDER: Rgba = hex(0xb4befe);
pub const SAPPHIRE: Rgba = hex(0x74c7ec);
pub const GREEN: Rgba = hex(0xa6e3a1);
pub const RED: Rgba = hex(0xf38ba8);
pub const PEACH: Rgba = hex(0xfab387);

/// A filled, clickable button. `primary` gets the accent colour; the rest are
/// muted surface buttons for secondary actions like cycling a language.
pub fn button(
    id: impl Into<ElementId>,
    label: impl Into<gpui::SharedString>,
    primary: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let (bg, fg, hover, active) = if primary {
        (BLUE, BASE, LAVENDER, SAPPHIRE)
    } else {
        (SURFACE, TEXT, SURFACE_HI, SURFACE)
    };
    div()
        .id(id)
        .px_4()
        .py_2()
        .rounded_md()
        .bg(bg)
        .text_color(fg)
        .text_sm()
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .active(move |s| s.bg(active))
        .on_click(on_click)
        .child(label.into())
}

/// Small uppercase caption above a control.
pub fn caption(text: impl Into<gpui::SharedString>) -> Div {
    div().text_xs().text_color(SUBTEXT).child(text.into())
}
