pub mod queued_event;
pub mod room_head;
mod room_message_element;
mod room_message_event;
mod room_message_event_renderable;
mod room_state_event;

use crate::chat::timeline_event::room_message_event::room_message_event;
use crate::chat::timeline_event::room_state_event::room_state_event;
use cntp_i18n::tr;
use gpui::http_client::anyhow;
use gpui::private::anyhow;
use gpui::{
    App, ElementId, Entity, InteractiveElement, IntoElement, ParentElement, RenderOnce, Window, div,
};
use matrix_sdk::Room;
use matrix_sdk::deserialized_responses::{TimelineEvent, TimelineEventKind};
use matrix_sdk::event_cache::RoomEventCache;
use matrix_sdk::linked_chunk::relational::IndexableItem;
use matrix_sdk::ruma::RoomId;
use matrix_sdk::ruma::events::{AnyMessageLikeEvent, AnyTimelineEvent};

#[derive(IntoElement)]
pub struct TimelineRow {
    event: TimelineEvent,
    previous_event: Option<TimelineEvent>,
    event_cache: Entity<RoomEventCache>,
    room: Room,
}

pub fn timeline_event(
    event: TimelineEvent,
    previous_event: Option<TimelineEvent>,
    event_cache: Entity<RoomEventCache>,
    room: Room,
) -> TimelineRow {
    TimelineRow {
        event,
        previous_event,
        event_cache,
        room,
    }
}

impl RenderOnce for TimelineRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = self.event.id().to_string();

        let event = resolve_event(&self.event, self.room.room_id());

        div()
            .id(ElementId::Name(id.into()))
            .child(match event {
                Ok(event) => match &event {
                    AnyTimelineEvent::MessageLike(message_like) => match message_like {
                        AnyMessageLikeEvent::Message(message) => match message.as_original() {
                            None => div().into_any_element(),
                            Some(original_message) => {
                                let message_content = original_message
                                    .content
                                    .text
                                    .find_plain()
                                    .unwrap_or_default();
                                div().child(message_content.to_string()).into_any_element()
                            }
                        },
                        AnyMessageLikeEvent::RoomMessage(room_message) => {
                            match room_message.as_original() {
                                None => div().into_any_element(),
                                Some(original_message) => room_message_event(
                                    original_message.content.clone(),
                                    Some(original_message.event_id.clone()),
                                    self.room,
                                    event.sender().to_owned(),
                                    self.previous_event,
                                    Some(self.event_cache),
                                    false,
                                )
                                .into_any_element(),
                            }
                        }
                        _ => div().into_any_element(),
                    },
                    AnyTimelineEvent::State(state) => {
                        room_state_event(state.clone(), self.room).into_any_element()
                    }
                },
                Err(e) => div()
                    .child(tr!(
                        "MESSAGE_DECRYPTION_FAILURE",
                        "Unable to decrypt this message. Check your verification \
                                        status and try again later."
                    ))
                    .into_any_element(),
            })
            .into_any_element()
    }
}

pub fn resolve_event(event: &TimelineEvent, room_id: &RoomId) -> anyhow::Result<AnyTimelineEvent> {
    match &event.kind {
        TimelineEventKind::Decrypted(decrypted) => match decrypted.event.deserialize() {
            Ok(event) => Ok(event),
            Err(_) => Err(anyhow!("Unknown Error")),
        },
        TimelineEventKind::UnableToDecrypt { .. } => Err(anyhow!("Unable to decrypt")),
        TimelineEventKind::PlainText { event } => match event.deserialize() {
            Ok(event) => Ok(event.into_full_event(room_id.to_owned())),
            Err(_) => Err(anyhow!("Unknown Error")),
        },
    }
}
