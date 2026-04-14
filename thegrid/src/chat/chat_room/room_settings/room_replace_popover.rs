use cntp_i18n::tr;
use contemporary::components::admonition::{admonition, AdmonitionSeverity};
use contemporary::components::button::button;
use contemporary::components::constrainer::constrainer;
use contemporary::components::context_menu::ContextMenuItem;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon::icon;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::pager::pager;
use contemporary::components::pager::slide_horizontal_animation::SlideHorizontalAnimation;
use contemporary::components::popover::popover;
use contemporary::components::spinner::spinner;
use contemporary::components::subtitle::subtitle;
use contemporary::components::toast::Toast;
use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, App, AsyncApp, AsyncWindowContext, Context, IntoElement, ParentElement,
    Render, Styled, WeakEntity, Window,
};
use matrix_sdk::ruma::api::client::discovery::get_capabilities::v3::RoomVersionStability;
use matrix_sdk::ruma::api::client::room::upgrade_room;
use matrix_sdk::ruma::api::client::room::upgrade_room::v3::Response;
use matrix_sdk::ruma::room::JoinRule;
use matrix_sdk::ruma::{OwnedRoomId, RoomVersionId};
use matrix_sdk::{HttpError, Room};
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::tokio_helper::TokioHelper;

pub struct RoomReplaceEvent {
    pub new_room_id: OwnedRoomId,
}

pub struct RoomReplacePopover {
    visible: bool,
    state: ReplacePopoverState,
    new_room_version: RoomVersionId,
    room: Room,
    error: Option<String>,

    on_replace: Box<dyn Fn(&RoomReplaceEvent, &mut Window, &mut App)>,
}

enum ReplacePopoverState {
    VersionSelect,
    ReplaceConfirm,
    InProgress,
}

impl ReplacePopoverState {
    fn page(&self) -> usize {
        match self {
            ReplacePopoverState::VersionSelect => 0,
            ReplacePopoverState::ReplaceConfirm => 1,
            ReplacePopoverState::InProgress => 2,
        }
    }
}

impl RoomReplacePopover {
    pub fn new(
        room: Room,
        on_replace: impl Fn(&RoomReplaceEvent, &mut Window, &mut App) + 'static,
        cx: &mut Context<Self>,
    ) -> Self {
        let session_manager = cx.global::<SessionManager>();
        let server_room_versions = &session_manager
            .capabilities()
            .read(cx)
            .capabilities()
            .room_versions;

        Self {
            visible: false,
            state: ReplacePopoverState::VersionSelect,
            new_room_version: server_room_versions.default.clone(),
            room,
            error: None,
            on_replace: Box::new(on_replace),
        }
    }

    pub fn open(&mut self, cx: &mut Context<Self>) {
        self.visible = true;
        cx.notify();
    }

    pub fn perform_replace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.state = ReplacePopoverState::InProgress;
        cx.notify();

        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx).clone();

        let new_room_version = self.new_room_version.clone();
        let room = self.room.clone();

        cx.spawn_in(
            window,
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncWindowContext| {
                let upgrade_request =
                    upgrade_room::v3::Request::new(room.room_id().to_owned(), new_room_version);
                let result = cx
                    .spawn_tokio(async move { client.send(upgrade_request).await })
                    .await;

                match result {
                    Ok(response) => {
                        // Leave the old room
                        if let Err(e) = cx.spawn_tokio(async move { room.leave().await }).await {
                            log::error!("Failed to leave room: {}", e);

                            // Continue anyway
                        }

                        let _ = cx.update(|window, cx| {
                            weak_this.update(cx, |this, cx| {
                                this.visible = false;
                                (this.on_replace)(
                                    &RoomReplaceEvent {
                                        new_room_id: response.replacement_room,
                                    },
                                    window,
                                    cx,
                                );
                                cx.notify()
                            })
                        });
                    }
                    Err(e) => {
                        let _ = weak_this.update(cx, |this, cx| {
                            this.error = Some(e.to_string());
                            this.state = ReplacePopoverState::ReplaceConfirm;
                            cx.notify()
                        });
                    }
                }
            },
        )
        .detach();
    }
}

impl Render for RoomReplacePopover {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let session_manager = cx.global::<SessionManager>();
        let server_room_versions = &session_manager
            .capabilities()
            .read(cx)
            .capabilities()
            .room_versions;

        let stable_room_versions = server_room_versions
            .available
            .iter()
            .filter(|(_, stability)| stability == &&RoomVersionStability::Stable);
        let unstable_room_versions = server_room_versions
            .available
            .iter()
            .filter(|(_, stability)| stability == &&RoomVersionStability::Unstable);

