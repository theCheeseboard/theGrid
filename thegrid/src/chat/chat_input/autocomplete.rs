use crate::chat::chat_input::{AutocompleteOption, AutocompleteState, ChatInput};
use crate::chat::chat_room::open_room::OpenRoom;
use contemporary::components::layer::layer;
use contemporary::styling::theme::{ThemeStorage, VariableColor};
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AsyncApp, Context, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    ScrollStrategy, StatefulInteractiveElement, Styled, UniformListScrollHandle, WeakEntity,
    Window, div, px, uniform_list,
};
use matrix_sdk::RoomMemberships;
use std::cmp::Ordering;
use std::ops::Range;
use std::rc::Rc;
use thegrid_common::mxc_image::{SizePolicy, mxc_image};
use thegrid_common::tokio_helper::TokioHelper;

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
                                    AutocompleteOption::User {
                                        user_id,
                                        avatar_url,
                                        display_name,
                                    } => div()
                                        .id(i)
                                        .flex()
                                        .items_center()
                                        .w_full()
                                        .p(px(2.))
                                        .gap(px(8.))
                                        .child(
                                            mxc_image(avatar_url)
                                                .fallback_image(user_id.clone())
                                                .rounded(theme.border_radius)
                                                .size_policy(SizePolicy::Constrain(32., 32.)),
                                        )
                                        .child(
                                            div().flex().flex_col().child(display_name).child(
                                                div()
                                                    .text_color(theme.foreground.disabled())
                                                    .child(user_id.to_string()),
                                            ),
                                        ),
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
                .track_scroll(scroll_handle.read(cx))
                .h(px(200.)),
            )
    }
}

pub fn calculate_autocomplete(
    chat_input: &mut ChatInput,
    epoch: u32,
    typed: String,
    room: WeakEntity<OpenRoom>,
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
    } else if last_word.starts_with("@") {
        chat_input.autocomplete_state = AutocompleteState::Loading;

        cx.spawn(
            async move |weak_chat_input: WeakEntity<ChatInput>, cx: &mut AsyncApp| {
                let state =
                    calculate_user_id_autocomplete(last_word, room, last_word_range, cx).await;
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

pub async fn calculate_user_id_autocomplete(
    typed: String,
    room: WeakEntity<OpenRoom>,
    range: Range<usize>,
    cx: &mut AsyncApp,
) -> AutocompleteState {
    let Some(room) = room
        .read_with(cx, |room, _| room.room.clone())
        .ok()
        .flatten()
    else {
        return AutocompleteState::Idle;
    };

    let Ok(room_members) = cx
        .spawn_tokio(async move { room.members(RoomMemberships::JOIN).await })
        .await
    else {
        return AutocompleteState::Idle;
    };

    let lowercase_typed = typed.trim_start_matches("@").to_lowercase();
    let options: Vec<_> = room_members
        .iter()
        .filter(|member| {
            member.user_id().as_str().starts_with(&typed)
                || member.display_name().is_some_and(|display_name| {
                    display_name
                        .replace(|c: char| c.is_whitespace(), "")
                        .to_lowercase()
                        .contains(&lowercase_typed)
                })
        })
        .map(|member| AutocompleteOption::User {
            user_id: member.user_id().to_owned(),
            avatar_url: member.avatar_url().map(|mxc_uri| mxc_uri.to_owned()),
            display_name: member
                .display_name()
                .map(|display_name| display_name.to_string())
                .unwrap_or(member.user_id().to_string()),
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
