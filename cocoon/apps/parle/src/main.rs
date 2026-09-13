//! parle — a native desktop language-learning app built on GPUI (Zed's UI
//! framework). Two pages for now: a dictionary backed by a free translation
//! API, and a story generator backed by a small local LLM.

mod dictionary;
mod gguf_tokenizer;
mod lang;
mod llm;
mod shell;
mod story;
mod text_input;
mod theme;

use gpui::{
    App, Application, Bounds, Focusable, KeyBinding, TitlebarOptions, WindowBounds, WindowOptions,
    actions, prelude::*, px, size,
};

use shell::Shell;

actions!(parle, [Quit]);

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    Application::new().run(|cx: &mut App| {
        cx.bind_keys(text_input::key_bindings());
        cx.bind_keys([
            KeyBinding::new("ctrl-q", Quit, None),
            KeyBinding::new("cmd-q", Quit, None),
        ]);
        cx.on_action(|_: &Quit, cx| cx.quit());

        // Quit once the last window is gone: on macOS apps otherwise linger
        // in the dock with no window.
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(760.), px(560.)), cx);
        cx.open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("parle".into()),
                    ..Default::default()
                }),
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                let shell = cx.new(Shell::new);
                let input = shell.read(cx).dictionary.read(cx).input().clone();
                window.focus(&input.focus_handle(cx));
                shell
            },
        )
        .expect("failed to open the main window");
        cx.activate(true);
    });
}
