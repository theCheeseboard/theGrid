use contemporary::components::button::button;
use contemporary::components::layer::layer;
use contemporary::styling::theme::Theme;
use emojis::Group;
use gpui::{
    App, Context, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use std::rc::Rc;

pub type EmojiSelectedListener = dyn Fn(&EmojiSelectedEvent, &mut Window, &mut App) + 'static;

#[derive(Clone)]
pub struct EmojiSelectedEvent {
    pub emoji: String,
}

pub struct EmojiFlyout {
    selected_group: Group,

    emoji_selected_listener: Option<Rc<Box<EmojiSelectedListener>>>,
}

impl EmojiFlyout {
    pub fn new(cx: &mut App) -> Self {
        Self {
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
}

impl Render for EmojiFlyout {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
            .child(
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
                                    cx.notify()
                                })),
                        )
                    }),
            )
            .child(
                self.selected_group.emojis().enumerate().fold(
                    div()
                        .id("emoji-selection-area")
                        .overflow_y_scroll()
                        .grid()
                        .grid_cols(10),
                    |david, (i, emoji)| {
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
    }
}
