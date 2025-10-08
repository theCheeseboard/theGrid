use crate::auth::verification_popover::VerificationPopover;
use crate::chat::displayed_room::DisplayedRoom;
use crate::chat::main_chat_surface::{ChangeRoomEvent, ChangeRoomHandler};
use cntp_i18n::{tr, trn};
use contemporary::components::button::button;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::pager::pager;
use contemporary::styling::theme::{Theme, VariableColor};
use gpui::http_client::anyhow;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AsyncApp, ElementId, InteractiveElement, IntoElement, ListAlignment, ListState,
    ParentElement, RenderOnce, StatefulInteractiveElement, Styled, Window, div, list, px, rgb,
};
use gpui_tokio::Tokio;
use matrix_sdk::ruma::events::key::verification::VerificationMethod;
use std::rc::Rc;
use thegrid::admonition::{AdmonitionSeverity, admonition};
use thegrid::session::session_manager::SessionManager;
use thegrid::session::verification_requests_cache::VerificationRequestDetails;
use thegrid::tokio_helper::TokioHelper;

#[derive(IntoElement)]
pub struct Sidebar {
    on_change_room: Option<Rc<Box<ChangeRoomHandler>>>,
}

#[derive(IntoElement)]
enum SidebarAlert {
    None,
    IncomingVerificationRequest(VerificationRequestDetails),
    VerifySession,
    UnverifiedDevices(usize),
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

    fn current_alert(&self, _: &mut Window, cx: &mut App) -> SidebarAlert {
        let session_manager = cx.global::<SessionManager>();

        let verification_requests = session_manager.verification_requests().read(cx);
        let shown_verification_requests: Vec<_> = verification_requests
            .pending_verification_requests
            .iter()
            .filter(|request| !request.inner.is_done() && !request.inner.is_cancelled())
            .collect();

        if !shown_verification_requests.is_empty() {
            return SidebarAlert::IncomingVerificationRequest(
                shown_verification_requests[0].clone(),
            );
        }

        let devices = session_manager.devices().read(cx);
        let unverified_devices = devices.unverified_devices();
        let all_devices = devices.devices();
        if all_devices.len() > 1 {
            if all_devices.len() == unverified_devices.len() + 1 {
                return SidebarAlert::VerifySession;
            }
            if !unverified_devices.is_empty() {
                return SidebarAlert::UnverifiedDevices(unverified_devices.len());
            }
        }

        SidebarAlert::None
    }
}

impl RenderOnce for Sidebar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let root_list_state =
            window.use_state(cx, |_, _| ListState::new(0, ListAlignment::Top, px(200.)));
        let current_notification = self.current_alert(window, cx);

        let session_manager = cx.global::<SessionManager>();

        let Some(session) = session_manager.current_session() else {
            return layer();
        };
        let Some(client) = session_manager.client() else {
            return layer();
        };

        let client = client.read(cx);

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
        let account = session_manager.current_account().read(cx);

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
            .child(current_notification)
            .child(
                layer()
                    .p(px(4.))
                    .flex()
                    .gap(px(4.))
                    .child(div().size(px(48.)).bg(rgb(0xff0000)))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .justify_center()
                            .gap(px(4.))
                            .child(account.display_name().unwrap_or_default())
                            .child(
                                div()
                                    .text_color(theme.foreground.disabled())
                                    .child(session.matrix_session.meta.user_id.to_string()),
                            ),
                    ),
            )
    }
}

impl RenderOnce for SidebarAlert {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let verification_popover = window.use_state(cx, |_, cx| VerificationPopover::new(cx));
        let verification_popover_clone = verification_popover.clone();

        let theme = cx.global::<Theme>();

