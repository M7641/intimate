//! Story page: pick a language, describe a story, and a small local model
//! writes it. The model is loaded lazily on the first request and then kept
//! for the life of the app; a worker thread streams text back to the view.

use std::sync::{Arc, Mutex};

use futures::StreamExt;
use futures::channel::mpsc;
use gpui::{Context, Entity, Subscription, Window, div, prelude::*};

use crate::lang::{self, Lang};
use crate::llm::{self, Model};
use crate::text_input::{TextInput, TextInputEvent};
use crate::theme;

pub struct StoryPage {
    prompt: Entity<TextInput>,
    lang: Lang,
    story: String,
    status: Status,
    /// `None` until the first story is requested.
    model: Arc<Mutex<Option<Model>>>,
    _submit: Subscription,
}

enum Status {
    Idle,
    Working(String),
    Error(String),
}

/// Messages from the worker thread to the view.
enum Event {
    Status(String),
    Text(String),
    Done,
    Failed(String),
}

impl StoryPage {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let prompt = cx.new(|cx| TextInput::new(cx, "a cat who is afraid of rain…"));
        let _submit = cx.subscribe(&prompt, |this, _, ev, cx| match ev {
            TextInputEvent::Submit => this.write(cx),
        });
        Self {
            prompt,
            lang: lang::LANGS[1],
            story: String::new(),
            status: Status::Idle,
            model: Arc::new(Mutex::new(None)),
            _submit,
        }
    }

    fn busy(&self) -> bool {
        matches!(self.status, Status::Working(_))
    }

    fn write(&mut self, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let request = self.prompt.read(cx).text().trim().to_string();
        if request.is_empty() {
            return;
        }
        self.story.clear();
        self.status = Status::Working("starting…".into());
        cx.notify();

        let prompt = llm::story_prompt(self.lang.name, &request);
        let model = self.model.clone();
        let (tx, mut rx) = mpsc::unbounded::<Event>();

        // A plain OS thread rather than GPUI's background pool: inference
        // pins a core for seconds and would otherwise starve other work.
        std::thread::Builder::new()
            .name("parle-llm".into())
            .spawn(move || {
                let send = |ev: Event| {
                    tx.unbounded_send(ev).ok();
                };
                let result = (|| -> anyhow::Result<()> {
                    let mut guard = model.lock().map_err(|_| anyhow::anyhow!("model lock poisoned"))?;
                    if guard.is_none() {
                        *guard = Some(Model::load(&mut |s| send(Event::Status(s)))?);
                    }
                    send(Event::Status("writing…".into()));
                    let model = guard.as_mut().expect("model loaded above");
                    model.generate(&prompt, 320, 0.7, &mut |text| send(Event::Text(text.to_string())))
                })();
                send(match result {
                    Ok(()) => Event::Done,
                    Err(e) => Event::Failed(format!("{e:#}")),
                });
            })
            .expect("spawn llm thread");

        // Pump events into the view on the UI thread until the sender drops.
        cx.spawn(async move |this, cx| {
            while let Some(ev) = rx.next().await {
                let alive = this.update(cx, |page, cx| {
                    match ev {
                        Event::Status(s) => page.status = Status::Working(s),
                        Event::Text(t) => page.story = t,
                        Event::Done => page.status = Status::Idle,
                        Event::Failed(e) => page.status = Status::Error(e),
                    }
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
    }
}

impl Render for StoryPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let busy = self.busy();
        let controls = div()
            .flex()
            .items_end()
            .gap_3()
            .child(
                div().flex().flex_col().gap_1().child(theme::caption("Language")).child(theme::button(
                    "lang",
                    self.lang.name,
                    false,
                    cx.listener(|this, _, _, cx| {
                        this.lang = lang::next(this.lang);
                        cx.notify();
                    }),
                )),
            )
            .child(div().flex_1())
            .child(theme::button(
                "write",
                if busy { "Writing…" } else { "Write a story" },
                !busy,
                cx.listener(|this, _, _, cx| this.write(cx)),
            ));

        let status = match &self.status {
            Status::Idle if self.story.is_empty() => div()
                .text_color(theme::SUBTEXT)
                .child("Describe a story and press Enter. The first run downloads a 675 MB model."),
            Status::Idle => div(),
            Status::Working(s) => div().text_color(theme::PEACH).child(s.clone()),
            Status::Error(e) => div().text_color(theme::RED).child(e.clone()),
        };

        div()
            .flex()
            .flex_col()
            .gap_5()
            .size_full()
            .child(controls)
            .child(self.prompt.clone())
            .child(status)
            .child(
                div()
                    .id("story-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .text_lg()
                    .line_height(gpui::px(28.))
                    .child(self.story.clone()),
            )
    }
}
