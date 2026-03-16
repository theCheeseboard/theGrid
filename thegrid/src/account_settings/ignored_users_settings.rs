use cntp_i18n::tr;
use contemporary::components::admonition::AdmonitionSeverity;
use contemporary::components::button::button;
use contemporary::components::constrainer::constrainer;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon::icon;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::subtitle::subtitle;
use contemporary::components::text_field::TextField;
use contemporary::components::toast::Toast;
use contemporary::styling::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, uniform_list, App, AppContext, AsyncApp, AsyncWindowContext, Context,
    ElementId, Entity, InteractiveElement, IntoElement, ListSizingBehavior, ParentElement, Render, Styled, WeakEntity,
    Window,
};
use matrix_sdk::ruma::{OwnedUserId, UserId};
use matrix_sdk::Error;
use std::ops::Range;
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::tokio_helper::TokioHelper;

pub struct IgnoredUsersSettings {
    ignore_user_field: Entity<TextField>,
    processing: bool,
}

impl IgnoredUsersSettings {
    pub fn new(cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let ignore_user_enter_listener =
                cx.listener(|this: &mut IgnoredUsersSettings, _, window, cx| {
                    cx.defer_in(window, |this, window, cx| this.ignore_user(window, cx));
                });
            let ignore_user_field = cx.new(|cx| {
                let mut text_field = TextField::new("ignore-user", cx);
                text_field.on_enter_press(ignore_user_enter_listener);
                text_field.set_placeholder(tr!("AUTH_MATRIX_ID_EXAMPLE").to_string().as_str());
                text_field
            });

            Self {
                ignore_user_field,
                processing: false,
            }
        })
    }

    pub fn ignore_user(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.processing {
            return;
        }

        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx).clone();

        let ignore_user_field_entity = self.ignore_user_field.clone();
        let ignore_user_field = ignore_user_field_entity.read(cx);
        let Ok(ignore_user_id) = UserId::parse(ignore_user_field.text()) else {
            self.ignore_user_field.update(cx, |field, cx| {
                field.flash_error(window, cx);
            });
            return;
        };

        self.processing = true;
        cx.notify();

        cx.spawn_in(
            window,
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncWindowContext| {
                let ignore_user_id_clone = ignore_user_id.clone();
                match cx
                    .spawn_tokio(async move {
                        client.account().ignore_user(&ignore_user_id_clone).await
                    })
                    .await
                {
                    Ok(_) => {
                        let _ = ignore_user_field_entity.update(cx, |field, cx| {
                            field.set_text("");
                            cx.notify();
                        });
                    }
                    Err(_) => {
                        let _ = cx.update(|window, cx| {
                            Toast::new()
                                .title(&tr!("IGNORE_ERROR_TITLE", "Unable to add to ignore list"))
                                .body(&tr!(
                                    "IGNORE_ERROR_TEXT",
                                    "Unable to add {{user}} to the ignore list",
                                    user = ignore_user_id.to_string()
                                ))
                                .severity(AdmonitionSeverity::Error)
                                .post(window, cx);
                        });
                    }
                }
                let _ = weak_this.update(cx, |this, cx| {
                    this.processing = false;
                    cx.notify();
                });
            },
        )
        .detach();
    }

    pub fn unignore_user(
        &mut self,
        ignore_user_id: OwnedUserId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.processing {
            return;
        }

        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx).clone();

        self.processing = true;
        cx.notify();

        cx.spawn_in(
            window,
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncWindowContext| {
                let ignore_user_id_clone = ignore_user_id.clone();
                match cx
                    .spawn_tokio(async move {
                        client.account().unignore_user(&ignore_user_id_clone).await
                    })
                    .await
                {
                    Ok(_) => {}
                    Err(_) => {
                        let _ = cx.update(|window, cx| {
                            Toast::new()
                                .title(&tr!(
                                    "UNGNORE_ERROR_TITLE",
                                    "Unable to remove from ignore list"
                                ))
                                .body(&tr!(
                                    "UNIGNORE_ERROR_TEXT",
                                    "Unable to remove {{user}} from the ignore list",
                                    user = ignore_user_id.to_string()
                                ))
                                .severity(AdmonitionSeverity::Error)
                                .post(window, cx);
                        });
                    }
                }

                let _ = weak_this.update(cx, |this, cx| {
                    this.processing = false;
                    cx.notify();
                });
            },
        )
        .detach();
    }
}

