use crate::chat::chat_input::{AutocompleteOption, AutocompleteState, ChatInput};
use contemporary::components::layer::layer;
use contemporary::styling::theme::ThemeStorage;
use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, uniform_list, App, AsyncApp, Context, InteractiveElement,
    IntoElement, ParentElement, RenderOnce, ScrollStrategy, StatefulInteractiveElement,
    Styled, UniformListScrollHandle, WeakEntity, Window,
};
use std::cmp::Ordering;
use std::ops::Range;
use std::rc::Rc;

pub struct ApplyAutcompleteEvent {
    pub option: AutocompleteOption,
}

#[derive(IntoElement)]
pub struct AutocompleteList {
    options: Vec<AutocompleteOption>,
    current_option: usize,
    on_apply: Rc<Box<dyn Fn(&ApplyAutcompleteEvent, &mut Window, &mut App)>>,
}

pub fn autocomplete_list(
    options: Vec<AutocompleteOption>,
    current_option: usize,
    on_apply: impl Fn(&ApplyAutcompleteEvent, &mut Window, &mut App) + 'static,
) -> AutocompleteList {
    AutocompleteList {
        options,
        current_option,
        on_apply: Rc::new(Box::new(on_apply)),
    }
}

impl RenderOnce for AutocompleteList {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let options = self.options;
        let current_option = self.current_option;
        let on_apply = self.on_apply;

        let scroll_handle = window.use_state(cx, |_, _| UniformListScrollHandle::new());
        let old_current_option = window.use_state(cx, |_, _| current_option);
        match old_current_option.read(cx).cmp(&current_option) {
            Ordering::Less => {
                old_current_option.write(cx, current_option);
                scroll_handle
                    .read(cx)
                    .scroll_to_item(current_option, ScrollStrategy::Bottom);
            }
            Ordering::Greater => {
                old_current_option.write(cx, current_option);
                scroll_handle
                    .read(cx)
                    .scroll_to_item(current_option, ScrollStrategy::Top);
            }
            _ => {}
        }

        let theme = cx.theme();

        layer()
            .border(px(1.))
            .border_color(theme.border_color)
            .child(
                uniform_list(
                    "autocomplete-list",
                    options.len(),
                    move |range, window, cx| {
                        let theme = cx.theme();
                        range
                            .map(|i| {
                                let option = options[i].clone();
                                let on_apply = on_apply.clone();
                                match option.clone() {
                                    AutocompleteOption::Emoji { name, emoji } => div()
                                        .id(i)
                                        .flex()
                                        .items_center()
                                        .w_full()
                                        .p(px(2.))
                                        .gap(px(8.))
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .size(px(32.))
                                                .child(emoji),
                                        )
                                        .child(name),
                                }
                                .when(i == current_option, |david| {
                                    david.bg(theme.layer_background)
                                })
                                .hover(|david| david.bg(theme.layer_background))
                                .on_click(move |_, window, cx| {
                                    on_apply.clone()(
                                        &ApplyAutcompleteEvent {
                                            option: option.clone(),
                                        },
                                        window,
                                        cx,
                                    );
                                })
                            })
                            .collect()
                    },
                )
                .track_scroll(scroll_handle.read(cx).clone())
                .h(px(100.)),
            )
    }
}

pub fn calculate_autocomplete(
    chat_input: &mut ChatInput,
    epoch: u32,
    typed: String,
    cx: &mut Context<ChatInput>,
) {
    // split the typed string into words
    let words: Vec<&str> = typed.split_whitespace().collect();

    if words.is_empty() {
        chat_input.autocomplete_state = AutocompleteState::Idle;
        return;
    }

    if words.len() == 1 {
        // Could be a command
        let first_word = words.first().unwrap();
        if first_word.starts_with('/') {
            return;
        }
    }

    let last_word = words.last().unwrap();
    let start = last_word.as_ptr() as usize - typed.as_ptr() as usize;
    let end = start + last_word.len();
    let last_word_range = start..end;
    let last_word = last_word.to_string();
    if last_word.starts_with(':') && !last_word.ends_with(':') {
        chat_input.autocomplete_state = AutocompleteState::Loading;

        cx.spawn(
            async move |weak_chat_input: WeakEntity<ChatInput>, cx: &mut AsyncApp| {
                let state = calculate_emoji_autocomplete(last_word, last_word_range).await;
                let _ = weak_chat_input.update(cx, |chat_input, cx| {
                    if chat_input.autocomplete_epoch == epoch {
                        chat_input.autocomplete_state = state;
                        cx.notify();
                    }
                });
            },
        )
        .detach();
        return;
    }

    chat_input.autocomplete_state = AutocompleteState::Idle;
}

pub async fn calculate_emoji_autocomplete(typed: String, range: Range<usize>) -> AutocompleteState {
    let emoji_name = typed.trim_start_matches(":").to_lowercase();
    let options: Vec<_> = emojis::iter()
        .flat_map(|emoji| {
            emoji
                .shortcodes()
                .filter(|shortcode| shortcode.starts_with(&emoji_name))
                .map(|shortcode| AutocompleteOption::Emoji {
                    name: format!(":{}:", shortcode),
                    emoji: emoji.to_string(),
                })
        })
        .collect();

    if options.is_empty() {
        AutocompleteState::Idle
    } else {
        AutocompleteState::Available {
            options,
            replace_range: range,
            current_option: 0,
        }
    }
}
