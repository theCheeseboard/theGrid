use cntp_i18n::tr;
use contemporary::components::layer::layer;
use contemporary::components::{button::button, text_field::TextField};
use contemporary::styling::theme::Theme;
use emojis::{Emoji, Group};
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use std::rc::Rc;

pub type EmojiSelectedListener = dyn Fn(&EmojiSelectedEvent, &mut Window, &mut App) + 'static;

#[derive(Clone)]
pub struct EmojiSelectedEvent {
    pub emoji: String,
}

pub struct EmojiFlyout {
    search_field: Entity<TextField>,
    selected_group: Group,
    visible_emoji: Vec<&'static Emoji>,

    emoji_selected_listener: Option<Rc<Box<EmojiSelectedListener>>>,
}

impl EmojiFlyout {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let text_changed_listener = Rc::new(cx.listener(|this, _, _, cx| {
            this.update_visible_emoji(cx);
        }));

        Self {
            search_field: cx.new(|cx| {
                let mut text_field = TextField::new("search-field", cx);
                text_field.set_placeholder(&tr!("SEARCH", "Search...").to_string().as_str());
                text_field.on_text_changed({
                    let text_changed_listener = text_changed_listener.clone();
                    move |event, window, cx| {
                        let event = event.clone();
                        let text_changed_listener = text_changed_listener.clone();
                        window.defer(cx, move |window, cx| {
                            text_changed_listener(&event, window, cx)
                        });
                    }
                });
                text_field
            }),
            visible_emoji: Group::SmileysAndEmotion.emojis().collect(),
            selected_group: Group::SmileysAndEmotion,
            emoji_selected_listener: None,
        }
    }

    pub fn set_emoji_selected_listener(
        &mut self,
        listener: impl Fn(&EmojiSelectedEvent, &mut Window, &mut App) + 'static,
    ) {
        self.emoji_selected_listener = Some(Rc::new(Box::new(listener)));
    }

    pub fn update_visible_emoji(&mut self, cx: &mut Context<Self>) {
        let search_query = self.search_field.read(cx).text().to_lowercase();
        if search_query.is_empty() {
            self.visible_emoji = self.selected_group.emojis().collect();
        } else {
            self.visible_emoji = emojis::iter()
                .filter(|emoji| {
                    emoji
                        .shortcodes()
                        .any(|shortcode| shortcode.to_lowercase().contains(&search_query))
                        || emoji.name().to_lowercase().contains(&search_query)
                })
                .collect();
        }
    }
}

impl Render for EmojiFlyout {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>();

        div()
            .bg(theme.background)
            .rounded(theme.border_radius)
            .w(px(300.))
            .h(px(300.))
            .border(px(1.))
            .border_color(theme.border_color)
            .occlude()
            .flex()
            .flex_col()
            .child(self.search_field.clone())
            .child(
                self.visible_emoji.iter().enumerate().fold(
                    div()
                        .id("emoji-selection-area")
                        .overflow_y_scroll()
                        .grid()
                        .grid_cols(10),
                    |david, (i, emoji)| {
                        let emoji = *emoji;
                        david.child(button(i).flat().child(emoji.as_str()).on_click(cx.listener(
                            move |this, _, window, cx| {
                                if let Some(emoji_selected_listener) = &this.emoji_selected_listener
                                {
                                    emoji_selected_listener(
                                        &EmojiSelectedEvent {
                                            emoji: emoji.as_str().to_string(),
                                        },
                                        window,
                                        cx,
                                    );
                                }
                                cx.notify()
                            },
                        )))
                    },
                ),
            )
            .when(self.search_field.read(cx).text().is_empty(), |david| {
                david.child(
                    Group::iter()
                        .enumerate()
                        .fold(layer().flex(), |layer, (i, group)| {
                            layer.child(
                                button(i)
                                    .flat()
                                    .child(group.emojis().next().unwrap().as_str())
                                    .checked_when(self.selected_group == group)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.selected_group = group;
                                        this.update_visible_emoji(cx);
                                        cx.notify()
                                    })),
                            )
                        }),
                )
            })
    }
}
