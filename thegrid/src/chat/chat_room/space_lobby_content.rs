use crate::chat::chat_room::open_room::OpenRoom;
use crate::chat::displayed_room::DisplayedRoom;
use crate::chat::join_room::create_room_popover::CreateRoomPopover;
use crate::chat::join_room::create_space_popover::CreateSpacePopover;
use cntp_i18n::{tr, trn};
use contemporary::components::admonition::AdmonitionSeverity;
use contemporary::components::button::button;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::skeleton::skeleton;
use contemporary::components::subtitle::subtitle;
use contemporary::components::toast::Toast;
use contemporary::styling::theme::{ThemeStorage, VariableColor};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, Entity, InteractiveElement, IntoElement, ListAlignment, ListScrollEvent,
    ListState, ParentElement, Render, Styled, Window, div, list, px,
};
use matrix_sdk::ruma::OwnedRoomId;
use matrix_sdk::ruma::room::JoinRuleSummary;
use matrix_sdk::{Room, RoomState};
use matrix_sdk_ui::spaces::SpaceRoom;
use matrix_sdk_ui::spaces::room_list::SpaceRoomListPaginationState;
use thegrid_common::mxc_image::{SizePolicy, mxc_image};
use thegrid_common::session::room_cache::RoomJoinEvent;
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::session::spaces_cache::SpaceRoomListEntity;

pub struct SpaceLobbyContent {
    displayed_room: Entity<DisplayedRoom>,
    open_room: Entity<OpenRoom>,
    space_rooms: Entity<SpaceRoomListEntity>,

    create_room_popover: Entity<CreateRoomPopover>,
    create_space_popover: Entity<CreateSpacePopover>,

    list_state: ListState,
}

enum JoinButtonType {
    View(Room),
    Knock,
    Join,
}

impl SpaceLobbyContent {
    pub fn new(
        displayed_room: Entity<DisplayedRoom>,
        open_room: Entity<OpenRoom>,
        create_room_popover: Entity<CreateRoomPopover>,
        create_space_popover: Entity<CreateSpacePopover>,
        cx: &mut Context<Self>,
    ) -> Self {
        let room_id = open_room.read(cx).room_id.clone();
        let session_manager = cx.global::<SessionManager>();
        let space_rooms = session_manager
            .spaces()
            .update(cx, |spaces, cx| spaces.space_room_list(room_id, cx));

        let list_state = ListState::new(0, ListAlignment::Top, px(200.));

        list_state.set_scroll_handler(cx.listener({
            let space_rooms = space_rooms.clone();
            move |this: &mut Self, event: &ListScrollEvent, _, cx| {
                let space_rooms = space_rooms.read(cx);
                if event.visible_range.end >= space_rooms.rooms().len().saturating_sub(3)
                    && space_rooms.pagination_state() != &SpaceRoomListPaginationState::Loading
                {
                    // Paginate
                    space_rooms.paginate()
                }
            }
        }));
        cx.observe(&space_rooms, {
            let list_state = list_state.clone();
            move |this, space_rooms, cx| {
                let space_rooms = space_rooms.read(cx);
                if list_state.item_count() != space_rooms.rooms().len() {
                    list_state.reset(space_rooms.rooms().len());
                }

                let last_offset = list_state.logical_scroll_top();
                list_state.reset(
                    space_rooms.rooms().len()
                        + match space_rooms.pagination_state() {
                            SpaceRoomListPaginationState::Idle { end_reached } if *end_reached => 0,
                            _ => 3,
                        },
                );
                list_state.scroll_to(last_offset);
            }
        })
        .detach();

        Self {
            displayed_room,
            open_room,
            space_rooms,
            list_state,
            create_room_popover,
            create_space_popover,
        }
    }

    fn change_room(&mut self, room_id: OwnedRoomId, cx: &mut Context<Self>) {
        self.displayed_room
            .write(cx, DisplayedRoom::Room(room_id.clone()));
    }

