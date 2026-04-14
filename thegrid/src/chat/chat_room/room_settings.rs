mod room_replace_popover;

use crate::chat::chat_room::open_room::OpenRoom;
use crate::chat::chat_room::room_settings::room_replace_popover::{
    RoomReplaceEvent, RoomReplacePopover,
};
use crate::chat::displayed_room::DisplayedRoom;
use crate::upload_mxc_dialog::{UploadMxcAcceptEvent, upload_mxc_dialog};
use cntp_i18n::{I18nString, tr};
use contemporary::components::button::{ButtonMenuOpenPolicy, button};
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
use contemporary::components::toast::Toast;
use contemporary::styling::theme::{Theme, VariableColor};
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, AsyncApp, ClickEvent, Context, ElementId, Entity, InteractiveElement,
    IntoElement, ParentElement, Render, Styled, WeakEntity, Window, div, px,
};
use matrix_sdk::ruma::api::client::room::Visibility;
use matrix_sdk::ruma::events::room::avatar::ImageInfo;
use matrix_sdk::ruma::room::JoinRule;
use matrix_sdk::ruma::{OwnedRoomAliasId, RoomAliasId, UInt};
use std::rc::Rc;
use thegrid_common::mxc_image::{SizePolicy, mxc_image};
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::tokio_helper::TokioHelper;

pub struct RoomSettings {
    open_room: Entity<OpenRoom>,
    on_back_click: Rc<Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
    on_members_click: Rc<Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
    edit_room_name_open: bool,
    edit_room_image_open: bool,
    new_name_text_field: Entity<TextField>,
    enable_encryption_open: bool,
    busy: bool,
    published_to_directory: bool,

    add_alias_open: bool,
    add_alias_text_field: Entity<TextField>,
}

impl RoomSettings {
    pub fn new(
        open_room: Entity<OpenRoom>,
        on_back_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
        on_members_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
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
            on_members_click: Rc::new(Box::new(on_members_click)),

            new_name_text_field: cx.new(|cx| {
                let mut text_field = TextField::new("new-name", cx);
                text_field.set_placeholder(
                    tr!("ROOM_NAME_PLACEHOLDER", "Room Name")
                        .to_string()
                        .as_str(),
                );
                text_field
            }),

            edit_room_name_open: false,
            edit_room_image_open: false,
            enable_encryption_open: false,
            busy: false,
            published_to_directory: false,

            add_alias_open: false,
            add_alias_text_field: cx.new(|cx| {
                let mut text_field = TextField::new("new-alias", cx);
                text_field.set_has_border(false);
                text_field.set_placeholder(tr!("ALIAS_PLACEHOLDER", "alias").to_string().as_str());
                text_field
            }),
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
                .spawn_tokio(async move {
                    room.privacy_settings()
                        .update_room_visibility(if was_published {
                            Visibility::Private
                        } else {
                            Visibility::Public
                        })
                        .await
                })
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

    fn render_room_aliases<'a>(
        &mut self,
        window: &mut Window,
        cx: &'a mut Context<'_, Self>,
    ) -> impl IntoElement {
        let loading = window.use_state(cx, |_, _| false);

        let theme = cx.global::<Theme>();

        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx).clone();
        let server_name = client.user_id().unwrap().server_name().to_owned();

        let open_room = self.open_room.read(cx);
        let room = self.open_room.read(cx).room.as_ref().unwrap();

        let is_space = room.is_space();

        let canonical_alias = room.canonical_alias();
        let alt_aliases = room.alt_aliases();
        let local_aliases = open_room.local_aliases();

        let mut public_aliases = Vec::new();
        if let Some(alias) = &canonical_alias {
            public_aliases.push(alias.clone());
        }
        public_aliases.extend(alt_aliases.clone());

        let mut all_aliases = public_aliases.clone();
        for local_alias in local_aliases.iter() {
            if !all_aliases.contains(local_alias) {
                all_aliases.push(local_alias.clone())
            }
        }

