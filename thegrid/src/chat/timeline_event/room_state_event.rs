use crate::chat::chat_room::open_room::OpenRoom;
use crate::chat::timeline_event::room_message_event::CachedRoomMember;
use cntp_i18n::{I18nString, Quote, tr};
use gpui::{
    App, AsyncApp, Entity, InteractiveElement, IntoElement, ParentElement, RenderOnce, Styled,
    Window, div, px, relative,
};
use matrix_sdk::Room;
use matrix_sdk::ruma::events::room::member::{
    MembershipChange, MembershipDetails, MembershipState, PossiblyRedactedRoomMemberEventContent,
    RoomMemberEventContent,
};
use matrix_sdk::ruma::events::{AnyFullStateEventContent, AnyStateEvent, FullStateEventContent};
use matrix_sdk::ruma::{OwnedUserId, UserId};
use thegrid::tokio_helper::TokioHelper;

#[derive(IntoElement)]
pub struct RoomStateEvent {
    event: AnyStateEvent,
    room: Entity<OpenRoom>,
}

pub fn room_state_event(event: AnyStateEvent, room: Entity<OpenRoom>) -> RoomStateEvent {
    RoomStateEvent { event, room }
}

enum StateDisplay {
    None,
    Text(I18nString),
}

impl RoomStateEvent {
    fn state_display(&self, cached_author: &Option<CachedRoomMember>) -> StateDisplay {
        let author = cached_author
            .as_ref()
            .map(|author| author.display_name().clone())
            .unwrap_or_else(|| self.event.sender().to_string());

        match self.event.content() {
            AnyFullStateEventContent::RoomName(FullStateEventContent::Original {
                content: event,
                ..
            }) => StateDisplay::Text(tr!(
                "ROOM_STATE_ROOM_NAME",
                "{{user}} changed the name of the room to {{new_name}}",
                user = author,
                new_name:Quote = event.name
            )),
            AnyFullStateEventContent::RoomMember(FullStateEventContent::Original {
                content: event,
                prev_content: previous_event,
            }) => self.room_member_event(author, event, previous_event),
            _ => StateDisplay::None,
        }
    }

    fn room_member_event(
        &self,
        author: String,
        event: RoomMemberEventContent,
        previous_event: Option<PossiblyRedactedRoomMemberEventContent>,
    ) -> StateDisplay {
        match UserId::parse(self.event.state_key()) {
            Ok(state_key) => {
                match event.membership_change(
                    previous_event
                        .as_ref()
                        .map(|previous_event| previous_event.details()),
                    self.event.sender(),
                    &state_key,
                ) {
                    MembershipChange::Joined => StateDisplay::Text(tr!(
                        "ROOM_STATE_ROOM_MEMBER_JOINED",
                        "{{user}} joined the room",
                        user = author
                    )),
                    MembershipChange::Left => StateDisplay::Text(tr!(
                        "ROOM_STATE_ROOM_MEMBER_LEFT",
                        "{{user}} left the room",
                        user = author
                    )),
                    MembershipChange::Banned | MembershipChange::KickedAndBanned => {
                        StateDisplay::Text(tr!(
                            "ROOM_STATE_ROOM_MEMBER_BANNED",
                            "{{moderator_user}} banned {{user}} from the room: {{reason}}",
                            moderator_user = author,
                            user = state_key.to_string(),
                            reason = event.reason.unwrap_or_else(|| tr!(
                                "EVENT_REASON_NONE",
                                "No reason was provided."
                            )
                            .into())
                        ))
                    }
                    MembershipChange::Unbanned => StateDisplay::Text(tr!(
                        "ROOM_STATE_ROOM_MEMBER_UNBANNED",
                        "{{user}} was unbanned from the room",
                        user = state_key.to_string(),
                    )),
                    MembershipChange::Kicked => StateDisplay::Text(tr!(
                        "ROOM_STATE_ROOM_MEMBER_KICKED",
                        "{{moderator_user}} kicked {{user}} from the room: {{reason}}",
                        moderator_user = author,
                        user = state_key.to_string(),
                        reason = event
                            .reason
                            .unwrap_or_else(|| tr!("EVENT_REASON_NONE").into())
                    )),
                    MembershipChange::Invited => StateDisplay::Text(tr!(
                        "ROOM_STATE_ROOM_MEMBER_INVITED",
                        "{{user}} was invited to the room",
                        user = author
                    )),
                    MembershipChange::Knocked => StateDisplay::Text(tr!(
                        "ROOM_STATE_ROOM_MEMBER_KNOCKED",
                        "{{user}} knocked on the room",
                        user = author
                    )),
                    _ => StateDisplay::None,
                }
            }
            Err(_) => StateDisplay::None,
        }
    }
}

impl RenderOnce for RoomStateEvent {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let cached_author = window.use_state(cx, |_, _| None);
        if cached_author.read(cx).is_none() {
            let author = self.event.sender().to_owned();
            let room = self.room.read(cx).room.clone().unwrap();

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

        match self.state_display(cached_author.read(cx)) {
            StateDisplay::None => div().into_any_element(),
            StateDisplay::Text(text) => div()
                .id("room-state")
                .flex()
                .m(px(2.))
                .my(px(6.))
                .max_w(relative(100.))
                .flex()
                .gap(px(4.))
                .child(div().w(px(40.)).mx(px(2.)))
                .child(div().child(text))
                .into_any_element(),
        }
    }
}
