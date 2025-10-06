use crate::actions::{AccountSwitcher, LogOut};
use crate::auth::logout_popover::logout_popover;
use crate::chat::chat_room::ChatRoom;
use crate::chat::displayed_room::DisplayedRoom;
use crate::chat::sidebar::{ChangeRoomEvent, sidebar};
use cntp_i18n::{i18n_manager, tr};
use contemporary::application::Details;
use contemporary::components::interstitial::interstitial;
use gpui::{
    App, AppContext, BorrowAppContext, Context, Entity, FocusHandle, InteractiveElement,
    IntoElement, ParentElement, Render, Styled, Window, div, px,
};
use log::info;
use thegrid::session::session_manager::SessionManager;

pub struct MainChatSurface {
    displayed_room: DisplayedRoom,
    chat_room: Option<Entity<ChatRoom>>,
    focus_handle: FocusHandle,

    logout_popover_visible: Entity<bool>,
}

impl MainChatSurface {
    pub fn new(cx: &mut App) -> Entity<MainChatSurface> {
        cx.new(|cx| {
            // let session_manager = cx.global::<SessionManager>();
            // let verification_requests = session_manager.verification_requests();
            // cx.observe(&verification_requests, |this, verification_requests, cx| {
            //     cx.notify()
            // })
            // .detach();

            MainChatSurface {
                displayed_room: DisplayedRoom::None,
                chat_room: None,
                focus_handle: cx.focus_handle(),
                logout_popover_visible: cx.new(|_| false),
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
            .on_action(cx.listener(|_, _: &AccountSwitcher, _, cx| {
                cx.update_global::<SessionManager, ()>(|session_manager, cx| {
                    session_manager.clear_session()
                });
                cx.notify()
            }))
            .size_full()
            .flex()
            .gap(px(2.))
            .child(
                sidebar().on_change_room(cx.listener(|this, event: &ChangeRoomEvent, _, cx| {
                    this.displayed_room = event.new_room.clone();
                    if let DisplayedRoom::Room(room_id) = &this.displayed_room {
                        this.chat_room = Some(ChatRoom::new(room_id.clone(), cx))
                    }
                    cx.notify();
                })),
            )
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
