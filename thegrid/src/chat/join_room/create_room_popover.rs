use cntp_i18n::tr;
use contemporary::components::button::button;
use contemporary::components::checkbox::{CheckState, CheckedChangeEvent, checkbox, radio_button};
use contemporary::components::constrainer::constrainer;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::pager::pager;
use contemporary::components::pager::slide_horizontal_animation::SlideHorizontalAnimation;
use contemporary::components::popover::popover;
use contemporary::components::spinner::spinner;
use contemporary::components::subtitle::subtitle;
use contemporary::components::text_field::TextField;
use gpui::prelude::FluentBuilder;
use gpui::{
    AsyncApp, Context, Entity, IntoElement, ParentElement, Render, Styled, WeakEntity, Window, div,
    px,
};
use matrix_sdk::ruma::api::client::room::Visibility;
use matrix_sdk::ruma::api::client::room::create_room::v3::{CreationContent, Request};
use matrix_sdk::ruma::serde::Raw;
use matrix_sdk::{Error, Room};
use thegrid::session::session_manager::SessionManager;
use thegrid::tokio_helper::TokioHelper;

pub struct CreateRoomPopover {
    visible: bool,
    processing: bool,

    name_field: Entity<TextField>,
    is_private_room: bool,
    encrypt: bool,
    federation: bool,
}

impl CreateRoomPopover {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let name_field =
            TextField::new(cx, "name", "".into(), tr!("ROOM_NAME", "Room Name").into());

        Self {
            visible: false,
            processing: false,
            name_field,
            is_private_room: true,
            encrypt: true,
            federation: true,
        }
    }

    pub fn open(&mut self, cx: &mut Context<Self>) {
        self.visible = true;
        self.is_private_room = true;
        self.encrypt = true;
        self.federation = true;
        self.processing = false;
        cx.notify()
    }

    pub fn create_room(&mut self, cx: &mut Context<Self>) {
        let room_name = self.name_field.read(cx).current_text(cx);
        if room_name.trim().is_empty() {
            // TODO: Indicate error
            return;
        }

        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx).clone();

        let mut request = Request::new();
        request.name = Some(room_name.to_string());
        request.visibility = match self.is_private_room {
            true => Visibility::Private,
            false => Visibility::Public,
        };
        request.creation_content = Some(
            Raw::new(&{
                let mut creation_content = CreationContent::new();
                creation_content.federate = self.federation;
                creation_content
            })
            .unwrap(),
        );

        let encrypt = self.encrypt;

        self.processing = true;
        cx.notify();

        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                match cx
                    .spawn_tokio(async move { client.create_room(request).await })
                    .await
                {
                    Ok(room) => {
                        if encrypt {
                            let _ = cx
                                .spawn_tokio(async move { room.enable_encryption().await })
                                .await;
                        }

                        weak_this
                            .update(cx, |this, cx| {
                                // TODO: Navigate to room
                                this.visible = false;
                                cx.notify();
                            })
                            .unwrap();
                    }
                    Err(e) => {
                        weak_this
                            .update(cx, |this, cx| {
                                // TODO: Show error message
                                this.processing = false;
                                cx.notify();
                            })
                            .unwrap();
                    }
                }
            },
        )
        .detach();
    }

    fn create_room_page_contents(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        constrainer("create-room-constrainer").child(
            layer()
                .flex()
                .flex_col()
                .p(px(8.))
                .w_full()
                .child(subtitle(tr!("CREATE_ROOM_OPTIONS", "Room Options")))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(tr!("CREATE_ROOM_DESCRIPTION", "Create a room!"))
                        .child(self.name_field.clone())
                        .child(
                            layer()
                                .p(px(8.))
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                .child(
                                    radio_button("room-visibility-private")
                                        .label(tr!(
                                            "CREATE_ROOM_VISIBILITY_PRIVATE",
                                            "Create Private Room"
                                        ))
                                        .when(self.is_private_room, |david| david.checked())
                                        .on_checked_changed(cx.listener(
                                            |this, event: &CheckedChangeEvent, _, cx| {
                                                if event.check_state == CheckState::On {
                                                    this.is_private_room = true;
                                                    cx.notify()
                                                }
                                            },
                                        )),
                                )
                                .child(
                                    radio_button("room-visibility-public")
                                        .label(tr!(
                                            "CREATE_ROOM_VISIBILITY_PUBLIC",
                                            "Create Public Room"
                                        ))
                                        .when(!self.is_private_room, |david| david.checked())
                                        .on_checked_changed(cx.listener(
                                            |this, event: &CheckedChangeEvent, _, cx| {
                                                if event.check_state == CheckState::On {
                                                    this.is_private_room = false;
                                                    cx.notify()
                                                }
                                            },
                                        )),
                                ),
                        )
                        .child(
                            layer()
                                .p(px(8.))
                                .flex()
                                .flex_col()
                                .child(
                                    checkbox("encrypt-box")
                                        .label(tr!("CREATE_ROOM_ENCRYPT", "Enable Encryption"))
                                        .when(self.encrypt, |david| david.checked())
                                        .on_checked_changed(cx.listener(
                                            |this, event: &CheckedChangeEvent, _, cx| {
                                                this.encrypt = match event.check_state {
                                                    CheckState::Off => false,
                                                    CheckState::On => true,
                                                    CheckState::Indeterminate => {
                                                        unreachable!()
                                                    }
                                                };
                                                cx.notify()
                                            },
                                        )),
                                )
                                .child(tr!(
                                    "CREATE_ROOM_ENCRYPT_SUBTEXT",
                                    "Once enabled, encryption cannot be disabled"
                                )),
                        )
                        .child(
                            button("do-create")
                                .child(icon_text(
                                    "list-add".into(),
                                    tr!("CREATE_ROOM", "Create Room").into(),
                                ))
                                .on_click(cx.listener(move |this, _, _, cx| this.create_room(cx))),
                        ),
                ),
        )
    }
}

impl Render for CreateRoomPopover {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        popover("create-room-popover")
            .visible(self.visible)
            .size_neg(100.)
            .anchor_bottom()
            .content(
                pager("create-room-pager", if self.processing { 1 } else { 0 })
                    .animation(SlideHorizontalAnimation::new())
                    .size_full()
                    .page(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(9.))
                            .child(
                                grandstand("create-room-grandstand")
                                    .text(tr!("CREATE_ROOM_TITLE", "Create Room"))
                                    .on_back_click(cx.listener(move |this, _, _, cx| {
                                        this.visible = false;
                                        cx.notify()
                                    })),
                            )
                            .child(self.create_room_page_contents(cx))
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
