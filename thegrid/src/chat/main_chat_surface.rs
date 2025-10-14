use crate::account_settings::AccountSettingsPage;
use crate::actions::{AccountSettings, AccountSwitcher, LogOut};
use crate::auth::logout_popover::logout_popover;
use crate::chat::chat_room::ChatRoom;
use crate::chat::displayed_room::DisplayedRoom;
use crate::chat::sidebar::Sidebar;
use crate::main_window::{
    MainWindowSurface, SurfaceChange, SurfaceChangeEvent, SurfaceChangeHandler,
};
use cntp_i18n::{i18n_manager, tr};
use contemporary::application::Details;
use contemporary::components::interstitial::interstitial;
use gpui::{
    App, AppContext, BorrowAppContext, Context, Entity, FocusHandle, InteractiveElement,
    IntoElement, ParentElement, Render, Styled, Window, div, px,
};
use log::info;
use std::rc::Rc;
use thegrid::session::session_manager::SessionManager;

pub type ChangeRoomHandler = dyn Fn(&ChangeRoomEvent, &mut Window, &mut App) + 'static;

#[derive(Clone)]
pub struct ChangeRoomEvent {
    pub new_room: DisplayedRoom,
}

pub struct MainChatSurface {
    sidebar: Entity<Sidebar>,

    displayed_room: DisplayedRoom,
    chat_room: Option<Entity<ChatRoom>>,
    focus_handle: FocusHandle,

    logout_popover_visible: Entity<bool>,

    on_surface_change: Rc<Box<SurfaceChangeHandler>>,
}

impl MainChatSurface {
    pub fn new(
        cx: &mut App,
        on_surface_change: impl Fn(&SurfaceChangeEvent, &mut Window, &mut App) + 'static,
    ) -> Entity<MainChatSurface> {
        cx.new(|cx| {
            // let session_manager = cx.global::<SessionManager>();
            // let verification_requests = session_manager.verification_requests();
            // cx.observe(&verification_requests, |this, verification_requests, cx| {
            //     cx.notify()
            // })
            // .detach();

            let change_room_handler = cx.listener(Self::on_change_room);
            let surface_change_handler =
                cx.listener(|this, event: &SurfaceChangeEvent, window, cx| {
                    (this.on_surface_change)(event, window, cx)
                });

            MainChatSurface {
                sidebar: cx.new(|cx| {
                    let mut sidebar = Sidebar::new(cx);
                    sidebar.on_change_room(change_room_handler);
                    sidebar.on_surface_change(surface_change_handler);
                    sidebar
                }),
                displayed_room: DisplayedRoom::None,
                chat_room: None,
                focus_handle: cx.focus_handle(),
                logout_popover_visible: cx.new(|_| false),
                on_surface_change: Rc::new(Box::new(on_surface_change)),
            }
        })
    }

    pub fn on_change_room(
        &mut self,
        event: &ChangeRoomEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.displayed_room = event.new_room.clone();
        if let DisplayedRoom::Room(room_id) = &self.displayed_room {
            self.chat_room = Some(ChatRoom::new(
                room_id.clone(),
                cx.listener(|this, event: &ChangeRoomEvent, window, cx| {
                    this.on_change_room(event, window, cx);
                }),
                cx,
            ))
        }
        cx.notify();
    }
}

impl Render for MainChatSurface {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let details = cx.global::<Details>();

        let locale = &i18n_manager!().locale;

        div()
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &LogOut, _, cx| {
                this.logout_popover_visible.write(cx, true);
                cx.notify()
            }))
            .on_action(cx.listener(|this, _: &AccountSettings, window, cx| {
                (this.on_surface_change)(
                    &SurfaceChangeEvent {
                        change: MainWindowSurface::AccountSettings(AccountSettingsPage::Profile)
                            .into(),
                    },
                    window,
                    cx,
                );
            }))
            .on_action(cx.listener(|_, _: &AccountSwitcher, _, cx| {
                cx.update_global::<SessionManager, ()>(|session_manager, cx| {
                    session_manager.clear_session()
                });
                cx.notify()
            }))
            .size_full()
            .flex()
            .gap(px(2.))
            .child(self.sidebar.clone())
            .child(
                div()
                    .child(match &self.displayed_room {
                        DisplayedRoom::None => interstitial()
                            .title(
                                tr!(
                                    "NO_DISPLAYED_ROOM_TITLE",
                                    "Welcome to {{application_name}}",
                                    application_name = details
                                        .generatable
                                        .application_name
                                        .resolve_languages_or_default(&locale.messages)
                                )
                                .into(),
                            )
                            .message(
                                tr!(
                                    "NO_DISPLAYED_ROOM_MESSAGE",
                                    "Choose a room to start chatting!"
                                )
                                .into(),
                            )
                            .size_full()
                            .into_any_element(),
                        DisplayedRoom::Room(_) => {
                            self.chat_room.as_ref().unwrap().clone().into_any_element()
                        }
                    })
                    .flex_grow(),
            )
            .child(logout_popover(self.logout_popover_visible.clone()))
    }
}
