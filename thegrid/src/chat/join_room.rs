use crate::chat::displayed_room::DisplayedRoom;
use crate::chat::join_room::create_room_popover::CreateRoomPopover;
use crate::mxc_image::{SizePolicy, mxc_image};
use cntp_i18n::tr;
use contemporary::components::button::button;
use contemporary::components::constrainer::constrainer;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::subtitle::subtitle;
use contemporary::styling::theme::{Theme, VariableColor};
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AsyncApp, Context, Element, Entity, InteractiveElement, IntoElement, ListAlignment,
    ListSizingBehavior, ListState, ParentElement, Render, RenderOnce, Styled, Subscription, Window,
    div, list, px,
};
use matrix_sdk::room::RoomMember;
use thegrid::session::room_cache::CachedRoom;
use thegrid::session::session_manager::SessionManager;
use thegrid::tokio_helper::TokioHelper;
use tracing::error;

pub mod create_room_popover;

pub struct JoinRoom {
    invitations: Vec<Entity<CachedRoom>>,
    invitations_list: ListState,
    create_room_popover: Entity<CreateRoomPopover>,
    displayed_room: Entity<DisplayedRoom>,

    room_cache_subscription: Option<Subscription>,
}

impl JoinRoom {
    pub fn new(
        cx: &mut Context<Self>,
        displayed_room: Entity<DisplayedRoom>,
        create_room_popover: Entity<CreateRoomPopover>,
    ) -> Self {
        cx.observe_global::<SessionManager>(|this, cx| {
            this.update_invitations(cx);

            let session_manager = cx.global::<SessionManager>();
            if session_manager.client().is_none() {
                this.room_cache_subscription = None;
                return;
            }

            let room_cache = session_manager.rooms();
            this.room_cache_subscription =
                Some(cx.observe(&room_cache, |this, _, cx| this.update_invitations(cx)));
        })
        .detach();

        Self {
            invitations: Vec::new(),
            invitations_list: ListState::new(0, ListAlignment::Top, px(200.)),
            create_room_popover,
            room_cache_subscription: None,
            displayed_room,
        }
    }

    fn update_invitations(&mut self, cx: &mut Context<Self>) {
        let session_manager = cx.global::<SessionManager>();
        if session_manager.client().is_none() {
            self.invitations = Vec::new();
            self.invitations_list.reset(0);
            return;
        }

        let rooms = session_manager.rooms().read(cx);
        let invitations = rooms.invited_rooms(cx);

        if self.invitations_list.item_count() != invitations.len() {
            self.invitations_list.reset(invitations.len());
        }
        self.invitations = invitations;
    }
}

impl Render for JoinRoom {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>();

        let displayed_room = self.displayed_room.clone();

        div()
            .bg(theme.background)
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .child(
                grandstand("join-room-grandstand")
                    .text(tr!("JOIN_ROOM", "Create or Join Room"))
                    .pt(px(36.)),
            )
            .child(
                constrainer("join-room-constrainer")
                    .flex()
                    .flex_col()
                    .w_full()
                    .p(px(8.))
                    .gap(px(8.))
                    .when(self.invitations_list.item_count() != 0, |david| {
                        david.child(
                            layer()
                                .flex()
                                .flex_col()
                                .p(px(8.))
                                .w_full()
                                .child(subtitle(tr!(
                                    "JOIN_ROOM_INVITATIONS",
                                    "Pending Invitations"
                                )))
                                .child(
                                    list(
                                        self.invitations_list.clone(),
                                        cx.processor(move |this, i, _, cx| {
                                            let invitation: &Entity<CachedRoom> =
                                                &this.invitations[i];
                                            div()
                                                .id(i)
                                                .py(px(2.))
                                                .child(Invitation {
                                                    room: invitation.clone(),
                                                    displayed_room: displayed_room.clone(),
                                                })
                                                .into_any_element()
                                        }),
                                    )
                                    .with_sizing_behavior(ListSizingBehavior::Infer),
                                ),
                        )
                    })
                    .child(
                        layer()
                            .flex()
                            .flex_col()
                            .p(px(8.))
                            .w_full()
                            .child(subtitle(tr!("CREATE_ROOM_OPTIONS", "Create Room")))
                            .child(
                                button("create-room")
                                    .child(icon_text("list-add".into(), tr!("CREATE_ROOM").into()))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.create_room_popover.update(
                                            cx,
                                            |create_room_popover, cx| {
                                                create_room_popover.open(cx);
                                                cx.notify();
                                            },
                                        )
                                    })),
                            ),
                    ),
            )
    }
}

