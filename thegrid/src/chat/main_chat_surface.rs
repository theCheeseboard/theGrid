use crate::account_settings::AccountSettingsPage;
use crate::actions::{AccountSettings, AccountSwitcher, LogOut};
use crate::auth::logout_popover::logout_popover;
use crate::chat::chat_room::ChatRoom;
use crate::chat::displayed_room::DisplayedRoom;
use crate::chat::sidebar::Sidebar;
use crate::main_window::{MainWindowSurface, SurfaceChangeEvent, SurfaceChangeHandler};
use cntp_i18n::{i18n_manager, tr};
use contemporary::application::Details;
use contemporary::components::interstitial::interstitial;
use gpui::{
    App, AppContext, BorrowAppContext, Context, Entity, FocusHandle, InteractiveElement,
    IntoElement, ParentElement, Render, Styled, Window, div, px,
};
use std::rc::Rc;
use thegrid::session::session_manager::SessionManager;

pub struct MainChatSurface {
    sidebar: Entity<Sidebar>,

    displayed_room: Entity<DisplayedRoom>,
    chat_room: Option<Entity<ChatRoom>>,
    focus_handle: FocusHandle,

    logout_popover_visible: Entity<bool>,

    on_surface_change: Rc<Box<SurfaceChangeHandler>>,
}

impl MainChatSurface {
    pub fn new(
        cx: &mut App,
        displayed_room: Entity<DisplayedRoom>,
        on_surface_change: impl Fn(&SurfaceChangeEvent, &mut Window, &mut App) + 'static,
    ) -> Entity<MainChatSurface> {
        cx.new(|cx| {
            let surface_change_handler =
                cx.listener(|this: &mut Self, event: &SurfaceChangeEvent, window, cx| {
                    (this.on_surface_change)(event, window, cx)
                });

            cx.observe(&displayed_room, |this, displayed_room, cx| {
                if let DisplayedRoom::Room(room_id) = displayed_room.read(cx) {
                    this.chat_room = Some(ChatRoom::new(room_id.clone(), displayed_room, cx))
                }
            })
            .detach();

            MainChatSurface {
                sidebar: cx.new(|cx| {
                    let mut sidebar = Sidebar::new(cx, displayed_room.clone());
                    sidebar.on_surface_change(surface_change_handler);
                    sidebar
                }),
                displayed_room,
                chat_room: None,
                focus_handle: cx.focus_handle(),
                logout_popover_visible: cx.new(|_| false),
                on_surface_change: Rc::new(Box::new(on_surface_change)),
            }
        })
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
                    .child(match &self.displayed_room.read(cx) {
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
