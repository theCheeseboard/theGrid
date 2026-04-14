use crate::actions::{
    AccountSettings, AccountSwitcher, CreateRoom, CreateSpace, DirectJoinRoom, LogOut,
};
use crate::auth::logout_popover::logout_popover;
use crate::chat::chat_room::ChatRoom;
use crate::chat::displayed_room::DisplayedRoom;
use crate::chat::join_room::JoinRoom;
use crate::chat::join_room::create_room_popover::CreateRoomPopover;
use crate::chat::join_room::create_space_popover::CreateSpacePopover;
use crate::chat::join_room::direct_join_room_popover::DirectJoinRoomPopover;
use crate::chat::room_directory::RoomDirectory;
use crate::chat::sidebar::Sidebar;
use cntp_i18n::{i18n_manager, tr};
use contemporary::application::Details;
use contemporary::components::interstitial::interstitial;
use gpui::{
    App, AppContext, BorrowAppContext, Context, Entity, FocusHandle, InteractiveElement,
    IntoElement, ParentElement, Render, Styled, Window, div, px,
};
use std::rc::Rc;
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::surfaces::{
    AccountSettingsDeepLink, MainWindowSurface, SurfaceChangeEvent, SurfaceChangeHandler,
};
use thegrid_rtc_livekit::call_disconnect_confirmation_dialog::CallDisconnectConfirmationDialog;

pub struct MainChatSurface {
    sidebar: Entity<Sidebar>,

    displayed_room: Entity<DisplayedRoom>,
    chat_room: Option<Entity<ChatRoom>>,
    join_room: Entity<JoinRoom>,
    room_directory: Option<Entity<RoomDirectory>>,
    focus_handle: FocusHandle,

    logout_popover_visible: Entity<bool>,
    create_room_popover: Entity<CreateRoomPopover>,
    create_space_popover: Entity<CreateSpacePopover>,
    direct_join_room_popover: Entity<DirectJoinRoomPopover>,

    on_surface_change: Rc<Box<SurfaceChangeHandler>>,
    call_disconnect_confirmation_dialog: Entity<CallDisconnectConfirmationDialog>,
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

            let create_room_popover =
                cx.new(|cx| CreateRoomPopover::new(displayed_room.clone(), cx));
            let create_space_popover =
                cx.new(|cx| CreateSpacePopover::new(displayed_room.clone(), cx));
            let direct_join_room_popover = cx.new(|cx| DirectJoinRoomPopover::new(cx));
            let call_disconnect_confirmation_dialog =
                cx.new(|cx| CallDisconnectConfirmationDialog::new(cx));

            cx.observe(&displayed_room, {
                let create_room_popover = create_room_popover.clone();
                let create_space_popover = create_space_popover.clone();
                move |this, displayed_room, cx| match displayed_room.read(cx).clone() {
                    DisplayedRoom::Room(room_id) => {
                        this.chat_room = Some(ChatRoom::new(
                            room_id.clone(),
                            displayed_room,
                            create_room_popover.clone(),
                            create_space_popover.clone(),
                            this.on_surface_change.clone(),
                            cx,
                        ))
                    }
                    DisplayedRoom::Directory(server_name) => {
                        this.room_directory =
                            Some(cx.new(|cx| {
                                RoomDirectory::new(server_name.clone(), displayed_room, cx)
                            }));
                    }
                    _ => {}
                }
            })
            .detach();

