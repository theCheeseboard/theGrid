use crate::chat::chat_room::open_room::OpenRoom;
use crate::mxc_image::{SizePolicy, mxc_image};
use cntp_i18n::tr;
use contemporary::components::button::button;
use contemporary::components::constrainer::constrainer;
use contemporary::components::context_menu::ContextMenuItem;
use contemporary::components::dialog_box::{StandardButton, dialog_box};
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon::icon;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::subtitle::subtitle;
use contemporary::components::switch::switch;
use contemporary::components::text_field::TextField;
use contemporary::styling::theme::{Theme, VariableColor};
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AsyncApp, ClickEvent, Context, Entity, IntoElement, ParentElement, Render, Styled,
    WeakEntity, Window, div, px,
};
use matrix_sdk::ruma::api::client::room::Visibility;
use matrix_sdk::ruma::room::JoinRule;
use matrix_sdk::{EncryptionState, RoomInfo};
use std::rc::Rc;
use thegrid::session::session_manager::SessionManager;
use thegrid::tokio_helper::TokioHelper;

pub struct RoomSettings {
    open_room: Entity<OpenRoom>,
    on_back_click: Rc<Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
    edit_room_name_open: bool,
    new_name_text_field: Entity<TextField>,
    enable_encryption_open: bool,
    busy: bool,
    published_to_directory: bool,
}

impl RoomSettings {
    pub fn new(
        open_room: Entity<OpenRoom>,
        on_back_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
        cx: &mut Context<RoomSettings>,
    ) -> Self {
        cx.observe(&open_room, |this, open_room, cx| {
            if let Some(room) = open_room.read(cx).room.as_ref() {
                let room = room.clone();
                cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                    let room_visibility = cx
                        .spawn_tokio(
                            async move { room.privacy_settings().get_room_visibility().await },
                        )
                        .await;

                    if let Ok(room_visibility) = room_visibility {
                        let _ = this.update(cx, |this, cx| {
                            this.published_to_directory = room_visibility == Visibility::Public;
                        });
                    }
                })
                .detach();
            }
        })
        .detach();

        Self {
            open_room,
            on_back_click: Rc::new(Box::new(on_back_click)),

            new_name_text_field: TextField::new(
                cx,
                "new-name",
                "".into(),
                tr!("ROOM_NAME_PLACEHOLDER", "Room Name").into(),
            ),

            edit_room_name_open: false,
            enable_encryption_open: false,
            busy: false,
            published_to_directory: false,
        }
    }

    pub fn set_room_join_rule(&mut self, join_rule: JoinRule, cx: &mut Context<Self>) {
        let room = self.open_room.read(cx).room.clone().unwrap();
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            if cx
                .spawn_tokio(
                    async move { room.privacy_settings().update_join_rule(join_rule).await },
                )
                .await
                .is_err()
            {
                this.update(cx, |this, cx| {
                    // TODO: Show the error
                    this.busy = false;
                    cx.notify()
                })
            } else {
                this.update(cx, |this, cx| {
                    this.busy = false;
                    cx.notify()
                })
            }
        })
        .detach();
    }

    pub fn toggle_room_publish_to_directory(&mut self, cx: &mut Context<Self>) {
        let was_published = self.published_to_directory;
        
        let room = self.open_room.read(cx).room.clone().unwrap();
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            if cx
                .spawn_tokio(
                    async move { room.privacy_settings().update_room_visibility(if was_published {
                        Visibility::Private
                    } else {
                        Visibility::Public
                    }).await },
                )
                .await
                .is_err()
            {
                let _ = this.update(cx, |this, cx| {
                    // TODO: Show the error
                    this.published_to_directory = was_published;
                    cx.notify()
                });
            }
        })
            .detach();
        
        self.published_to_directory = !self.published_to_directory;
        cx.notify()
    }
}

impl Render for RoomSettings {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let on_back_click = self.on_back_click.clone();
        let theme = cx.global::<Theme>();

        let Some(room) = self.open_room.read(cx).room.as_ref() else {
            return div();
        };
        let room = room.clone();
        let room_2 = room.clone();
        let room_3 = room.clone();