        layer()
            .flex()
            .flex_col()
            .p(px(8.))
            .w_full()
            .child(subtitle(tr!("ROOM_ALIASES", "Aliases")))
            .child(if is_space {
                tr!(
                    "ROOM_ALIASES_DESCRIPTION_SPACE",
                    "Aliases can be used to join this space directly, \
                    if the access policy is set to Public. Public aliases \
                    can be used by anyone, whilst non-public aliases can only \
                    be used by users on your homeserver."
                )
            } else {
                tr!(
                    "ROOM_ALIASES_DESCRIPTION",
                    "Aliases can be used to join this room directly, \
                    if the access policy is set to Public. Public aliases \
                    can be used by anyone, whilst non-public aliases can only \
                    be used by users on your homeserver."
                )
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(all_aliases.iter().fold(
                        div().flex().flex_col().gap(px(2.)),
                        |david, alias| {
                            let remove_listener = cx.listener({
                                let loading = loading.clone();
                                let alias = alias.clone();
                                move |this, _, window, cx| {
                                    let loading = loading.clone();
                                    let alias = alias.clone();
                                    loading.write(cx, true);

                                    let alias_clone = alias.clone();
                                    let callback = cx.listener(move |this, result, window, cx| {
                                        if let Err(e) = result {
                                            Toast::new()
                                                .title(&tr!(
                                                    "ROOM_ALIAS_REMOVE_ERROR_TITLE",
                                                    "Unable to remove room alias"
                                                ))
                                                .body(&tr!(
                                                    "ROOM_ALIAS_REMOVE_ERROR_TEXT",
                                                    "Could not remove the room alias {{alias}}",
                                                    alias:quote = alias_clone
                                                ))
                                                .post(window, cx)
                                        };

                                        let _ = loading.write(cx, false);
                                    });
                                    this.open_room.update(cx, |open_room, cx| {
                                        open_room
                                            .unpublish_local_alias(alias, callback, window, cx);
                                    });
                                }
                            });

                            let modify_public_aliases =
                                |canonical_alias: Option<OwnedRoomAliasId>,
                                 alt_aliases: Vec<OwnedRoomAliasId>,
                                 error_message_title: I18nString,
                                 error_message: I18nString,
                                 cx: &'a Context<Self>| {
                                    let loading = loading.clone();
                                    cx.listener(move |this: &mut Self, _, window: &mut Window, cx: &mut Context<Self>| {
                                        let loading = loading.clone();
                                        let canonical_alias = canonical_alias.clone();
                                        let alt_aliases = alt_aliases.clone();
                                        let error_message_title = error_message_title.clone();
                                        let error_message = error_message.clone();
                                        loading.write(cx, true);
                                        let callback =
                                            cx.listener(move |this, result, window, cx| {
                                                match result {
                                                    Ok(_) => {
                                                        cx.notify();
                                                    }
                                                    Err(e) => Toast::new()
                                                        .title(&error_message_title)
                                                        .body(&error_message)
                                                        .post(window, cx),
                                                }
                                                let _ = loading.write(cx, false);
                                            });
                                        this.open_room.update(cx, |open_room, cx| {
                                            open_room.publish_public_aliases(
                                                canonical_alias,
                                                alt_aliases,
                                                callback,
                                                window,
                                                cx,
                                            );
                                        });
                                    })
                                };

                            let is_canonical = canonical_alias
                                .as_ref()
                                .is_some_and(|canonical_alias| canonical_alias == alias);
                            let is_public = public_aliases.contains(&alias);
                            let mut menu = vec![
                                ContextMenuItem::separator()
                                    .label(if is_public {
                                        tr!(
                                            "ALIAS_PUBLIC_CONTEXT_MENU_TITLE",
                                            "For public alias {{alias}}",
                                            alias:quote = alias
                                        )
                                    } else {
                                        tr!(
                                            "ALIAS_CONTEXT_MENU_TITLE",
                                            "For alias {{alias}}",
                                            alias:quote = alias
                                        )
                                    })
                                    .build(),
                            ];
                            if is_public {
                                if is_canonical {
                                    menu.push(
                                        ContextMenuItem::menu_item()
                                            .label(tr!("ALIAS_UNSET_CANONICAL", "Remove Main"))
                                            .on_triggered({
                                                let canonical_alias = None;
                                                let mut alt_aliases = alt_aliases.clone();
                                                alt_aliases.push(alias.clone());
                                                let alias = alias.clone();

                                                modify_public_aliases(
                                                    canonical_alias,
                                                    alt_aliases,
                                                    tr!(
                                                        "ALIAS_UNSET_CANONICAL_ERROR_TITLE",
                                                        "Unable to remove the main alias"
                                                    ),
                                                    tr!(
                                                        "ALIAS_UNSET_CANONICAL_ERROR_MESSAGE",
                                                        "Could not remove the main alias from \
                                                        the room",
                                                    ),
                                                    cx,
                                                )
                                            })
                                            .build(),
                                    )
                                } else {
                                    menu.push(
                                        ContextMenuItem::menu_item()
                                            .label(tr!("ALIAS_MAKE_CANONICAL", "Make Main"))
                                            .on_triggered({
                                                let mut canonical_alias = canonical_alias.clone();
                                                let old_canonical_alias = canonical_alias.replace(alias.clone());
                                                let mut alt_aliases = alt_aliases.clone();
                                                if let Some(alias) = old_canonical_alias {
                                                    alt_aliases.push(alias);
                                                }
                                                let alias = alias.clone();
                                                alt_aliases.retain(|alt_alias| alt_alias != &alias);

                                                modify_public_aliases(
                                                    canonical_alias,
                                                    alt_aliases,
                                                    tr!(
                                                        "ALIAS_MAKE_CANONICAL_ERROR_TITLE",
                                                        "Unable to set main alias"
                                                    ),
                                                    tr!(
                                                        "ALIAS_MAKE_CANONICAL_ERROR_MESSAGE",
                                                        "Could not set the alias {{alias}} \
                                                        as the main alias for the room",
                                                        alias:quote = alias
                                                    ),
                                                    cx,
                                                )
                                            })
                                            .build(),
                                    )
                                }
                                menu.push(
                                    ContextMenuItem::menu_item()
                                        .label(tr!("ALIAS_UNPUBLISH", "Unpublish"))
                                        .on_triggered({
                                            let alias = alias.clone();
                                            let canonical_alias =
                                                canonical_alias
                                                    .clone()
                                                    .and_then(|canonical_alias|
                                                        if canonical_alias == alias {
                                                            None
                                                        } else {
                                                            Some(canonical_alias)
                                                        });
                                            let mut alt_aliases = alt_aliases.clone();
                                            alt_aliases.retain(|alt_alias| alt_alias != &alias);

                                            modify_public_aliases(
                                                canonical_alias,
                                                alt_aliases,
                                                tr!(
                                                    "ALIAS_UNPUBLISH_ERROR_TITLE",
                                                    "Unable to unpublish alias"
                                                ),
                                                tr!(
                                                    "ALIAS_UNPUBLISH_ERROR_MESSAGE",
                                                    "Could not unpublish the alias {{alias}} \
                                                    from the room",
                                                    alias:quote = alias
                                                ),
                                                cx,
                                            )
                                        })
                                        .build(),
                                )
                            } else {
                                menu.push(
                                    ContextMenuItem::menu_item()
                                        .label(tr!("ALIAS_MAKE_PUBLIC", "Make Public"))
                                        .on_triggered({
                                            let canonical_alias = canonical_alias.clone();
                                            let mut alt_aliases = alt_aliases.clone();
                                            alt_aliases.push(alias.clone());
                                            let alias = alias.clone();

                                            modify_public_aliases(
                                                canonical_alias,
                                                alt_aliases,
                                                tr!(
                                                    "ALIAS_MAKE_PUBLIC_ERROR_TITLE",
                                                    "Unable to make public alias"
                                                ),
                                                tr!(
                                                    "ALIAS_MAKE_PUBLIC_ERROR_MESSAGE",
                                                    "Could not publish the alias {{alias}} \
                                                    to the room",
                                                    alias:quote = alias
                                                ),
                                                cx,
                                            )
                                        })
                                        .build(),
                                )
                            }
                            if alias.server_name() == &server_name {
                                menu.push(ContextMenuItem::separator().build());
                                menu.push(ContextMenuItem::menu_item().label(tr!(
                                    "ROOM_ALIAS_REMOVE",
                                    "Remove Alias"
                                ))
                                    .icon("edit-delete").on_triggered(remove_listener).build());
                            }

                            david.child(
                                div().id(ElementId::Name(alias.to_string().into())).child(
                                    layer()
                                        .p(px(4.))
                                        .gap(px(4.))
                                        .flex()
                                        .items_center()
                                        .child(alias.to_string())
                                        .when(is_canonical, |david| {
                                            david.child(
                                                div()
                                                    .rounded(theme.border_radius)
                                                    .bg(theme.info_accent_color)
                                                    .p(px(2.))
                                                    .child(tr!("ROOM_ALIAS_CANONICAL", "Main")),
                                            )
                                        })
                                        .when(is_public, |david| {
                                            david.child(
                                                div()
                                                    .rounded(theme.border_radius)
                                                    .bg(theme.warning_accent_color)
                                                    .p(px(2.))
                                                    .child(tr!("ROOM_ALIAS_PUBLIC", "Public")),
                                            )
                                        })
                                        .child(div().flex_grow())
                                        .child(
                                            button("overflow-menu")
                                                .child(icon("application-menu"))
                                                .when(*loading.read(cx), |david| {
                                                    david.disabled()
                                                })
                                                .with_menu(menu)
                                                .with_menu_open_policy(
                                                    ButtonMenuOpenPolicy::AnyClick,
                                                )
                                        ),
                                ),
                            )
                        },
                    ))
                    .child(
                        button("add-alias")
                            .child(icon_text(
                                "list-add",
                                tr!("ROOM_ALIAS_ADD", "Add Alias"),
                            ))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.add_alias_open = true;
                                this.add_alias_text_field.update(cx, |text_field, cx| {
                                    text_field.set_text("");
                                });
                                cx.notify()
                            })),
                    ),
            )
            .child(
                dialog_box("add-alias")
                    .visible(self.add_alias_open)
                    .processing(*loading.read(cx))
                    .title(tr!("ROOM_ALIAS_ADD"))
                    .content(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(if is_space {
                                tr!(
                                    "ROOM_ALIAS_DESCRIPTION_SPACE",
                                    "What alias do you want to set for this space?"
                                )
                            } else {
                                tr!(
                                    "ROOM_ALIAS_DESCRIPTION",
                                    "What alias do you want to set for this room?"
                                )
                            })
                            .child(
                                layer()
                                    .flex()
                                    .items_center()
                                    .p(px(4.))
                                    .child("#")
                                    .child(self.add_alias_text_field.clone())
                                    .child(format!(":{server_name}")),
                            ),
                    )
                    .standard_button(
                        StandardButton::Cancel,
                        cx.listener(|this, _, _, cx| {
                            this.add_alias_open = false;
                            cx.notify()
                        }),
                    )
                    .button(
                        button("add-alias")
                            .child(icon_text("list-add", tr!("ROOM_ALIAS_ADD")))
                            .on_click(cx.listener({
                                let loading = loading.clone();
                                let server_name = server_name.clone();
                                move |this, _, window, cx| {
                                    let loading = loading.clone();
                                    let server_name = server_name.clone();

                                    let Ok(alias) = RoomAliasId::parse(format!(
                                        "#{}:{}",
                                        this.add_alias_text_field.read(cx).text(),
                                        server_name
                                    )) else {
                                        this.add_alias_text_field.update(cx, |text_field, cx| {
                                            text_field.flash_error(window, cx);
                                        });
                                        return;
                                    };

                                    loading.write(cx, true);
                                    let callback = cx.listener(move |this, result, _, cx| {
                                        match result {
                                            Ok(_) => {
                                                this.add_alias_open = false;
                                                cx.notify();
                                            }
                                            Err(e) => {
                                                // TODO: Show error
                                            }
                                        }
                                        let _ = loading.write(cx, false);
                                    });
                                    this.open_room.update(cx, |open_room, cx| {
                                        open_room.publish_local_alias(alias, callback, window, cx);
                                    });
                                }
                            })),
                    ),
            )
    }

    fn render_room_replace(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let on_replace = cx.listener(|this, event: &RoomReplaceEvent, window, cx| {
            let displayed_room = this.open_room.read(cx).displayed_room.clone();
            displayed_room.write(cx, DisplayedRoom::Room(event.new_room_id.clone()));
        });
        let room = self.open_room.read(cx).room.as_ref().unwrap().clone();
        let replace_popover =
            window.use_state(cx, |_, cx| RoomReplacePopover::new(room, on_replace, cx));

        layer()
            .flex()
            .flex_col()
            .p(px(8.))
            .w_full()
            .child(subtitle(tr!("ROOM_REPLACE", "Replace Room")))
            .child(tr!(
                "ROOM_REPLACE_DESCRIPTION",
                "Replacing the room can be done to reset the state of the room if the room is \
                unstable. It can also be used to upgrade the room to a new version to take \
                advantage of new features and improvements in newer room versions."
            ))
            .child(
                button("room-replace-button")
                    .child(icon_text("im-room", tr!("ROOM_REPLACE")))
                    .on_click({
                        let replace_popover = replace_popover.clone();
                        move |_, _, cx| {
                            replace_popover.update(cx, |update_popover, cx| {
                                update_popover.open(cx);
                            });
                        }
                    }),
            )
            .child(replace_popover.clone())
    }
}

