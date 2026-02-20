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
    AppContext, AsyncApp, Context, Entity, IntoElement, ParentElement, Render, Styled, WeakEntity,
    Window, div, px,
};
use matrix_sdk::ruma::api::client::room::Visibility;
use matrix_sdk::ruma::api::client::room::create_room::v3::{CreationContent, Request};
use matrix_sdk::ruma::matrix_uri::MatrixId;
use matrix_sdk::ruma::serde::Raw;
use matrix_sdk::ruma::{MatrixToUri, OwnedRoomOrAliasId};
use matrix_sdk::{Error, Room};
use thegrid::session::session_manager::SessionManager;
use thegrid::tokio_helper::TokioHelper;

pub struct DirectJoinRoomPopover {
    visible: bool,
    processing: bool,

    id_field: Entity<TextField>,
}

impl DirectJoinRoomPopover {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let id_field = cx.new(|cx| {
            let mut text_field = TextField::new("name", cx);
            text_field.set_placeholder(
                tr!("ROOM_ADDRESS", "#community:server")
                    .to_string()
                    .as_str(),
            );
            text_field
        });

        cx.observe(&id_field, |this, id_field, cx| {}).detach();

        Self {
            visible: false,
            processing: false,
            id_field,
        }
    }

    pub fn open(&mut self, cx: &mut Context<Self>) {
        self.visible = true;
        self.processing = false;
        self.id_field.update(cx, |id_field, cx| {
            id_field.set_text("");
        });
        cx.notify()
    }

    pub fn join_room(&mut self, cx: &mut Context<Self>) {
        let room_id_text = self.id_field.read(cx).text();
        if room_id_text.trim().is_empty() {
            // TODO: Indicate error
            return;
        }

        let lookup_result = match OwnedRoomOrAliasId::try_from(room_id_text) {
            Ok(room_or_alias) => Some((room_or_alias, Default::default())),
            Err(_) => match MatrixToUri::parse(room_id_text) {
                Ok(uri) => match uri.id() {
                    MatrixId::Room(room) => {
                        Some((OwnedRoomOrAliasId::from(room.clone()), uri.via().to_vec()))
                    }
                    MatrixId::RoomAlias(room_alias) => {
                        Some((OwnedRoomOrAliasId::from(room_alias.clone()), uri.via().to_vec()))
                    }
                    _ => None,
                },
                Err(_) => None,
            },
        };

        let Some((room_or_alias, vias)) = lookup_result else {
            // TODO: Indicate error
            return;
        };

        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx).clone();

        self.processing = true;
        cx.notify();

        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                match cx
                    .spawn_tokio(async move {
                        client.join_room_by_id_or_alias(&room_or_alias, &vias).await
                    })
                    .await
                {
                    Ok(room) => {
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

    fn direct_join_room_page_contents(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        constrainer("create-room-constrainer").child(
            layer()
                .flex()
                .flex_col()
                .p(px(8.))
                .w_full()
                .child(subtitle(tr!("DIRECT_JOIN_ROOM")))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(tr!(
                            "DIRECT_JOIN_ROOM_DESCRIPTION",
                            "If you know the room address or have a room link, you can join it \
                            directly."
                        ))
                        .child(self.id_field.clone())
                        .child(
                            button("do-join")
                                .child(icon_text(
                                    "list-add".into(),
                                    tr!("JOIN_ROOM", "Join Room").into(),
                                ))
                                .on_click(cx.listener(move |this, _, _, cx| this.join_room(cx))),
                        ),
                ),
        )
    }
}

impl Render for DirectJoinRoomPopover {
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
                                    .text(tr!("DIRECT_JOIN_ROOM"))
                                    .on_back_click(cx.listener(move |this, _, _, cx| {
                                        this.visible = false;
                                        cx.notify()
                                    })),
                            )
                            .child(self.direct_join_room_page_contents(cx))
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