#[derive(IntoElement)]
struct Invitation {
    room: Entity<CachedRoom>,
    displayed_room: Entity<DisplayedRoom>,
}

impl Invitation {
    fn accept_invite(
        room: Entity<CachedRoom>,
        displayed_room: Entity<DisplayedRoom>,
        processing: Entity<bool>,
        cx: &mut App,
    ) {
        processing.write(cx, true);
        let room = room.read(cx).inner.clone();
        let room_id = room.room_id().to_owned();
        cx.spawn(async move |cx: &mut AsyncApp| {
            if let Err(e) = cx.spawn_tokio(async move { room.join().await }).await {
                error!("Unable to accept invite: {e}");
                processing.write(cx, false).unwrap();
            } else {
                displayed_room
                    .write(cx, DisplayedRoom::Room(room_id))
                    .unwrap();
            };
        })
        .detach();
    }

    fn reject_invite(room: Entity<CachedRoom>, processing: Entity<bool>, cx: &mut App) {
        processing.write(cx, true);
        let room = room.read(cx).inner.clone();
        cx.spawn(async move |cx: &mut AsyncApp| {
            if let Err(e) = cx.spawn_tokio(async move { room.leave().await }).await {
                error!("Unable to reject invite: {e}");
                processing.write(cx, false).unwrap();
            };
        })
        .detach();
    }
}

impl RenderOnce for Invitation {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let processing = window.use_state(cx, |_, _| false);
        let theme = cx.global::<Theme>();
        let room = self.room.read(cx);
        let invite_details = room.invite_details().unwrap();
        let inviter = invite_details.inviter;

        let room_entity = self.room.clone();
        let room_entity_2 = self.room.clone();
        let processing_clone = processing.clone();
        let processing_clone_2 = processing.clone();
        let processing = processing.read(cx);

        let displayed_room = self.displayed_room;

        layer()
            .p(px(4.))
            .gap(px(4.))
            .w_full()
            .flex()
            .items_center()
            .child(
                mxc_image(room.inner.avatar_url().or_else(|| {
                    inviter.clone().and_then(|inviter| {
                        inviter.avatar_url().map(|avatar_url| avatar_url.to_owned())
                    })
                }))
                .rounded(theme.border_radius)
                .size(px(40.))
                .size_policy(SizePolicy::Fit),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(div().flex().gap(px(8.)).child(room.display_name()).child(
                        div().text_color(theme.foreground.disabled()).when_else(
                            room.inner.is_space(),
                            |david| david.child(tr!("ROOM_TYPE_SPACE", "Space")),
                            |david| david.child(tr!("ROOM_TYPE_ROOM", "Room")),
                        ),
                    ))
                    .when_some(inviter, |david, inviter| {
                        david.child(div().text_color(theme.foreground.disabled()).child(tr!(
                            "INVITE_INVITED_BY",
                            "Invited by {{inviter}}",
                            inviter = inviter.user_id().to_string()
                        )))
                    }),
            )
            .child(div().flex_grow())
            .child(
                div()
                    .rounded(theme.border_radius)
                    .bg(theme.button_background)
                    .flex()
                    .child(
                        button("invite-reject")
                            .child(icon_text(
                                "edit-delete".into(),
                                tr!("INVITE_DECLINE", "Decline").into(),
                            ))
                            .destructive()
                            .when(*processing, |david| david.disabled())
                            .on_click(move |_, _, cx| {
                                Self::reject_invite(
                                    room_entity.clone(),
                                    processing_clone.clone(),
                                    cx,
                                );
                            }),
                    )
                    .child(
                        button("invite-accept")
                            .child(icon_text(
                                "dialog-ok".into(),
                                tr!("INVITE_ACCEPT", "Accept").into(),
                            ))
                            .when(*processing, |david| david.disabled())
                            .on_click(move |_, _, cx| {
                                Self::accept_invite(
                                    room_entity_2.clone(),
                                    displayed_room.clone(),
                                    processing_clone_2.clone(),
                                    cx,
                                );
                            }),
                    ),
            )
    }
}
