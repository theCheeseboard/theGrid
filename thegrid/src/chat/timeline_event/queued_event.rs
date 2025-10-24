use crate::chat::chat_room::open_room::OpenRoom;
use crate::chat::timeline_event::room_message_element::RoomMessageElement;
use crate::chat::timeline_event::room_message_event::room_message_event;
use gpui::{
    Context, Entity, InteractiveElement, IntoElement, ParentElement, Render, SharedString, Styled,
    Window, div,
};
use matrix_sdk::Room;
use matrix_sdk::deserialized_responses::TimelineEvent;
use matrix_sdk::event_cache::RoomEventCache;
use matrix_sdk::ruma::events::OriginalMessageLikeEvent;
use matrix_sdk::ruma::events::room::MediaSource;
use matrix_sdk::ruma::{OwnedTransactionId, TransactionId};
use matrix_sdk::send_queue::{AbstractProgress, LocalEcho, LocalEchoContent};
use matrix_sdk::store::SerializableEventContent;
use thegrid::session::session_manager::SessionManager;

pub struct QueuedEvent {
    local_echo: LocalEcho,
    room: Entity<OpenRoom>,
    pub previous_event: Option<TimelineEvent>,
}

impl QueuedEvent {
    pub fn new(
        local_echo: LocalEcho,
        room: Entity<OpenRoom>,
        cx: &mut Context<QueuedEvent>,
    ) -> Self {
        Self {
            local_echo,
            room,
            previous_event: None,
        }
    }

    pub fn transaction_id(&self) -> &TransactionId {
        &self.local_echo.transaction_id
    }

    pub fn notify_send_error(&mut self, recoverable: bool, cx: &mut Context<QueuedEvent>) {}

    pub fn notify_unwedged(&mut self, cx: &mut Context<QueuedEvent>) {}

    pub fn notify_content_replaced(
        &mut self,
        new_content: SerializableEventContent,
        cx: &mut Context<QueuedEvent>,
    ) {
    }

    pub fn notify_upload_progress(
        &mut self,
        file: Option<MediaSource>,
        index: u64,
        progress: AbstractProgress,
        cx: &mut Context<QueuedEvent>,
    ) {
    }
}

impl Render for QueuedEvent {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match &self.local_echo.content {
            LocalEchoContent::Event {
                serialized_event,
                send_error,
                ..
            } => {
                let session_manager = cx.global::<SessionManager>();
                let client = session_manager.client().unwrap().read(cx);
                let deserialized_event = serialized_event.deserialize().unwrap();
                let transaction_id = self.local_echo.transaction_id.to_string();

                div()
                    .id(SharedString::from(transaction_id))
                    .opacity(0.7)
                    .child(room_message_event(
                        deserialized_event,
                        None,
                        self.room.clone(),
                        client.user_id().unwrap().to_owned(),
                        self.previous_event.clone(),
                        None,
                        self.previous_event.is_none(),
                        |_, _, _| {},
                    ))
                    .into_any_element()
            }
            LocalEchoContent::React { .. } => div().into_any_element(),
        }
    }
}
