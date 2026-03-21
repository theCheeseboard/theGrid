use crate::chat::chat_room::open_room::OpenRoom;
use cntp_i18n::{tr, trn};
use contemporary::components::button::button;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::skeleton::skeleton;
use contemporary::components::subtitle::subtitle;
use contemporary::styling::theme::{ThemeStorage, VariableColor};
use gpui::{
    div, list, px, AnyElement, Context, Entity, InteractiveElement,
    IntoElement, ListAlignment, ListScrollEvent, ListState, ParentElement, Render, Styled, Window,
};
use matrix_sdk::RoomState;
use matrix_sdk_ui::spaces::room_list::SpaceRoomListPaginationState;
use thegrid_common::mxc_image::{mxc_image, SizePolicy};
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::session::spaces_cache::SpaceRoomListEntity;

pub struct SpaceLobbyContent {
    open_room: Entity<OpenRoom>,
    space_rooms: Entity<SpaceRoomListEntity>,

    list_state: ListState,
}

impl SpaceLobbyContent {
    pub fn new(open_room: Entity<OpenRoom>, cx: &mut Context<Self>) -> Self {
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
                            SpaceRoomListPaginationState::Idle { end_reached: true } => 0,
                            _ => 3,
                        },
                );
                list_state.scroll_to(last_offset);
            }
        })
        .detach();

        Self {
            open_room,
            space_rooms,
            list_state,
        }
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
                                        .child(skeleton("join-skeleton").child(
                                            button("join-button").child(icon_text(
                                                "list-add".into(),
                                                tr!("JOIN_ROOM").into(),
                                            )),
                                        )),
                                ),
                        ),
                )
                .into_any_element();
        };

        let session_manager = cx.global::<SessionManager>();
        let joined_room = session_manager
            .rooms()
            .read(cx)
            .room(&space_room.room_id)
            .and_then(|room| {
                let room = &room.read(cx).inner;
                if room.state() == RoomState::Joined {
                    Some(room)
                } else {
                    None
                }
            });

        // let view_button_type = if let Some(joined_room) = joined_room {
        //     crate::chat::room_directory::JoinButtonType::View(joined_room.clone())
        // } else if room_description.join_rule == JoinRuleKind::Knock {
        //     crate::chat::room_directory::JoinButtonType::Knock
        // } else {
        //     crate::chat::room_directory::JoinButtonType::Join
        // };

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
                            .rounded(theme.border_radius)
                            .size(px(40.))
                            .size_policy(SizePolicy::Fit),
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
                                    )), // .child(match view_button_type {
                                        //     crate::chat::room_directory::JoinButtonType::View(room) => {
                                        //         let room_id = room.room_id().to_owned();
                                        //         button("view-button")
                                        //             .child(icon_text(
                                        //                 "go-next".into(),
                                        //                 tr!("VIEW_ROOM", "View Room").into(),
                                        //             ))
                                        //             .on_click(cx.listener(
                                        //                 move |this, _, window, cx| {
                                        //                     this.change_room(room_id.clone(), cx);
                                        //                 },
                                        //             ))
                                        //     }
                                        //     crate::chat::room_directory::JoinButtonType::Knock => {
                                        //         button("join-button")
                                        //             .when(joining_room, |button| button.disabled())
                                        //             .child(icon_text(
                                        //                 "list-add".into(),
                                        //                 tr!("KNOCK_ON_ROOM", "Knock").into(),
                                        //             ))
                                        //             .on_click(cx.listener(
                                        //                 move |this, _, window, cx| {
                                        //                     this.join_room(
                                        //                         room_id.clone(),
                                        //                         true,
                                        //                         window,
                                        //                         cx,
                                        //                     );
                                        //                 },
                                        //             ))
                                        //     }
                                        //     crate::chat::room_directory::JoinButtonType::Join => {
                                        //         button("join-button")
                                        //             .when(joining_room, |button| button.disabled())
                                        //             .child(icon_text(
                                        //                 "list-add".into(),
                                        //                 tr!("JOIN_ROOM").into(),
                                        //             ))
                                        //             .on_click(cx.listener(
                                        //                 move |this, _, window, cx| {
                                        //                     this.join_room(
                                        //                         room_id.clone(),
                                        //                         false,
                                        //                         window,
                                        //                         cx,
                                        //                     );
                                        //                 },
                                        //             ))
                                        //     }
                                        // }),
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
                            .size(px(150.))
                            .size_policy(SizePolicy::Fit),
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
                                button("create-room")
                                    .child(icon_text("list-add".into(), tr!("CREATE_ROOM").into())),
                            )
                            .child(button("create-space").child(icon_text(
                                "list-add".into(),
                                tr!("CREATE_SUBSPACE", "Create Subordinate Space").into(),
                            ))),
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
