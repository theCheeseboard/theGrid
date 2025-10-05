use crate::chat::displayed_room::DisplayedRoom;
use cntp_i18n::tr;
use contemporary::components::grandstand::grandstand;
use contemporary::components::layer::layer;
use contemporary::components::pager::pager;
use gpui::{div, list, px, App, ElementId, InteractiveElement, IntoElement, ListAlignment, ListState, ParentElement, RenderOnce, StatefulInteractiveElement, Styled, Window};
use std::rc::Rc;
use thegrid::session::session_manager::SessionManager;

type ChangeRoomHandler = dyn Fn(&ChangeRoomEvent, &mut Window, &mut App) + 'static;

#[derive(Clone)]
pub struct ChangeRoomEvent {
    pub new_room: DisplayedRoom,
}

#[derive(IntoElement)]
pub struct Sidebar {
    on_change_room: Option<Rc<Box<ChangeRoomHandler>>>,
}

pub fn sidebar() -> Sidebar {
    Sidebar {
        on_change_room: None,
    }
}

impl Sidebar {
    pub fn on_change_room(
        mut self,
        on_change_room: impl Fn(&ChangeRoomEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change_room = Some(Rc::new(Box::new(on_change_room)));
        self
    }
}

impl RenderOnce for Sidebar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let root_list_state =
            window.use_state(cx, |_, _| ListState::new(0, ListAlignment::Top, px(200.)));

        let session_manager = cx.global::<SessionManager>();

        let Some(session) = session_manager.current_session() else {
            return layer();
        };
        let Some(client) = session_manager.client() else {
            return layer();
        };

        let client = client.read(cx);

        let root_rooms: Vec<_> = client
            .joined_rooms()
            .iter()
            .filter(|room| !room.is_space())
            .cloned()
            .collect();
        let root_list_state = root_list_state.read(cx);
        if root_rooms.len() != root_list_state.item_count() {
            root_list_state.reset(root_rooms.len());
        }

        let change_room_handler = self.on_change_room.unwrap().clone();

        layer()
            .w(px(300.))
            .flex()
            .flex_col()
            .child(
                pager("sidebar-pager", 0).flex_grow().page(
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
                                list(root_list_state.clone(), move |i, _, cx| {
                                    let room = &root_rooms[i];
                                    let room_id = room.room_id().to_owned();
                                    let change_room_handler = change_room_handler.clone();
                                    div()
                                        .id(ElementId::Name(room.room_id().to_string().into()))
                                        .p(px(2.))
                                        .child(room.name().unwrap_or_default())
                                        .on_click(move |_, window, cx| {
                                            let event = ChangeRoomEvent {
                                                new_room: DisplayedRoom::Room(room_id.clone())
                                            };
                                            change_room_handler(&event, window, cx);
                                        })
                                        .into_any_element()
                                })
                                .flex()
                                .flex_col()
                                .h_full(),
                            ),
                        )
                        .into_any_element(),
                ),
            )
            .child(
                layer()
                    .p(px(4.))
                    .flex()
                    .child(session.matrix_session.meta.user_id.to_string()),
            )
    }
}
