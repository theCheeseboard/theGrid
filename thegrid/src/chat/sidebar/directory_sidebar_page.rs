use crate::chat::displayed_room::DisplayedRoom;
use crate::chat::sidebar::Sidebar;
use cntp_i18n::tr;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon::icon;
use contemporary::styling::theme::ThemeStorage;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, Context, ElementId, Entity, InteractiveElement, IntoElement, ListAlignment,
    ListState, ParentElement, Render, StatefulInteractiveElement, Styled, Window, div, list, px,
};
use matrix_sdk::{OwnedServerName, ServerName};
use thegrid::session::session_manager::SessionManager;

pub struct DirectorySidebarPage {
    list_state: ListState,
    sidebar: Entity<Sidebar>,
    displayed_room: Entity<DisplayedRoom>,

    servers: Vec<OwnedServerName>,
}

impl DirectorySidebarPage {
    pub fn new(
        cx: &mut App,
        sidebar: Entity<Sidebar>,
        displayed_room: Entity<DisplayedRoom>,
    ) -> Self {
        let mut servers = Vec::new();

        let session_manager = cx.global::<SessionManager>();
        if let Some(client) = session_manager.client() {
            servers.push(client.read(cx).user_id().unwrap().server_name().to_owned())
        }

        servers.push(ServerName::parse("matrix.org").unwrap());
        servers.push(ServerName::parse("mozilla.org").unwrap());

        Self {
            list_state: ListState::new(servers.len() + 1, ListAlignment::Top, px(200.)),
            sidebar,
            servers,
            displayed_room,
        }
    }

    fn update_server_list(&mut self) {
        self.list_state.reset(self.servers.len() + 1);
    }

    fn change_server(
        &mut self,
        server_name: OwnedServerName,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let session_manager = cx.global::<SessionManager>();
        let room_cache = session_manager.rooms().read(cx);
        self.displayed_room
            .write(cx, DisplayedRoom::Directory(server_name.clone()));
    }
}

impl Render for DirectorySidebarPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .h_full()
            .child(
                grandstand("sidebar-grandstand")
                    .text(tr!("ROOM_DIRECTORY", "Room Directory"))
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
                            let theme = cx.theme();

                            if this.servers.len() == i {
                                div()
                                    .id("other-server-button")
                                    .child(
                                        div()
                                            .id("item")
                                            .flex()
                                            .w_full()
                                            .items_center()
                                            .m(px(2.))
                                            .p(px(2.))
                                            .gap(px(4.))
                                            .rounded(theme.border_radius)
                                            .child(icon("list-add".into()))
                                            .child(tr!(
                                                "ROOM_DIRECTORY_OTHER_SERVER",
                                                "Another server..."
                                            )), // .on_click(move |event, window, cx| {
                                                //     on_click(event, window, cx);
                                                // })
                                    )
                                    .into_any_element()
                            } else {
                                let server: &OwnedServerName = &this.servers[i];
                                let server = server.clone();

                                let current_server = match this.displayed_room.read(cx) {
                                    DisplayedRoom::Directory(server_name) => {
                                        Some(server_name.clone())
                                    }
                                    _ => None,
                                };

                                div()
                                    .id(ElementId::Name(server.to_string().into()))
                                    .child(
                                        div()
                                            .id("item")
                                            .flex()
                                            .w_full()
                                            .items_center()
                                            .m(px(2.))
                                            .p(px(2.))
                                            .gap(px(4.))
                                            .rounded(theme.border_radius)
                                            .when(
                                                current_server.is_some_and(|current_server| {
                                                    current_server == *server
                                                }),
                                                |david| david.bg(theme.button_background),
                                            )
                                            .child(icon(
                                                if i == 0 { "default" } else { "drive-harddisk" }
                                                    .into(),
                                            ))
                                            .child(server.to_string())
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.change_server(server.clone(), window, cx);
                                            })),
                                    )
                                    .into_any_element()
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
