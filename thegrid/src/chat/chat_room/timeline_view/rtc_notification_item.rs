use crate::chat::chat_room::timeline_view::state_change_element::state_change_element;
use cntp_i18n::{Quote, tr};
use gpui::{App, IntoElement, ParentElement, RenderOnce, Window, div};
use matrix_sdk_ui::timeline::{EventTimelineItem, MemberProfileChange};

#[derive(IntoElement)]
pub struct RtcNotificationItem {
    rtc_notification: EventTimelineItem,
}

pub fn rtc_notification_item(rtc_notification: EventTimelineItem) -> RtcNotificationItem {
    RtcNotificationItem { rtc_notification }
}

impl RenderOnce for RtcNotificationItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div().child(tr!("RTC_NOTIFICATION_TEXT", "Call started"))
    }
}
