use crate::auth::verification_popover::VerificationPopover;
use crate::chat::displayed_room::DisplayedRoom;
use cntp_i18n::tr;
use contemporary::components::button::button;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::pager::pager;
use contemporary::styling::theme::Theme;
use gpui::http_client::anyhow;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AsyncApp, ElementId, InteractiveElement, IntoElement, ListAlignment, ListState,
    ParentElement, RenderOnce, StatefulInteractiveElement, Styled, Window, div, list, px,
};
use gpui_tokio::Tokio;
use matrix_sdk::ruma::events::key::verification::VerificationMethod;
use std::rc::Rc;
use thegrid::admonition::admonition;
use thegrid::session::session_manager::SessionManager;

type ChangeRoomHandler = dyn Fn(&ChangeRoomEvent, &mut Window, &mut App) + 'static;

#[derive(Clone)]
pub struct ChangeRoomEvent {
    pub new_room: DisplayedRoom,
}

#[derive(IntoElement)]
pub struct Sidebar {
    on_change_room: Option<Rc<Box<ChangeRoomHandler>>>,
}

pub fn sidebar() -> Sidebar {
    Sidebar {
        on_change_room: None,
    }
}

impl Sidebar {
    pub fn on_change_room(
        mut self,
        on_change_room: impl Fn(&ChangeRoomEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change_room = Some(Rc::new(Box::new(on_change_room)));
        self
    }
}

impl RenderOnce for Sidebar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let root_list_state =
            window.use_state(cx, |_, _| ListState::new(0, ListAlignment::Top, px(200.)));
        let verification_popover = window.use_state(cx, |_, cx| VerificationPopover::new(cx));

        let session_manager = cx.global::<SessionManager>();

        let Some(session) = session_manager.current_session() else {
            return layer();
        };
        let Some(client) = session_manager.client() else {
            return layer();
        };

        let client = client.read(cx);
        let verification_requests = session_manager.verification_requests();
        let verification_requests = verification_requests.read(cx);
        let shown_verification_requests: Vec<_> = verification_requests
            .pending_verification_requests
            .iter()
            .filter(|request| !request.inner.is_done() && !request.inner.is_cancelled())
            .collect();

        let root_rooms: Vec<_> = client
            .joined_rooms()
            .iter()
            .filter(|room| !room.is_space())
            .cloned()
            .collect();
        let root_list_state = root_list_state.read(cx);
        if root_rooms.len() != root_list_state.item_count() {
            root_list_state.reset(root_rooms.len());
        }

        let change_room_handler = self.on_change_room.unwrap().clone();

        let verification_popover_clone = verification_popover.clone();

        let theme = cx.global::<Theme>();

        layer()
            .w(px(300.))
            .flex()
            .flex_col()
            .child(
                pager("sidebar-pager", 0).flex_grow().page(
                    div()
                        .flex()
                        .flex_col()
                        .h_full()
                        .child(
                            grandstand("sidebar-grandstand")
                                .text(tr!("ROOMS_SPACES", "Rooms and Spaces"))
                                .pt(px(36.)),
                        )
                        .child(
                            div().flex_grow().child(
                                list(root_list_state.clone(), move |i, _, cx| {
                                    let room = &root_rooms[i];
                                    let room_id = room.room_id().to_owned();
                                    let change_room_handler = change_room_handler.clone();
                                    div()
                                        .id(ElementId::Name(room.room_id().to_string().into()))
                                        .p(px(2.))
                                        .child(room.name().unwrap_or_default())
                                        .on_click(move |_, window, cx| {
                                            let event = ChangeRoomEvent {
                                                new_room: DisplayedRoom::Room(room_id.clone()),
                                            };
                                            change_room_handler(&event, window, cx);
                                        })
                                        .into_any_element()
                                })
                                .flex()
                                .flex_col()
                                .h_full(),
                            ),
                        )
                        .into_any_element(),
                ),
            )
            .when(!shown_verification_requests.is_empty(), |david| {
                let first_verification_request = shown_verification_requests[0].clone();
                let first_verification_request_clone = first_verification_request.clone();

                david.child(
                    div().p(px(4.)).child(
                        admonition()
                            .title(tr!(
                                "INCOMING_VERIFICATION",
                                "Incoming Verification Request"
                            ))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(4.))
                                    .child(if first_verification_request.inner.is_self_verification() {
                                        tr!(
                                            "INCOMING_SELF_VERIFICATION_DESCRIPTION",
                                            "Verify your other device to share encryption keys. \
                                            The other device will be able to decrypt your messages.",
                                        )
                                    } else {
                                        tr!(
                                            "INCOMING_VERIFICATION_DESCRIPTION",
                                            "Respond to the verification request"
                                        )
                                    })
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .rounded(theme.border_radius)
                                            .bg(theme.button_background)
                                            .child(
                                                button("verification-request-accept")
                                                    .child(icon_text(
                                                        "dialog-ok".into(),
                                                        tr!(
                                                            "INCOMING_VERIFICATION_ACCEPT",
                                                            "Verify Now"
                                                        )
                                                        .into(),
                                                    ))
                                                    .on_click(move |_, _, cx| {
                                                        let verification_request =
                                                            first_verification_request.clone();
                                                        let verification_request_clone =
                                                            verification_request.clone();

                                                        cx.spawn(async move |cx: &mut AsyncApp| {
                                                            Tokio::spawn(cx, async move {
                                                                verification_request_clone
                                                                    .clone().inner
                                                                    .accept_with_methods(vec![
                                                                        VerificationMethod::SasV1,
                                                                    ])
                                                                    .await
                                                                    .map_err(|e| anyhow!(e))
                                                            })
                                                            .unwrap()
                                                            .await
                                                        })
                                                        .detach();

                                                        verification_popover.update(
                                                            cx,
                                                            |verification_popover, cx| {
                                                                verification_popover
                                                                    .set_verification_request(
                                                                        verification_request,
                                                                        cx,
                                                                    );
                                                            },
                                                        );
                                                    }),
                                            )
                                            .child(
                                                button("verification-request-decline")
                                                    .child(icon_text(
                                                        "dialog-cancel".into(),
                                                        tr!("INCOMING_VERIFICATION_DECLINE", "Don't Verify")
                                                            .into(),
                                                    ))
                                                    .on_click(move |_, _, cx| {
                                                        let verification_request =
                                                            first_verification_request_clone
                                                                .clone();

                                                        cx.spawn(async move |cx: &mut AsyncApp| {
                                                            Tokio::spawn(cx, async move {
                                                                verification_request
                                                                    .clone()
                                                                    .inner
                                                                    .cancel()
                                                                    .await
                                                                    .map_err(|e| anyhow!(e))
                                                            })
                                                            .unwrap()
                                                            .await
                                                        })
                                                        .detach()
                                                    }),
                                            ),
                                    ),
                            ),
                    ),
                )
            })
            .child(
                layer()
                    .p(px(4.))
                    .flex()
                    .child(session.matrix_session.meta.user_id.to_string()),
            )
            .child(verification_popover_clone.clone().into_any_element())
    }
}
