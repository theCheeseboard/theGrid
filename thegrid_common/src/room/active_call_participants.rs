use crate::session::session_manager::SessionManager;
use crate::tokio_helper::TokioHelper;
use gpui::{App, AppContext, AsyncApp, Entity, WeakEntity};
use matrix_sdk::room::RoomMember;
use matrix_sdk::ruma::OwnedRoomId;
use matrix_sdk::ruma::events::{AnyPossiblyRedactedStateEventContent, AnySyncStateEvent};

pub fn track_active_call_participants(
    room_id: OwnedRoomId,
    cx: &mut App,
) -> Entity<Vec<RoomMember>> {
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
            async move |weak_this: WeakEntity<Vec<RoomMember>>, cx: &mut AsyncApp| {
                loop {
                    let room = room.clone();
                    let active_call_participants = room.active_room_call_participants();

                    let mut call_participants = Vec::new();
                    for participant in active_call_participants {
                        let room = room.clone();
                        if let Ok(Some(member)) = cx
                            .spawn_tokio(async move { room.get_member(&participant).await })
                            .await
                        {
                            call_participants.push(member);
                        }
                    }

                    if weak_this
                        .update(cx, |this, cx| *this = call_participants)
                        .is_err()
                    {
                        client.remove_event_handler(room_update);
                        return;
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
