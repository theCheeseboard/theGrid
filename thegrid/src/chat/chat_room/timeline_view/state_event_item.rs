use crate::chat::chat_room::timeline_view::state_change_element::state_change_element;
use cntp_i18n::{tr, Quote};
use gpui::{div, App, IntoElement, RenderOnce, Window};
use matrix_sdk::ruma::events::room::history_visibility::HistoryVisibility;
use matrix_sdk::ruma::events::room::join_rules::RoomJoinRulesEventContent;
use matrix_sdk::ruma::events::room::name::RoomNameEventContent;
use matrix_sdk::ruma::events::{FullStateEventContent, StateEventType};
use matrix_sdk::ruma::room::JoinRule;
use matrix_sdk::ruma::OwnedUserId;
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
                Some("im-room".to_string()),
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
                Some("im-room".to_string()),
                tr!(
                    "ROOM_STATE_ROOM_TOPIC",
                    "{{user}} changed the topic for the room",
                    user = sender
                ),
            )
            .into_any_element(),
            AnyOtherFullStateEventContent::RoomAvatar(_) => state_change_element(
                Some("im-room".to_string()),
                tr!(
                    "ROOM_STATE_ROOM_AVATAR",
                    "{{user}} changed the picture for the room",
                    user = sender
                ),
            )
            .into_any_element(),
            AnyOtherFullStateEventContent::RoomEncryption(_) => state_change_element(
                Some("im-room".to_string()),
                tr!(
                    "ROOM_STATE_ROOM_ENCRYPTION",
                    "{{user}} enabled encryption for the room",
                    user = sender
                ),
            )
            .into_any_element(),
            AnyOtherFullStateEventContent::RoomPowerLevels(_) => state_change_element(
                Some("im-room".to_string()),
                tr!(
                    "ROOM_STATE_POWER_LEVELS",
                    "{{user}} updated permissions in the room",
                    user = sender
                ),
            )
            .into_any_element(),
            AnyOtherFullStateEventContent::RoomTombstone(_) => state_change_element(
                Some("im-room".to_string()),
                tr!(
                    "ROOM_STATE_TOMBSTONE",
                    "{{user}} upgraded the room",
                    user = sender
                ),
            )
            .into_any_element(),
            AnyOtherFullStateEventContent::RoomJoinRules(event) => state_change_element(
                Some("im-room".to_string()),
                match event {
                    FullStateEventContent::Original { content, .. } => match content.join_rule {
                        JoinRule::Invite => Some(tr!(
                            "ROOM_STATE_JOIN_RULES_INVITE",
                            "{{user}} made the room invite-only",
                            user = sender
                        )),
                        JoinRule::Knock => Some(tr!(
                            "ROOM_STATE_JOIN_RULES_KNOCK",
                            "{{user}} made the room knockable",
                            user = sender
                        )),
                        JoinRule::Private => Some(tr!(
                            "ROOM_STATE_JOIN_RULES_PRIVATE",
                            "{{user}} made the room private",
                            user = sender
                        )),
                        JoinRule::Public => Some(tr!(
                            "ROOM_STATE_JOIN_RULES_PUBLIC",
                            "{{user}} made the room public",
                            user = sender
                        )),
                        _ => None,
                    },
                    FullStateEventContent::Redacted(_) => None,
                }
                .unwrap_or(tr!(
                    "ROOM_STATE_JOIN_RULES_REDACTED",
                    "{{user}} changed the access rules for the room",
                    user = sender
                )),
            )
                .into_any_element(),
            AnyOtherFullStateEventContent::SpaceParent(_) => state_change_element(
                Some("im-room".to_string()),
                tr!(
                    "ROOM_STATE_SPACE_PARENT_REDACTED",
                    "{{user}} added the room to a space",
                    user = sender
                ),
            )
            .into_any_element(),
            AnyOtherFullStateEventContent::RoomHistoryVisibility(event) => state_change_element(
                Some("im-room".to_string()),
                match event {
                    FullStateEventContent::Original { content, .. } => match content.history_visibility {
                        HistoryVisibility::Invited => Some(tr!(
                            "ROOM_STATE_JOIN_RULES_INVITED",
                            "{{user}} allowed people to see messages from when they were invited",
                            user = sender
                        )),
                        HistoryVisibility::Joined => Some(tr!(
                            "ROOM_STATE_JOIN_RULES_JOINED",
                            "{{user}} allowed people to see messages from when they \
                            joined the room",
                            user = sender
                        )),
                        HistoryVisibility::Shared => Some(tr!(
                            "ROOM_STATE_JOIN_RULES_SHARED",
                            "{{user}} allowed people in the room to see all messages since \
                            the room was created",
                            user = sender
                        )),
                        HistoryVisibility::WorldReadable => Some(tr!(
                            "ROOM_STATE_JOIN_RULES_WORLD_READABLE",
                            "{{user}} allowed anyone to see all messages in the room, even if \
                            they are not currently in the room",
                            user = sender
                        )),
                        _ => None,
                    },
                    FullStateEventContent::Redacted(_) => None,
                }
                    .unwrap_or(tr!(
                    "ROOM_STATE_HISTORY_VISIBILITY_REDACTED",
                    "{{user}} changed who can see historical messages in the room",
                    user = sender
                )),
            )
                .into_any_element(),
            _ => div().into_any_element(),
        }
    }
}
