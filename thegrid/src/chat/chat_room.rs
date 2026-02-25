mod attachments_view;
mod chat_bar;
pub mod invite_popover;
pub mod open_room;
mod room_members;
mod room_settings;
mod timeline;
mod timeline_view;
mod user_action_dialogs;

use crate::chat::chat_room::attachments_view::AttachmentsView;
use crate::chat::chat_room::open_room::OpenRoom;
use crate::chat::chat_room::room_members::RoomMembers;
use crate::chat::chat_room::room_settings::RoomSettings;
use crate::chat::chat_room::timeline_view::TimelineView;
use crate::chat::chat_room::user_action_dialogs::UserActionDialogs;
use crate::chat::displayed_room::DisplayedRoom;
use cntp_i18n::tr;
use contemporary::components::admonition::admonition;
use contemporary::components::button::button;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon::icon;
use contemporary::components::pager::lift_animation::LiftAnimation;
use contemporary::components::pager::pager;
use gpui::prelude::FluentBuilder;
use gpui::{
    AnimationExt, App, AppContext, Context, Entity, ExternalPaths, InteractiveElement, IntoElement,
    ParentElement, Render, StatefulInteractiveElement, Styled, Window, div, px,
};
use matrix_sdk::ruma::OwnedRoomId;
use matrix_sdk::ruma::events::tag::TagName;
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::tokio_helper::TokioHelper;
use timeline_view::author_flyout::{AuthorFlyoutUserActionEvent, UserAction};

pub struct ChatRoom {
    open_room: Entity<OpenRoom>,
    room_settings: Entity<RoomSettings>,
    room_members: Entity<RoomMembers>,
    user_action_dialogs: Entity<UserActionDialogs>,
    timeline_view: Entity<TimelineView>,
    current_page: ChatRoomPage,
}

enum ChatRoomPage {
    Chat,
    Settings,
    Members,
}

impl ChatRoom {
    pub fn new(
        room_id: OwnedRoomId,
        displayed_room: Entity<DisplayedRoom>,
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

            let trigger_user_action_listener = cx.listener(Self::trigger_user_action);
            let timeline_view = cx.new(|cx| {
                TimelineView::new(
                    open_room.clone(),
                    displayed_room.clone(),
                    trigger_user_action_listener,
                    cx,
                )
            });

            Self {
                open_room,
                user_action_dialogs,
                room_settings,
                room_members,
                current_page: ChatRoomPage::Chat,
                timeline_view,
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
        let chat_bar = open_room.chat_bar.clone();

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
                                // .when(
                                //     room.room_type().is_some_and(|room_type| {
                                //         room_type.to_string() == "org.matrix.msc3417.call"
                                //     }),
                                //     |david| {
                                //         david.child(
                                //             button("call-start")
                                //                 .flat()
                                //                 .child(icon("call-start".into()))
                                //                 .on_click(cx.listener(|this, _, _, cx| {})),
                                //         )
                                //     },
                                // )
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
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_grow()
                                .child(self.timeline_view.clone())
                                .when(!pending_attachments.is_empty(), |david| {
                                    david.child(AttachmentsView {
                                        open_room: self.open_room.clone(),
                                    })
                                }),
                        )
                        .when(
                            open_room.tags.contains_key(&TagName::ServerNotice),
                            |david| {
                                david.child(
                                    div().px(px(2.)).pb(px(2.)).child(
                                        admonition()
                                            .title(tr!("SERVER_NOTICE_ROOM_TITLE", "Official Room"))
                                            .child(tr!(
                                                "SERVER_NOTICE_ROOM_CONTENT",
                                                "Notices from your homeserver will appear \
                                                    in this room."
                                            )),
                                    ),
                                )
                            },
                        )
                        .child(chat_bar)
                        .child(
                            div()
                                .absolute()
                                .left_0()
                                .top_0()
                                .size_full()
                                .on_drop(cx.listener(|this, event: &ExternalPaths, _, cx| {
                                    this.open_room.update(cx, |open_room, cx| {
                                        for path in event.paths() {
                                            open_room.attach_from_disk(path.clone(), cx);
                                        }
                                    });
                                })),
                        )
                        .into_any_element(),
                )
                .page(self.room_settings.clone().into_any_element())
                .page(self.room_members.clone().into_any_element()),
            )
            .child(self.user_action_dialogs.clone())
    }
}
