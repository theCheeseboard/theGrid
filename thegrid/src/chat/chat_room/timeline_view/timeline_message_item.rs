use cntp_i18n::{Quote, tr};
use contemporary::components::button::button;
use contemporary::components::context_menu::ContextMenuItem;
use contemporary::components::icon::icon;
use contemporary::components::spinner::spinner;
use contemporary::styling::theme::{Theme, VariableColor};
use directories::UserDirs;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AsyncApp, BorrowAppContext, Entity, IntoElement, ParentElement, RenderOnce, Styled,
    Window, div, px, rgba,
};
use matrix_sdk::ruma::events::room::message::{FileMessageEventContent, MessageType};
use matrix_sdk_ui::timeline::{
    EmbeddedEvent, MsgLikeContent, MsgLikeKind, TimelineDetails, TimelineItemContent,
};
use std::fs::copy;
use thegrid_common::mxc_image::{SizePolicy, mxc_image};
use thegrid_common::session::media_cache::{MediaCacheEntry, MediaFile, MediaState};
use thegrid_common::session::session_manager::SessionManager;
use thegrid_text_rendering::TextView;

#[derive(IntoElement)]
pub struct TimelineMessageItem {
    content: MsgLikeContent,
}

pub fn timeline_message_item(content: MsgLikeContent) -> TimelineMessageItem {
    TimelineMessageItem { content }
}

impl RenderOnce for TimelineMessageItem {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();
        div()
            .flex()
            .flex_col()
            .when_some(self.content.in_reply_to, |david, reply_details| {
                david.child(
                    div()
                        .flex()
                        .text_color(theme.foreground.disabled())
                        .text_size(theme.system_font_size * 0.8)
                        // TODO: RTL?
                        .child("⬐ ")
                        .child({
                            let reply_details = match reply_details.event {
                                TimelineDetails::Ready(reply) => match reply.content {
                                    TimelineItemContent::MsgLike(msg_like) => match msg_like.kind {
                                        MsgLikeKind::Message(message) => Some(
                                            div()
                                                .flex()
                                                .child(msgtype_to_message_line(
                                                    message.msgtype(),
                                                    true,
                                                    window,
                                                    cx,
                                                ))
                                                .into_any_element(),
                                        ),
                                        _ => None,
                                    },
                                    _ => None,
                                },
                                _ => None,
                            };

                            reply_details.unwrap_or_else(|| {
                                tr!("REPLY_UNAVAILABLE", "Reply message could not be loaded")
                                    .into_any_element()
                            })
                        }),
                )
            })
            .child(match self.content.kind {
                MsgLikeKind::Message(message) => div().child(msgtype_to_message_line(
                    message.msgtype(),
                    false,
                    window,
                    cx,
                )),
                _ => div(),
            })
    }
}

pub fn msgtype_to_message_line<'a>(
    msgtype: &MessageType,
    as_reply: bool,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement + 'a {
    match msgtype {
        MessageType::Emote(emote) => div().child(emote.body.clone()).into_any_element(),
        MessageType::Image(image) => div()
            .child(
                mxc_image(image.source.clone())
                    .min_w(px(100.))
                    .min_h(px(30.))
                    .size_policy(SizePolicy::Constrain(500., 500.)),
            )
            .into_any_element(),
        MessageType::Text(text) => {
            let body = match &text.formatted {
                None => text.body.clone().into_any_element(),
                Some(formatted) => TextView::html("html-text", formatted.body.clone(), window, cx)
                    .into_any_element(),
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
                                    .unwrap_or("application-octet-stream".into())
                                    .into(),
                            )
                            .size(24.),
                        )
                        .child(file.filename().to_string())
                        .child(match media_file.media_state {
                            MediaState::Idle | MediaState::Failed => button("download-button")
                                .child(icon("cloud-download".into()))
                                .on_click(move |_, _, cx| {
                                    download_file(file.clone(), media_file_entity.clone(), cx);
                                })
                                .into_any_element(),
                            MediaState::Loading => spinner().size(px(16.)).into_any_element(),
                            MediaState::Loaded(_) => button("open-button")
                                .child(icon("document-open".into()))
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
        _ => "Unknown Message".into_any_element(),
    }
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
