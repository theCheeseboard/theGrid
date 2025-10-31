use crate::chat::chat_room::open_room::OpenRoom;
use crate::chat::chat_room::timeline_view::author_flyout::{
    AuthorFlyoutUserActionEvent, AuthorFlyoutUserActionListener, author_flyout,
};
use crate::mxc_image::{SizePolicy, mxc_image};
use cntp_i18n::tr;
use contemporary::components::anchorer::WithAnchorer;
use contemporary::components::button::button;
use contemporary::components::constrainer::constrainer;
use contemporary::components::grandstand::grandstand;
use contemporary::components::layer::layer;
use contemporary::components::subtitle::subtitle;
use contemporary::styling::theme::{Theme, VariableColor};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, AppContext, AsyncApp, ClickEvent, Context, Element, Entity,
    InteractiveElement, IntoElement, ListAlignment, ListState, ParentElement, Render,
    StatefulInteractiveElement, Styled, WeakEntity, Window, div, list, px, rgb,
};
use matrix_sdk::RoomMemberships;
use matrix_sdk::room::{RoomMember, RoomMemberRole};
use matrix_sdk::ruma::events::room::member::MembershipState;
use std::cmp::Reverse;
use std::rc::Rc;
use thegrid::tokio_helper::TokioHelper;

pub struct RoomMembers {
    open_room: Entity<OpenRoom>,
    on_back_click: Rc<Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
    on_user_action: Box<AuthorFlyoutUserActionListener>,

    members: Vec<RoomMember>,
    displayed_members: Vec<RoomMember>,
    filter: RoomMemberFilter,
    list_state: ListState,
}

#[derive(Clone, Copy, PartialEq)]
enum RoomMemberFilter {
    Joined,
    Invited,
    Banned,
}

impl RoomMembers {
    pub fn new(
        open_room: Entity<OpenRoom>,
        on_back_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
        on_user_action: impl Fn(&AuthorFlyoutUserActionEvent, &mut Window, &mut App) + 'static,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&open_room, |this, _, cx| {
            this.update_members(cx);
        })
        .detach();

        let list_state = ListState::new(0, ListAlignment::Top, px(200.));

        Self {
            open_room,
            on_back_click: Rc::new(Box::new(on_back_click)),
            members: Vec::new(),
            displayed_members: Vec::new(),
            on_user_action: Box::new(on_user_action),
            filter: RoomMemberFilter::Joined,
            list_state,
        }
    }

    pub fn update_members(&mut self, cx: &mut Context<Self>) {
        if let Some(room) = self.open_room.read(cx).room.as_ref() {
            let room = room.clone();
            cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let members = cx
                    .spawn_tokio(async move { room.members(RoomMemberships::all()).await })
                    .await;

                if let Ok(mut members) = members {
                    members.sort_unstable_by_key(|member| {
                        (
                            Reverse(member.power_level()),
                            member
                                .display_name()
                                .map(|name| name.to_string())
                                .unwrap_or_else(|| member.user_id().to_string()),
                        )
                    });
                    let _ = this.update(cx, |this, cx| {
                        this.members = members;
                        this.update_displayed_members(cx);
                        cx.notify()
                    });
                }
            })
            .detach();
        }
    }

    fn update_displayed_members(&mut self, cx: &mut Context<Self>) {
        self.displayed_members = match self.filter {
            RoomMemberFilter::Joined => self
                .members
                .iter()
                .filter(|member| *member.membership() == MembershipState::Join)
                .cloned()
                .collect(),
            RoomMemberFilter::Invited => self
                .members
                .iter()
                .filter(|member| *member.membership() == MembershipState::Invite)
                .cloned()
                .collect(),
            RoomMemberFilter::Banned => self
                .members
                .iter()
                .filter(|member| *member.membership() == MembershipState::Ban)
                .cloned()
                .collect(),
        };
        self.list_state.reset(self.displayed_members.len());
        cx.notify();
    }

    fn render_list_item(
        &mut self,
        i: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let member: &RoomMember = &self.displayed_members[i];
        let suggested_role = member.suggested_role_for_power_level();

        let author_flyout_open_entity = window.use_keyed_state(i, cx, |_, _| false);
        let author_flyout_open_entity_2 = author_flyout_open_entity.clone();
        let member_entity = cx.new(|_| Some(member.clone()));

        let theme = cx.global::<Theme>();
        let author_flyout_open = *author_flyout_open_entity.read(cx);
        let open_room = self.open_room.clone();
        let on_user_action =
            cx.listener(move |this, event, window, cx| (this.on_user_action)(event, window, cx));

        div()
            .id(i)
            .flex()
            .w_full()
            .my(px(2.))
            .p(px(2.))
            .gap(px(2.))
            .rounded(theme.border_radius)
            .cursor_pointer()
            .items_center()
            .child(
                mxc_image(member.avatar_url().map(|url| url.to_owned()))
                    .rounded(theme.border_radius)
                    .size(px(40.))
                    .size_policy(SizePolicy::Fit),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(2.))
                            .child(
                                member
                                    .display_name()
                                    .map(|name| name.to_string())
                                    .unwrap_or_else(|| member.user_id().to_string()),
                            )
                            .when(
                                suggested_role == RoomMemberRole::Administrator
                                    && *member.membership() == MembershipState::Join,
                                |david| {
                                    david.child(
                                        div()
                                            .rounded(theme.border_radius)
                                            .bg(theme.error_accent_color)
                                            .p(px(2.))
                                            .child(tr!("POWER_LEVEL_ADMINISTRATOR")),
                                    )
                                },
                            )
                            .when(
                                suggested_role == RoomMemberRole::Moderator
                                    && *member.membership() == MembershipState::Join,
                                |david| {
                                    david.child(
                                        div()
                                            .rounded(theme.border_radius)
                                            .bg(theme.info_accent_color)
                                            .p(px(2.))
                                            .child(tr!("POWER_LEVEL_MODERATOR")),
                                    )
                                },
                            ),
                    )
                    .child(
                        div()
                            .text_color(theme.foreground.disabled())
                            .child(member.user_id().to_string()),
                    ),
            )
            .with_anchorer(move |david, bounds| {
                david.child(author_flyout(
                    bounds,
                    author_flyout_open,
                    member_entity,
                    open_room,
                    move |_, _, cx| {
                        author_flyout_open_entity.write(cx, false);
                    },
                    on_user_action,
                ))
            })
            .hover(|david| david.bg(theme.background.hover()))
            .on_click(move |_, _, cx| {
                author_flyout_open_entity_2.write(cx, true);
            })
            .into_any_element()
    }
}

