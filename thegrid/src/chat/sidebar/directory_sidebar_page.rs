use crate::chat::displayed_room::DisplayedRoom;
use crate::chat::sidebar::Sidebar;
use cntp_i18n::tr;
use contemporary::components::button::button;
use contemporary::components::dialog_box::{StandardButton, dialog_box};
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon::icon;
use contemporary::components::icon_text::icon_text;
use contemporary::components::text_field::TextField;
use contemporary::styling::theme::ThemeStorage;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, AsyncApp, Context, ElementId, Entity, InteractiveElement, IntoElement,
    ListAlignment, ListState, ParentElement, Render, StatefulInteractiveElement, Styled,
    WeakEntity, Window, div, list, px,
};
use matrix_sdk::{OwnedServerName, ServerName};
use thegrid::session::session_manager::SessionManager;

pub struct DirectorySidebarPage {
    list_state: ListState,
    sidebar: Entity<Sidebar>,
    displayed_room: Entity<DisplayedRoom>,
    add_dialog_visible: bool,
    new_homeserver_text_field: Entity<TextField>,

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
            add_dialog_visible: false,

            new_homeserver_text_field: cx.new(|cx| {
                let mut text_field = TextField::new("homeserver", cx);
                text_field.set_placeholder("matrix.org");
                text_field
            }),
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
                                            ))
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.add_dialog_visible = true;
                                                cx.notify();
                                            })),
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
            .child(
                dialog_box("directory-homeserver-dialog")
                    .visible(self.add_dialog_visible)
                    .title(tr!("ROOM_DIRECTORY_ADD_SERVER", "Browse another server").into())
                    .content(
                        div()
                            .flex()
                            .flex_col()
                            .w(px(500.))
                            .gap(px(12.))
                            .child(tr!(
                                "ROOM_DIRECTORY_ADD_SERVER_DESCRIPTION",
                                "If you want to find communities on another server, you can enter \
                                the address of the homeserver below."
                            ))
                            .child(self.new_homeserver_text_field.clone().into_any_element()),
                    )
                    .standard_button(
                        StandardButton::Cancel,
                        cx.listener(|this, _, _, cx| {
                            this.add_dialog_visible = false;
                            cx.notify()
                        }),
                    )
                    .button(
                        button("add-server-button")
                            .child(icon_text(
                                "dialog-ok".into(),
                                tr!("ROOM_DIRECTORY_BROWSE_BUTTON", "Browse Server Directory")
                                    .into(),
                            ))
                            .on_click(cx.listener(|this, _, window, cx| {
                                let homeserver = this.new_homeserver_text_field.read(cx).text();
                                match ServerName::parse(homeserver) {
                                    Ok(homeserver) => {
                                        if !this.servers.contains(&homeserver) {
                                            this.servers.push(homeserver.clone());
                                            this.update_server_list();
                                        }

                                        this.change_server(homeserver, window, cx);
                                        this.add_dialog_visible = false;
                                        cx.notify();
                                    }
                                    Err(e) => this.new_homeserver_text_field.update(
                                        cx,
                                        |text_field, cx| {
                                            text_field.flash_error(window, cx);
                                        },
                                    ),
                                }
                            })),
                    ),
            )
    }
}