impl Render for RoomSettings {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let on_back_click = self.on_back_click.clone();
        let theme = cx.global::<Theme>();

        let Some(room) = self.open_room.read(cx).room.as_ref() else {
            return div();
        };

        let is_space = room.is_space();

        let room = room.clone();
        let room_2 = room.clone();
        let room_3 = room.clone();

        let room_name = room
            .cached_display_name()
            .map(|name| name.to_string())
            .or_else(|| room.name())
            .unwrap_or_default();

        div()
            .flex()
            .flex_col()
            .bg(theme.background)
            .size_full()
            .child(
                grandstand("room-settings-grandstand")
                    .text(if is_space {
                        tr!("SPACE_SETTINGS", "Space Settings")
                    } else {
                        tr!("ROOM_SETTINGS", "Room Settings")
                    })
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
                                    .fallback_image(room.room_id())
                                    .rounded(theme.border_radius)
                                    .size_policy(SizePolicy::Constrain(48., 48.)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .justify_center()
                                    .gap(px(4.))
                                    .child(room_name.clone())
                                    .when(!is_space, |david| {
                                        david.child(div().flex().when_else(
                                            room.encryption_state().is_encrypted(),
                                            |david| {
                                                david.child(
                                                    div()
                                                        .rounded(theme.border_radius)
                                                        .bg(theme.info_accent_color)
                                                        .p(px(2.))
                                                        .child(tr!(
                                                            "ROOM_ENCRYPTION_BADGE",
                                                            "Encrypted",
                                                        )),
                                                )
                                            },
                                            |david| {
                                                david.child(
                                                    div()
                                                        .rounded(theme.border_radius)
                                                        .bg(theme.warning_accent_color)
                                                        .p(px(2.))
                                                        .child(tr!(
                                                            "ROOM_NO_ENCRYPTION_BADGE",
                                                            "Not Encrypted",
                                                        )),
                                                )
                                            },
                                        ))
                                    }),
                            ),
                    )
                    .when_some(room.topic(), |david, topic| {
                        david.child(div().text_color(theme.foreground.disabled()).child(topic))
                    })
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
                                                "edit-rename",
                                                tr!("ROOM_CHANGE_NAME", "Change Name"),
                                            ))
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.new_name_text_field.update(
                                                    cx,
                                                    |text_field, cx| {
                                                        text_field.set_text(room_name.as_str());
                                                    },
                                                );
                                                this.edit_room_name_open = true;
                                                cx.notify()
                                            })),
                                    )
                                    .child(
                                        button("room-change-profile-picture")
                                            .child(icon_text(
                                                "edit-rename",
                                                tr!("ROOM_CHANGE_PICTURE", "Change Picture"),
                                            ))
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.edit_room_image_open = true;
                                                cx.notify()
                                            })),
                                    )
                                    .child(
                                        button("room-view-members")
                                            .child(icon_text(
                                                "user",
                                                tr!("ROOM_VIEW_MEMBERS", "Manage Members"),
                                            ))
                                            .on_click(cx.listener(|this, event, window, cx| {
                                                (this.on_members_click)(event, window, cx);
                                            })),
                                    )
                                    .when(
                                        !room.encryption_state().is_encrypted() && !is_space,
                                        |david| {
                                            david.child(
                                                button("room-encryption-enable")
                                                    .child(icon_text(
                                                        "padlock",
                                                        tr!(
                                                            "ROOM_ENCRYPTION_ENABLE",
                                                            "Enable Encryption"
                                                        ),
                                                    ))
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.enable_encryption_open = true;
                                                        cx.notify();
                                                    })),
                                            )
                                        },
                                    ),
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
                                            .child(tr!("ROOM_ACCESS", "Access Policy"))
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
                                                    .child(icon("arrow-down"))
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
                    )
                    .child(self.render_room_aliases(window, cx))
                    .child(self.render_room_replace(window, cx)),
            )
            .child(
                dialog_box("edit-room-name")
                    .visible(self.edit_room_name_open)
                    .processing(self.busy)
                    .title(tr!("ROOM_CHANGE_NAME"))
                    .content(
                        div()
                            .flex()
                            .flex_col()
                            .w(px(500.))
                            .gap(px(12.))
                            .child(if is_space {
                                tr!(
                                    "SPACE_CHANGE_NAME_DESCRIPTION",
                                    "What do you want to call this space?"
                                )
                            } else {
                                tr!(
                                    "ROOM_CHANGE_NAME_DESCRIPTION",
                                    "What do you want to call this room?"
                                )
                            })
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
                            .child(icon_text("dialog-ok", tr!("ROOM_CHANGE_NAME")))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let room = room_2.clone();
                                let new_display_name =
                                    this.new_name_text_field.read(cx).text().to_string();

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
            .child(upload_mxc_dialog(
                tr!("ROOM_CHANGE_PICTURE"),
                self.edit_room_image_open,
                "dialog-ok".into(),
                tr!("ROOM_CHANGE_PICTURE").into(),
                cx.listener(move |this, _, _, cx| {
                    this.edit_room_image_open = false;
                    cx.notify();
                }),
                cx.listener({
                    let room = room.clone();
                    move |this, event: &UploadMxcAcceptEvent, _, cx| {
                        let mxc_url = event.mxc_url.clone();

                        let mut image_info = ImageInfo::new();
                        image_info.height = UInt::new(event.height);
                        image_info.width = UInt::new(event.width);
                        image_info.blurhash = event.blur_hash.clone();
                        image_info.size = UInt::new(event.file_size);
                        cx.spawn({
                            let room = room.clone();
                            async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                                if cx
                                    .spawn_tokio(async move {
                                        room.set_avatar_url(&mxc_url, Some(image_info)).await
                                    })
                                    .await
                                    .is_err()
                                {
                                    this.update(cx, |this, cx| {
                                        this.edit_room_image_open = false;
                                        cx.notify()
                                    })
                                } else {
                                    this.update(cx, |this, cx| {
                                        this.edit_room_image_open = false;
                                        cx.notify()
                                    })
                                }
                            }
                        })
                        .detach();
                        cx.notify()
                    }
                }),
            ))
            .child(
                dialog_box("enable-encryption")
                    .visible(self.enable_encryption_open)
                    .processing(self.busy)
                    .title(tr!("ROOM_ENCRYPTION_ENABLE"))
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
                            .child(icon_text("dialog-ok", tr!("ROOM_ENCRYPTION_ENABLE")))
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
