//! The window's root view: a header with two tabs, and whichever page is
//! active underneath. Both pages live for the life of the window so switching
//! tabs keeps their state.

use gpui::{Context, Entity, Window, div, prelude::*};

use crate::dictionary::DictionaryPage;
use crate::story::StoryPage;
use crate::theme;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Dictionary,
    Story,
}

pub struct Shell {
    page: Page,
    pub dictionary: Entity<DictionaryPage>,
    story: Entity<StoryPage>,
}

impl Shell {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            page: Page::Dictionary,
            dictionary: cx.new(DictionaryPage::new),
            story: cx.new(StoryPage::new),
        }
    }

    fn tab(&self, page: Page, label: &'static str, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.page == page;
        div()
            .id(label)
            .px_3()
            .py_1()
            .rounded_md()
            .text_sm()
            .cursor_pointer()
            .when(active, |s| s.bg(theme::SURFACE).text_color(theme::TEXT))
            .when(!active, |s| s.text_color(theme::SUBTEXT).hover(|s| s.text_color(theme::TEXT)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.page = page;
                cx.notify();
            }))
            .child(label)
    }
}

impl Render for Shell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let header = div()
            .flex()
            .items_center()
            .gap_2()
            .px_6()
            .py_3()
            .bg(theme::MANTLE)
            .border_b_1()
            .border_color(theme::SURFACE)
            .child(div().text_lg().text_color(theme::LAVENDER).mr_4().child("parle"))
            .child(self.tab(Page::Dictionary, "Dictionary", cx))
            .child(self.tab(Page::Story, "Story", cx));

        let body = div().flex_1().min_h_0().p_6().child(match self.page {
            Page::Dictionary => self.dictionary.clone().into_any_element(),
            Page::Story => self.story.clone().into_any_element(),
        });

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme::BASE)
            .text_color(theme::TEXT)
            .font_family("Geist Variable")
            .child(header)
            .child(body)
    }
}
