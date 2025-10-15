use crate::chat::timeline_event::resolve_event;
use crate::mxc_image::{SizePolicy, mxc_image};
use cntp_i18n::tr;
use contemporary::styling::theme::{Theme, VariableColor};
use gpui::http_client::anyhow;
use gpui::prelude::FluentBuilder;
use gpui::private::anyhow;
use gpui::{
    AnyElement, App, AsyncApp, Context, Element, Entity, InteractiveElement, IntoElement,
    ParentElement, RenderOnce, Styled, WeakEntity, Window, div, px, relative, rgb, rgba,
};
use log::info;
use matrix_sdk::Room;
use matrix_sdk::crypto::types::events::room::Event;
use matrix_sdk::deserialized_responses::{TimelineEvent, TimelineEventKind};
use matrix_sdk::event_cache::RoomEventCache;
use matrix_sdk::room::RoomMember;
use matrix_sdk::ruma::events::room::message::{
    MessageType, Relation, RoomMessageEventContent, RoomMessageEventContentWithoutRelation,
};
use matrix_sdk::ruma::events::{
    AnyMessageLikeEvent, AnyTimelineEvent, MessageLikeEventContent, OriginalMessageLikeEvent,
};
use matrix_sdk::ruma::{OwnedMxcUri, OwnedRoomId, OwnedUserId, RoomId};
use std::rc::Rc;
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
    event_cache: Entity<RoomEventCache>,
}

#[derive(Clone)]
pub enum CachedRoomMember {
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
    event_cache: Entity<RoomEventCache>,
) -> RoomMessageEvent<T>
where
    T: RoomMessageEventRenderable,
{
    RoomMessageEvent {
        event,
        room,
        timeline_event,
        previous_event,
        event_cache,
    }
}

impl<T> RenderOnce for RoomMessageEvent<T>
where
    T: RoomMessageEventRenderable + 'static,
{
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let event_id = self.event.event_id.clone();
        let relations_entity = window.use_state(cx, |_, cx| {
            let event_cache = self.event_cache.read(cx).clone();
            cx.spawn(
                async move |weak_this: WeakEntity<Vec<TimelineEvent>>, cx: &mut AsyncApp| {
                    if let Ok(related) = cx
                        .spawn_tokio(async move {
                            event_cache
                                .find_event_with_relations(&event_id, None)
                                .await
                                .ok_or(anyhow!("Error"))
                        })
                        .await
                    {
                        if let Some(this) = weak_this.upgrade() {
                            info!("related: {:?}", related.1);
                            let _ = this.write(cx, related.1);
                        }
                    };
                },
            )
            .detach();

            Vec::<TimelineEvent>::new()
        });

        // TODO: Invalidate relations_entity when new data comes through the event cache

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

        let author = cached_author.read(cx).clone().unwrap();
        let author_id = author.user_id();

        let is_head_event = if let Some(previous_event) = self.previous_event {
            let event = resolve_event(&previous_event, self.room.room_id());

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

        if !self.event.content.should_render() {
            return div().id("room-message");
        }

        let theme = cx.global::<Theme>();
        let content = || {
            for relation in relations_entity.read(cx).iter() {
                if let Ok(resolved) = resolve_event(relation, self.room.room_id())
                    && let AnyTimelineEvent::MessageLike(AnyMessageLikeEvent::RoomMessage(
                        room_message,
                    )) = resolved
                    && let Some(original) = room_message.as_original()
                    && let Some(Relation::Replacement(replacement_relation)) =
                        &original.content.relates_to
                {
                    return div()
                        .flex()
                        .flex_col()
                        .child(msgtype_to_message_line(
                            &replacement_relation.new_content.msgtype,
                            theme,
                        ))
                        .child(
                            div()
                                .flex()
                                .text_color(theme.foreground.disabled())
                                .text_size(theme.system_font_size * 0.8)
                                // TODO: RTL?
                                .child("⬑ ")
                                .child(tr!("EDITED_MESSAGE_INDICATOR", "(edited)")),
                        )
                        .into_any_element();
                }
            }
            self.event.content.message_line(theme).into_any_element()
        };

        div()
            .id("room-message")
            .flex()
            .m(px(2.))
            .max_w(relative(100.))
            .when_else(
                is_head_event,
                |david| {
                    david.child(
                        div()
                            .id("container")
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
                                div()
                                    .id("content")
                                    .flex()
                                    .flex_col()
                                    .child(div().child(author.display_name()).child(content())),
                            ),
                    )
                },
                |david| {
                    david
                        .flex()
                        .gap(px(4.))
                        .child(div().w(px(40.)).mx(px(2.)))
                        .child(div().child(content()))
                },
            )
    }
}

trait RoomMessageEventRenderable: MessageLikeEventContent {
    fn message_line(&self, theme: &Theme) -> impl IntoElement;
    fn should_render(&self) -> bool;
}

impl RoomMessageEventRenderable for RoomMessageEventContent {
    fn message_line(&self, theme: &Theme) -> impl IntoElement {
        div().child(msgtype_to_message_line(&self.msgtype, theme))
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
}

fn msgtype_to_message_line(msgtype: &MessageType, theme: &Theme) -> impl IntoElement {
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
    }
}
