use crate::chat::chat_room::timeline_view::message_error_item::message_error_item;
use crate::chat::chat_room::timeline_view::reply_fragment::reply_fragment_in_reply_to;
use cntp_i18n::{i18n_manager, tr, Quote, I18N_MANAGER};
use contemporary::components::button::button;
use contemporary::components::context_menu::ContextMenuItem;
use contemporary::components::icon::icon;
use contemporary::components::spinner::spinner;
use contemporary::styling::theme::{Theme, ThemeStorage};
use directories::UserDirs;
use gpui::prelude::FluentBuilder;
use gpui::{
    canvas, div, point, px, rgba, AnyElement, App, AsyncApp,
    BorrowAppContext, Entity, IntoElement, ParentElement, Path, RenderOnce, Styled, Window,
};
use matrix_sdk::ruma::events::room::message::{
    FileMessageEventContent, FormattedBody, MessageType,
};
use matrix_sdk::ruma::OwnedUserId;
use matrix_sdk_ui::timeline::{MsgLikeContent, MsgLikeKind, Profile, TimelineDetails};
use std::fs::copy;
use thegrid_common::mxc_image::{mxc_image, SizePolicy};
use thegrid_common::session::media_cache::{MediaCacheEntry, MediaFile, MediaState};
use thegrid_common::session::session_manager::SessionManager;
use thegrid_text_rendering::TextView;

#[derive(IntoElement)]
pub struct TimelineMessageItem {
    content: MsgLikeContent,
    sender_profile: TimelineDetails<Profile>,
    sender: OwnedUserId,
}

pub fn timeline_message_item(
    content: MsgLikeContent,
    sender_profile: TimelineDetails<Profile>,
    sender: OwnedUserId,
) -> TimelineMessageItem {
    TimelineMessageItem {
        content,
        sender_profile,
        sender,
    }
}

impl RenderOnce for TimelineMessageItem {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();

        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx).clone();

        let reactions = self.content.reactions;
        div()
            .flex()
            .flex_col()
            .when_some(self.content.in_reply_to, |david, reply_details| {
                david.child(reply_fragment_in_reply_to(reply_details))
            })
            .child(match self.content.kind {
                MsgLikeKind::Message(message) => div().child(msgtype_to_message_line(
                    message.msgtype(),
                    self.sender,
                    self.sender_profile,
                    false,
                    window,
                    cx,
                )),
                MsgLikeKind::UnableToDecrypt(_) => div().child(message_error_item(
                    "exception",
                    tr!("MESSAGE_UNABLE_TO_DECRYPT", "Unable to decrypt"),
                    cx,
                )),
                _ => div(),
            })
            .when(!reactions.is_empty(), |david| {
                david.child(reactions.iter().fold(
                    div().flex().mt(px(4.)).gap(px(4.)),
                    |david, (reaction, reactees)| {
                        david.child(
                            div()
                                .flex()
                                .p(px(2.))
                                .gap(px(2.))
                                .border(px(1.))
                                .border_color(theme.border_color)
                                .when_else(
                                    reactees.contains_key(&client.user_id().unwrap().to_owned()),
                                    |david| david.bg(theme.info_accent_color),
                                    |david| david.bg(theme.layer_background),
                                )
                                .rounded(theme.border_radius)
                                .child(reaction.clone())
                                .child(i18n_manager!().locale.format_decimal(reactees.len())),
                        )
                    },
                ))
            })
    }
}

