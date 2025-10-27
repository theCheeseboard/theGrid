use cntp_i18n::tr;
use gpui::{App, IntoElement, RenderOnce, Window, div};
use matrix_sdk::ruma::events::FullStateEventContent;
use matrix_sdk::ruma::events::room::member::RoomMemberEventContent;
use matrix_sdk_ui::timeline::{MembershipChange, RoomMembershipChange};

#[derive(IntoElement)]
pub struct MembershipChangeItem {
    membership_change: RoomMembershipChange,
}

pub fn membership_change_item(membership_change: RoomMembershipChange) -> MembershipChangeItem {
    MembershipChangeItem { membership_change }
}

impl RenderOnce for MembershipChangeItem {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        match self.membership_change.change() {
            Some(MembershipChange::Joined) => tr!(
                "ROOM_STATE_ROOM_MEMBER_JOINED",
                "{{user}} joined the room",
                user = self.membership_change.display_name().unwrap_or_default()
            )
            .into_any_element(),
            Some(MembershipChange::Left) => tr!(
                "ROOM_STATE_ROOM_MEMBER_LEFT",
                "{{user}} left the room",
                user = self.membership_change.display_name().unwrap_or_default()
            )
            .into_any_element(),
            Some(MembershipChange::Banned) | Some(MembershipChange::KickedAndBanned) => {
                let reason = match self.membership_change.content() {
                    FullStateEventContent::Original { content, .. } => content.reason.clone(),
                    FullStateEventContent::Redacted(_) => None,
                };
                tr!(
                    "ROOM_STATE_ROOM_MEMBER_BANNED",
                    "{{user}} was banned from the room: {{reason}}",
                    reason = reason.unwrap_or_else(|| tr!(
                        "EVENT_REASON_NONE",
                        "No reason was provided."
                    )
                    .into())
                )
                .into_any_element()
            }
            Some(MembershipChange::Unbanned) => tr!(
                "ROOM_STATE_ROOM_MEMBER_UNBANNED",
                "{{user}} was unbanned from the room",
                user = self.membership_change.display_name().unwrap_or_default(),
            )
            .into_any_element(),
            Some(MembershipChange::Kicked) => {
                let reason = match self.membership_change.content() {
                    FullStateEventContent::Original { content, .. } => content.reason.clone(),
                    FullStateEventContent::Redacted(_) => None,
                };
                tr!(
                    "ROOM_STATE_ROOM_MEMBER_KICKED",
                    "{{user}} was kicked from the room: {{reason}}",
                    user = self.membership_change.display_name().unwrap_or_default(),
                    reason = reason.unwrap_or_else(|| tr!("EVENT_REASON_NONE").into())
                )
                .into_any_element()
            }
            Some(MembershipChange::Invited) => tr!(
                "ROOM_STATE_ROOM_MEMBER_INVITED",
                "{{user}} was invited to the room",
                user = self.membership_change.display_name().unwrap_or_default()
            )
            .into_any_element(),
            Some(MembershipChange::Knocked) => tr!(
                "ROOM_STATE_ROOM_MEMBER_KNOCKED",
                "{{user}} knocked on the room",
                user = self.membership_change.display_name().unwrap_or_default()
            )
            .into_any_element(),
            _ => div().into_any_element(),
        }
    }
}