impl Render for IgnoredUsersSettings {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>();

        let session_manager = cx.global::<SessionManager>();
        let ignored_users = session_manager
            .ignored_users()
            .read(cx)
            .ignore_user_list()
            .clone();

        div()
            .bg(theme.background)
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .child(
                grandstand("ignored-users-grandstand")
                    .text(tr!("ACCOUNT_SETTINGS_IGNORED_USERS"))
                    .pt(px(36.)),
            )
            .child(
                constrainer("ignored-users")
                    .flex()
                    .flex_col()
                    .w_full()
                    .p(px(8.))
                    .gap(px(8.))
                    .child(
                        layer()
                            .flex()
                            .flex_col()
                            .p(px(8.))
                            .gap(px(4.))
                            .w_full()
                            .child(subtitle(tr!("IGNORED_USERS", "Ignore User")))
                            .child(div().child(tr!(
                                "IGNORED_USERS_DESCRIPTION",
                                "If someone is disturbing you, you can add them to your ignore \
                                list. You won't see messages from them, and any invitations from \
                                them will be hidden. They will still be able to read any messages \
                                that you send, and they will continue to be present in calls."
                            )))
                            .child(self.ignore_user_field.clone())
                            .child(
                                button("ignore-user-button")
                                    .when(self.processing, |david| david.disabled())
                                    .child(icon_text(
                                        "im-ban-user".into(),
                                        tr!("IGNORED_USERS_ADD_BUTTON", "Add to ignore list")
                                            .into(),
                                    ))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.ignore_user(window, cx)
                                    })),
                            ),
                    )
                    .child(
                        layer()
                            .flex()
                            .flex_col()
                            .p(px(8.))
                            .w_full()
                            .child(subtitle(tr!("IGNORED_USERS_IGNORED", "Ignored Users")))
                            .when(ignored_users.is_empty(), |david| {
                                david.child(tr!(
                                    "IGNORED_USERS_NO_USERS",
                                    "No users have been added to your ignore list."
                                ))
                            })
                            .child(
                                uniform_list(
                                    "ignored-users-list",
                                    ignored_users.len(),
                                    cx.processor(move |this, range: Range<usize>, window, cx| {
                                        range
                                            .map(|index| {
                                                let Some(user) = ignored_users.get(index).cloned()
                                                else {
                                                    return div().into_any_element();
                                                };

                                                div()
                                                    .id(ElementId::Name(user.clone().into()))
                                                    .py(px(2.))
                                                    .child(
                                                        layer()
                                                            .p(px(2.))
                                                            .flex()
                                                            .items_center()
                                                            .child(
                                                                div()
                                                                    .flex_grow()
                                                                    .child(user.clone()),
                                                            )
                                                            .child(
                                                                button("delete")
                                                                    .destructive()
                                                                    .child(icon(
                                                                        "list-remove".into(),
                                                                    ))
                                                                    .when(
                                                                        this.processing,
                                                                        |david| david.disabled(),
                                                                    )
                                                                    .on_click(cx.listener(
                                                                        move |this, _, window, cx| {
                                                                            this.unignore_user(
                                                                                UserId::parse(user.clone())
                                                                                    .unwrap(),
                                                                                window,
                                                                                cx,
                                                                            )
                                                                        },
                                                                    )),
                                                            ),
                                                    )
                                                    .into_any_element()
                                            })
                                            .collect()
                                    }),
                                )
                                .with_sizing_behavior(ListSizingBehavior::Infer),
                            ),
                    ),
            )
    }
}
