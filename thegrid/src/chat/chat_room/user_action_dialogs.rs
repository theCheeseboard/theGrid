use cntp_i18n::tr;
use contemporary::components::admonition::{AdmonitionSeverity, admonition};
use contemporary::components::button::button;
use contemporary::components::checkbox::radio_button;
use contemporary::components::dialog_box::{StandardButton, dialog_box};
use contemporary::components::icon_text::icon_text;
use contemporary::components::text_field::TextField;
use contemporary::styling::theme::{Theme, VariableColor};
use gpui::prelude::FluentBuilder;
use gpui::{
    AsyncApp, ClickEvent, Context, IntoElement, ParentElement, Render, Styled, WeakEntity, Window,
    div, px,
};
use log::error;
use matrix_sdk::Room;
use matrix_sdk::room::{RoomMember, RoomMemberRole};
use matrix_sdk::ruma::OwnedRoomId;
use matrix_sdk::ruma::events::room::power_levels::UserPowerLevel;
use thegrid::session::session_manager::SessionManager;
use thegrid::tokio_helper::TokioHelper;

pub struct UserActionDialogs {
    current_dialog: Option<CurrentDialog>,
    room: Option<Room>,
    current_user: Option<RoomMember>,
    busy: bool,
}

#[derive(Clone)]
struct CurrentDialog {
    dialog_type: DialogType,
    acting_user: RoomMember,
}

#[derive(Clone, PartialEq)]
enum DialogType {
    PowerLevel,
    Ban,
    Kick,
}

impl UserActionDialogs {
    pub fn new(room_id: OwnedRoomId, cx: &mut Context<Self>) -> Self {
        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx).clone();

        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let Some(room) = client.get_room(&room_id) else {
                    return;
                };

                let room_clone = room.clone();

                if let Ok(Some(us)) = cx
                    .spawn_tokio(async move { room.get_member(room.own_user_id()).await })
                    .await
                {
                    let _ = weak_this.update(cx, |this, cx| {
                        this.room = Some(room_clone);
                        this.current_user = Some(us);
                        cx.notify()
                    });
                }
            },
        )
        .detach();

        Self {
            current_dialog: None,
            room: None,
            current_user: None,
            busy: false,
        }
    }

    pub fn open_power_level_dialog(&mut self, acting_user: RoomMember) {
        self.current_dialog = Some(CurrentDialog {
            dialog_type: DialogType::PowerLevel,
            acting_user,
        })
    }

    pub fn open_kick_dialog(&mut self, acting_user: RoomMember) {
        self.current_dialog = Some(CurrentDialog {
            dialog_type: DialogType::Kick,
            acting_user,
        })
    }

    pub fn open_ban_dialog(&mut self, acting_user: RoomMember) {
        self.current_dialog = Some(CurrentDialog {
            dialog_type: DialogType::Ban,
            acting_user,
        })
    }

    pub fn update_power_level(
        &mut self,
        new_power_level: UserPowerLevel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let UserPowerLevel::Int(new_power_level) = new_power_level else {
            panic!("new_power_level must not be infinite");
        };

        let dialog = self.current_dialog.as_ref().unwrap();
        let acting_user_id = dialog.acting_user.user_id().to_owned();
        let room = self.room.clone().unwrap();

        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                if let Err(e) = cx
                    .spawn_tokio(async move {
                        room.update_power_levels(vec![(&acting_user_id, new_power_level)])
                            .await
                    })
                    .await
                {
                    error!("Error setting power levels: {e:?}");
                    let _ = weak_this.update(cx, |this, cx| {
                        this.busy = false;
                        cx.notify()
                    });
                } else {
                    let _ = weak_this.update(cx, |this, cx| {
                        this.current_dialog = None;
                        this.busy = false;
                        cx.notify()
                    });
                }
            },
        )
        .detach();

        self.busy = true;
        cx.notify();
    }

    pub fn evict_user(
        &mut self,
        reason: String,
        is_ban: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let dialog = self.current_dialog.as_ref().unwrap();
        let acting_user_id = dialog.acting_user.user_id().to_owned();
        let room = self.room.clone().unwrap();

        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                if let Err(e) = cx
                    .spawn_tokio(async move {
                        let reason = if reason.is_empty() {
                            None
                        } else {
                            Some(reason.as_str())
                        };

                        if is_ban {
                            room.ban_user(&acting_user_id, reason).await
                        } else {
                            room.kick_user(&acting_user_id, reason).await
                        }
                    })
                    .await
                {
                    error!("Error evicting user: {e:?}");
                    let _ = weak_this.update(cx, |this, cx| {
                        this.busy = false;
                        cx.notify()
                    });
                } else {
                    let _ = weak_this.update(cx, |this, cx| {
                        this.current_dialog = None;
                        this.busy = false;
                        cx.notify()
                    });
                }
            },
        )
        .detach();

        self.busy = true;
        cx.notify();
    }
}

