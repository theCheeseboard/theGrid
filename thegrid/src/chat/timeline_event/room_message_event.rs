use crate::chat::timeline_event::author_flyout::{
    AuthorFlyoutUserActionEvent, AuthorFlyoutUserActionListener,
};
use crate::chat::timeline_event::resolve_event;
use crate::chat::timeline_event::room_message_element::RoomMessageElement;
use crate::chat::timeline_event::room_message_event_renderable::{
    RoomMessageEventRenderable, msgtype_to_message_line,
};
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
use matrix_sdk::ruma::{OwnedEventId, OwnedMxcUri, OwnedRoomId, OwnedUserId, RoomId};
use std::rc::Rc;
use thegrid::session::session_manager::SessionManager;
use thegrid::thegrid_error::TheGridError;
use thegrid::tokio_helper::TokioHelper;
use tokio::io::AsyncReadExt;

#[derive(IntoElement)]
pub struct RoomMessageEvent<T>
where
    T: RoomMessageEventRenderable + 'static,
{
    event: T,
    event_id: Option<OwnedEventId>,
    room: Room,
    author: OwnedUserId,
    previous_event: Option<TimelineEvent>,
    event_cache: Option<Entity<RoomEventCache>>,
    force_not_head_event: bool,
    on_user_action: Box<AuthorFlyoutUserActionListener>,
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
    event: T,
    event_id: Option<OwnedEventId>,
    room: Room,
    author: OwnedUserId,
    previous_event: Option<TimelineEvent>,
    event_cache: Option<Entity<RoomEventCache>>,
    force_not_head_event: bool,
    on_user_action: impl Fn(&AuthorFlyoutUserActionEvent, &mut Window, &mut App) + 'static,
) -> RoomMessageEvent<T>
where
    T: RoomMessageEventRenderable,
{
    RoomMessageEvent {
        event,
        event_id,
        room,
        author,
        previous_event,
        event_cache,
        force_not_head_event,
        on_user_action: Box::new(on_user_action),
    }
}

impl<T> RenderOnce for RoomMessageEvent<T>
where
    T: RoomMessageEventRenderable + 'static,
{
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let event_id = self.event_id.clone();
        let relations_entity = window.use_state(cx, |_, cx| {
            if let Some(event_cache) = &self.event_cache {
                let event_cache = event_cache.read(cx).clone();
                if let Some(event_id) = event_id {
                    cx.spawn(
                        async move |weak_this: WeakEntity<Vec<TimelineEvent>>,
                                    cx: &mut AsyncApp| {
                            if let Ok(related) = cx
                                .spawn_tokio(async move {
                                    event_cache
                                        .find_event_with_relations(&event_id, None)
                                        .await
                                        .ok_or(TheGridError::new("Unable to find event"))
                                })
                                .await
                            {
                                if let Some(this) = weak_this.upgrade() {
                                    let _ = this.write(cx, related.1);
                                }
                            };
                        },
                    )
                    .detach();
                }
            }

            Vec::<TimelineEvent>::new()
        });

        // TODO: Invalidate relations_entity when new data comes through the event cache

        let reply_message = window.use_state(cx, |_, cx| {
            if let Some(event_cache) = &self.event_cache {
                let event_cache = event_cache.read(cx).clone();
                let reply = self.event.reply_to();
                if let Some(reply) = reply {
                    cx.spawn(
                        async move |weak_this: WeakEntity<Option<TimelineEvent>>,
                                    cx: &mut AsyncApp| {
                            if let Ok(reply_event) = cx
                                .spawn_tokio(async move {
                                    event_cache
                                        .find_event(&reply)
                                        .await
                                        .ok_or(TheGridError::new("Unable to find reply"))
                                })
                                .await
                            {
                                if let Some(this) = weak_this.upgrade() {
                                    let _ = this.write(cx, Some(reply_event));
                                }
                            };
                        },
                    )
                    .detach();
                }
            }

            None
        });

        let cached_author = window.use_state(cx, |_, _| None);
        if cached_author.read(cx).is_none() {
            let room = self.room.clone();

            cached_author.write(cx, Some(CachedRoomMember::UserId(self.author.clone())));

            let cached_author_clone = cached_author.clone();

            let author = self.author.clone();
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

        let is_head_event = if self.force_not_head_event {
            false
        } else if let Some(previous_event) = self.previous_event {
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

        if !self.event.should_render() {
            return div().id("room-message").into_any_element();
        }

        let reply = reply_message.update(cx, |reply_message, cx| {
            reply_message.as_ref().and_then(|reply| {
                if let Ok(resolved) = resolve_event(reply, self.room.room_id())
                    && let AnyTimelineEvent::MessageLike(AnyMessageLikeEvent::RoomMessage(
                        room_message,
                    )) = resolved
                    && let Some(original) = room_message.as_original()
                {
                    Some(
                        div()
                            .flex()
                            .mb(px(2.))
                            .child(msgtype_to_message_line(
                                &original.content.msgtype,
                                true,
                                window,
                                cx,
                            ))
                            .into_any_element(),
                    )
                } else {
                    None
                }
            })
        });

        let content = || {
            let mut message_line = self.event.message_line(window, cx).into_any_element();
            let mut is_edited = false;

            let relations = relations_entity.read(cx).clone();
            for relation in relations.iter() {
                if let Ok(resolved) = resolve_event(relation, self.room.room_id())
                    && let AnyTimelineEvent::MessageLike(AnyMessageLikeEvent::RoomMessage(
                        room_message,
                    )) = resolved
                    && let Some(original) = room_message.as_original()
                    && let Some(Relation::Replacement(replacement_relation)) =
                        &original.content.relates_to
                {
                    message_line = msgtype_to_message_line(
                        &replacement_relation.new_content.msgtype,
                        false,
                        window,
                        cx,
                    )
                    .into_any_element();
                    is_edited = true;
                }
            }

            let theme = cx.global::<Theme>();

            div()
                .flex()
                .flex_col()
                .when_some(reply, |david, reply| {
                    david.child(
                        div()
                            .flex()
                            .text_color(theme.foreground.disabled())
                            .text_size(theme.system_font_size * 0.8)
                            // TODO: RTL?
                            .child("⬐ ")
                            .child(reply),
                    )
                })
                .child(message_line)
                .when(is_edited, |david| {
                    david.child(
                        div()
                            .flex()
                            .text_color(theme.foreground.disabled())
                            .text_size(theme.system_font_size * 0.8)
                            // TODO: RTL?
                            .child("⬑ ")
                            .child(tr!("EDITED_MESSAGE_INDICATOR", "(edited)")),
                    )
                })
                .into_any_element()
        };

        RoomMessageElement {
            author: if is_head_event { Some(author) } else { None },
            room: self.room.clone(),
            on_user_action: self.on_user_action,
            content: content(),
        }
        .into_any_element()
    }
}