    fn join_room(
        &mut self,
        room_id: OwnedRoomId,
        knock: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let session_manager = cx.global::<SessionManager>();
        let callback = cx.listener({
            let room_id = room_id.clone();
            move |this, event: &RoomJoinEvent, window, cx| {
                if let Err(e) = &event.result {
                    Toast::new()
                        .title(tr!("JOIN_ERROR_TITLE", "Unable to join room").as_ref())
                        .body(
                            tr!(
                                "JOIN_ERROR_TEXT",
                                "Unable to join the room {{room}}",
                                room = room_id.to_string()
                            )
                            .as_ref(),
                        )
                        .severity(AdmonitionSeverity::Error)
                        .post(window, cx);
                }
                cx.notify();
            }
        });
        session_manager.rooms().update(cx, |rooms, cx| {
            rooms.join_room(room_id, knock, vec![], callback, window, cx);
        });
    }

    fn render_list_item(
        &mut self,
        i: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let space_rooms = self.space_rooms.read(cx);
        let Some(space_room) = space_rooms.rooms().get(i) else {
            // Out of bounds!
            return div()
                .id(i)
                .overflow_x_hidden()
                .py(px(2.))
                .child(
                    layer()
                        .overflow_x_hidden()
                        .flex()
                        .gap(px(4.))
                        .p(px(4.))
                        .child(div().child(skeleton("image-skeleton").child(div().size(px(40.)))))
                        .child(
                            div()
                                .overflow_x_hidden()
                                .flex()
                                .flex_col()
                                .flex_grow()
                                .gap(px(4.))
                                .child(skeleton("name-skeleton").w(px(150.)))
                                .child(skeleton("topic-skeleton").w(px(350.)))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(4.))
                                        .child(div().flex_grow())
                                        .child(skeleton("count-skeleton").w(px(100.)))
                                        .child(skeleton("alias-skeleton").w(px(150.)))
                                        .child(
                                            skeleton("join-skeleton").child(
                                                button("join-button")
                                                    .child(icon_text("list-add", tr!("JOIN_ROOM"))),
                                            ),
                                        ),
                                ),
                        ),
                )
                .into_any_element();
        };

        let session_manager = cx.global::<SessionManager>();
        let room_manager = session_manager.rooms().read(cx);
        let joined_room = room_manager.room(&space_room.room_id).and_then(|room| {
            let room = &room.read(cx).inner;
            if room.state() == RoomState::Joined {
                Some(room)
            } else {
                None
            }
        });

        let view_button_type = if let Some(joined_room) = joined_room {
            JoinButtonType::View(joined_room.clone())
        } else if space_room
            .join_rule
            .as_ref()
            .is_some_and(|join_rule| join_rule == &JoinRuleSummary::Knock)
        {
            JoinButtonType::Knock
        } else {
            JoinButtonType::Join
        };

        let room_id = space_room.room_id.clone();
        let joining_room = room_manager.joining_room(room_id.clone());

        div()
            .id(i)
            .overflow_x_hidden()
            .py(px(2.))
            .child(
                layer()
                    .overflow_x_hidden()
                    .flex()
                    .gap(px(4.))
                    .p(px(4.))
                    .child(
                        mxc_image(space_room.avatar_url.clone())
                            .fallback_image(&space_room.room_id)
                            .rounded(theme.border_radius)
                            .size_policy(SizePolicy::Constrain(40., 40.)),
                    )
                    .child(
                        div()
                            .overflow_x_hidden()
                            .flex()
                            .flex_col()
                            .flex_grow()
                            .gap(px(4.))
                            .child(div().child(space_room.name.clone().unwrap_or("".into())))
                            .child(
                                div()
                                    .overflow_x_hidden()
                                    .text_color(theme.foreground.disabled())
                                    .child(space_room.topic.clone().unwrap_or("".into())),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(4.))
                                    .child(div().flex_grow())
                                    .child(div().text_color(theme.foreground.disabled()).child(
                                        trn!(
                                            "ROOM_DIRECTORY_MEMBER_COUNT",
                                            "{{count}} member",
                                            "{{count}} members",
                                            count = space_room.num_joined_members as isize
                                        ),
                                    ))
                                    .child(match view_button_type {
                                        JoinButtonType::View(room) => {
                                            let room_id = room.room_id().to_owned();
                                            button("view-button")
                                                .child(icon_text("go-next", tr!("VIEW_ROOM")))
                                                .on_click(cx.listener(
                                                    move |this, _, window, cx| {
                                                        this.change_room(room_id.clone(), cx);
                                                    },
                                                ))
                                        }
                                        JoinButtonType::Knock => button("join-button")
                                            .when(joining_room, |button| button.disabled())
                                            .child(icon_text("list-add", tr!("KNOCK_ON_ROOM")))
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.join_room(room_id.clone(), true, window, cx);
                                            })),
                                        JoinButtonType::Join => button("join-button")
                                            .when(joining_room, |button| button.disabled())
                                            .child(icon_text("list-add", tr!("JOIN_ROOM")))
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.join_room(room_id.clone(), false, window, cx);
                                            })),
                                    }),
                            ),
                    ),
            )
            .into_any_element()
    }
}

