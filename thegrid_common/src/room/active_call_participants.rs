use crate::session::session_manager::SessionManager;
use crate::tokio_helper::TokioHelper;
use gpui::{App, AppContext, AsyncApp, Entity, WeakEntity};
use matrix_sdk::deserialized_responses::RawAnySyncOrStrippedState;
use matrix_sdk::room::RoomMember;
use matrix_sdk::ruma::events::call::member::{
    ActiveFocus, CallMemberEventContent, Focus, FocusSelection,
};
use matrix_sdk::ruma::events::{
    AnyPossiblyRedactedStateEventContent, AnySyncStateEvent, StateEventType,
};
use matrix_sdk::ruma::{MilliSecondsSinceUnixEpoch, OwnedRoomId};

#[derive(Clone)]
pub struct ActiveCallParticipant {
    pub room_member: RoomMember,
    pub sfu: Option<String>,
}

pub fn track_active_call_participants(
    room_id: OwnedRoomId,
    cx: &mut App,
) -> Entity<Vec<ActiveCallParticipant>> {
    cx.new(|cx| {
        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx).clone();
        let Some(room) = session_manager.rooms().read(cx).room(&room_id) else {
            return Vec::new();
        };

        let room = room.read(cx).inner.clone();
        let (tx, rx) = async_channel::bounded(1);
        let room_update = room.add_event_handler(|ev: AnySyncStateEvent| async move {
            if let AnyPossiblyRedactedStateEventContent::CallMember(_) = ev.content() {
                let _ = tx.send(ev).await;
            }
        });

        cx.spawn(
            async move |weak_this: WeakEntity<Vec<ActiveCallParticipant>>, cx: &mut AsyncApp| {
                loop {
                    let room = room.clone();
                    let active_call_participants = room.active_room_call_participants();

                    if let Ok(call_member_state_events) = cx
                        .spawn_tokio({
                            let room = room.clone();
                            async move { room.get_state_events(StateEventType::CallMember).await }
                        })
                        .await
                    {
                        let mut call_participants = Vec::new();
                        for participant in call_member_state_events {
                            // TODO: Ignore stale events
                            let RawAnySyncOrStrippedState::Sync(event) = participant else {
                                continue;
                            };

                            let Ok(AnySyncStateEvent::CallMember(event)) = event.deserialize()
                            else {
                                continue;
                            };

                            let Some(event) = event.as_original() else {
                                continue;
                            };

                            let member_id = event.sender.clone();
                            let CallMemberEventContent::SessionContent(content) = &event.content
                            else {
                                continue;
                            };

                            let joined_at = content
                                .created_ts
                                .map(|ts| ts.min(event.origin_server_ts))
                                .unwrap_or(event.origin_server_ts);
                            if joined_at
                                .checked_add(content.expires)
                                .is_none_or(|expiry| expiry < MilliSecondsSinceUnixEpoch::now())
                            {
                                continue;
                            }

                            let ActiveFocus::Livekit(livekit_focus) = &content.focus_active else {
                                continue;
                            };

                            if livekit_focus.focus_selection != FocusSelection::OldestMembership
                                && livekit_focus.focus_selection != "multi_sfu".into()
                            {
                                continue;
                            };

                            let Some(Focus::Livekit(preferred_focus)) =
                                content.foci_preferred.first()
                            else {
                                continue;
                            };

                            if let Ok(Some(member)) = cx
                                .spawn_tokio({
                                    let room = room.clone();
                                    async move { room.get_member(&member_id).await }
                                })
                                .await
                            {
                                call_participants.push(ActiveCallParticipant {
                                    room_member: member,
                                    sfu: Some(preferred_focus.service_url.clone()),
                                });
                            }
                        }

                        if weak_this
                            .update(cx, |this, cx| *this = call_participants)
                            .is_err()
                        {
                            client.remove_event_handler(room_update);
                            return;
                        }
                    }

                    if rx.recv().await.is_err() {
                        client.remove_event_handler(room_update);
                        return;
                    }
                }
            },
        )
        .detach();

        Vec::new()
    })
}