pub fn msgtype_to_message_line<'a>(
    msgtype: &MessageType,
    sender: OwnedUserId,
    sender_profile: TimelineDetails<Profile>,
    as_reply: bool,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement + 'a {
    let theme = cx.theme();
    match msgtype {
        MessageType::Emote(emote) => div()
            .flex()
            .items_center()
            .when_some(
                match sender_profile {
                    TimelineDetails::Ready(profile) => Some(profile),
                    _ => None,
                },
                |david, profile| {
                    david.child(
                        mxc_image(profile.avatar_url.clone())
                            .size(px(24.))
                            .size_policy(SizePolicy::Fit)
                            .rounded(theme.border_radius)
                            .fallback_image(sender)
                            .mr(px(2.)),
                    )
                },
            )
            .child(
                canvas(
                    |bounds, _, _| {
                        // TODO: RTL?
                        let mut path = Path::new(bounds.top_right());
                        path.line_to(point(bounds.left(), bounds.center().y));
                        path.line_to(bounds.bottom_right());
                        path
                    },
                    |_, path, window, cx| {
                        let theme = cx.theme();
                        window.paint_path(path, theme.layer_background)
                    },
                )
                .w(px(12.))
                .h(px(24.)),
            )
            .child(
                div()
                    .min_h(px(24.))
                    .bg(theme.layer_background)
                    .rounded_tr(theme.border_radius)
                    .rounded_br(theme.border_radius)
                    .flex()
                    .child(div().p(px(2.)).italic().child(emote.body.clone())),
            )
            .into_any_element(),
        MessageType::Image(image) => div()
            .child(
                mxc_image(image.source.clone())
                    .min_w(px(100.))
                    .min_h(px(30.))
                    .size_policy(SizePolicy::Constrain(500., 500.)),
            )
            .into_any_element(),
        MessageType::Text(text) => div()
            .child(text_message(
                as_reply,
                &text.body,
                &text.formatted,
                window,
                cx,
            ))
            .into_any_element(),
        MessageType::Notice(notice) => {
            let theme = cx.theme();

            div()
                .font_family(theme.monospaced_font_family.clone())
                .child(text_message(
                    as_reply,
                    &notice.body,
                    &notice.formatted,
                    window,
                    cx,
                ))
                .into_any_element()
        }
        MessageType::File(file) => {
            let file = file.clone();
            let media_file_entity = cx.update_global::<SessionManager, _>(|session_manager, cx| {
                let media_cache = session_manager.media();
                let media_cache_entry = MediaCacheEntry::from(file.source.clone());
                media_cache.media_file_lazy(media_cache_entry, cx)
            });
            let media_file_entity_2 = media_file_entity.clone();
            let media_file_entity_3 = media_file_entity.clone();
            let media_file = media_file_entity.read(cx);
            let theme = cx.global::<Theme>();

            let file_name = file.filename();

            div()
                .p(px(2.))
                .bg(rgba(0x00C8FF10))
                .rounded(theme.border_radius)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .child(
                            icon(
                                file.info
                                    .as_ref()
                                    .and_then(|file_info| file_info.mimetype.clone())
                                    .map(|mimetype| mimetype.replace("/", "-"))
                                    .unwrap_or("application-octet-stream".into()),
                            )
                            .size(24.),
                        )
                        .child(file.filename().to_string())
                        .child(match media_file.media_state {
                            MediaState::Idle | MediaState::Failed => button("download-button")
                                .child(icon("cloud-download"))
                                .on_click(move |_, _, cx| {
                                    download_file(file.clone(), media_file_entity.clone(), cx);
                                })
                                .into_any_element(),
                            MediaState::Loading => spinner().size(px(16.)).into_any_element(),
                            MediaState::Loaded(_) => button("open-button")
                                .child(icon("document-open"))
                                .with_menu(vec![
                                    ContextMenuItem::separator()
                                        .label(tr!(
                                            "FILE_OPEN_MENU_HEADER",
                                            "For downloaded file {{filename}}",
                                            filename:Quote = file_name
                                        ))
                                        .build(),
                                    ContextMenuItem::menu_item()
                                        .label(tr!("FILE_OPEN", "Open"))
                                        .icon("document-open")
                                        .on_triggered(move |_, _, cx| {
                                            let media_file = media_file_entity_2.read(cx);

                                            let MediaState::Loaded(media_file) =
                                                &media_file.media_state
                                            else {
                                                return;
                                            };

                                            cx.open_with_system(media_file.path())
                                        })
                                        .build(),
                                    ContextMenuItem::menu_item()
                                        .label(tr!("FILE_SAVE_AS", "Save As..."))
                                        .icon("document-save-as")
                                        .on_triggered(move |_, _, cx| {
                                            save_file(
                                                file.clone(),
                                                media_file_entity_3.clone(),
                                                cx,
                                            );
                                        })
                                        .build(),
                                ])
                                .into_any_element(),
                        }),
                )
                .into_any_element()
        }
        MessageType::VerificationRequest(verification_request) => {
            "Key Verification Request".into_any_element()
        }
        _ => message_error_item(
            "dialog-warning",
            tr!("MESSAGE_UNSUPPORTED", "Unsupported Message"),
            cx,
        )
        .into_any_element(),
    }
}

fn text_message(
    as_reply: bool,
    body: &String,
    formatted: &Option<FormattedBody>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let body = match &formatted {
        None => body.clone().into_any_element(),
        Some(formatted) => {
            TextView::html("html-text", formatted.body.clone(), window, cx).into_any_element()
        }
    };

    let theme = cx.global::<Theme>();
    div()
        .flex()
        .w_full()
        .max_w_full()
        .child(
            div()
                .max_w_full()
                .p(px(6.))
                .when_else(
                    as_reply,
                    |david| david.bg(rgba(0x00C8FF05)),
                    |david| david.bg(rgba(0x00C8FF10)),
                )
                .rounded(theme.border_radius)
                .child(body),
        )
        .into_any_element()
}

fn download_file(file: FileMessageEventContent, media_file: Entity<MediaFile>, cx: &mut App) {
    // Trigger a job
    media_file.update(cx, |media_file, cx| {
        media_file.request_media(
            Some(file.filename().into()),
            file.info.and_then(|info| info.mimetype),
            true,
            cx,
        );
    })
}

fn save_file(file: FileMessageEventContent, media_file: Entity<MediaFile>, cx: &mut App) {
    let user_dirs = UserDirs::new().unwrap();
    let prompt = cx.prompt_for_new_path(user_dirs.download_dir().unwrap(), Some(file.filename()));
    let media_file = media_file.read(cx);
    let MediaState::Loaded(media_file) = &media_file.media_state else {
        return;
    };

    let path = media_file.path().to_path_buf();
    cx.spawn(async move |_: &mut AsyncApp| {
        if let Some(new_path) = prompt.await.ok().and_then(|result| result.ok()).flatten() {
            // Save the file to the path
            copy(path, new_path).unwrap();
        };
    })
    .detach();
}
