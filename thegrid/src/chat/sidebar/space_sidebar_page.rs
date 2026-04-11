use crate::chat::chat_room::invite_popover::InvitePopover;
use crate::chat::displayed_room::DisplayedRoom;
use crate::chat::sidebar::sidebar_list::{sidebar_list, SidebarItem, SidebarListEvent};
use crate::chat::sidebar::standard_room_element::InviteEvent;
use crate::chat::sidebar::{Sidebar, SidebarPage};
use cntp_i18n::tr;
use contemporary::components::button::button;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon_text::icon_text;
use contemporary::components::interstitial::interstitial;
use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, AppContext, Context, Entity, IntoElement, ListAlignment, ListState,
    ParentElement, Render, Styled, Window,
};
use matrix_sdk::ruma::OwnedRoomId;
use thegrid_common::session::room_cache::RoomCategory;
use thegrid_common::session::session_manager::SessionManager;

pub struct SpaceSidebarPage {
    room_id: OwnedRoomId,
    list_state: ListState,
    sidebar: Entity<Sidebar>,
    displayed_room: Entity<DisplayedRoom>,
    invite_popover: Entity<InvitePopover>,
    items: Vec<SidebarItem>,
    have_rooms: bool,
}

impl SpaceSidebarPage {
    pub fn new(
        room_id: OwnedRoomId,
        sidebar: Entity<Sidebar>,
        displayed_room: Entity<DisplayedRoom>,
        cx: &mut Context<Self>,
    ) -> Self {
        let invite_popover = cx.new(|cx| InvitePopover::new(cx));

        let session_manager = cx.global::<SessionManager>();
        let room_cache = session_manager.rooms();
        cx.observe(&room_cache, |this, _, cx| this.update_sidebar_rooms(cx))
            .detach();

        let mut this = Self {
            list_state: ListState::new(0, ListAlignment::Top, px(200.)),
            room_id,
            sidebar,
            displayed_room,
            invite_popover,
            items: Vec::new(),
            have_rooms: false,
        };
        this.update_sidebar_rooms(cx);
        this
    }

    fn update_sidebar_rooms(&mut self, cx: &mut Context<Self>) {
        let session_manager = cx.global::<SessionManager>();
        let room_cache = session_manager.rooms();
        let room_cache = room_cache.read(cx);

        let mut items = Vec::new();
        items.push(SidebarItem::SpaceLobby(
            room_cache.room(&self.room_id).unwrap(),
        ));

        let space_rooms = room_cache
            .rooms_in_category(RoomCategory::Space(self.room_id.clone()), cx)
            .clone();

        let mut rooms = Vec::new();
        let mut subordinate_spaces = Vec::new();

        for room_entity in space_rooms {
            if room_entity.read(cx).inner.is_space() {
                subordinate_spaces.push(SidebarItem::Space(room_entity.clone()));
            } else {
                rooms.push(SidebarItem::Room(room_entity.clone()));
            }
        }

        self.have_rooms = false;
        if !subordinate_spaces.is_empty() {
            items.push(SidebarItem::Heading(
                tr!("SPACE_SIDEBAR_SUBORDINATE_SPACES", "Subordinate Spaces").into(),
            ));
            items.extend(subordinate_spaces);
            self.have_rooms = true;
        }

        if !rooms.is_empty() {
            items.push(SidebarItem::Heading(tr!("ROOT_SIDEBAR_ROOMS").into()));
            items.extend(rooms);
            self.have_rooms = true;
        }

        self.items = items;

        if self.items.len() != self.list_state.item_count() {
            self.list_state.reset(self.items.len());
        }

        cx.notify();
    }

    fn change_room(&mut self, room_id: OwnedRoomId, _: &mut Window, cx: &mut Context<Self>) {
        let session_manager = cx.global::<SessionManager>();
        let room_cache = session_manager.rooms().read(cx);

        let room = room_cache.room(&room_id).unwrap().read(cx);
        if room.inner.is_space() && room_id != self.room_id {
            let sidebar = self.sidebar.clone();
            let sidebar_page = cx.new(|cx| {
                SpaceSidebarPage::new(room_id.clone(), sidebar, self.displayed_room.clone(), cx)
            });
            self.sidebar.update(cx, |sidebar, cx| {
                sidebar.push_page(SidebarPage::Space(sidebar_page))
            });
        } else {
            self.displayed_room
                .write(cx, DisplayedRoom::Room(room_id.clone()));
        }
    }

    fn invite_to_room(&mut self, event: &InviteEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.invite_popover.update(cx, |invite_popover, cx| {
            invite_popover.open_invite_popover(event.room_id.clone(), cx)
        })
    }
}

impl Render for SpaceSidebarPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let session_manager = cx.global::<SessionManager>();
        let room_cache = session_manager.rooms().read(cx);

        let this_room = room_cache.room(&self.room_id).unwrap().read(cx);

        div()
            .flex()
            .flex_col()
            .h_full()
            .child(
                grandstand("sidebar-grandstand")
                    .text(this_room.inner.name().unwrap_or_default())
                    .pt(px(36.))
                    .on_back_click(cx.listener(|this, _, _, cx| {
                        this.sidebar.update(cx, |sidebar, cx| {
                            sidebar.pop_page();
                            cx.notify();
                        })
                    })),
            )
            .when_else(
                self.have_rooms,
                |david| {
                    david.child(div().flex_grow().child(sidebar_list(
                        self.list_state.clone(),
                        self.items.clone(),
                        self.displayed_room.clone(),
                        cx.listener(|this, event, window, cx| match event {
                            SidebarListEvent::ChangeRoom(room_id) => {
                                this.change_room(room_id.clone(), window, cx)
                            }
                            SidebarListEvent::InviteToRoom(invite_event) => {
                                this.invite_to_room(invite_event, window, cx)
                            }
                            _ => {}
                        }),
                    )))
                },
                |david| {
                    david.child(
                        interstitial()
                            .icon("im-room")
                            .title(tr!("SPACE_SIDEBAR_NO_ROOMS", "No joined rooms"))
                            .message(tr!(
                                "SPACE_SIDEBAR_NO_ROOMS_MESSAGE",
                                "You haven't joined any rooms in this space. Check out the lobby \
                                to find rooms to join!"
                            ))
                            .child(
                                button("open-lobby")
                                    .child(icon_text(
                                        "map-globe",
                                        tr!("SPACE_SIDEBAR_OPEN_LOBBY", "Open Lobby"),
                                    ))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.change_room(this.room_id.clone(), window, cx);
                                    })),
                            )
                            .flex_grow(),
                    )
                },
            )
            .child(self.invite_popover.clone())
    }
}
