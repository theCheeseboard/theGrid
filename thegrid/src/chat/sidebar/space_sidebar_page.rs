use crate::chat::displayed_room::DisplayedRoom;
use crate::chat::main_chat_surface::{ChangeRoomEvent, ChangeRoomHandler};
use crate::chat::sidebar::{Sidebar, SidebarPage};
use cntp_i18n::tr;
use contemporary::components::grandstand::grandstand;
use gpui::{
    App, AppContext, Context, ElementId, Entity, InteractiveElement, IntoElement, ListAlignment,
    ListState, ParentElement, Render, StatefulInteractiveElement, Styled, Window, div, list, px,
};
use matrix_sdk::ruma::OwnedRoomId;
use std::rc::Rc;
use thegrid::session::room_cache::{CachedRoom, RoomCategory};
use thegrid::session::session_manager::SessionManager;

pub struct SpaceSidebarPage {
    room_id: OwnedRoomId,
    list_state: ListState,
    sidebar: Entity<Sidebar>,
    on_change_room: Rc<Box<ChangeRoomHandler>>,
}

impl SpaceSidebarPage {
    pub fn new(
        cx: &mut App,
        room_id: OwnedRoomId,
        sidebar: Entity<Sidebar>,
        on_change_room: impl Fn(&ChangeRoomEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            list_state: ListState::new(0, ListAlignment::Top, px(200.)),
            room_id,
            sidebar,
            on_change_room: Rc::new(Box::new(on_change_room)),
        }
    }

    fn change_room(&mut self, room_id: OwnedRoomId, window: &mut Window, cx: &mut Context<Self>) {
        let session_manager = cx.global::<SessionManager>();
        let room_cache = session_manager.rooms().read(cx);

        let room = room_cache.room(&room_id).unwrap().read(cx);
        if room.inner.is_space() {
            let sidebar = self.sidebar.clone();
            let on_change_room = self.on_change_room.clone();
            let sidebar_page = cx.new(|cx| {
                let on_change_room = on_change_room.clone();
                let page = SpaceSidebarPage::new(
                    cx,
                    room_id.clone(),
                    sidebar,
                    move |event, window, cx| {
                        on_change_room(&event, window, cx);
                    },
                );
                page
            });
            self.sidebar.update(cx, |sidebar, cx| {
                sidebar.push_page(SidebarPage::Space(sidebar_page))
            });
        } else {
            let event = ChangeRoomEvent {
                new_room: DisplayedRoom::Room(room_id.clone()),
            };
            (self.on_change_room)(&event, window, cx);
        }
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

        let on_change_room = self.on_change_room.clone();

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
                            let room: &Entity<CachedRoom> = &root_rooms[i];
                            let room = room.read(cx);
                            let room_id = room.inner.room_id().to_owned();
                            div()
                                .id(ElementId::Name(room.inner.room_id().to_string().into()))
                                .p(px(2.))
                                .child(
                                    room.inner
                                        .cached_display_name()
                                        .map(|name| name.to_string())
                                        .or_else(|| room.inner.name())
                                        .unwrap_or_default(),
                                )
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.change_room(room_id.clone(), window, cx);
                                }))
                                .into_any_element()
                        }),
                    )
                    .flex()
                    .flex_col()
                    .h_full(),
                ),
            )
    }
}
