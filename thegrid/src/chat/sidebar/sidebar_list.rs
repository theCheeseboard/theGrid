use crate::chat::displayed_room::DisplayedRoom;
use crate::chat::sidebar::standard_room_element::{
    InviteEvent, StandardRoomElement, StandardRoomElementType,
};
use cntp_i18n::{tr, trn};
use contemporary::components::icon::icon;
use contemporary::components::subtitle::subtitle;
use contemporary::styling::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, ElementId, Entity, InteractiveElement, IntoElement, ListState, ParentElement, RenderOnce,
    StatefulInteractiveElement, Styled, Window, div, list, px,
};
use matrix_sdk::ruma::OwnedRoomId;
use std::rc::Rc;
use thegrid_common::mxc_image::{SizePolicy, mxc_image};
use thegrid_common::session::room_cache::CachedRoom;
use thegrid_common::session::session_manager::SessionManager;

#[derive(IntoElement)]
pub struct SidebarList {
    items: Vec<SidebarItem>,
    list_state: ListState,
    displayed_room: Entity<DisplayedRoom>,
    event_handler: Rc<Box<dyn Fn(&SidebarListEvent, &mut Window, &mut App)>>,
}

pub enum SidebarListEvent {
    OpenDirectory,
    ChangeRoom(OwnedRoomId),
    InviteToRoom(InviteEvent),
}

#[derive(Clone)]
pub enum SidebarItem {
    Heading(String),
    Room(Entity<CachedRoom>),
    Space(Entity<CachedRoom>),
    SpaceLobby(Entity<CachedRoom>),
    Create,
    Directory,
}

pub fn sidebar_list(
    list_state: ListState,
    items: Vec<SidebarItem>,
    displayed_room: Entity<DisplayedRoom>,
    callback: impl Fn(&SidebarListEvent, &mut Window, &mut App) + 'static,
) -> SidebarList {
    SidebarList {
        list_state,
        items,
        displayed_room,
        event_handler: Rc::new(Box::new(callback)),
    }
}

impl RenderOnce for SidebarList {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let items = self.items;
        let displayed_room_entity = self.displayed_room;
        let event_handler = self.event_handler;
        list(self.list_state, move |i, _, cx| {
            let theme = cx.global::<Theme>();
            let item = &items[i];

            let displayed_room = displayed_room_entity.read(cx);

            let current_room = match displayed_room {
                DisplayedRoom::Room(room_id) => Some(room_id.clone()),
                _ => None,
            };

            match item {
                SidebarItem::Create => {
                    let session_manager = cx.global::<SessionManager>();
                    let room_cache = session_manager.rooms().read(cx);
                    let invited_rooms = room_cache.invited_rooms(cx);

                    div()
                        .id("create-join")
                        .m(px(2.))
                        .p(px(2.))
                        .gap(px(4.))
                        .rounded(theme.border_radius)
                        .flex()
                        .w_full()
                        .items_center()
                        .child(icon("list-add"))
                        .child(tr!("SIDEBAR_CREATE_JOIN", "Create or Join"))
                        .when(!invited_rooms.is_empty(), |david| {
                            david.child(
                                div()
                                    .rounded(theme.border_radius)
                                    .bg(theme.info_accent_color)
                                    .p(px(2.))
                                    .child(trn!(
                                        "SIDEBAR_PENDING_INVITES",
                                        "{{count}} invite",
                                        "{{count}} invites",
                                        count = invited_rooms.len() as isize
                                    )),
                            )
                        })
                        .when(
                            matches!(displayed_room, DisplayedRoom::CreateRoom),
                            |david| david.bg(theme.button_background),
                        )
                        .on_click({
                            let displayed_room_entity = displayed_room_entity.clone();
                            move |_, window, cx| {
                                displayed_room_entity.write(cx, DisplayedRoom::CreateRoom);
                            }
                        })
                        .into_any_element()
                }
                SidebarItem::Directory => div()
                    .id("directory")
                    .m(px(2.))
                    .p(px(2.))
                    .gap(px(4.))
                    .rounded(theme.border_radius)
                    .flex()
                    .w_full()
                    .items_center()
                    .child(icon("map-globe"))
                    .child(tr!("SIDEBAR_DIRECTORY", "Room Directory"))
                    .on_click({
                        let event_handler = event_handler.clone();
                        move |_, window, cx| {
                            event_handler(&SidebarListEvent::OpenDirectory, window, cx)
                        }
                    })
                    .into_any_element(),
                SidebarItem::Heading(heading) => div()
                    .pt(px(8.))
                    .pl(px(4.))
                    .child(subtitle(heading))
                    .into_any_element(),
                SidebarItem::Room(room_entity) | SidebarItem::SpaceLobby(room_entity) => {
                    let room = room_entity.read(cx);
                    let room_id = room.inner.room_id().to_owned();

                    div()
                        .id(ElementId::Name(room.inner.room_id().to_string().into()))
                        .child(StandardRoomElement {
                            room: room_entity.clone(),
                            render_as: if matches!(item, SidebarItem::SpaceLobby(_)) {
                                StandardRoomElementType::Space
                            } else {
                                StandardRoomElementType::Room
                            },
                            current_room,
                            on_click: Rc::new(Box::new({
                                let event_handler = event_handler.clone();
                                move |_, window, cx| {
                                    event_handler(
                                        &SidebarListEvent::ChangeRoom(room_id.clone()),
                                        window,
                                        cx,
                                    )
                                }
                            })),
                            on_invite: Rc::new(Box::new({
                                let event_handler = event_handler.clone();
                                move |event, window, cx| {
                                    event_handler(
                                        &SidebarListEvent::InviteToRoom(event.clone()),
                                        window,
                                        cx,
                                    )
                                }
                            })),
                        })
                        .into_any_element()
                }
                SidebarItem::Space(room) => {
                    let room = room.read(cx);
                    let room_id = room.inner.room_id().to_owned();
                    div()
                        .flex()
                        .items_center()
                        .id(ElementId::Name(room.inner.room_id().to_string().into()))
                        .m(px(2.))
                        .p(px(2.))
                        .gap(px(2.))
                        .child(
                            mxc_image(room.inner.avatar_url())
                                .fallback_image(room.inner.room_id())
                                .size_policy(SizePolicy::Constrain(32., 32.))
                                .rounded(theme.border_radius),
                        )
                        .child(
                            room.inner
                                .cached_display_name()
                                .map(|name| name.to_string())
                                .or_else(|| room.inner.name())
                                .unwrap_or_default(),
                        )
                        .on_click({
                            let event_handler = event_handler.clone();
                            move |_, window, cx| {
                                event_handler(
                                    &SidebarListEvent::ChangeRoom(room_id.clone()),
                                    window,
                                    cx,
                                )
                            }
                        })
                        .into_any_element()
                }
            }
        })
        .flex()
        .flex_col()
        .h_full()
    }
}
