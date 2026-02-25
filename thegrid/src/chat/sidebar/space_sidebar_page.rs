use crate::chat::chat_room::invite_popover::InvitePopover;
use crate::chat::displayed_room::DisplayedRoom;
use crate::chat::sidebar::standard_room_element::{InviteEvent, StandardRoomElement};
use crate::chat::sidebar::{Sidebar, SidebarPage};
use cntp_i18n::tr;
use contemporary::components::context_menu::ContextMenuItem;
use contemporary::components::grandstand::grandstand;
use contemporary::styling::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, Context, ElementId, Entity, FontWeight, InteractiveElement, IntoElement,
    ListAlignment, ListState, ParentElement, Render, StatefulInteractiveElement, Styled, Window,
    div, list, px,
};
use matrix_sdk::ruma::OwnedRoomId;
use std::rc::Rc;
use thegrid_common::session::room_cache::{CachedRoom, RoomCategory};
use thegrid_common::session::session_manager::SessionManager;

pub struct SpaceSidebarPage {
    room_id: OwnedRoomId,
    list_state: ListState,
    sidebar: Entity<Sidebar>,
    displayed_room: Entity<DisplayedRoom>,
    invite_popover: Entity<InvitePopover>,
}

impl SpaceSidebarPage {
    pub fn new(
        cx: &mut App,
        room_id: OwnedRoomId,
        sidebar: Entity<Sidebar>,
        displayed_room: Entity<DisplayedRoom>,
    ) -> Self {
        let invite_popover = cx.new(|cx| InvitePopover::new(cx));

        Self {
            list_state: ListState::new(0, ListAlignment::Top, px(200.)),
            room_id,
            sidebar,
            displayed_room,
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
                let page = SpaceSidebarPage::new(
                    cx,
                    room_id.clone(),
                    sidebar,
                    self.displayed_room.clone(),
                );
                page
            });
            self.sidebar.update(cx, |sidebar, cx| {
                sidebar.push_page(SidebarPage::Space(sidebar_page))
            });
        } else {
            self.displayed_room
                .write(cx, DisplayedRoom::Room(room_id.clone()));
        }
    }

    fn invite_to_room(&mut self, event: &InviteEvent, window: &mut Window, cx: &mut Context<Self>) {
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

        let root_rooms = room_cache
            .rooms_in_category(RoomCategory::Space(self.room_id.clone()), cx)
            .clone();

        if root_rooms.len() != self.list_state.item_count() {
            self.list_state.reset(root_rooms.len());
        }

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
            .child(
                div().flex_grow().child(
                    list(
                        self.list_state.clone(),
                        cx.processor(move |this, i, _, cx| {
                            let room_entity: &Entity<CachedRoom> = &root_rooms[i];
                            let room = room_entity.read(cx);
                            let room_id = room.inner.room_id().to_owned();

                            let current_room = match this.displayed_room.read(cx) {
                                DisplayedRoom::Room(room_id) => Some(room_id.clone()),
                                _ => None,
                            };
                            div()
                                .id(ElementId::Name(room.inner.room_id().to_string().into()))
                                .child(StandardRoomElement {
                                    room: room_entity.clone(),
                                    current_room,
                                    on_click: Rc::new(Box::new(cx.listener(
                                        move |this, _, window, cx| {
                                            this.change_room(room_id.clone(), window, cx);
                                        },
                                    ))),
                                    on_invite: Rc::new(Box::new(cx.listener(Self::invite_to_room))),
                                })
                                .into_any_element()
                        }),
                    )
                    .flex()
                    .flex_col()
                    .h_full(),
                ),
            )
            .child(self.invite_popover.clone())
    }
}
