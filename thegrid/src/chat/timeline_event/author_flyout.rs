use crate::chat::timeline_event::room_message_event::CachedRoomMember;
use crate::main_window::SurfaceChange;
use crate::mxc_image::{SizePolicy, mxc_image};
use cntp_i18n::{Quote, tr, trn};
use contemporary::components::admonition::{AdmonitionSeverity, admonition};
use contemporary::components::button::button;
use contemporary::components::flyout::flyout;
use contemporary::components::layer::layer;
use contemporary::components::spinner::spinner;
use contemporary::components::subtitle::subtitle;
use contemporary::styling::theme::{Theme, VariableColor};
use gpui::prelude::FluentBuilder;
use gpui::{
    App, Bounds, InteractiveElement, IntoElement, ParentElement, Pixels, RenderOnce, Styled,
    Window, div, px,
};
use matrix_sdk::Room;
use matrix_sdk::room::RoomMemberRole;
use matrix_sdk::ruma::events::room::power_levels::UserPowerLevel;

pub type AuthorFlyoutCloseListener =
    dyn Fn(&AuthorFlyoutCloseEvent, &mut Window, &mut App) + 'static;

#[derive(Clone)]
pub struct AuthorFlyoutCloseEvent;

#[derive(IntoElement)]
pub struct AuthorFlyout {
    bounds: Bounds<Pixels>,
    visible: bool,
    author: CachedRoomMember,
    room: Room,
    on_close: Box<AuthorFlyoutCloseListener>,
}

pub fn author_flyout(
    bounds: Bounds<Pixels>,
    visible: bool,
    author: CachedRoomMember,
    room: Room,
    on_close: impl Fn(&AuthorFlyoutCloseEvent, &mut Window, &mut App) + 'static,
) -> AuthorFlyout {
    AuthorFlyout {
        bounds,
        visible,
        author,
        room,
        on_close: Box::new(on_close),
    }
}

impl RenderOnce for AuthorFlyout {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let on_close = self.on_close;
        let theme = cx.global::<Theme>();

        let display_name = self
            .room
            .cached_display_name()
            .map(|name| name.to_string())
            .or_else(|| self.room.name())
            .unwrap_or_default();

        flyout(self.bounds)
            .visible(self.visible)
            .anchor_top_right()
            .child(match &self.author {
                CachedRoomMember::RoomMember(room_member) => {
                    let suggested_role = room_member.suggested_role_for_power_level();
                    div()
                        .occlude()
                        .flex()
                        .flex_col()
                        .p(px(8.))
                        .gap(px(4.))
                        .child(
                            mxc_image(self.author.avatar())
                                .size(px(128.))
                                .size_policy(SizePolicy::Fit)
                                .rounded(theme.border_radius),
                        )
                        .child(
                            div()
                                .text_size(theme.heading_font_size)
                                .child(self.author.display_name()),
                        )
                        .child(
                            div()
                                .text_color(theme.foreground.disabled())
                                .child(room_member.user_id().to_string()),
                        )
                        .child(
                            layer()
                                .p(px(4.))
                                .flex()
                                .flex_col()
                                .child(subtitle(
                                    tr!("IN_ROOM", "In {{room}}", room:Quote = display_name),
                                ))
                                .child(
                                    div()
                                        .flex()
                                        .gap(px(4.))
                                        .items_center()
                                        .child(tr!(
                                            "AUTHOR_STATUS_POWER_LEVEL",
                                            "Power Level: {{power_level}}",
                                            power_level = match room_member.normalized_power_level()
                                            {
                                                UserPowerLevel::Infinite => {
                                                    tr!("POWER_LEVEL_INFINITE", "Infinite").into()
                                                }
                                                UserPowerLevel::Int(power_level) => {
                                                    power_level.to_string()
                                                }
                                                _ => "?".into(),
                                            }
                                        ))
                                        .when(
                                            suggested_role == RoomMemberRole::Administrator,
                                            |david| {
                                                david.child(
                                                    div()
                                                        .rounded(theme.border_radius)
                                                        .bg(theme.error_accent_color)
                                                        .p(px(2.))
                                                        .child(tr!(
                                                            "POWER_LEVEL_ADMINISTRATOR",
                                                            "Administrator"
                                                        )),
                                                )
                                            },
                                        )
                                        .when(
                                            suggested_role == RoomMemberRole::Moderator,
                                            |david| {
                                                david.child(
                                                    div()
                                                        .rounded(theme.border_radius)
                                                        .bg(theme.info_accent_color)
                                                        .p(px(2.))
                                                        .child(tr!(
                                                            "POWER_LEVEL_MODERATOR",
                                                            "Moderator"
                                                        )),
                                                )
                                            },
                                        )
                                        .child(div().flex_grow())
                                        .child(
                                            button("change-power-level")
                                                .child(tr!("CHANGE_POWER_LEVEL", "Change...")),
                                        ),
                                ),
                        )
                        .into_any_element()
                }
                CachedRoomMember::UserId(_) => div()
                    .m(px(8.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(spinner())
                    .into_any_element(),
            })
            .on_close(move |_, window, cx| {
                on_close(&AuthorFlyoutCloseEvent, window, cx);
            })
    }
}
