use crate::chat::chat_room::invite_popover::InvitePopover;
use crate::chat::displayed_room::DisplayedRoom;
use crate::chat::sidebar::directory_sidebar_page::DirectorySidebarPage;
use crate::chat::sidebar::sidebar_list::{sidebar_list, SidebarItem, SidebarListEvent};
use crate::chat::sidebar::space_sidebar_page::SpaceSidebarPage;
use crate::chat::sidebar::standard_room_element::InviteEvent;
use crate::chat::sidebar::{Sidebar, SidebarPage};
use cntp_i18n::tr;
use contemporary::components::grandstand::grandstand;
use gpui::{
    div, px, AppContext, Context, Entity, IntoElement, ListAlignment, ListState,
    ParentElement, Render, Styled, Subscription, Window,
};
use matrix_sdk::ruma::OwnedRoomId;
use thegrid_common::session::room_cache::{CachedRoom, RoomCategory};
use thegrid_common::session::session_manager::SessionManager;

pub struct RootSidebarPage {
    list_state: ListState,
    sidebar: Entity<Sidebar>,
    displayed_room: Entity<DisplayedRoom>,
    items: Vec<SidebarItem>,
    room_cache_subscription: Option<Subscription>,
    invite_popover: Entity<InvitePopover>,
}

impl RootSidebarPage {
    pub fn new(
        sidebar: Entity<Sidebar>,
        displayed_room: Entity<DisplayedRoom>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe_global::<SessionManager>(|this, cx| {
            this.update_sidebar_rooms(cx);

            let session_manager = cx.global::<SessionManager>();
            if session_manager.client().is_none() {
                this.room_cache_subscription = None;
                return;
            }

            let room_cache = session_manager.rooms();
            this.room_cache_subscription =
                Some(cx.observe(&room_cache, |this, _, cx| this.update_sidebar_rooms(cx)));
        })
        .detach();

        let invite_popover = cx.new(|cx| InvitePopover::new(cx));

        Self {
            list_state: ListState::new(0, ListAlignment::Top, px(200.)),
            sidebar,
            displayed_room,
            items: Vec::new(),
            room_cache_subscription: None,
            invite_popover,
        }
    }

    fn change_room(&mut self, room_id: OwnedRoomId, window: &mut Window, cx: &mut Context<Self>) {
        let session_manager = cx.global::<SessionManager>();
        let room_cache = session_manager.rooms().read(cx);

        let room = room_cache.room(&room_id).unwrap().read(cx);
        if room.inner.is_space() {
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

    fn open_directory(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let sidebar = self.sidebar.clone();
        let directory_page =
            cx.new(|cx| DirectorySidebarPage::new(cx, sidebar, self.displayed_room.clone()));
        self.sidebar.update(cx, |sidebar, cx| {
            sidebar.push_page(SidebarPage::Directory(directory_page))
        });
    }

    fn invite_to_room(&mut self, event: &InviteEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.invite_popover.update(cx, |invite_popover, cx| {
            invite_popover.open_invite_popover(event.room_id.clone(), cx)
        })
    }

    fn update_sidebar_rooms(&mut self, cx: &mut Context<Self>) {
        let session_manager = cx.global::<SessionManager>();
        if session_manager.client().is_none() {
            self.list_state.reset(0);
            self.items = Vec::new();
            return;
        }

        let room_cache = session_manager.rooms().read(cx);
        let root_rooms: Vec<Entity<CachedRoom>> =
            room_cache.rooms_in_category(RoomCategory::Root, cx).clone();

        let mut spaces = root_rooms
            .iter()
            .filter(|room| room.read(cx).inner.is_space())
            .map(|room| SidebarItem::Space(room.clone()))
            .collect::<Vec<_>>();
        let mut direct_rooms = root_rooms
            .iter()
            .filter(|room| !room.read(cx).inner.is_space())
            .filter(|room| room.read(cx).is_direct())
            .map(|room| SidebarItem::Room(room.clone()))
            .collect::<Vec<_>>();
        let mut rooms = root_rooms
            .iter()
            .filter(|room| !room.read(cx).inner.is_space())
            .filter(|room| !room.read(cx).is_direct())
            .map(|room| SidebarItem::Room(room.clone()))
            .collect::<Vec<_>>();

        let mut vec = Vec::new();
        vec.push(SidebarItem::Create);
        vec.push(SidebarItem::Directory);
        if !spaces.is_empty() {
            vec.push(SidebarItem::Heading(
                tr!("ROOT_SIDEBAR_SPACES", "Spaces").into(),
            ));
            vec.append(&mut spaces);
        }
        if !direct_rooms.is_empty() {
            vec.push(SidebarItem::Heading(
                tr!("ROOT_DIRECT_ROOMS", "1:1 Conversations").into(),
            ));
            vec.append(&mut direct_rooms);
        }
        if !rooms.is_empty() {
            vec.push(SidebarItem::Heading(
                tr!("ROOT_SIDEBAR_ROOMS", "Rooms").into(),
            ));
            vec.append(&mut rooms);
        }

        if self.list_state.item_count() != vec.len() {
            self.list_state.reset(vec.len());
        }
        self.items = vec;
    }
}

impl Render for RootSidebarPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .h_full()
            .child(
                grandstand("sidebar-grandstand")
                    .text(tr!("ROOMS_SPACES", "Rooms and Spaces"))
                    .pt(px(36.)),
            )
            .child(div().flex_grow().child(sidebar_list(
                self.list_state.clone(),
                self.items.clone(),
                self.displayed_room.clone(),
                cx.listener(move |this, event, window, cx| match event {
                    SidebarListEvent::OpenDirectory => this.open_directory(window, cx),
                    SidebarListEvent::ChangeRoom(room_id) => {
                        this.change_room(room_id.clone(), window, cx)
                    }
                    SidebarListEvent::InviteToRoom(invite_event) => {
                        this.invite_to_room(invite_event, window, cx)
                    }
                }),
            )))
            .child(self.invite_popover.clone())
    }
}
