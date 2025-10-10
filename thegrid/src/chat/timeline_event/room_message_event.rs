use crate::mxc_image::{SizePolicy, mxc_image};
use cntp_i18n::tr;
use contemporary::styling::theme::Theme;
use gpui::http_client::anyhow;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AsyncApp, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px, relative, rgb,
    rgba,
};
use matrix_sdk::Room;
use matrix_sdk::deserialized_responses::{TimelineEvent, TimelineEventKind};
use matrix_sdk::room::RoomMember;
use matrix_sdk::ruma::events::room::message::{MessageType, RoomMessageEventContent};
use matrix_sdk::ruma::events::{
    AnyMessageLikeEvent, AnyTimelineEvent, MessageLikeEventContent, OriginalMessageLikeEvent,
};
use matrix_sdk::ruma::{OwnedMxcUri, OwnedUserId};
use thegrid::session::session_manager::SessionManager;
use thegrid::tokio_helper::TokioHelper;
use tokio::io::AsyncReadExt;

#[derive(IntoElement)]
pub struct RoomMessageEvent<T>
where
    T: RoomMessageEventRenderable + 'static,
{
    event: OriginalMessageLikeEvent<T>,
    room: Room,
    timeline_event: AnyTimelineEvent,
    previous_event: Option<TimelineEvent>,
}

#[derive(Clone)]
enum CachedRoomMember {
    RoomMember(RoomMember),
    UserId(OwnedUserId),
}

impl CachedRoomMember {
    pub fn display_name(&self) -> String {
        match self {
            CachedRoomMember::RoomMember(room_member) => room_member
                .display_name()
                .map(|name| name.to_string())
                .unwrap_or_else(|| room_member.user_id().to_string()),
            CachedRoomMember::UserId(user_id) => user_id.to_string(),
        }
    }

    pub fn user_id(&self) -> OwnedUserId {
        match self {
            CachedRoomMember::RoomMember(room_member) => room_member.user_id().to_owned(),
            CachedRoomMember::UserId(user_id) => user_id.clone(),
        }
    }

    pub fn avatar(&self) -> Option<OwnedMxcUri> {
        match self {
            CachedRoomMember::RoomMember(room_member) => {
                room_member.avatar_url().map(|url| url.to_owned())
            }
            CachedRoomMember::UserId(_) => None,
        }
    }
}

pub fn room_message_event<T>(
    event: OriginalMessageLikeEvent<T>,
    room: Room,
    timeline_event: AnyTimelineEvent,
    previous_event: Option<TimelineEvent>,
) -> RoomMessageEvent<T>
where
    T: RoomMessageEventRenderable,
{
    RoomMessageEvent {
        event,
        room,
        timeline_event,
        previous_event,
    }
}

impl<T> RenderOnce for RoomMessageEvent<T>
where
    T: RoomMessageEventRenderable + 'static,
{
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let cached_author = window.use_state(cx, |_, _| None);
        if cached_author.read(cx).is_none() {
            let author = self.timeline_event.sender().to_owned();
            let room = self.room.clone();

            cached_author.write(cx, Some(CachedRoomMember::UserId(author.to_owned())));

            let cached_author_clone = cached_author.clone();

            cx.spawn(async move |cx: &mut AsyncApp| {
                let room_member = cx
                    .spawn_tokio(async move { room.get_member(&author).await })
                    .await
                    .ok()
                    .flatten();

                if let Some(room_member) = room_member {
                    let _ = cached_author_clone
                        .write(cx, Some(CachedRoomMember::RoomMember(room_member)));
                }
            })
            .detach();
        }

        let theme = cx.global::<Theme>();
        let author = cached_author.read(cx).clone().unwrap();
        let author_id = author.user_id();

        let is_head_event = if let Some(previous_event) = self.previous_event {
            let event = match &previous_event.kind {
                TimelineEventKind::Decrypted(decrypted) => match decrypted.event.deserialize() {
                    Ok(event) => Ok(event),
                    Err(_) => Err(anyhow!("Unknown Error")),
                },
                TimelineEventKind::UnableToDecrypt { .. } => Err(anyhow!("Unable to decrypt")),
                TimelineEventKind::PlainText { event } => match event.deserialize() {
                    Ok(event) => Ok(event.into_full_event(self.room.room_id().to_owned())),
                    Err(_) => Err(anyhow!("Unknown Error")),
                },
            };

            match event {
                Ok(AnyTimelineEvent::MessageLike(message_like)) => match message_like {
                    AnyMessageLikeEvent::Message(message) => match message.as_original() {
                        None => true,
                        Some(original_message) => original_message.sender != author_id,
                    },
                    AnyMessageLikeEvent::RoomMessage(room_message) => {
                        match room_message.as_original() {
                            None => true,
                            Some(original_message) => original_message.sender != author_id,
                        }
                    }
                    _ => true,
                },
                _ => true,
            }
        } else {
            true
        };

        div().flex().m(px(2.)).when_else(
            is_head_event,
            |david| {
                david.child(
                    div()
                        .flex()
                        .gap(px(4.))
                        .child(
                            mxc_image(author.avatar())
                                .size(px(40.))
                                .m(px(2.))
                                .size_policy(SizePolicy::Fit)
                                .rounded(theme.border_radius),
                        )
                        .child(
                            div().flex().flex_col().child(
                                div()
                                    .child(author.display_name())
                                    .child(self.event.content.message_line(theme)),
                            ),
                        ),
                )
            },
            |david| {
                david
                    .flex()
                    .gap(px(4.))
                    .child(div().w(px(40.)).mx(px(2.)))
                    .child(div().child(self.event.content.message_line(theme)))
            },
        )
    }
}

trait RoomMessageEventRenderable: MessageLikeEventContent {
    fn message_line(&self, theme: &Theme) -> impl IntoElement;
}

impl RoomMessageEventRenderable for RoomMessageEventContent {
    fn message_line(&self, theme: &Theme) -> impl IntoElement {
        div().child(match &self.msgtype {
            MessageType::Emote(emote) => div().child(emote.body.clone()).into_any_element(),
            MessageType::Image(image) => div()
                .child(
                    mxc_image(image.source.clone())
                        .min_w(px(100.))
                        .min_h(px(30.))
                        .size_policy(SizePolicy::Constrain(500., 500.)),
                )
                .into_any_element(),
            MessageType::Text(text) => div()
                .p(px(2.))
                .bg(rgba(0x00C8FF10))
                .rounded(theme.border_radius)
                .max_w(relative(0.8))
                .child(text.body.clone())
                .into_any_element(),
            MessageType::VerificationRequest(verification_request) => {
                "Key Verification Request".into_any_element()
            }
            _ => "Unknown Message".into_any_element(),
        })
    }
}
