use gpui::AsyncApp;
use matrix_sdk::Room;
use matrix_sdk::deserialized_responses::RawAnySyncOrStrippedState;
use matrix_sdk::ruma::api::client::discovery::discover_homeserver::RtcFocusInfo;
use matrix_sdk::ruma::events::call::member::{
    ActiveFocus, CallMemberEventContent, Focus, FocusSelection,
};
use matrix_sdk::ruma::events::{AnySyncStateEvent, StateEventType};
use thegrid_common::tokio_helper::TokioHelper;

pub enum FocusUrlError {
    RoomError,
    NoRtcFocus,
}

pub async fn get_focus_url(
    room: Room,
    rtc_foci: Vec<RtcFocusInfo>,
    cx: &mut AsyncApp,
) -> Result<String, FocusUrlError> {
    let room_id = room.room_id().to_owned();
    let Ok(call_member_state_events) = cx
        .spawn_tokio(async move { room.get_state_events(StateEventType::CallMember).await })
        .await
    else {
        return Err(FocusUrlError::RoomError);
    };

    let service_url = call_member_state_events
        .iter()
        .find_map(|state_event| {
            let RawAnySyncOrStrippedState::Sync(event) = state_event else {
                return None;
            };

            let Ok(AnySyncStateEvent::CallMember(event)) = event.deserialize() else {
                return None;
            };

            let event = event.as_original()?;
            let CallMemberEventContent::SessionContent(content) = &event.content else {
                return None;
            };

            let ActiveFocus::Livekit(livekit_focus) = &content.focus_active else {
                return None;
            };

            if livekit_focus.focus_selection != FocusSelection::OldestMembership {
                return None;
            };

            content.foci_preferred.iter().find_map(|focus| {
                let Focus::Livekit(lk_focus) = focus else {
                    return None;
                };

                if lk_focus.alias != room_id {
                    return None;
                }

                Some(lk_focus.service_url.clone())
            })
        })
        .or_else(|| {
            let Some(RtcFocusInfo::LiveKit(livekit_focus)) = rtc_foci
                .iter()
                .find(|focus| matches!(focus, RtcFocusInfo::LiveKit(_)))
            else {
                return None;
            };

            Some(livekit_focus.service_url.clone())
        });

    service_url.ok_or(FocusUrlError::NoRtcFocus)
}