        div()
            .flex()
            .flex_col()
            .bg(theme.background)
            .size_full()
            .child(
                grandstand("room-settings-grandstand")
                    .text(tr!("ROOM_SETTINGS", "Room Settings"))
                    .pt(px(36.))
                    .on_back_click(move |event, window, cx| {
                        on_back_click.clone()(event, window, cx);
                    }),
            )
            .child(
                constrainer("profile")
                    .flex()
                    .flex_col()
                    .w_full()
                    .p(px(8.))
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .gap(px(4.))
                            .child(
                                mxc_image(room.avatar_url())
                                    .rounded(theme.border_radius)
                                    .size(px(48.))
                                    .size_policy(SizePolicy::Fit),
                            )
                            .child(
                                div().flex().flex_col().justify_center().gap(px(4.)).child(
                                    room.cached_display_name()
                                        .map(|name| name.to_string())
                                        .or_else(|| room.name())
                                        .unwrap_or_default(),
                                ),
                            ),
                    )
                    .child(
                        layer()
                            .flex()
                            .flex_col()
                            .p(px(8.))
                            .w_full()
                            .child(subtitle(tr!("ACTIONS")))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .bg(theme.button_background)
                                    .rounded(theme.border_radius)
                                    .child(
                                        button("room-change-display-name")
                                            .child(icon_text(
                                                "edit-rename".into(),
                                                tr!("ROOM_CHANGE_NAME", "Change Room Name").into(),
                                            ))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                // TODO: Set the text field text to the current display name
                                                this.edit_room_name_open = true;
                                                cx.notify()
                                            })),
                                    )
                                    .child(button("room-change-profile-picture").child(icon_text(
                                        "edit-rename".into(),
                                        tr!("ROOM_CHANGE_PICTURE", "Change Room Picture").into(),
                                    )))
                                    .child(button("room-view-members").child(icon_text(
                                        "user".into(),
                                        tr!("ROOM_VIEW_MEMBERS", "Manage Room Members").into(),
                                    ))),
                            ),
                    )
                    .child(
                        layer()
                            .flex()
                            .flex_col()
                            .p(px(8.))
                            .w_full()
                            .child(subtitle(tr!("ROOM_ENCRYPTION", "Room Encryption")))
                            .when_else(
                                room.encryption_state().is_encrypted(),
                                |david| {
                                    david.child(tr!(
                                        "ROOM_ENCRYPTION_ENABLED_TEXT",
                                        "Room encryption is enabled. Messages in this room cannot \
                                        be seen by anyone else - not even the homeserver \
                                        administrators."
                                    ))
                                },
                                |david| {
                                    david
                                        .child(tr!(
                                            "ROOM_ENCRYPTION_DISABLED_TEXT",
                                            "Room encryption is disabled."
                                        ))
                                        .child(
                                            button("room-encryption-enable")
                                                .child(tr!(
                                                    "ROOM_ENCRYPTION_ENABLE",
                                                    "Enable Encryption"
                                                ))
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.enable_encryption_open = true;
                                                    cx.notify();
                                                })),
                                        )
                                },
                            ),
                    )
                    .child(
                        layer()
                            .flex()
                            .flex_col()
                            .p(px(8.))
                            .w_full()
                            .child(subtitle(tr!(
                                "ROOM_ACCESS_VISIBILITY",
                                "Access and Visibility"
                            )))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.))
                                    .child(
                                        layer()
                                            .p(px(4.))
                                            .gap(px(4.))
                                            .items_center()
                                            .flex()
                                            .child(tr!("ROOM_ACCESS", "Room Access"))
                                            .child(div().flex_grow())
                                            .child(match room.join_rule() {
                                                Some(JoinRule::Public) => {
                                                    tr!("ROOM_ACCESS_PUBLIC", "Public")
                                                }
                                                Some(JoinRule::Knock) => {
                                                    tr!(
                                                        "ROOM_ACCESS_KNOCK_INVITE",
                                                        "Knock & Invite"
                                                    )
                                                }
                                                Some(JoinRule::Invite) => {
                                                    tr!("ROOM_ACCESS_INVITE_ONLY", "Invite Only")
                                                }
                                                _ => {
                                                    tr!("ROOM_ACCESS_UNKNOWN", "Unknown")
                                                }
                                            })
                                            .child(
                                                button("change-room-access")
                                                    .child(icon("arrow-down".into()))
                                                    .with_menu(vec![
                                                        ContextMenuItem::menu_item()
                                                            .label(tr!("ROOM_ACCESS_INVITE_ONLY"))
                                                            .on_triggered(cx.listener(
                                                                |this, _, _, cx| {
                                                                    this.set_room_join_rule(
                                                                        JoinRule::Invite,
                                                                        cx,
                                                                    )
                                                                },
                                                            ))
                                                            .build(),
                                                        ContextMenuItem::menu_item()
                                                            .label(tr!("ROOM_ACCESS_KNOCK_INVITE"))
                                                            .on_triggered(cx.listener(
                                                                |this, _, _, cx| {
                                                                    this.set_room_join_rule(
                                                                        JoinRule::Knock,
                                                                        cx,
                                                                    )
                                                                },
                                                            ))
                                                            .build(),
                                                        ContextMenuItem::menu_item()
                                                            .label(tr!("ROOM_ACCESS_PUBLIC"))
                                                            .on_triggered(cx.listener(
                                                                |this, _, _, cx| {
                                                                    this.set_room_join_rule(
                                                                        JoinRule::Public,
                                                                        cx,
                                                                    )
                                                                },
                                                            ))
                                                            .build(),
                                                    ]),
                                            ),
                                    )
                                    .child(
                                        layer()
                                            .p(px(4.))
                                            .gap(px(4.))
                                            .items_center()
                                            .flex()
                                            .child(tr!(
                                                "ROOM_PUBLISH_TO_DIRECTORY",
                                                "Publish to Server Directory"
                                            ))
                                            .child(div().flex_grow())
                                            .child(
                                                switch("publish-to-server-directory")
                                                    .when(self.published_to_directory, |david| {
                                                        david.checked()
                                                    })
                                                    .on_change(cx.listener(|this, _, _, cx| {
                                                        this.toggle_room_publish_to_directory(cx);
                                                    })),
                                            ),
                                    ),
                            ),
                    ),
            )
            .child(
                dialog_box("edit-room-name")
                    .visible(self.edit_room_name_open)
                    .processing(self.busy)
                    .title(tr!("ROOM_CHANGE_NAME").into())
                    .content(
                        div()
                            .flex()
                            .flex_col()
                            .w(px(500.))
                            .gap(px(12.))
                            .child(tr!(
                                "ROOM_CHANGE_NAME_DESCRIPTION",
                                "What do you want to call this room?"
                            ))
                            .child(self.new_name_text_field.clone().into_any_element()),
                    )
                    .standard_button(
                        StandardButton::Cancel,
                        cx.listener(|this, _, _, cx| {
                            this.edit_room_name_open = false;
                            cx.notify()
                        }),
                    )
                    .button(
                        button("change-room_name-button")
                            .child(icon_text(
                                "dialog-ok".into(),
                                tr!("ROOM_CHANGE_NAME").into(),
                            ))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let room = room_2.clone();
                                let new_display_name =
                                    this.new_name_text_field.read(cx).current_text(cx);

                                this.busy = true;
                                cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                                    if cx
                                        .spawn_tokio(async move {
                                            room.set_name(new_display_name.to_string()).await
                                        })
                                        .await
                                        .is_err()
                                    {
                                        this.update(cx, |this, cx| {
                                            // TODO: Show the error
                                            this.busy = false;
                                            cx.notify()
                                        })
                                    } else {
                                        this.update(cx, |this, cx| {
                                            this.edit_room_name_open = false;
                                            this.busy = false;
                                            cx.notify()
                                        })
                                    }
                                })
                                .detach();
                                cx.notify()
                            })),
                    ),
            )
            .child(
                dialog_box("enable-encryption")
                    .visible(self.enable_encryption_open)
                    .processing(self.busy)
                    .title(tr!("ROOM_ENCRYPTION_ENABLE").into())
                    .content(tr!(
                        "ROOM_ENCRYPTION_ENABLE_DESCRIPTION",
                        "By enabling encryption, you will prevent anyone joining the room \
                            from being able to read message history. If there are any bots or \
                            services in this room, they may also stop working.\n\n\
                            Once enabled, encryption cannot be turned off.\n\n\
                            Do you want to enable encryption for this room?"
                    ))
                    .standard_button(
                        StandardButton::Cancel,
                        cx.listener(|this, _, _, cx| {
                            this.enable_encryption_open = false;
                            cx.notify()
                        }),
                    )
                    .button(
                        button("encryption-enable-button")
                            .destructive()
                            .child(icon_text(
                                "dialog-ok".into(),
                                tr!("ROOM_ENCRYPTION_ENABLE").into(),
                            ))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let room = room_3.clone();

                                this.busy = true;
                                cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                                    if cx
                                        .spawn_tokio(async move { room.enable_encryption().await })
                                        .await
                                        .is_err()
                                    {
                                        this.update(cx, |this, cx| {
                                            // TODO: Show the error
                                            this.busy = false;
                                            cx.notify()
                                        })
                                    } else {
                                        this.update(cx, |this, cx| {
                                            this.enable_encryption_open = false;
                                            this.busy = false;
                                            cx.notify()
                                        })
                                    }
                                })
                                .detach();
                                cx.notify()
                            })),
                    ),
            )
    }
}
