use crate::chat::chat_room::timeline_view::state_change_element::state_change_element;
use cntp_i18n::{Quote, tr};
use gpui::{App, IntoElement, RenderOnce, Window, div};
use matrix_sdk::ruma::OwnedUserId;
use matrix_sdk::ruma::events::room::name::RoomNameEventContent;
use matrix_sdk::ruma::events::{FullStateEventContent, StateEventType};
use matrix_sdk_ui::timeline::{
    AnyOtherFullStateEventContent, OtherState, Profile, TimelineDetails,
};

#[derive(IntoElement)]
pub struct StateEventItem {
    state: OtherState,
    sender_profile: TimelineDetails<Profile>,
    sender: OwnedUserId,
}

pub fn state_event_item(
    state: OtherState,
    sender_profile: TimelineDetails<Profile>,
    sender: OwnedUserId,
) -> StateEventItem {
    StateEventItem {
        state,
        sender_profile,
        sender,
    }
}

impl RenderOnce for StateEventItem {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let sender = match self.sender_profile {
            TimelineDetails::Ready(profile) => profile.display_name,
            _ => None,
        }
        .unwrap_or_else(|| self.sender.to_string());

        match self.state.content() {
            AnyOtherFullStateEventContent::RoomName(event) => state_change_element(
                None,
                match event {
                    FullStateEventContent::Original { content, .. } => {
                        tr!(
                            "ROOM_STATE_ROOM_NAME",
                            "{{user}} changed the name of the room to {{new_name}}",
                            user = sender,
                            new_name:Quote = content.name
                        )
                    }
                    FullStateEventContent::Redacted(_) => {
                        tr!(
                            "ROOM_STATE_ROOM_NAME_REDACTED",
                            "{{user}} changed the name of the room",
                            user = sender
                        )
                    }
                },
            )
            .into_any_element(),
            AnyOtherFullStateEventContent::RoomTopic(_) => state_change_element(
                None,
                tr!(
                    "ROOM_STATE_ROOM_TOPIC",
                    "{{user}} changed the topic for the room",
                    user = sender
                ),
            )
            .into_any_element(),
            AnyOtherFullStateEventContent::RoomAvatar(_) => state_change_element(
                None,
                tr!(
                    "ROOM_STATE_ROOM_AVATAR",
                    "{{user}} changed the picture for the room",
                    user = sender
                ),
            )
            .into_any_element(),
            AnyOtherFullStateEventContent::RoomEncryption(_) => state_change_element(
                None,
                tr!(
                    "ROOM_STATE_ROOM_ENCRYPTION",
                    "{{user}} enabled encryption for the room",
                    user = sender
                ),
            )
            .into_any_element(),
            AnyOtherFullStateEventContent::RoomPowerLevels(_) => state_change_element(
                None,
                tr!(
                    "ROOM_STATE_POWER_LEVELS",
                    "{{user}} updated permissions in the room",
                    user = sender
                ),
            )
            .into_any_element(),
            AnyOtherFullStateEventContent::RoomTombstone(_) => state_change_element(
                None,
                tr!(
                    "ROOM_STATE_TOMBSTONE",
                    "{{user}} upgraded the room",
                    user = sender
                ),
            )
            .into_any_element(),
            _ => div().into_any_element(),
        }
    }
}
