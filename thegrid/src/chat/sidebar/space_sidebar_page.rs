use crate::chat::displayed_room::DisplayedRoom;
use crate::chat::sidebar::{Sidebar, SidebarPage};
use contemporary::components::grandstand::grandstand;
use contemporary::styling::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, Context, ElementId, Entity, FontWeight, InteractiveElement, IntoElement,
    ListAlignment, ListState, ParentElement, Render, StatefulInteractiveElement, Styled, Window,
    div, list, px,
};
use matrix_sdk::ruma::OwnedRoomId;
use thegrid::session::room_cache::{CachedRoom, RoomCategory};
use thegrid::session::session_manager::SessionManager;

pub struct SpaceSidebarPage {
    room_id: OwnedRoomId,
    list_state: ListState,
    sidebar: Entity<Sidebar>,
    displayed_room: Entity<DisplayedRoom>,
}

impl SpaceSidebarPage {
    pub fn new(
        cx: &mut App,
        room_id: OwnedRoomId,
        sidebar: Entity<Sidebar>,
        displayed_room: Entity<DisplayedRoom>,
    ) -> Self {
        Self {
            list_state: ListState::new(0, ListAlignment::Top, px(200.)),
            room_id,
            sidebar,
            displayed_room,
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
                            let theme = cx.global::<Theme>();
                            let room: &Entity<CachedRoom> = &root_rooms[i];
                            let room = room.read(cx);
                            let room_id = room.inner.room_id().to_owned();

                            let current_room = match this.displayed_room.read(cx) {
                                DisplayedRoom::Room(room_id) => Some(room_id.clone()),
                                _ => None,
                            };

                            div()
                                .flex()
                                .w_full()
                                .items_center()
                                .id(ElementId::Name(room.inner.room_id().to_string().into()))
                                .m(px(2.))
                                .p(px(2.))
                                .rounded(theme.border_radius)
                                .when(
                                    current_room
                                        .is_some_and(|current_room| current_room == room_id),
                                    |david| david.bg(theme.button_background),
                                )
                                .child(
                                    room.inner
                                        .cached_display_name()
                                        .map(|name| name.to_string())
                                        .or_else(|| room.inner.name())
                                        .unwrap_or_default(),
                                )
                                .child(div().flex_grow())
                                .when_else(
                                    room.inner.unread_notification_counts().notification_count > 0,
                                    |david| {
                                        david.font_weight(FontWeight::BOLD).child(
                                            div()
                                                .rounded(theme.border_radius)
                                                .bg(theme.error_accent_color)
                                                .p(px(2.))
                                                .child(
                                                    room.inner
                                                        .unread_notification_counts()
                                                        .notification_count
                                                        .to_string(),
                                                ),
                                        )
                                    },
                                    |david| {
                                        david.when(room.inner.num_unread_messages() > 0, |david| {
                                            david.child(
                                                div()
                                                    .bg(theme.foreground)
                                                    .size(px(8.))
                                                    .rounded(px(4.)),
                                            )
                                        })
                                    },
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
