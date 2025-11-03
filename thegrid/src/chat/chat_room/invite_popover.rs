use cntp_i18n::tr;
use contemporary::components::anchorer::WithAnchorer;
use contemporary::components::button::button;
use contemporary::components::constrainer::constrainer;
use contemporary::components::flyout::flyout;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::pager::pager;
use contemporary::components::pager::slide_horizontal_animation::SlideHorizontalAnimation;
use contemporary::components::popover::popover;
use contemporary::components::spinner::spinner;
use contemporary::components::subtitle::subtitle;
use contemporary::components::text_field::TextField;
use contemporary::styling::theme::Theme;
use gpui::{
    AppContext, AsyncApp, Context, Entity, IntoElement, ParentElement, Render, Styled, WeakEntity,
    Window, div, px,
};
use matrix_sdk::ruma::{OwnedRoomId, OwnedUserId, UserId};
use thegrid::session::session_manager::SessionManager;
use thegrid::tokio_helper::TokioHelper;

pub struct InvitePopover {
    room_id: Option<OwnedRoomId>,
    invite_search: Entity<TextField>,
    pending_invites: Vec<OwnedUserId>,

    busy: bool,
}

impl InvitePopover {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            room_id: None,
            pending_invites: Vec::new(),
            invite_search: cx.new(|cx| {
                let mut text_field = TextField::new("invite-search", cx);
                text_field.set_placeholder(
                    tr!("INVITE_SEARCH_PLACEHOLDER", "Search for users...")
                        .to_string()
                        .as_str(),
                );
                text_field
            }),

            busy: false,
        }
    }

    pub fn open_invite_popover(&mut self, room_id: OwnedRoomId, cx: &mut Context<Self>) {
        self.room_id = Some(room_id);
        self.pending_invites = Vec::new();
        cx.notify();
    }

    pub fn perform_invite(&mut self, cx: &mut Context<Self>) {
        // TODO: Remove when we have search
        let Ok(user_id) = UserId::parse(self.invite_search.read(cx).text()) else {
            return;
        };
        self.pending_invites.push(user_id);

        let pending_invites = self.pending_invites.clone();

        let session_manager = cx.global::<SessionManager>();
        let cached_room = session_manager
            .rooms()
            .read(cx)
            .room(self.room_id.as_ref().unwrap())
            .unwrap()
            .read(cx);
        let room = cached_room.inner.clone();
        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                for user_id in pending_invites.iter() {
                    let user_id = user_id.clone();
                    let room = room.clone();

                    // TODO: Handle errors
                    let _ = cx
                        .spawn_tokio(async move { room.invite_user_by_id(&user_id).await })
                        .await;
                }

                let _ = weak_this.update(cx, |this, cx| {
                    this.busy = false;
                    this.room_id = None;
                    cx.notify();
                });
            },
        )
        .detach();

        self.busy = true;
        cx.notify();
    }
}

impl Render for InvitePopover {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();

        popover("invite-popover")
            .visible(self.room_id.is_some())
            .size_neg(100.)
            .anchor_bottom()
            .content(
                pager("invite-pager", if self.busy { 1 } else { 0 })
                    .animation(SlideHorizontalAnimation::new())
                    .size_full()
                    .page(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(9.))
                            .child(
                                grandstand("invite-grandstand")
                                    .text(tr!("INVITE_TITLE", "Invite to room"))
                                    .on_back_click(cx.listener(move |this, _, _, cx| {
                                        this.room_id = None;
                                        cx.notify()
                                    })),
                            )
                            .child(
                                constrainer("invite-constrainer").child(
                                    layer()
                                        .flex()
                                        .flex_col()
                                        .p(px(8.))
                                        .w_full()
                                        .child(subtitle(tr!("INVITE_SUBTITLE", "Invite to room")))
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap(px(8.))
                                                .child(tr!(
                                                    "INVITE_DESCRIPTION",
                                                    "Who do you want to invite to this room?"
                                                ))
                                                .child(
                                                    div()
                                                        .child(self.invite_search.clone())
                                                        .with_anchorer(
                                                            move |david, bounds, _, _| {
                                                                david.child(
                                                                    flyout(bounds)
                                                                        .visible(false)
                                                                        .anchor_bottom_left()
                                                                        .w(bounds.size.width)
                                                                        .max_h(px(100.))
                                                                        .bg(theme.background)
                                                                        .border(px(1.))
                                                                        .border_color(
                                                                            theme.border_color,
                                                                        )
                                                                        .rounded(
                                                                            theme.border_radius,
                                                                        ),
                                                                )
                                                            },
                                                        ),
                                                )
                                                .child(
                                                    button("do-invite-out")
                                                        .child(icon_text(
                                                            "user".into(),
                                                            tr!("INVITE_ACTION", "Invite to room")
                                                                .into(),
                                                        ))
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.perform_invite(cx)
                                                            },
                                                        )),
                                                ),
                                        ),
                                ),
                            )
                            .into_any_element(),
                    )
                    .page(
                        div()
                            .size_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(spinner())
                            .into_any_element(),
                    ),
            )
    }
}
