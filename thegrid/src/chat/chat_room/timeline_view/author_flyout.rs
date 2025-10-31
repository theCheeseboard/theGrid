use crate::chat::chat_room::open_room::OpenRoom;
use crate::chat::displayed_room::DisplayedRoom;
use crate::mxc_image::{SizePolicy, mxc_image};
use cntp_i18n::{Quote, tr};
use contemporary::components::button::button;
use contemporary::components::flyout::flyout;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::spinner::spinner;
use contemporary::components::subtitle::subtitle;
use contemporary::components::text_field::TextField;
use contemporary::styling::theme::{Theme, VariableColor};
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AsyncApp, Bounds, Entity, InteractiveElement, IntoElement, ParentElement, Pixels,
    RenderOnce, Styled, Window, div, px,
};
use matrix_sdk::Room;
use matrix_sdk::room::{RoomMember, RoomMemberRole};
use matrix_sdk::ruma::OwnedUserId;
use matrix_sdk::ruma::events::room::member::MembershipState;
use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;
use matrix_sdk::ruma::events::room::power_levels::UserPowerLevel;
use matrix_sdk_ui::timeline::RoomExt;
use std::rc::Rc;
use thegrid::session::session_manager::SessionManager;
use thegrid::tokio_helper::TokioHelper;
use thegrid_text_rendering::Text;

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
    Unban,
}

#[derive(IntoElement)]
pub struct AuthorFlyout {
    bounds: Bounds<Pixels>,
    visible: bool,
    author: Entity<Option<RoomMember>>,
    room: Entity<OpenRoom>,
    displayed_room: Entity<DisplayedRoom>,
    on_close: Box<AuthorFlyoutCloseListener>,
    on_user_action: Box<AuthorFlyoutUserActionListener>,
}

pub fn author_flyout(
    bounds: Bounds<Pixels>,
    visible: bool,
    author: Entity<Option<RoomMember>>,
    room: Entity<OpenRoom>,
    displayed_room: Entity<DisplayedRoom>,
    on_close: impl Fn(&AuthorFlyoutCloseEvent, &mut Window, &mut App) + 'static,
    on_user_action: impl Fn(&AuthorFlyoutUserActionEvent, &mut Window, &mut App) + 'static,
) -> AuthorFlyout {
    AuthorFlyout {
        bounds,
        visible,
        author,
        room,
        displayed_room,
        on_close: Box::new(on_close),
        on_user_action: Box::new(on_user_action),
    }
}

impl AuthorFlyout {
    fn send_direct_message(
        message: String,
        existing_room: Option<Room>,
        dm_target: OwnedUserId,
        displayed_room: Entity<DisplayedRoom>,
        cx: &mut App,
    ) {
        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx).clone();

        // Send a message to the DM room, creating one if
        // it doesn't exist
        cx.spawn(async move |cx: &mut AsyncApp| {
            let room = match existing_room {
                None => {
                    let Ok(room) = cx
                        .spawn_tokio(async move { client.create_dm(&dm_target).await })
                        .await
                    else {
                        // TODO: Show error
                        return;
                    };

                    room
                }
                Some(room) => room,
            };

            let _ = displayed_room.write(cx, DisplayedRoom::Room(room.room_id().to_owned()));

            cx.spawn(async move |cx: &mut AsyncApp| {
                let _ = cx
                    .spawn_tokio(async move {
                        room.timeline()
                            .await?
                            .send(RoomMessageEventContent::text_plain(message).into())
                            .await
                    })
                    .await;
            })
            .detach();
        })
        .detach();
    }
}

impl RenderOnce for AuthorFlyout {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let on_close = Rc::new(self.on_close);
        let on_close_2 = on_close.clone();
        let on_close_3 = on_close.clone();
        let on_close_4 = on_close.clone();
        let on_close_5 = on_close.clone();
        let on_close_6 = on_close.clone();
        let on_close_7 = on_close.clone();
        let on_close_8 = on_close.clone();
        let on_close_9 = on_close.clone();

