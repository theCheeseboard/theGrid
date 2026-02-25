pub mod author_flyout;
mod membership_change_item;
mod profile_change_item;
pub mod room_head;
mod rtc_notification_item;
mod state_change_element;
mod state_event_item;
mod timeline_item;
mod timeline_message_item;

use crate::chat::chat_room::open_room::OpenRoom;
use crate::chat::chat_room::timeline::Timeline;
use crate::chat::chat_room::timeline_view::author_flyout::{
    AuthorFlyoutUserActionEvent, AuthorFlyoutUserActionListener,
};
use crate::chat::chat_room::timeline_view::timeline_item::timeline_item;
use crate::chat::displayed_room::DisplayedRoom;
use gpui::{
    App, AsyncApp, Context, Element, ElementId, Entity, InteractiveElement, IntoElement,
    ListAlignment, ListOffset, ListScrollEvent, ListSizingBehavior, ListState, ParentElement,
    Render, Styled, Window, div, list, px, rgb,
};
use image::open;
use log::info;
use matrix_sdk::ruma::api::client::receipt::create_receipt::v3::ReceiptType;
use std::rc::Rc;
use thegrid_common::tokio_helper::TokioHelper;

pub struct TimelineView {
    open_room: Entity<OpenRoom>,
    displayed_room: Entity<DisplayedRoom>,
    list_state: ListState,
    pagination_pending: bool,
    on_user_action: Box<AuthorFlyoutUserActionListener>,
}

impl TimelineView {
    pub fn new(
        open_room: Entity<OpenRoom>,
        displayed_room: Entity<DisplayedRoom>,
        on_user_action: impl Fn(&AuthorFlyoutUserActionEvent, &mut Window, &mut App) + 'static,
        cx: &mut Context<TimelineView>,
    ) -> TimelineView {
        cx.observe(&open_room, |this, _, cx| {
            this.connect_open_room(cx);
            cx.notify();
        })
        .detach();

        let list_state = ListState::new(0, ListAlignment::Bottom, px(200.));
        list_state.set_scroll_handler(cx.listener(
            |this: &mut Self, event: &ListScrollEvent, _, cx| {
                let this_entity = cx.entity();
                this.open_room.update(cx, |open_room, cx| {
                    let Some(timeline_entity) = open_room.timeline.as_ref() else {
                        return;
                    };
                    let timeline = timeline_entity.read(cx);
                    let timeline_inner = timeline_entity.clone().read(cx).inner.clone();
                    if event.visible_range.end == timeline.timeline_items().len() {
                        cx.spawn(async move |_, cx: &mut AsyncApp| {
                            let _ = cx
                                .spawn_tokio(async move {
                                    timeline_inner.mark_as_read(ReceiptType::Read).await
                                })
                                .await;
                        })
                        .detach();
                    } else if event.visible_range.start < 5 {
                        // Paginate
                        this.pagination_pending = true;
                        cx.spawn(async move |_, cx: &mut AsyncApp| {
                            let _ = cx
                                .spawn_tokio(
                                    async move { timeline_inner.paginate_backwards(50).await },
                                )
                                .await;
                            this_entity.update(cx, |this, cx| {
                                this.pagination_pending = false;
                                cx.notify();
                            })
                        })
                        .detach();
                    }
                });
                cx.notify();
            },
        ));

        let mut this = Self {
            open_room,
            displayed_room,
            list_state,
            pagination_pending: false,
            on_user_action: Box::new(on_user_action),
        };
        this.connect_open_room(cx);
        this
    }

    pub fn connect_open_room(&mut self, cx: &mut Context<Self>) {
        let open_room = self.open_room.read(cx);
        if let Some(timeline) = open_room.timeline.clone() {
            cx.observe(&timeline, |this, _, cx| {
                this.update_timeline_items(cx);
                cx.notify()
            })
            .detach();
            self.update_timeline_items(cx);
        }
    }

    pub fn update_timeline_items(&mut self, cx: &mut Context<Self>) {
        let open_room = self.open_room.read(cx);
        let Some(timeline_entity) = open_room.timeline.as_ref() else {
            return;
        };
        let timeline_entity = timeline_entity.clone();
        let timeline = timeline_entity.read(cx);

        info!("Updating timeline items");

        if timeline.timeline_items().len() != self.list_state.item_count() {
            let mut last_offset = self.list_state.logical_scroll_top();
            if let Some(new_items) = timeline
                .timeline_items()
                .len()
                .checked_sub(self.list_state.item_count())
            {
                last_offset.item_ix += new_items;
                self.list_state.reset(timeline.timeline_items().len());
                self.list_state.scroll_to(last_offset);
            }
        }
    }
}

impl Render for TimelineView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let open_room = self.open_room.read(cx);
        let Some(timeline_entity) = open_room.timeline.as_ref() else {
            return div().flex_grow().into_any_element();
        };
        let timeline_entity = timeline_entity.clone();
        let room_id = open_room.room_id.clone();

        let open_room = self.open_room.clone();

        div()
            .flex_grow()
            .child(
                list(
                    self.list_state.clone(),
                    cx.processor(move |this, i, window, cx| {
                        let timeline = timeline_entity.read(cx);
                        let timeline_items = timeline.timeline_items();
                        let item = timeline_items[i].clone();
                        let previous_item = if i == 0 {
                            None
                        } else {
                            timeline_items.get(i - 1).cloned()
                        };

                        div()
                            .id(ElementId::Name(item.unique_id().0.clone().into()))
                            .py(px(2.))
                            .child(timeline_item(
                                item,
                                previous_item,
                                open_room.clone(),
                                this.displayed_room.clone(),
                                cx.listener(|this, event, window, cx| {
                                    (this.on_user_action)(event, window, cx)
                                }),
                            ))
                            .into_any_element()
                    }),
                )
                .size_full(),
            )
            .into_any_element()
    }
}
