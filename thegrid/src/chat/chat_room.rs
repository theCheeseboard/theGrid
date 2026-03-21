mod attachments_view;
mod call_members_view;
mod chat_bar;
pub mod invite_popover;
pub mod open_room;
mod room_members;
mod room_settings;
mod room_timeline_content;
mod space_lobby_content;
mod timeline;
mod timeline_view;
mod user_action_dialogs;

use crate::chat::chat_room::open_room::OpenRoom;
use crate::chat::chat_room::room_members::RoomMembers;
use crate::chat::chat_room::room_settings::RoomSettings;
use crate::chat::chat_room::room_timeline_content::RoomTimelineContent;
use crate::chat::chat_room::space_lobby_content::SpaceLobbyContent;
use crate::chat::chat_room::user_action_dialogs::UserActionDialogs;
use crate::chat::displayed_room::DisplayedRoom;
use cntp_i18n::tr;
use contemporary::components::button::button;
use contemporary::components::dialog_box::{dialog_box, StandardButton};
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon::icon;
use contemporary::components::pager::lift_animation::LiftAnimation;
use contemporary::components::pager::pager;
use contemporary::components::spinner::spinner;
use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, AnimationExt, App, AppContext, BorrowAppContext, Context,
    Entity, InteractiveElement, IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled,
    VisualContext, Window,
};
use matrix_sdk::ruma::OwnedRoomId;
use smol::stream::StreamExt;
use std::rc::Rc;
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::surfaces::{
    MainWindowSurface, SurfaceChange, SurfaceChangeEvent, SurfaceChangeHandler,
};
use thegrid_common::tokio_helper::TokioHelper;
use timeline_view::author_flyout::{AuthorFlyoutUserActionEvent, UserAction};

pub struct ChatRoom {
    open_room: Entity<OpenRoom>,
    room_settings: Entity<RoomSettings>,
    room_members: Entity<RoomMembers>,
    user_action_dialogs: Entity<UserActionDialogs>,
    current_page: ChatRoomPage,
    view: ChatRoomView,

    on_surface_change: Rc<Box<SurfaceChangeHandler>>,

    microphone_access_dialog: bool,
}

enum ChatRoomPage {
    Chat,
    Settings,
    Members,
}

#[derive(Clone)]
enum ChatRoomView {
    Loading,
    Timeline(Entity<RoomTimelineContent>),
    SpaceLobby(Entity<SpaceLobbyContent>),
}

impl ChatRoom {
    pub fn new(
        room_id: OwnedRoomId,
        displayed_room: Entity<DisplayedRoom>,
        on_surface_change: Rc<Box<SurfaceChangeHandler>>,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx| {
            let open_room = cx.new(|cx| OpenRoom::new(room_id.clone(), displayed_room.clone(), cx));
            let user_action_dialogs = cx.new(|cx| UserActionDialogs::new(room_id.clone(), cx));

            let settings_back_click = cx.listener(|this: &mut ChatRoom, _, _, cx| {
                this.current_page = ChatRoomPage::Chat;
                cx.notify();
            });
            let members_click = cx.listener(|this: &mut ChatRoom, _, _, cx| {
                this.current_page = ChatRoomPage::Members;
                cx.notify();
            });
            let room_settings = cx.new(|cx| {
                RoomSettings::new(open_room.clone(), settings_back_click, members_click, cx)
            });

            let members_back_click = cx.listener(|this: &mut ChatRoom, _, _, cx| {
                this.current_page = ChatRoomPage::Settings;
                cx.notify();
            });
            let trigger_user_action_listener = cx.listener(Self::trigger_user_action);

            let room_members = cx.new(|cx| {
                RoomMembers::new(
                    open_room.clone(),
                    displayed_room.clone(),
                    members_back_click,
                    trigger_user_action_listener,
                    cx,
                )
            });

            cx.observe(&open_room, {
                let displayed_room = displayed_room.clone();
                let on_surface_change = on_surface_change.clone();
                move |this, open_room, cx| {
                    if !matches!(this.view, ChatRoomView::Loading) {
                        return;
                    }

                    let Some(room) = &open_room.read(cx).room else {
                        return;
                    };

                    if room.is_space() {
                        this.view = ChatRoomView::SpaceLobby(cx.new({
                            let open_room = open_room.clone();
                            move |cx| SpaceLobbyContent::new(open_room, cx)
                        }))
                    } else {
                        let trigger_user_action_listener = cx.listener(Self::trigger_user_action);
                        this.view = ChatRoomView::Timeline(cx.new({
                            let displayed_room = displayed_room.clone();
                            let on_surface_change = on_surface_change.clone();
                            let open_room = open_room.clone();
                            move |cx| {
                                RoomTimelineContent::new(
                                    displayed_room,
                                    open_room,
                                    on_surface_change,
                                    trigger_user_action_listener,
                                    cx,
                                )
                            }
                        }))
                    }
                    cx.notify();
                }
            })
            .detach();

            Self {
                open_room,
                user_action_dialogs,
                room_settings,
                room_members,
                current_page: ChatRoomPage::Chat,
                on_surface_change,
                view: ChatRoomView::Loading,

                microphone_access_dialog: false,
            }
        })
    }

    fn trigger_user_action(
        &mut self,
        user_action: &AuthorFlyoutUserActionEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.user_action_dialogs
            .update(cx, |user_action_dialogs, cx| {
                match user_action.action {
                    UserAction::ChangePowerLevel => {
                        user_action_dialogs.open_power_level_dialog(user_action.user.clone());
                    }
                    UserAction::Kick => {
                        user_action_dialogs.open_kick_dialog(user_action.user.clone());
                    }
                    UserAction::Ban => {
                        user_action_dialogs.open_ban_dialog(user_action.user.clone());
                    }
                    UserAction::Unban => {
                        user_action_dialogs.open_unban_dialog(user_action.user.clone());
                    }
                }

                cx.notify()
            })
    }

    fn start_call(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let room_id = self.open_room.read(cx).room_id.clone();
        (self.on_surface_change)(
            &SurfaceChangeEvent {
                change: SurfaceChange::Push(MainWindowSurface::Call(room_id)),
            },
            window,
            cx,
        );
    }
}