        div()
            .child(match self {
                SidebarAlert::None => div(),
                SidebarAlert::IncomingVerificationRequest(verification_request) => {
                    let verification_request_clone = verification_request.clone();

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
                                    .child(if verification_request.inner.is_self_verification() {
                                        tr!(
                                            "INCOMING_SELF_VERIFICATION_DESCRIPTION",
                                            "Verify your other device ({{device_id}}) to share \
                                             encryption keys. The other device will be able to \
                                             decrypt your messages.",
                                            device_id = verification_request
                                                .device_id
                                                .clone()
                                                .map(|id| id.to_string())
                                                .unwrap_or_else(|| tr!(
                                                    "UNKNOWN_DEVICE",
                                                    "Unknown Device"
                                                )
                                                .to_string())
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
                                                            verification_request.clone();
                                                        let verification_request_clone =
                                                            verification_request.clone();

                                                        cx.spawn(async move |cx: &mut AsyncApp| {
                                                            Tokio::spawn(cx, async move {
                                                                verification_request_clone
                                                                    .clone()
                                                                    .inner
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
                                                        tr!(
                                                            "INCOMING_VERIFICATION_DECLINE",
                                                            "Don't Verify"
                                                        )
                                                        .into(),
                                                    ))
                                                    .on_click(move |_, _, cx| {
                                                        let verification_request =
                                                            verification_request_clone.clone();

                                                        cx.spawn(async move |cx: &mut AsyncApp| {
                                                            cx.spawn_tokio(async move {
                                                                verification_request
                                                                    .clone()
                                                                    .inner
                                                                    .cancel()
                                                                    .await
                                                            })
                                                            .await
                                                        })
                                                        .detach()
                                                    }),
                                            ),
                                    ),
                            ),
                    )
                }
                SidebarAlert::VerifySession => div().p(px(4.)).child(
                    admonition()
                        .severity(AdmonitionSeverity::Warning)
                        .title(tr!("VERIFY_SESSION", "Verify Session"))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                .child(tr!(
                                    "VERIFY_SESSION_DESCRIPTION",
                                    "Verify this session to access encrypted messages sent from \
                                    other devices.",
                                ))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .rounded(theme.border_radius)
                                        .bg(theme.button_background)
                                        .child(
                                            button("verify-now")
                                                .child(icon_text(
                                                    "edit-copy".into(),
                                                    tr!(
                                                        "VERIFY_SESSION_OTHER_DEVICE",
                                                        "Verify with another verified device"
                                                    )
                                                    .into(),
                                                ))
                                                .on_click(move |_, _, cx| {
                                                    verification_popover.update(
                                                        cx,
                                                        |verification_popover, cx| {
                                                            verification_popover
                                                                .trigger_outgoing_verification(cx)
                                                        },
                                                    );
                                                }),
                                        )
                                        .child(
                                            button("verify-recovery")
                                                .child(icon_text(
                                                    "visibility".into(),
                                                    tr!(
                                                        "VERIFY_SESSION_RECOVERY_KEY",
                                                        "Enter Recovery Key"
                                                    )
                                                    .into(),
                                                ))
                                                .on_click(move |_, _, cx| {}),
                                        )
                                        .child(
                                            button("reset-crypto")
                                                .destructive()
                                                .child(icon_text(
                                                    "view-refresh".into(),
                                                    tr!(
                                                        "VERIFY_SESSION_RESET_CRYPTO",
                                                        "Reset Recovery Key"
                                                    )
                                                    .into(),
                                                ))
                                                .on_click(move |_, _, cx| {}),
                                        ),
                                ),
                        ),
                ),
                SidebarAlert::UnverifiedDevices(count) => div().p(px(4.)).child(
                    admonition()
                        .severity(AdmonitionSeverity::Warning)
                        .title(tr!("UNVERIFIED_DEVICES", "Unverified devices"))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                .child(trn!(
                                    "UNVERIFIED_DEVICES_DESCRIPTION",
                                    "{{count}} unverified device has access to your account. \
                                    Verify it to share encryption keys, or log it out to \
                                    maintain account security.",
                                    "{{count}} unverified devices have access to your account. \
                                    Verify them to share encryption keys, or log them out to \
                                    maintain account security.",
                                    count = count as isize
                                ))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .rounded(theme.border_radius)
                                        .bg(theme.button_background)
                                        .child(
                                            button("verify-now")
                                                .child(icon_text(
                                                    "phone".into(),
                                                    tr!(
                                                        "UNVERIFIED_DEVICES_VIEW_DEVICES",
                                                        "View Devices"
                                                    )
                                                    .into(),
                                                ))
                                                .on_click(move |_, _, cx| {}),
                                        ),
                                ),
                        ),
                ),
            })
            .child(verification_popover_clone.clone().into_any_element())
    }
}
