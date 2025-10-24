use crate::chat::chat_room::open_room::OpenRoom;
use crate::chat::timeline_event::room_message_event::CachedRoomMember;
use crate::main_window::SurfaceChange;
use crate::mxc_image::{SizePolicy, mxc_image};
use cntp_i18n::{Quote, tr, trn};
use contemporary::components::admonition::{AdmonitionSeverity, admonition};
use contemporary::components::button::button;
use contemporary::components::flyout::flyout;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::spinner::spinner;
use contemporary::components::subtitle::subtitle;
use contemporary::styling::theme::{Theme, VariableColor};
use gpui::prelude::FluentBuilder;
use gpui::{
    App, Bounds, Entity, InteractiveElement, IntoElement, ParentElement, Pixels, RenderOnce,
    Styled, Window, div, px,
};
use matrix_sdk::Room;
use matrix_sdk::room::{RoomMember, RoomMemberRole};
use matrix_sdk::ruma::events::room::member::MembershipState;
use matrix_sdk::ruma::events::room::power_levels::UserPowerLevel;
use std::rc::Rc;

pub type AuthorFlyoutCloseListener =
    dyn Fn(&AuthorFlyoutCloseEvent, &mut Window, &mut App) + 'static;
pub type AuthorFlyoutUserActionListener =
    dyn Fn(&AuthorFlyoutUserActionEvent, &mut Window, &mut App) + 'static;

#[derive(Clone)]
pub struct AuthorFlyoutCloseEvent;

#[derive(Clone)]
pub struct AuthorFlyoutUserActionEvent {
    pub action: UserAction,
    pub user: RoomMember,
    pub room: Room,
}

#[derive(Clone)]
pub enum UserAction {
    ChangePowerLevel,
    Kick,
    Ban,
}

#[derive(IntoElement)]
pub struct AuthorFlyout {
    bounds: Bounds<Pixels>,
    visible: bool,
    author: CachedRoomMember,
    room: Entity<OpenRoom>,
    on_close: Box<AuthorFlyoutCloseListener>,
    on_user_action: Box<AuthorFlyoutUserActionListener>,
}

pub fn author_flyout(
    bounds: Bounds<Pixels>,
    visible: bool,
    author: CachedRoomMember,
    room: Entity<OpenRoom>,
    on_close: impl Fn(&AuthorFlyoutCloseEvent, &mut Window, &mut App) + 'static,
    on_user_action: impl Fn(&AuthorFlyoutUserActionEvent, &mut Window, &mut App) + 'static,
) -> AuthorFlyout {
    AuthorFlyout {
        bounds,
        visible,
        author,
        room,
        on_close: Box::new(on_close),
        on_user_action: Box::new(on_user_action),
    }
}

impl RenderOnce for AuthorFlyout {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let on_close = Rc::new(self.on_close);
        let on_close_2 = on_close.clone();
        let on_close_3 = on_close.clone();
        let on_close_4 = on_close.clone();

        let on_user_action = Rc::new(self.on_user_action);
        let on_user_action_2 = on_user_action.clone();
        let on_user_action_3 = on_user_action.clone();

        let theme = cx.global::<Theme>();
        let room = self.room.read(cx).room.clone().unwrap();

        let display_name = room
            .cached_display_name()
            .map(|name| name.to_string())
            .or_else(|| room.name())
            .unwrap_or_default();

        let room = room.clone();
        let room_2 = room.clone();
        let room_3 = room.clone();

        flyout(self.bounds)
            .visible(self.visible)
            .anchor_top_right()
            .child(match &self.author {
                CachedRoomMember::RoomMember(room_member) => {
                    let room_member = room_member.clone();
                    let room_member_2 = room_member.clone();
                    let room_member_3 = room_member.clone();
                    let suggested_role = room_member.suggested_role_for_power_level();

                    let membership = room_member.membership().clone();
                    let joined = membership == MembershipState::Join;
                    let me = self.room.read(cx).current_user.clone().unwrap();
                    let can_ban =
                        me.can_ban() && me.power_level() > room_member.power_level() && joined;
                    let can_kick =
                        me.can_kick() && me.power_level() > room_member.power_level() && joined;

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
                                .when(joined, |david| {
                                    david.child(
                                        div()
                                            .flex()
                                            .gap(px(4.))
                                            .items_center()
                                            .child(tr!(
                                                "AUTHOR_STATUS_POWER_LEVEL",
                                                "Power Level: {{power_level}}",
                                                power_level =
                                                    match room_member.normalized_power_level() {
                                                        UserPowerLevel::Infinite => {
                                                            tr!("POWER_LEVEL_INFINITE", "Infinite")
                                                                .into()
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
                                                            .child(tr!("POWER_LEVEL_MODERATOR",)),
                                                    )
                                                },
                                            )
                                            .child(div().flex_grow())
                                            .child(
                                                button("change-power-level")
                                                    .child(tr!("CHANGE_POWER_LEVEL", "Change..."))
                                                    .on_click(move |_, window, cx| {
                                                        on_close_2(
                                                            &AuthorFlyoutCloseEvent,
                                                            window,
                                                            cx,
                                                        );
                                                        on_user_action(
                                                            &AuthorFlyoutUserActionEvent {
                                                                action:
                                                                    UserAction::ChangePowerLevel,
                                                                room: room.clone(),
                                                                user: room_member.clone(),
                                                            },
                                                            window,
                                                            cx,
                                                        );
                                                    }),
                                            ),
                                    )
                                })
                                .when(membership == MembershipState::Ban, |david| {
                                    david.child(tr!("USER_BANNED_PROMPT", "This user is banned"))
                                })
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .bg(theme.destructive_accent_color)
                                        .rounded(theme.border_radius)
                                        .when(can_kick, |david| {
                                            david.child(
                                                button("kick-button")
                                                    .destructive()
                                                    .child(icon_text(
                                                        "im-kick-user".into(),
                                                        tr!("KICK", "Kick").into(),
                                                    ))
                                                    .on_click(move |_, window, cx| {
                                                        on_close_3(
                                                            &AuthorFlyoutCloseEvent,
                                                            window,
                                                            cx,
                                                        );
                                                        on_user_action_2(
                                                            &AuthorFlyoutUserActionEvent {
                                                                action: UserAction::Kick,
                                                                room: room_2.clone(),
                                                                user: room_member_2.clone(),
                                                            },
                                                            window,
                                                            cx,
                                                        );
                                                    }),
                                            )
                                        })
                                        .when(can_ban, |david| {
                                            david.child(
                                                button("ban-button")
                                                    .destructive()
                                                    .child(icon_text(
                                                        "im-ban-user".into(),
                                                        tr!("BAN", "Ban").into(),
                                                    ))
                                                    .on_click(move |_, window, cx| {
                                                        on_close_4(
                                                            &AuthorFlyoutCloseEvent,
                                                            window,
                                                            cx,
                                                        );
                                                        on_user_action_3(
                                                            &AuthorFlyoutUserActionEvent {
                                                                action: UserAction::Ban,
                                                                room: room_3.clone(),
                                                                user: room_member_3.clone(),
                                                            },
                                                            window,
                                                            cx,
                                                        );
                                                    }),
                                            )
                                        }),
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