impl Render for ChatRoom {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let session_manager = cx.global::<SessionManager>();
        let Some(client) = session_manager.client() else {
            return div();
        };

        let client = client.read(cx);
        let open_room = self.open_room.read(cx);

        let Some(room) = &open_room.room else {
            return div().flex().flex_col().size_full().child(
                grandstand("main-area-grandstand")
                    .text(tr!("UNKNOWN_ROOM", "Unknown Room"))
                    .pt(px(36.)),
            );
        };

        let room_clone = self.open_room.clone();
        let pending_attachments = &open_room.pending_attachments;

        let room_id = room.room_id().to_owned();
        let room_id_2 = room.room_id().to_owned();

        let call_members = open_room.active_call_users.read(cx).clone();

        div()
            .size_full()
            .child(
                pager(
                    "chat-room-pager",
                    match self.current_page {
                        ChatRoomPage::Chat => 0,
                        ChatRoomPage::Settings => 1,
                        ChatRoomPage::Members => 2,
                    },
                )
                .animation(LiftAnimation::new())
                .size_full()
                .page(
                    div()
                        .flex()
                        .flex_col()
                        .size_full()
                        .child(
                            grandstand("main-area-grandstand")
                                .text(
                                    room.cached_display_name()
                                        .map(|name| name.to_string())
                                        .or_else(|| room.name())
                                        .unwrap_or_default(),
                                )
                                .pt(px(36.))
                                .child(
                                    button("call-start")
                                        .flat()
                                        .child(icon("call-start".into()))
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.start_call(window, cx);
                                        })),
                                )
                                .child(
                                    button("room-settings-button")
                                        .flat()
                                        .child(icon("configure".into()))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.current_page = ChatRoomPage::Settings;
                                            cx.notify()
                                        })),
                                ),
                        )
                        .child(match self.view.clone() {
                            ChatRoomView::Loading => div()
                                .flex_grow()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(spinner())
                                .into_any_element(),
                            ChatRoomView::Timeline(timeline_view) => {
                                timeline_view.into_any_element()
                            }
                            ChatRoomView::SpaceLobby(space_lobby) => space_lobby.into_any_element(),
                        })
                        .into_any_element(),
                )
                .page(self.room_settings.clone().into_any_element())
                .page(self.room_members.clone().into_any_element()),
            )
            .child(self.user_action_dialogs.clone())
            .child(
                dialog_box("microphone-access")
                    .visible(self.microphone_access_dialog)
                    .title(
                        tr!(
                            "PERMISSION_MICROPHONE_DENIED_TITLE",
                            "Unable to access the microphone"
                        )
                        .into(),
                    )
                    .content(tr!(
                        "PERMISSION_MICROPHONE_DENIED_CONTENT",
                        "theGrid needs access to your microphone. Check your privacy settings \
                        and allow theGrid to access the microphone to start a voice call."
                    ))
                    .standard_button(
                        StandardButton::Sorry,
                        cx.listener(|this, _, _, cx| {
                            this.microphone_access_dialog = false;
                            cx.notify();
                        }),
                    ),
            )
    }
}
