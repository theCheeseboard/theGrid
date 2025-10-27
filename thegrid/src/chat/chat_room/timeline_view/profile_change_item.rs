use crate::chat::chat_room::timeline_view::state_change_element::state_change_element;
use cntp_i18n::{Quote, tr};
use gpui::{App, IntoElement, RenderOnce, Window, div};
use matrix_sdk_ui::timeline::MemberProfileChange;

#[derive(IntoElement)]
pub struct ProfileChangeItem {
    profile_change: MemberProfileChange,
}

pub fn profile_change_item(profile_change: MemberProfileChange) -> ProfileChangeItem {
    ProfileChangeItem { profile_change }
}

impl RenderOnce for ProfileChangeItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let user = self.profile_change.user_id();
        if let Some(display_name) = self.profile_change.displayname_change()
            && let Some(_) = self.profile_change.avatar_url_change()
        {
            let new_display_name = display_name.new.clone().unwrap_or_default();
            state_change_element(
                Some("user".into()),
                tr!(
                    "PROFILE_UPDATE_DISPLAY_NAME_AVATAR",
                    "{{user}} updated their profile picture and their display name to {{new_name}}",
                    user = user.to_string(),
                    new_name:Quote = new_display_name
                ),
            )
            .into_any_element()
        } else if let Some(display_name) = self.profile_change.displayname_change() {
            let old_display_name = display_name.old.clone().unwrap_or_default();
            let new_display_name = display_name.new.clone().unwrap_or_default();
            state_change_element(
                Some("user".into()),
                tr!(
                    "PROFILE_UPDATE_DISPLAY_NAME",
                    "{{user}} updated their display name from {{old_name}} to {{new_name}}",
                    user = user.to_string(),
                    old_name:Quote = old_display_name,
                    new_name:Quote = new_display_name
                ),
            )
            .into_any_element()
        } else if let Some(_) = self.profile_change.avatar_url_change() {
            state_change_element(
                Some("user".into()),
                tr!(
                    "PROFILE_UPDATE_AVATAR",
                    "{{user}} updated their profile picture",
                    user = user.to_string()
                ),
            )
            .into_any_element()
        } else {
            div().into_any_element()
        }
    }
}
