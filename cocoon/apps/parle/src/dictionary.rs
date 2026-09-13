//! Dictionary page: type a word, get its translation.
//!
//! Backed by MyMemory (<https://mymemory.translated.net>), which is free and
//! needs no key for a few thousand characters a day. It returns a best match
//! plus the translation-memory entries it was drawn from, which we show as
//! alternatives — surprisingly useful for a learner.

use gpui::{Context, Entity, SharedString, Subscription, Window, div, prelude::*};
use serde::Deserialize;

use crate::lang::{self, Lang};
use crate::text_input::{TextInput, TextInputEvent};
use crate::theme;

pub struct DictionaryPage {
    input: Entity<TextInput>,
    from: Lang,
    to: Lang,
    state: State,
    _submit: Subscription,
}

enum State {
    Idle,
    Loading(String),
    Done(Translation),
    Error(String),
}

/// What we keep from MyMemory's response.
#[derive(Clone)]
pub struct Translation {
    pub word: String,
    pub text: String,
    /// (translation, quality 0-100) from the translation memory.
    pub matches: Vec<(String, String)>,
}

impl DictionaryPage {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| TextInput::new(cx, "a word or short phrase…"));
        let _submit = cx.subscribe(&input, |this, _, ev, cx| match ev {
            TextInputEvent::Submit => this.lookup(cx),
        });
        Self {
            input,
            from: lang::LANGS[0],
            to: lang::LANGS[1],
            state: State::Idle,
            _submit,
        }
    }

    pub fn input(&self) -> &Entity<TextInput> {
        &self.input
    }

    fn lookup(&mut self, cx: &mut Context<Self>) {
        let word = self.input.read(cx).text().trim().to_string();
        if word.is_empty() {
            return;
        }
        let (from, to) = (self.from, self.to);
        self.state = State::Loading(word.clone());
        cx.notify();

        // Blocking HTTP on a background thread; the result hops back to the
        // UI thread through the weak handle, which fails harmlessly if the
        // page has gone away in the meantime.
        let fetch = cx
            .background_executor()
            .spawn(async move { translate(&word, from, to) });
        cx.spawn(async move |this, cx| {
            let result = fetch.await;
            this.update(cx, |page, cx| {
                page.state = match result {
                    Ok(t) => State::Done(t),
                    Err(e) => State::Error(e.to_string()),
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn swap(&mut self, cx: &mut Context<Self>) {
        std::mem::swap(&mut self.from, &mut self.to);
        cx.notify();
    }
}

impl Render for DictionaryPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let controls = div()
            .flex()
            .items_end()
            .gap_3()
            .child(
                div().flex().flex_col().gap_1().child(theme::caption("From")).child(theme::button(
                    "from",
                    self.from.name,
                    false,
                    cx.listener(|this, _, _, cx| {
                        this.from = lang::next(this.from);
                        cx.notify();
                    }),
                )),
            )
            .child(theme::button("swap", "⇄", false, cx.listener(|this, _, _, cx| this.swap(cx))))
            .child(
                div().flex().flex_col().gap_1().child(theme::caption("To")).child(theme::button(
                    "to",
                    self.to.name,
                    false,
                    cx.listener(|this, _, _, cx| {
                        this.to = lang::next(this.to);
                        cx.notify();
                    }),
                )),
            )
            .child(div().flex_1())
            .child(theme::button(
                "translate",
                "Translate",
                true,
                cx.listener(|this, _, _, cx| this.lookup(cx)),
            ));

        let result: gpui::AnyElement = match &self.state {
            State::Idle => div()
                .text_color(theme::SUBTEXT)
                .child("Type a word and press Enter.")
                .into_any_element(),
            State::Loading(w) => div()
                .text_color(theme::SUBTEXT)
                .child(format!("Looking up “{w}”…"))
                .into_any_element(),
            State::Error(e) => div().text_color(theme::RED).child(e.clone()).into_any_element(),
            State::Done(t) => div()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(div().text_sm().text_color(theme::SUBTEXT).child(t.word.clone()))
                        .child(div().text_3xl().text_color(theme::GREEN).child(t.text.clone())),
                )
                .when(!t.matches.is_empty(), |el| {
                    el.child(theme::caption("Also seen as")).children(t.matches.iter().map(
                        |(alt, quality)| {
                            div()
                                .flex()
                                .gap_3()
                                .items_baseline()
                                .child(div().child(SharedString::from(alt.clone())))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme::SUBTEXT)
                                        .child(format!("quality {quality}")),
                                )
                        },
                    ))
                })
                .into_any_element(),
        };

        div()
            .flex()
            .flex_col()
            .gap_5()
            .size_full()
            .child(controls)
            .child(self.input.clone())
            .child(div().id("dict-result").flex_1().min_h_0().overflow_y_scroll().child(result))
    }
}

// ── MyMemory client ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct Response {
    #[serde(rename = "responseData")]
    data: ResponseData,
    #[serde(rename = "responseStatus")]
    status: serde_json::Value,
    #[serde(rename = "responseDetails", default)]
    details: String,
    #[serde(default)]
    matches: Vec<Match>,
}

#[derive(Deserialize)]
struct ResponseData {
    #[serde(rename = "translatedText")]
    translated_text: String,
}

#[derive(Deserialize)]
struct Match {
    translation: String,
    /// Quality is sometimes a number, sometimes a string; keep it loose.
    #[serde(default)]
    quality: serde_json::Value,
}

fn translate(word: &str, from: Lang, to: Lang) -> anyhow::Result<Translation> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;
    let body = client
        .get("https://api.mymemory.translated.net/get")
        .query(&[("q", word), ("langpair", &format!("{}|{}", from.code, to.code))])
        .send()?
        .error_for_status()?
        .text()?;
    let resp: Response = serde_json::from_str(&body).map_err(|e| anyhow::anyhow!("MyMemory reply: {e}"))?;

    // MyMemory reports its own errors with HTTP 200 and a status field.
    if resp.status.as_i64() != Some(200) && resp.status.as_str() != Some("200") {
        anyhow::bail!("MyMemory: {}", resp.details);
    }

    let text = resp.data.translated_text;
    let matches = resp
        .matches
        .into_iter()
        .map(|m| (m.translation, m.quality.to_string().trim_matches('"').to_string()))
        .filter(|(t, _)| !t.eq_ignore_ascii_case(&text) && !t.trim().is_empty())
        .take(6)
        .collect();

    Ok(Translation { word: word.to_string(), text, matches })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hits the real API; run with `cargo test -p parle -- --ignored`.
    #[test]
    #[ignore = "needs network"]
    fn translates_a_word() {
        let t = translate("butterfly", lang::LANGS[0], lang::LANGS[1]).unwrap();
        assert_eq!(t.text.to_lowercase(), "papillon");
    }
}
