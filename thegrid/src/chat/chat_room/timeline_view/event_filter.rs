use matrix_sdk::ruma::events::{AnySyncStateEvent, AnySyncTimelineEvent};
use matrix_sdk::ruma::room_version_rules::RoomVersionRules;
use matrix_sdk_ui::timeline::default_event_filter;

pub fn event_filter(event: &AnySyncTimelineEvent, room_version_rules: &RoomVersionRules) -> bool {
    // Filter out events we don't support

    match event {
        AnySyncTimelineEvent::MessageLike(_) => {}
        AnySyncTimelineEvent::State(state_event) => match state_event {
            AnySyncStateEvent::RoomCreate(_) => return false,
            AnySyncStateEvent::RoomGuestAccess(_) => return false,
            _ => {}
        },
    }

    default_event_filter(event, room_version_rules)
}