        let room_versions_menu = stable_room_versions
            .chain(unstable_room_versions)
            .map(|(room_version, room_version_stability)| {
                ContextMenuItem::menu_item()
                    .label(match room_version_stability {
                        RoomVersionStability::Unstable => tr!(
                            "ROOM_VERSION_UNSTABLE",
                            "{{room_version}} (experimental)",
                            room_version = room_version.as_str()
                        )
                        .to_string(),
                        _ => room_version.as_str().to_string(),
                    })
                    .on_triggered(cx.listener({
                        let room_version = room_version.clone();
                        move |this, _, _, cx| {
                            this.new_room_version = room_version.clone();
                            cx.notify();
                        }
                    }))
                    .build()
            })
            .collect();

        popover("replace-room-popover")
            .visible(self.visible)
            .size_neg(100.)
            .anchor_bottom()
            .content(
                pager("replace-room-pager", self.state.page())
                    .size_full()
                    .animation(SlideHorizontalAnimation::new())
                    .page(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(9.))
                            .child(
                                grandstand("replace-room-popover-grandstand")
                                    .text(tr!("POPOVER_REPLACE_ROOM_GRANDSTAND", "Replace Room"))
                                    .on_back_click(cx.listener(move |this, _, _, cx| {
                                        this.visible = false;
                                        cx.notify()
                                    })),
                            )
                            .child(
                                constrainer("replace-room-popover-constrainer").child(
                                    layer()
                                        .flex()
                                        .flex_col()
                                        .p(px(8.))
                                        .w_full()
                                        .child(subtitle(tr!(
                                            "POPOVER_REPLACE_ROOM_OPTIONS",
                                            "New Room Options"
                                        )))
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap(px(8.))
                                                .child(
                                                    layer()
                                                        .p(px(4.))
                                                        .gap(px(4.))
                                                        .items_center()
                                                        .flex()
                                                        .child(tr!("ROOM_VERSION", "Room Version"))
                                                        .child(div().flex_grow())
                                                        .child(
                                                            self.new_room_version
                                                                .as_str()
                                                                .to_string(),
                                                        )
                                                        .child(
                                                            button("change-room-version")
                                                                .child(icon("arrow-down"))
                                                                .with_menu(room_versions_menu),
                                                        ),
                                                )
                                                // TODO: Support secondary room creators
                                                .child(
                                                    button("replace-room-ok")
                                                        .child(icon_text(
                                                            "im-room",
                                                            tr!("ROOM_REPLACE"),
                                                        ))
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.state =
                                                                ReplacePopoverState::ReplaceConfirm;
                                                            cx.notify()
                                                        })),
                                                ),
                                        ),
                                ),
                            )
                            .into_any_element(),
                    )
                    .page(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(9.))
                            .child(
                                grandstand("replace-room-popover-grandstand")
                                    .text(tr!("POPOVER_REPLACE_ROOM_GRANDSTAND"))
                                    .on_back_click(cx.listener(move |this, _, _, cx| {
                                        this.state = ReplacePopoverState::VersionSelect;
                                        cx.notify()
                                    })),
                            )
                            .child(
                                constrainer("replace-room-popover-constrainer").child(
                                    layer()
                                        .flex()
                                        .flex_col()
                                        .p(px(8.))
                                        .w_full()
                                        .child(subtitle(tr!("DESTRUCTIVE_CONFIRM", "This is it!")))
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap(px(8.))
                                                .child(tr!(
                                                    "POPOVER_REPLACE_ROOM_FINAL_CONFIRM",
                                                    "Replacing this room will create a new room, \
                                                    and a message will be left in this room \
                                                    directing everyone to the new room. Existing \
                                                    messages will stay in this room, and it will \
                                                    be archived."
                                                ))
                                                .child(tr!(
                                                    "POPOVER_REPLACE_ROOM_FINAL_CONFIRM_2",
                                                    "Until more people join the new room, \
                                                    you will be the only person in it. Existing \
                                                    users will not join the new room automatically."
                                                ))
                                                .child(tr!(
                                                    "POPOVER_REPLACE_ROOM_FINAL_CONFIRM_3",
                                                    "You will leave this room once it has been \
                                                    replaced. This action is irreversible."
                                                ))
                                                .when_some(self.error.clone(), |david, error| {
                                                    david.child(
                                                        admonition()
                                                            .severity(AdmonitionSeverity::Error)
                                                            .title(tr!(
                                                                "ROOM_REPLACE_ERROR_TITLE",
                                                                "Unable to replace the room"
                                                            ))
                                                            .child(error),
                                                    )
                                                })
                                                .child(
                                                    button("replace-room-ok")
                                                        .destructive()
                                                        .child(icon_text(
                                                            "im-room",
                                                            tr!("ROOM_REPLACE"),
                                                        ))
                                                        .on_click(cx.listener(
                                                            |this, _, window, cx| {
                                                                this.perform_replace(window, cx);
                                                            },
                                                        )),
                                                ),
                                        ),
                                ),
                            )
                            .into_any_element(),
                    )
                    .page(
                        div()
                            .size_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(spinner())
                            .into_any_element(),
                    ),
            )
    }
}