impl Render for SpaceLobbyContent {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let open_room = self.open_room.read(cx);
        let room = open_room.room.as_ref().unwrap();

        // We don't need to fill out all these fields - only the ones that are used by
        // the space selection logic.
        let space_room = SpaceRoom {
            room_id: room.room_id().to_owned(),
            canonical_alias: None,
            name: room.name(),
            display_name: room
                .cached_display_name()
                .map(|name| name.to_string())
                .or_else(|| room.name())
                .unwrap_or_default(),
            topic: None,
            avatar_url: None,
            room_type: None,
            num_joined_members: 0,
            join_rule: None,
            world_readable: None,
            guest_can_join: false,
            is_direct: None,
            children_count: 0,
            state: None,
            heroes: None,
            via: vec![],
        };

        let theme = cx.theme();

        div()
            .flex_grow()
            .flex()
            .justify_center()
            .size_full()
            .gap(px(8.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .p(px(4.))
                    .gap(px(4.))
                    .w(px(250.))
                    .child(
                        mxc_image(room.avatar_url())
                            .fallback_image(room.room_id())
                            .rounded(theme.border_radius)
                            .size_policy(SizePolicy::Constrain(150., 150.)),
                    )
                    .child(tr!("SPACE_LOBBY_INTRO", "Welcome to the lobby"))
                    .child(
                        div().text_size(theme.heading_font_size).child(
                            room.cached_display_name()
                                .map(|name| name.to_string())
                                .or_else(|| room.name())
                                .unwrap_or_default(),
                        ),
                    )
                    .child(
                        layer()
                            .w(px(250.))
                            .p(px(8.))
                            .gap(px(8.))
                            .child(subtitle(tr!("ACTIONS")))
                            .child(
                                div()
                                    .bg(theme.button_background)
                                    .rounded(theme.border_radius)
                                    .flex()
                                    .flex_col()
                                    .child(
                                        button("create-room")
                                            .child(icon_text("list-add", tr!("CREATE_ROOM")))
                                            .on_click(cx.listener({
                                                let space_room = space_room.clone();
                                                move |this, _, _, cx| {
                                                    let space_room = space_room.clone();
                                                    this.create_room_popover.update(
                                                        cx,
                                                        move |create_room_popover, cx| {
                                                            create_room_popover
                                                                .open(Some(space_room.clone()), cx);
                                                        },
                                                    );
                                                }
                                            })),
                                    )
                                    .child(
                                        button("create-space")
                                            .child(icon_text(
                                                "list-add",
                                                tr!("CREATE_SUBSPACE", "Create Subordinate Space"),
                                            ))
                                            .on_click(cx.listener({
                                                let space_room = space_room.clone();
                                                move |this, _, _, cx| {
                                                    let space_room = space_room.clone();
                                                    this.create_space_popover.update(
                                                        cx,
                                                        move |create_space_popover, cx| {
                                                            create_space_popover
                                                                .open(Some(space_room.clone()), cx);
                                                        },
                                                    );
                                                }
                                            })),
                                    ),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .max_w(px(600.))
                    .size_full()
                    .px(px(8.))
                    .gap(px(8.))
                    .child(
                        list(
                            self.list_state.clone(),
                            cx.processor(Self::render_list_item),
                        )
                        .size_full()
                        .into_any_element(),
                    ),
            )
    }
}
