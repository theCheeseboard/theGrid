use crate::mxc_image::{SizePolicy, mxc_image};
use cntp_i18n::{Quote, tr};
use contemporary::components::button::{ButtonMenuOpenPolicy, button};
use contemporary::components::context_menu::ContextMenuItem;
use contemporary::components::icon::icon;
use contemporary::components::spinner::spinner;
use contemporary::styling::theme::Theme;
use directories::UserDirs;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AsyncApp, BorrowAppContext, Entity, IntoElement, ParentElement, Styled, WeakEntity,
    Window, div, px, relative, rgba,
};
use matrix_sdk::ruma::OwnedEventId;
use matrix_sdk::ruma::events::room::message::{
    FileMessageEventContent, FormattedBody, MessageType, Relation, RoomMessageEventContent,
    RoomMessageEventContentWithoutRelation,
};
use matrix_sdk::ruma::events::{
    AnyMessageLikeEvent, AnyMessageLikeEventContent, MessageLikeEventContent,
};
use std::fs::copy;
use thegrid::session::media_cache::{MediaCacheEntry, MediaFile, MediaState};
use thegrid::session::session_manager::SessionManager;
use thegrid_text_rendering::TextView;

pub trait RoomMessageEventRenderable: MessageLikeEventContent {
    fn message_line(&self, window: &mut Window, cx: &mut App) -> impl IntoElement;
    fn should_render(&self) -> bool;
    fn reply_to(&self) -> Option<OwnedEventId>;
}

impl RoomMessageEventRenderable for RoomMessageEventContent {
    fn message_line(&self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        div().child(msgtype_to_message_line(&self.msgtype, false, window, cx))
    }

    fn should_render(&self) -> bool {
        self.relates_to
            .as_ref()
            .map(|relates_to| match relates_to {
                Relation::Reply { .. } => true,
                Relation::Replacement(_) => false,
                _ => true,
            })
            .unwrap_or(true)
    }

    fn reply_to(&self) -> Option<OwnedEventId> {
        self.relates_to
            .as_ref()
            .and_then(|relates_to| match relates_to {
                Relation::Reply { in_reply_to } => Some(in_reply_to.event_id.clone()),
                _ => None,
            })
    }
}

impl RoomMessageEventRenderable for AnyMessageLikeEventContent {
    fn message_line(&self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        match self {
            AnyMessageLikeEventContent::RoomMessage(msg) => div()
                .child(msgtype_to_message_line(&msg.msgtype, false, window, cx))
                .into_any_element(),
            _ => div().into_any_element(),
        }
    }

    fn should_render(&self) -> bool {
        true
    }

    fn reply_to(&self) -> Option<OwnedEventId> {
        match self {
            AnyMessageLikeEventContent::RoomMessage(msg) => {
                msg.relates_to
                    .as_ref()
                    .and_then(|relation| match &relation {
                        Relation::Reply { in_reply_to } => Some(in_reply_to.event_id.clone()),
                        _ => None,
                    })
            }
            _ => None,
        }
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
                .p(px(2.))
                .when_else(
                    as_reply,
                    |david| david.bg(rgba(0x00C8FF05)),
                    |david| david.bg(rgba(0x00C8FF10)),
                )
                .rounded(theme.border_radius)
                .child(body)
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
                .max_w(relative(0.8))
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