            MainChatSurface {
                sidebar: cx.new(|cx| {
                    let mut sidebar = Sidebar::new(cx, displayed_room.clone());
                    sidebar.on_surface_change(surface_change_handler);
                    sidebar
                }),
                join_room: cx.new(|cx| {
                    JoinRoom::new(
                        cx,
                        displayed_room.clone(),
                        create_room_popover.clone(),
                        create_space_popover.clone(),
                        direct_join_room_popover.clone(),
                    )
                }),
                displayed_room,
                chat_room: None,
                room_directory: None,
                focus_handle: cx.focus_handle(),
                logout_popover_visible: cx.new(|_| false),
                on_surface_change: Rc::new(Box::new(on_surface_change)),
                create_room_popover,
                create_space_popover,
                direct_join_room_popover,
                call_disconnect_confirmation_dialog,
            }
        })
    }

    pub fn log_out(&mut self, _: &LogOut, window: &mut Window, cx: &mut Context<Self>) {
        let on_complete = cx.listener(|this, _, _, cx| {
            this.logout_popover_visible.write(cx, true);
            cx.notify()
        });

        self.call_disconnect_confirmation_dialog.update(
            cx,
            |call_disconnect_confirmation_dialog, cx| {
                call_disconnect_confirmation_dialog.ensure_calls_disconnected(
                    window,
                    cx,
                    on_complete,
                );
            },
        )
    }

    pub fn account_settings(
        &mut self,
        _: &AccountSettings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        (self.on_surface_change)(
            &SurfaceChangeEvent {
                change: MainWindowSurface::AccountSettings(AccountSettingsDeepLink::Profile).into(),
            },
            window,
            cx,
        );
    }

    pub fn account_switcher(
        &mut self,
        _: &AccountSwitcher,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.call_disconnect_confirmation_dialog.update(
            cx,
            |call_disconnect_confirmation_dialog, cx| {
                call_disconnect_confirmation_dialog.ensure_calls_disconnected(
                    window,
                    cx,
                    cx.listener(|this, _, _, cx| {
                        cx.update_global::<SessionManager, ()>(|session_manager, cx| {
                            session_manager.clear_session()
                        });
                        cx.notify()
                    }),
                );
            },
        )
    }

    pub fn create_room(&mut self, _: &CreateRoom, _: &mut Window, cx: &mut Context<Self>) {
        self.create_room_popover
            .update(cx, |create_room_popover, cx| {
                create_room_popover.open(None, cx)
            })
    }

    pub fn create_space(&mut self, _: &CreateSpace, _: &mut Window, cx: &mut Context<Self>) {
        self.create_space_popover
            .update(cx, |create_space_popover, cx| {
                create_space_popover.open(None, cx)
            })
    }

    pub fn direct_join_room(&mut self, _: &DirectJoinRoom, _: &mut Window, cx: &mut Context<Self>) {
        self.direct_join_room_popover
            .update(cx, |direct_join_room_popover, cx| {
                direct_join_room_popover.open(cx)
            })
    }
}

impl Render for MainChatSurface {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let details = cx.global::<Details>();

        let locale = &i18n_manager!().locale;

        div()
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::log_out))
            .on_action(cx.listener(Self::account_settings))
            .on_action(cx.listener(Self::account_switcher))
            .on_action(cx.listener(Self::create_room))
            .on_action(cx.listener(Self::create_space))
            .on_action(cx.listener(Self::direct_join_room))
            .size_full()
            .child(
                div()
                    .size_full()
                    .flex()
                    .gap(px(2.))
                    .child(self.sidebar.clone())
                    .child(
                        div()
                            .child(match &self.displayed_room.read(cx) {
                                DisplayedRoom::None => interstitial()
                                    .title(tr!(
                                        "NO_DISPLAYED_ROOM_TITLE",
                                        "Welcome to {{application_name}}",
                                        application_name = details
                                            .generatable
                                            .application_name
                                            .resolve_languages_or_default(&locale.messages)
                                    ))
                                    .message(tr!(
                                        "NO_DISPLAYED_ROOM_MESSAGE",
                                        "Choose a room to start chatting!"
                                    ))
                                    .size_full()
                                    .into_any_element(),
                                DisplayedRoom::Room(_) => {
                                    self.chat_room.as_ref().unwrap().clone().into_any_element()
                                }
                                DisplayedRoom::CreateRoom => {
                                    self.join_room.clone().into_any_element()
                                }
                                DisplayedRoom::Directory(_) => self
                                    .room_directory
                                    .as_ref()
                                    .unwrap()
                                    .clone()
                                    .into_any_element(),
                            })
                            .flex_grow(),
                    ),
            )
            .child(logout_popover(self.logout_popover_visible.clone()))
            .child(self.create_room_popover.clone())
            .child(self.create_space_popover.clone())
            .child(self.direct_join_room_popover.clone())
            .child(self.call_disconnect_confirmation_dialog.clone())
    }
}
