use crate::chat::chat_room::timeline_view::state_change_element::state_change_element;
use cntp_i18n::tr;
use gpui::{App, IntoElement, RenderOnce, Window, div};
use matrix_sdk::ruma::OwnedUserId;
use matrix_sdk::ruma::events::StateEventType;
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
            _ => div().into_any_element(),
        }
    }
}