impl Render for UserActionDialogs {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .child(
                dialog_box("power-level-dialog-box")
                    .visible(self.current_dialog.as_ref().is_some_and(|current_dialog| {
                        current_dialog.dialog_type == DialogType::PowerLevel
                    }))
                    .processing(self.busy)
                    .when_some(self.current_dialog.clone(), |david, dialog| {
                        let new_power_level = window
                            .use_state(cx, |_, _| dialog.acting_user.normalized_power_level());
                        let new_power_level_value = *new_power_level.read(cx);

                        let new_power_level_1 = new_power_level.clone();
                        let new_power_level_2 = new_power_level.clone();
                        let new_power_level_3 = new_power_level.clone();

                        let my_power_level =
                            self.current_user.as_ref().unwrap().normalized_power_level();

                        david
                            .title(tr!("POWER_LEVEL_UPDATE_TITLE", "Update Power Level").into())
                            .content(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(4.))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .child(tr!(
                                            "POWER_LEVEL_UPDATE_TEXT",
                                            "What power level do you want to assign to {{user}}?",
                                            user = dialog.acting_user.user_id().to_string()
                                        ))
                                            .when(
                                                my_power_level
                                                    >= RoomMemberRole::Administrator
                                                    .suggested_power_level(),
                                                |david| {
                                                    david.child(
                                                        radio_button("power-level-administrator")
                                                            .label(tr!(
                                                            "POWER_LEVEL_ADMINISTRATOR",
                                                            "Administrator"
                                                        ))
                                                            .when(
                                                                new_power_level_value
                                                                    == RoomMemberRole::Administrator
                                                                    .suggested_power_level(),
                                                                |david| david.checked(),
                                                            )
                                                            .on_checked_changed(move |_, _, cx| {
                                                                new_power_level_1.write(
                                                                    cx,
                                                                    RoomMemberRole::Administrator
                                                                        .suggested_power_level(),
                                                                )
                                                            }),
                                                    )
                                                },
                                            )
                                            .when(
                                                my_power_level
                                                    >= RoomMemberRole::Moderator
                                                    .suggested_power_level(),
                                                |david| {
                                                    david.child(
                                                        radio_button("power-level-moderator")
                                                            .label(tr!(
                                                            "POWER_LEVEL_MODERATOR",
                                                            "Moderator"
                                                        ))
                                                            .when(
                                                                new_power_level_value
                                                                    == RoomMemberRole::Moderator
                                                                    .suggested_power_level(),
                                                                |david| david.checked(),
                                                            )
                                                            .on_checked_changed(move |_, _, cx| {
                                                                new_power_level_2.write(
                                                                    cx,
                                                                    RoomMemberRole::Moderator
                                                                        .suggested_power_level(),
                                                                )
                                                            }),
                                                    )
                                                },
                                            )
                                            .when(
                                                my_power_level
                                                    >= RoomMemberRole::User.suggested_power_level(),
                                                |david| {
                                                    david.child(
                                                        radio_button("power-level-user")
                                                            .label(tr!("POWER_LEVEL_USER", "User"))
                                                            .when(
                                                                new_power_level_value
                                                                    == RoomMemberRole::User
                                                                    .suggested_power_level(),
                                                                |david| david.checked(),
                                                            )
                                                            .on_checked_changed(move |_, _, cx| {
                                                                new_power_level_3.write(
                                                                    cx,
                                                                    RoomMemberRole::User
                                                                        .suggested_power_level(),
                                                                )
                                                            }),
                                                    )
                                                },
                                            ),
                                    )
                                    .when(
                                        !dialog.acting_user.is_account_user()
                                            && new_power_level_value >= my_power_level,
                                        |david| {
                                            david.child(
                                                admonition()
                                                    .severity(AdmonitionSeverity::Warning)
                                                    .title(tr!("HEADS_UP"))
                                                    .child(tr!(
                                                "POWER_LEVEL_UPDATE_PROMOTION_WARNING",
                                                "Once this promotion takes effect, you will not \
                                                be able to undo it, as you will lack sufficient \
                                                permissions to change the power level of {{user}}.",
                                                user = dialog.acting_user.user_id().to_string()
                                            )),
                                            )
                                        },
                                    )
                                    .when(
                                        dialog.acting_user.is_account_user()
                                            && new_power_level_value < my_power_level,
                                        |david| {
                                            david.child(
                                                admonition()
                                                    .severity(AdmonitionSeverity::Warning)
                                                    .title(tr!("HEADS_UP"))
                                                    .child(tr!(
                                                    "POWER_LEVEL_UPDATE_DEMOTION_WARNING",
                                                    "Once this demotion takes effect, you will not \
                                                    be able to undo it, as you will lack \
                                                    sufficient permissions to promote yourself \
                                                    again."
                                                )),
                                            )
                                        },
                                    ),
                            )
                            .standard_button(
                                StandardButton::Cancel,
                                cx.listener(|this, _, _, cx| {
                                    this.current_dialog = None;
                                    cx.notify()
                                }),
                            )
                            .button(
                                button("set-power-level")
                                    .child(icon_text(
                                        "dialog-ok".into(),
                                        tr!("POWER_LEVEL_UPDATE_ACTION", "Set Power Level").into(),
                                    ))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.update_power_level(new_power_level_value, window, cx);
                                    })),
                            )
                    }),
            )
            .child(
                dialog_box("kick-dialog-box")
                    .visible(self.current_dialog.as_ref().is_some_and(|current_dialog| {
                        current_dialog.dialog_type == DialogType::Kick
                    }))
                    .processing(self.busy)
                    .when_some(self.current_dialog.clone(), |david, dialog| {
                        let reason_field = window.use_state(cx, |_, cx| {
                            let mut text_field = TextField::new("reason-field", cx);
                            text_field.set_placeholder(
                                tr!("MODERATION_ACTION_REASON_PLACEHOLDER", "Reason (optional)")
                                    .to_string()
                                    .as_str(),
                            );
                            text_field
                        });
                        let theme = cx.global::<Theme>();

                        david
                            .title(tr!("KICK_TITLE", "Kick").into())
                            .content(
                                div().flex().flex_col().gap(px(4.)).child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .w(px(500.))
                                        .child(tr!(
                                            "KICK_TEXT",
                                            "Do you want to kick {{user}}?",
                                            user = dialog.acting_user.user_id().to_string()
                                        ))
                                        .child(div().text_color(theme.foreground.disabled()).child(
                                            tr!(
                                                "KICK_DESCRIPTION",
                                                "They will leave the room, but can rejoin if the \
                                                room is public, or if they are re-invited."
                                            ),
                                        ))
                                        .child(reason_field.clone()),
                                ),
                            )
                            .standard_button(
                                StandardButton::Cancel,
                                cx.listener(|this, _, _, cx| {
                                    this.current_dialog = None;
                                    cx.notify()
                                }),
                            )
                            .button(
                                button("kick")
                                    .destructive()
                                    .child(icon_text(
                                        "im-kick-user".into(),
                                        tr!("KICK_ACTION", "Kick").into(),
                                    ))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        let reason = reason_field.read(cx).text().to_string();
                                        this.evict_user(reason.into(), false, window, cx);
                                    })),
                            )
                    }),
            )
            .child(
                dialog_box("ban-dialog-box")
                    .visible(self.current_dialog.as_ref().is_some_and(|current_dialog| {
                        current_dialog.dialog_type == DialogType::Ban
                    }))
                    .processing(self.busy)
                    .when_some(self.current_dialog.clone(), |david, dialog| {
                        let reason_field = window.use_state(cx, |_, cx| {
                            let mut text_field = TextField::new("reason-field", cx);
                            text_field.set_placeholder(
                                tr!("MODERATION_ACTION_REASON_PLACEHOLDER")
                                    .to_string()
                                    .as_str(),
                            );
                            text_field
                        });
                        let theme = cx.global::<Theme>();

                        david
                            .title(tr!("BAN_TITLE", "Ban").into())
                            .content(
                                div().flex().flex_col().gap(px(4.)).child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .w(px(500.))
                                        .child(tr!(
                                            "BAN_TEXT",
                                            "Do you want to ban {{user}}?",
                                            user = dialog.acting_user.user_id().to_string()
                                        ))
                                        .child(div().text_color(theme.foreground.disabled()).child(
                                            tr!(
                                                "BAN_DESCRIPTION",
                                                "They will leave the room and won't be able to \
                                                rejoin until their ban is lifted."
                                            ),
                                        ))
                                        .child(reason_field.clone()),
                                ),
                            )
                            .standard_button(
                                StandardButton::Cancel,
                                cx.listener(|this, _, _, cx| {
                                    this.current_dialog = None;
                                    cx.notify()
                                }),
                            )
                            .button(
                                button("ban")
                                    .destructive()
                                    .child(icon_text(
                                        "im-ban-user".into(),
                                        tr!("BAN_ACTION", "Ban").into(),
                                    ))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        let reason = reason_field.read(cx).text().to_string();
                                        this.evict_user(reason.into(), true, window, cx);
                                    })),
                            )
                    }),
            )
    }
}
