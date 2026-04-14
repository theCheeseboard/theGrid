use crate::chat::chat_room::attachments_view::AttachmentsView;
use crate::chat::chat_room::call_members_view::CallMembersView;
use crate::chat::chat_room::open_room::OpenRoom;
use crate::chat::chat_room::timeline_view::TimelineView;
use crate::chat::chat_room::timeline_view::author_flyout::AuthorFlyoutUserActionEvent;
use crate::chat::displayed_room::DisplayedRoom;
use cntp_i18n::tr;
use contemporary::components::admonition::admonition;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, Context, Entity, ExternalPaths, InteractiveElement, IntoElement,
    ParentElement, Render, Styled, Window, div, px,
};
use matrix_sdk::ruma::events::tag::TagName;
use std::rc::Rc;
use thegrid_common::surfaces::{
    MainWindowSurface, SurfaceChange, SurfaceChangeEvent, SurfaceChangeHandler,
};

pub struct RoomTimelineContent {
    open_room: Entity<OpenRoom>,
    timeline_view: Entity<TimelineView>,

    on_surface_change: Rc<Box<SurfaceChangeHandler>>,
}

impl RoomTimelineContent {
    pub fn new(
        displayed_room: Entity<DisplayedRoom>,
        open_room: Entity<OpenRoom>,
        on_surface_change: Rc<Box<SurfaceChangeHandler>>,
        on_user_action: impl Fn(&AuthorFlyoutUserActionEvent, &mut Window, &mut App) + 'static,
        cx: &mut Context<Self>,
    ) -> Self {
        let timeline_view = cx.new(|cx| {
            TimelineView::new(
                open_room.clone(),
                displayed_room.clone(),
                on_user_action,
                cx,
            )
        });

        Self {
            open_room,
            timeline_view,
            on_surface_change,
        }
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

impl Render for RoomTimelineContent {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let open_room = self.open_room.read(cx);
        let call_members = open_room.active_call_users.read(cx).clone();
        let pending_attachments = &open_room.pending_attachments;
        let chat_bar = open_room.chat_bar.clone();

        div()
            .flex()
            .flex_col()
            .flex_grow()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_grow()
                    .child(self.timeline_view.clone())
                    .when(!call_members.is_empty(), |david| {
                        david.child(CallMembersView {
                            members: call_members,
                            start_call: Box::new(cx.listener(|this, _, window, cx| {
                                this.start_call(window, cx);
                            })),
                        })
                    })
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
                                    "Notices from your homeserver will appear in this room."
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
    }
}
