use crate::chat::displayed_room::DisplayedRoom;
use crate::chat::sidebar::space_sidebar_page::SpaceSidebarPage;
use crate::chat::sidebar::{Sidebar, SidebarPage};
use crate::mxc_image::{SizePolicy, mxc_image};
use cntp_i18n::{tr, trn};
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon::icon;
use contemporary::components::layer::layer;
use contemporary::components::subtitle::subtitle;
use contemporary::styling::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::{
    AppContext, Context, Element, ElementId, Entity, InteractiveElement, IntoElement,
    ListAlignment, ListState, ParentElement, Render, StatefulInteractiveElement, Styled,
    Subscription, Window, div, list, px,
};
use matrix_sdk::ruma::OwnedRoomId;
use thegrid::session::room_cache::{CachedRoom, RoomCategory};
use thegrid::session::session_manager::SessionManager;

pub struct RootSidebarPage {
    list_state: ListState,
    sidebar: Entity<Sidebar>,
    displayed_room: Entity<DisplayedRoom>,
    items: Vec<SidebarItem>,
    room_cache_subscription: Option<Subscription>,
}

pub enum SidebarItem {
    Heading(String),
    Room(Entity<CachedRoom>),
    Space(Entity<CachedRoom>),
    Create,
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

        Self {
            list_state: ListState::new(0, ListAlignment::Top, px(200.)),
            sidebar,
            displayed_room,
            items: Vec::new(),
            room_cache_subscription: None,
        }
    }

    fn change_room(&mut self, room_id: OwnedRoomId, window: &mut Window, cx: &mut Context<Self>) {
        let session_manager = cx.global::<SessionManager>();
        let room_cache = session_manager.rooms().read(cx);

        let room = room_cache.room(&room_id).unwrap().read(cx);
        if room.inner.is_space() {
            let sidebar = self.sidebar.clone();
            let sidebar_page = cx.new(|cx| {
                SpaceSidebarPage::new(cx, room_id.clone(), sidebar, self.displayed_room.clone())
            });
            self.sidebar.update(cx, |sidebar, cx| {
                sidebar.push_page(SidebarPage::Space(sidebar_page))
            });
        } else {
            self.displayed_room
                .write(cx, DisplayedRoom::Room(room_id.clone()));
        }
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
        let mut rooms = root_rooms
            .iter()
            .filter(|room| !room.read(cx).inner.is_space())
            .map(|room| SidebarItem::Room(room.clone()))
            .collect::<Vec<_>>();

        let mut vec = Vec::new();
        vec.push(SidebarItem::Create);
        if !spaces.is_empty() {
            vec.push(SidebarItem::Heading(
                tr!("ROOT_SIDEBAR_SPACES", "Spaces").into(),
            ));
            vec.append(&mut spaces);
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
            .child(
                div().flex_grow().child(
                    list(
                        self.list_state.clone(),
                        cx.processor(move |this, i, _, cx| {
                            let theme = cx.global::<Theme>();
                            let item = &this.items[i];

                            let displayed_room = this.displayed_room.read(cx);

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
                                        .child(icon("list-add".into()))
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
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.displayed_room
                                                .write(cx, DisplayedRoom::CreateRoom);
                                        }))
                                        .into_any_element()
                                }
                                SidebarItem::Heading(heading) => {
                                    div().pt(px(4.)).child(subtitle(heading)).into_any_element()
                                }
                                SidebarItem::Room(room) => {
                                    let room = room.read(cx);
                                    let room_id = room.inner.room_id().to_owned();

                                    div()
                                        .flex()
                                        .w_full()
                                        .items_center()
                                        .id(ElementId::Name(
                                            room.inner.room_id().to_string().into(),
                                        ))
                                        .m(px(2.))
                                        .p(px(2.))
                                        .rounded(theme.border_radius)
                                        .when(
                                            current_room.is_some_and(|current_room| {
                                                current_room == room_id
                                            }),
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
                                            room.inner.num_unread_notifications() > 0,
                                            |david| {
                                                david.child(
                                                    div()
                                                        .rounded(theme.border_radius)
                                                        .bg(theme.error_accent_color)
                                                        .p(px(2.))
                                                        .child(
                                                            room.inner
                                                                .num_unread_notifications()
                                                                .to_string(),
                                                        ),
                                                )
                                            },
                                            |david| {
                                                david.when(
                                                    room.inner.num_unread_messages() > 0,
                                                    |david| {
                                                        david.child(
                                                            div()
                                                                .bg(theme.foreground)
                                                                .size(px(8.))
                                                                .rounded(px(4.)),
                                                        )
                                                    },
                                                )
                                            },
                                        )
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.change_room(room_id.clone(), window, cx);
                                        }))
                                        .into_any_element()
                                }
                                SidebarItem::Space(room) => {
                                    let room = room.read(cx);
                                    let room_id = room.inner.room_id().to_owned();
                                    div()
                                        .flex()
                                        .items_center()
                                        .id(ElementId::Name(
                                            room.inner.room_id().to_string().into(),
                                        ))
                                        .m(px(2.))
                                        .p(px(2.))
                                        .gap(px(2.))
                                        .child(
                                            mxc_image(room.inner.avatar_url())
                                                .size(px(32.))
                                                .size_policy(SizePolicy::Fit)
                                                .rounded(theme.border_radius),
                                        )
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
                                }
                            }
                        }),
                    )
                    .flex()
                    .flex_col()
                    .h_full(),
                ),
            )
    }
}