        let on_user_action = Rc::new(self.on_user_action);
        let on_user_action_2 = on_user_action.clone();
        let on_user_action_3 = on_user_action.clone();
        let on_user_action_4 = on_user_action.clone();

        let displayed_room = self.displayed_room;
        let displayed_room_2 = displayed_room.clone();

        let room = self.room.read(cx).room.clone().unwrap();

        let display_name = room
            .cached_display_name()
            .map(|name| name.to_string())
            .or_else(|| room.name())
            .unwrap_or_default();

        let room = room.clone();
        let room_2 = room.clone();
        let room_3 = room.clone();
        let room_4 = room.clone();
        let room_5 = room.clone();
        let room_6 = room.clone();

        flyout(self.bounds)
            .render_as_deferred(true)
            .visible(self.visible)
            .anchor_top_right()
            .child(match &self.author.read(cx) {
                Some(room_member) => {
                    let room_member = room_member.clone();
                    let room_member_2 = room_member.clone();
                    let room_member_3 = room_member.clone();
                    let room_member_4 = room_member.clone();
                    let room_member_5 = room_member.clone();
                    let room_member_id = room_member.user_id().to_owned();
                    let room_member_id_2 = room_member_id.clone();
                    let room_member_id_3 = room_member_id.clone();
                    let room_member_id_4 = room_member_id.clone();
                    let suggested_role = room_member.suggested_role_for_power_level();

                    let session_manager = cx.global::<SessionManager>();
                    let client = session_manager.client().unwrap().read(cx).clone();
                    let current_dm_room = client.get_dm_room(room_member.user_id());
                    let current_dm_room_2 = current_dm_room.clone();

                    let direct_message_box = window.use_state(cx, |_, cx| {
                        let mut text_field = TextField::new("direct-message", cx);
                        text_field.set_placeholder(
                            tr!("DIRECT_MESSAGE_PLACEHOLDER", "Message")
                                .to_string()
                                .as_str(),
                        );
                        text_field.on_enter_press(cx.listener(
                            move |direct_message_box: &mut TextField, _, window, cx| {
                                let message = direct_message_box.text().to_string();
                                if message.is_empty() {
                                    return;
                                }

                                on_close_9(&AuthorFlyoutCloseEvent, window, cx);

                                Self::send_direct_message(
                                    message,
                                    current_dm_room_2.clone(),
                                    room_member_id_4.clone(),
                                    displayed_room_2.clone(),
                                    cx,
                                );
                            },
                        ));
                        text_field
                    });
                    let direct_message_box_2 = direct_message_box.clone();

                    let membership = room_member.membership().clone();
                    let joined = membership == MembershipState::Join;
                    let me = self.room.read(cx).current_user.clone().unwrap();
                    let can_invite = me.can_invite() && membership == MembershipState::Leave;
                    let can_retract_invite = me.can_kick() && membership == MembershipState::Invite;
                    let can_ban =
                        me.can_ban() && me.power_level() > room_member.power_level() && joined;
                    let can_kick =
                        me.can_kick() && me.power_level() > room_member.power_level() && joined;
                    let can_unban = me.can_ban() && membership == MembershipState::Ban;
                    let is_ignored = room_member.is_ignored();

                    let theme = cx.global::<Theme>();

                    div()
                        .occlude()
                        .flex()
                        .flex_col()
                        .p(px(8.))
                        .gap(px(4.))
                        .child(
                            mxc_image(room_member.avatar_url().map(|url| url.to_owned()))
                                .size(px(128.))
                                .size_policy(SizePolicy::Fit)
                                .rounded(theme.border_radius),
                        )
                        .child(
                            div().text_size(theme.heading_font_size).child(
                                room_member
                                    .display_name()
                                    .map(|name| name.to_string())
                                    .unwrap_or_else(|| room_member.user_id().to_string()),
                            ),
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
                                        .when(can_invite, |david| {
                                            david.child(
                                                button("invite-button")
                                                    .child(icon_text(
                                                        "user".into(),
                                                        tr!("INVITE_USER", "Invite").into(),
                                                    ))
                                                    .on_click(move |_, window, cx| {
                                                        let room = room_4.clone();
                                                        let room_member_id = room_member_id.clone();
                                                        on_close_5(
                                                            &AuthorFlyoutCloseEvent,
                                                            window,
                                                            cx,
                                                        );
                                                        cx.spawn(async |cx: &mut AsyncApp| {
                                                            let _ = cx
                                                                .spawn_tokio(async move {
                                                                    room.invite_user_by_id(
                                                                        &room_member_id,
                                                                    )
                                                                    .await
                                                                })
                                                                .await;
                                                        })
                                                        .detach();
                                                    }),
                                            )
                                        })
                                        .when(can_retract_invite, |david| {
                                            david.child(
                                                button("retract-invite-button")
                                                    .child(icon_text(
                                                        "im-kick-user".into(),
                                                        tr!(
                                                            "RETRACT_INVITE_USER",
                                                            "Retract Invite"
                                                        )
                                                        .into(),
                                                    ))
                                                    .destructive()
                                                    .on_click(move |_, window, cx| {
                                                        let room = room_5.clone();
                                                        let room_member_id =
                                                            room_member_id_2.clone();
                                                        on_close_6(
                                                            &AuthorFlyoutCloseEvent,
                                                            window,
                                                            cx,
                                                        );
                                                        cx.spawn(async |cx: &mut AsyncApp| {
                                                            let _ = cx
                                                                .spawn_tokio(async move {
                                                                    room.kick_user(
                                                                        &room_member_id,
                                                                        None,
                                                                    )
                                                                    .await
                                                                })
                                                                .await;
                                                        })
                                                        .detach();
                                                    }),
                                            )
                                        })
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
                                        })
                                        .when(can_unban, |david| {
                                            david.child(
                                                button("unban-button")
                                                    .child(icon_text(
                                                        "user".into(),
                                                        tr!("UNBAN", "Lift Ban").into(),
                                                    ))
                                                    .on_click(move |_, window, cx| {
                                                        on_close_7(
                                                            &AuthorFlyoutCloseEvent,
                                                            window,
                                                            cx,
                                                        );
                                                        on_user_action_4(
                                                            &AuthorFlyoutUserActionEvent {
                                                                action: UserAction::Unban,
                                                                room: room_6.clone(),
                                                                user: room_member_4.clone(),
                                                            },
                                                            window,
                                                            cx,
                                                        );
                                                    }),
                                            )
                                        }),
                                ),
                        )
                        .when(!is_ignored && !room_member_5.is_account_user(), |david| {
                            david.child(
                                layer()
                                    .p(px(4.))
                                    .flex()
                                    .flex_col()
                                    .child(subtitle(tr!("DIRECT_MESSAGE", "Direct Message")))
                                    .child(direct_message_box)
                                    .child(
                                        button("send-button")
                                            .child(icon_text(
                                                "mail-send".into(),
                                                match current_dm_room {
                                                    None => {
                                                        tr!(
                                                            "DIRECT_MESSAGE_OPEN_SEND",
                                                            "Invite to DM and Send"
                                                        )
                                                    }
                                                    Some(_) => {
                                                        tr!("DIRECT_MESSAGE_SEND", "Send")
                                                    }
                                                }
                                                .into(),
                                            ))
                                            .on_click(move |_, window, cx| {
                                                let message = direct_message_box_2
                                                    .read(cx)
                                                    .text()
                                                    .to_string();
                                                if message.is_empty() {
                                                    return;
                                                }

                                                on_close_8(&AuthorFlyoutCloseEvent, window, cx);

                                                Self::send_direct_message(
                                                    message,
                                                    current_dm_room.clone(),
                                                    room_member_id_3.clone(),
                                                    displayed_room.clone(),
                                                    cx,
                                                );
                                            }),
                                    ),
                            )
                        })
                        .into_any_element()
                }
                None => div()
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