impl Render for RoomMembers {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let on_back_click = self.on_back_click.clone();
        let theme = cx.global::<Theme>();

        let Some(room) = self.open_room.read(cx).room.as_ref() else {
            return div();
        };
        let room = room.clone();
        let room_2 = room.clone();
        let room_3 = room.clone();

        let room_name = room
            .cached_display_name()
            .map(|name| name.to_string())
            .or_else(|| room.name())
            .unwrap_or_default();

        div()
            .flex()
            .flex_col()
            .bg(theme.background)
            .size_full()
            .child(
                grandstand("room-members-grandstand")
                    .text(tr!("ROOM_MEMBERS", "Room Members"))
                    .pt(px(36.))
                    .on_back_click(move |event, window, cx| {
                        on_back_click.clone()(event, window, cx);
                    }),
            )
            .child(
                div()
                    .flex()
                    .justify_center()
                    .size_full()
                    .gap(px(8.))
                    .child(
                        div().p(px(4.)).child(
                            layer()
                                .p(px(8.))
                                .gap(px(8.))
                                .w(px(100.))
                                .child(subtitle(tr!("MEMBER_LIST_FILTERS", "Filters")))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .bg(theme.button_background)
                                        .rounded(theme.border_radius)
                                        .child(
                                            button("filter-joined")
                                                .child(tr!("MEMBER_LIST_FILTER_JOINED", "Joined"))
                                                .checked_when(
                                                    self.filter == RoomMemberFilter::Joined,
                                                )
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.filter = RoomMemberFilter::Joined;
                                                    this.update_displayed_members(cx);
                                                })),
                                        )
                                        .child(
                                            button("filter-invited")
                                                .child(tr!("MEMBER_LIST_FILTER_INVITED", "Invited"))
                                                .checked_when(
                                                    self.filter == RoomMemberFilter::Invited,
                                                )
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.filter = RoomMemberFilter::Invited;
                                                    this.update_displayed_members(cx);
                                                })),
                                        )
                                        .child(
                                            button("filter-banned")
                                                .child(tr!("MEMBER_LIST_FILTER_BANNED", "Banned"))
                                                .checked_when(
                                                    self.filter == RoomMemberFilter::Banned,
                                                )
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.filter = RoomMemberFilter::Banned;
                                                    this.update_displayed_members(cx);
                                                })),
                                        ),
                                ),
                        ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .max_w(px(600.))
                            .size_full()
                            .px(px(8.))
                            .gap(px(8.))
                            .child(
                                list(
                                    self.list_state.clone(),
                                    cx.processor(Self::render_list_item),
                                )
                                .size_full(),
                            ),
                    ),
            )
    }
}
