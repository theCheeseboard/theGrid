use crate::session::session_manager::SessionManager;
use cntp_i18n::{Quote, tr};
use contemporary::notification::Notification;
use gpui::App;
use log::info;
use matrix_sdk::Room;
use matrix_sdk::deserialized_responses::RawAnySyncOrStrippedTimelineEvent;
use matrix_sdk::ruma::events::room::member::MembershipState;
use matrix_sdk::ruma::events::room::message::MessageType;
use matrix_sdk::ruma::events::{
    AnyMessageLikeEventContent, AnyStateEventContent, AnyStrippedStateEvent, AnySyncTimelineEvent,
};
use matrix_sdk::ruma::html::{HtmlSanitizerMode, RemoveReplyFallback};

pub fn trigger_notification(
    notification: matrix_sdk::sync::Notification,
    room: Room,
    cx: &mut App,
) {
    let session_manager = cx.global::<SessionManager>();
    let this_user = session_manager
        .client()
        .unwrap()
        .read(cx)
        .user_id()
        .unwrap()
        .to_owned();
    let Some(room) = session_manager.rooms().read(cx).room(room.room_id()) else {
        info!("Tried to send notification, but room not found");
        return;
    };
    let room = room.read(cx);

    match notification.event {
        RawAnySyncOrStrippedTimelineEvent::Sync(sync) => match sync.deserialize() {
            Ok(AnySyncTimelineEvent::MessageLike(message_like)) => {
                if let Some(AnyMessageLikeEventContent::RoomMessage(mut message)) =
                    message_like.original_content()
                {
                    message.sanitize(HtmlSanitizerMode::Compat, RemoveReplyFallback::Yes);

                    let room_display_name = room.display_name();
                    let summary = tr!(
                        "NOTIFICATION_MESSAGE_SUMMARY",
                        "{{sender}} in {{room}}",
                        sender = message_like.sender().to_string(),
                        room:Quote = room_display_name
                    )
                    .to_string();

                    match message.msgtype {
                        MessageType::Text(content) => {
                            Notification::new()
                                .summary(summary.as_str())
                                .body(content.body.as_str())
                                .post(cx);
                        }
                        MessageType::Image(content) => {
                            Notification::new()
                                .summary(summary.as_str())
                                .body(
                                    tr!("NOTIFICATION_MESSAGE_BODY_IMAGE", "sent an image")
                                        .to_string()
                                        .as_str(),
                                )
                                .post(cx);
                        }
                        MessageType::File(content) => {
                            Notification::new()
                                .summary(summary.as_str())
                                .body(
                                    tr!(
                                        "NOTIFICATION_MESSAGE_BODY_FILE",
                                        "sent {{filename}}",
                                        filename = content.filename.unwrap_or_default()
                                    )
                                    .to_string()
                                    .as_str(),
                                )
                                .post(cx);
                        }
                        MessageType::Audio(content) => {
                            Notification::new()
                                .summary(summary.as_str())
                                .body(
                                    tr!("NOTIFICATION_MESSAGE_BODY_AUDIO", "sent a voice message",)
                                        .to_string()
                                        .as_str(),
                                )
                                .post(cx);
                        }
                        MessageType::Video(content) => {
                            Notification::new()
                                .summary(summary.as_str())
                                .body(
                                    tr!("NOTIFICATION_MESSAGE_BODY_VIDEO", "sent a video",)
                                        .to_string()
                                        .as_str(),
                                )
                                .post(cx);
                        }
                        _ => {}
                    }
                }
            }
            Ok(AnySyncTimelineEvent::State(state_event)) => {
                if let Some(AnyStateEventContent::RoomMember(room_member_event)) =
                    state_event.original_content()
                {
                    if room_member_event.membership == MembershipState::Invite
                        && state_event.state_key() == this_user
                    {
                        let room_display_name = room.display_name();

                        Notification::new()
                            .summary(
                                tr!("NOTIFICATION_INVITE_SUMMARY", "New room invitation")
                                    .to_string()
                                    .as_str(),
                            )
                            .body(
                                tr!(
                                    "NOTIFICATION_INVITE_BODY",
                                    "{{user}} invited you to join {{room}}",
                                    user = state_event.sender().to_string(),
                                    room:Quote = room_display_name
                                )
                                .to_string()
                                .as_str(),
                            )
                            .post(cx);
                    }
                }
            }
            _ => {}
        },
        RawAnySyncOrStrippedTimelineEvent::Stripped(stripped) => {
            if let Ok(AnyStrippedStateEvent::RoomMember(room_member_event)) = stripped.deserialize()
            {
                if room_member_event.content.membership == MembershipState::Invite
                    && room_member_event.state_key == this_user
                {
                    let room_display_name = room.display_name();

                    Notification::new()
                        .summary(tr!("NOTIFICATION_INVITE_SUMMARY").to_string().as_str())
                        .body(
                            tr!(
                                "NOTIFICATION_INVITE_BODY",
                                user = room_member_event.sender.to_string(),
                                room:Quote = room_display_name
                            )
                            .to_string()
                            .as_str(),
                        )
                        .post(cx);
                }
            }
        }
    }
}

fn send_invite_notification() {}
