mod attachments_view;
mod chat_bar;
pub mod open_room;
mod room_settings;
mod user_action_dialogs;

use crate::chat::chat_room::attachments_view::AttachmentsView;
use crate::chat::chat_room::open_room::OpenRoom;
use crate::chat::chat_room::room_settings::RoomSettings;
use crate::chat::chat_room::user_action_dialogs::UserActionDialogs;
use crate::chat::displayed_room::DisplayedRoom;
use crate::chat::timeline_event::author_flyout::{AuthorFlyoutUserActionEvent, UserAction};
use crate::chat::timeline_event::queued_event::QueuedEvent;
use crate::chat::timeline_event::room_head::room_head;
use crate::chat::timeline_event::timeline_event;
use cntp_i18n::tr;
use contemporary::components::button::button;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon::icon;
use contemporary::components::pager::lift_animation::LiftAnimation;
use contemporary::components::pager::pager;
use contemporary::components::spinner::spinner;
use gpui::prelude::FluentBuilder;
use gpui::{
    AnimationExt, App, AppContext, Context, Entity, ExternalPaths, InteractiveElement, IntoElement,
    ListAlignment, ListOffset, ListScrollEvent, ListState, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, div, list, px,
};
use matrix_sdk::deserialized_responses::TimelineEvent;
use matrix_sdk::event_cache::RoomPaginationStatus;
use matrix_sdk::ruma::OwnedRoomId;
use thegrid::session::session_manager::SessionManager;
use thegrid::tokio_helper::TokioHelper;

pub struct ChatRoom {
    open_room: Entity<OpenRoom>,
    room_settings: Entity<RoomSettings>,
    user_action_dialogs: Entity<UserActionDialogs>,
    pub list_state: ListState,
    show_settings: bool,
}

impl ChatRoom {
    pub fn new(
        room_id: OwnedRoomId,
        displayed_room: Entity<DisplayedRoom>,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx| {
            let open_room = cx.new(|cx| OpenRoom::new(room_id.clone(), displayed_room, cx));
            let user_action_dialogs = cx.new(|cx| UserActionDialogs::new(room_id.clone(), cx));

            let list_state = ListState::new(0, ListAlignment::Bottom, px(200.));
            list_state.set_scroll_handler(cx.listener(
                |this: &mut Self, event: &ListScrollEvent, _, cx| {
                    this.open_room.update(cx, |open_room, cx| {
                        if event.visible_range.end == open_room.events.len() {
                            open_room.send_read_receipt(cx);
                        }
                    })
                },
            ));

            let back_click = cx.listener(|this, _, _, cx| {
                this.show_settings = false;
                cx.notify();
            });
            let room_settings = cx.new(|cx| RoomSettings::new(open_room.clone(), back_click, cx));

            Self {
                open_room,
                user_action_dialogs,
                list_state,
                room_settings,
                show_settings: false,
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
        let events = open_room.events.clone();
        let queued = &open_room.queued;
        if events.len() + queued.len() + 1 != self.list_state.item_count() {
            self.list_state.reset(events.len() + queued.len() + 1);
            self.list_state.scroll_to(ListOffset {
                item_ix: events.len() + 1,
                offset_in_item: px(0.),
            })
        }

        let pagination_status = open_room.pagination_status;
        let pending_attachments = &open_room.pending_attachments;

        let room_id = room.room_id().to_owned();
        let chat_bar = open_room.chat_bar.clone();

        div()
            .size_full()
            .child(
                pager("chat-room-pager", if self.show_settings { 1 } else { 0 })
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
                                        button("room-settings-button")
                                            .flat()
                                            .child(icon("configure".into()))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.show_settings = true;
                                                cx.notify()
                                            })),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_grow()
                                    .child(
                                        list(
                                            self.list_state.clone(),
                                            cx.processor(move |this, i, _, cx| {
                                                let trigger_user_action_listener =
                                                    cx.listener(Self::trigger_user_action);
                                                this.open_room.update(cx, |open_room, cx| {
                                                    if i == 0 {
                                                        match pagination_status {
                                                            RoomPaginationStatus::Idle {
                                                                hit_timeline_start,
                                                            } => {
                                                                if hit_timeline_start {
                                                                    room_head(room_id.clone())
                                                                        .into_any_element()
                                                                } else {
                                                                    div()
                                                                        .child("Not Paginating")
                                                                        .into_any_element()
                                                                }
                                                            }
                                                            RoomPaginationStatus::Paginating => {
                                                                div()
                                                                    .w_full()
                                                                    .flex()
                                                                    .py(px(12.))
                                                                    .items_center()
                                                                    .justify_center()
                                                                    .child(spinner())
                                                                    .into_any_element()
                                                            }
                                                        }
                                                    } else if i < events.len() + 1 {
                                                        let event: &TimelineEvent = &events[i - 1];
                                                        let event = event.clone();
                                                        let previous_event = if i == 1 {
                                                            None
                                                        } else {
                                                            events.get(i - 2).cloned()
                                                        };

                                                        let event_cache =
                                                            open_room.event_cache.clone().unwrap();

                                                        timeline_event(
                                                            event,
                                                            previous_event,
                                                            event_cache,
                                                            room_clone.clone(),
                                                            trigger_user_action_listener,
                                                        )
                                                        .into_any_element()
                                                    } else {
                                                        let event: &Entity<QueuedEvent> =
                                                            &open_room.queued[i - events.len() - 1];
                                                        let previous_event = if i == 1 {
                                                            None
                                                        } else {
                                                            events.get(i - 2).cloned()
                                                        };

                                                        event.update(cx, |event, cx| {
                                                            event.previous_event = previous_event;
                                                        });

                                                        event.clone().into_any_element()
                                                    }
                                                })
                                            }),
                                        )
                                        .flex()
                                        .flex_col()
                                        .h_full(),
                                    )
                                    .when(!pending_attachments.is_empty(), |david| {
                                        david.child(AttachmentsView {
                                            open_room: self.open_room.clone(),
                                        })
                                    }),
                            )
                            .child(chat_bar)
                            .child(div().absolute().left_0().top_0().size_full().on_drop(
                                cx.listener(|this, event: &ExternalPaths, _, cx| {
                                    this.open_room.update(cx, |open_room, cx| {
                                        for path in event.paths() {
                                            open_room.attach_from_disk(path.clone(), cx);
                                        }
                                    });
                                }),
                            ))
                            .into_any_element(),
                    )
                    .page(self.room_settings.clone().into_any_element()),
            )
            .child(self.user_action_dialogs.clone())
    }
}
