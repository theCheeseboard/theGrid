mod root_sidebar_page;
mod space_sidebar_page;

use crate::account_settings::AccountSettingsPage;
use crate::account_settings::security_settings::recovery_key_reset_popover::RecoveryKeyResetPopover;
use crate::auth::recovery_passphrase_popover::RecoveryPassphrasePopover;
use crate::auth::verification_popover::VerificationPopover;
use crate::chat::displayed_room::DisplayedRoom;
use crate::chat::sidebar::root_sidebar_page::RootSidebarPage;
use crate::chat::sidebar::space_sidebar_page::SpaceSidebarPage;
use crate::main_window::{MainWindowSurface, SurfaceChangeEvent, SurfaceChangeHandler};
use crate::mxc_image::{SizePolicy, mxc_image};
use cntp_i18n::{tr, trn};
use contemporary::components::button::button;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::pager::pager;
use contemporary::components::pager::slide_horizontal_animation::SlideHorizontalAnimation;
use contemporary::styling::theme::{Theme, VariableColor};
use gpui::http_client::anyhow;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, AsyncApp, BorrowAppContext, Context, Entity, InteractiveElement, IntoElement,
    ListAlignment, ListState, ParentElement, Render, RenderOnce, StatefulInteractiveElement,
    Styled, Window, div, px,
};
use gpui_tokio::Tokio;
use matrix_sdk::encryption::recovery::RecoveryState;
use matrix_sdk::ruma::events::key::verification::VerificationMethod;
use matrix_sdk::ruma::room_id;
use std::rc::Rc;
use thegrid::admonition::{AdmonitionSeverity, admonition};
use thegrid::session::error_handling::{ClientError, RecoverableClientError};
use thegrid::session::room_cache::RoomCategory;
use thegrid::session::session_manager::SessionManager;
use thegrid::session::verification_requests_cache::VerificationRequestDetails;
use thegrid::tokio_helper::TokioHelper;

pub struct Sidebar {
    displayed_room: Entity<DisplayedRoom>,
    on_surface_change: Option<Rc<Box<SurfaceChangeHandler>>>,

    current_page: usize,
    pages: Vec<SidebarPage>,
}

pub enum SidebarPage {
    Root(Entity<RootSidebarPage>),
    Space(Entity<SpaceSidebarPage>),
}

#[derive(IntoElement)]
enum SidebarAlert {
    None,
    IncomingVerificationRequest(VerificationRequestDetails),
    SetupRecovery,
    VerifySession(bool),
    UnverifiedDevices(usize, Option<Rc<Box<SurfaceChangeHandler>>>),
    ClientError(RecoverableClientError),
}

impl Sidebar {
    pub fn new(cx: &mut Context<Self>, displayed_room: Entity<DisplayedRoom>) -> Self {
        let self_entity = cx.entity();
        let displayed_room_clone = displayed_room.clone();

        Sidebar {
            displayed_room,
            on_surface_change: None,
            current_page: 0,
            pages: vec![SidebarPage::Root(cx.new(|cx| {
                RootSidebarPage::new(self_entity, displayed_room_clone, cx)
            }))],
        }
    }

    pub fn on_surface_change(
        &mut self,
        on_surface_change: impl Fn(&SurfaceChangeEvent, &mut Window, &mut App) + 'static,
    ) {
        self.on_surface_change = Some(Rc::new(Box::new(on_surface_change)));
    }

    fn current_alert(&self, _: &mut Window, cx: &mut App) -> SidebarAlert {
        let session_manager = cx.global::<SessionManager>();

        if let ClientError::Recoverable(recoverable_error) = session_manager.error() {
            return SidebarAlert::ClientError(recoverable_error);
        }

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

        let client = session_manager.client().unwrap().read(cx);
        let recovery = client.encryption().recovery();
        if recovery.state() == RecoveryState::Disabled {
            return SidebarAlert::SetupRecovery;
        }

        let account = session_manager.current_account().read(cx);
        let devices = session_manager.devices().read(cx);
        if let Some(identity) = account.identity()
            && !identity.is_verified()
        {
            return SidebarAlert::VerifySession(devices.is_last_device());
        }

        let unverified_devices = devices.unverified_devices();
        if !unverified_devices.is_empty() {
            return SidebarAlert::UnverifiedDevices(
                unverified_devices.len(),
                self.on_surface_change.clone(),
            );
        }

        SidebarAlert::None
    }

    pub fn push_page(&mut self, page: SidebarPage) {
        self.pages.truncate(self.current_page + 1);
        self.pages.push(page);
        self.current_page += 1;
    }

    pub fn pop_page(&mut self) {
        self.current_page -= 1;
    }
}

impl Render for Sidebar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root_list_state =
            window.use_state(cx, |_, _| ListState::new(0, ListAlignment::Top, px(200.)));
        let current_notification = self.current_alert(window, cx);

        let session_manager = cx.global::<SessionManager>();

        let Some(session) = session_manager.current_session() else {
            return layer();
        };

        let room_cache = session_manager.rooms().read(cx);

        let root_rooms = room_cache
            .rooms_in_category(
                RoomCategory::Space(room_id!("!kqHArBQfzKMdLtLrpX:bnbdiscord.net").to_owned()),
                cx,
            )
            .clone();
        let root_list_state = root_list_state.read(cx);
        if root_rooms.len() != root_list_state.item_count() {
            root_list_state.reset(root_rooms.len());
        }

        let account = session_manager.current_account().read(cx);

        let theme = cx.global::<Theme>();

        layer()
            .w(px(300.))
            .flex()
            .flex_col()
            .child(
                self.pages.iter().fold(
                    pager("sidebar-pager", self.current_page)
                        .animation(SlideHorizontalAnimation::new())
                        .flex_grow(),
                    |pager, page| match page {
                        SidebarPage::Root(root_sidebar_page) => {
                            pager.page(root_sidebar_page.clone().into_any_element())
                        }
                        SidebarPage::Space(space_sidebar_page) => {
                            pager.page(space_sidebar_page.clone().into_any_element())
                        }
                    },
                ),
            )
            .child(current_notification)
            .child(
                layer()
                    .p(px(4.))
                    .flex()
                    .gap(px(4.))
                    .child(
                        mxc_image(account.avatar_url())
                            .rounded(theme.border_radius)
                            .size(px(48.))
                            .size_policy(SizePolicy::Fit),
                    )
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

        let recovery_key_reset_popover =
            window.use_state(cx, |_, cx| RecoveryKeyResetPopover::new(cx));
        let recovery_key_reset_popover_clone = recovery_key_reset_popover.clone();

        let recovery_passphrase_popover =
            window.use_state(cx, |_, cx| RecoveryPassphrasePopover::new(cx));
        let recovery_passphrase_popover_clone = recovery_passphrase_popover.clone();

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
                SidebarAlert::SetupRecovery => div().p(px(4.)).child(
                    admonition()
                        .severity(AdmonitionSeverity::Warning)
                        .title(tr!("SETUP_RECOVERY", "Set up recovery"))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                .child(tr!(
                                    "SETUP_RECOVERY_DESCRIPTION",
                                    "Set up a recovery key for your account so you don't lose \
                                    access to your encrypted messages",
                                ))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .rounded(theme.border_radius)
                                        .bg(theme.button_background)
                                        .child(
                                            button("setup-now")
                                                .child(icon_text(
                                                    "configure".into(),
                                                    tr!(
                                                        "SETUP_RECOVERY_NOW",
                                                        "Set up now"
                                                    )
                                                        .into(),
                                                ))
                                                .on_click(move |_, _, cx| {
                                                    recovery_key_reset_popover.update(cx,
                                                          |recovery_key_reset_popover,
                                                           cx| {
                                                              recovery_key_reset_popover
                                                                  .open(cx);
                                                              cx.notify();
                                                          })
                                                }),
                                        )
                                ),
                        ),
                ),
                SidebarAlert::VerifySession(is_last_device) => div().p(px(4.)).child(
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
                                        .when(!is_last_device, |david| {
                                            david.child(
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
                                        })
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
                                                .on_click(move |_, _, cx| {
                                                    recovery_passphrase_popover.update(
                                                        cx,
                                                        |recovery_passphrase_popover, cx| {
                                                            recovery_passphrase_popover
                                                                .set_visible(true);
                                                            cx.notify()
                                                        },
                                                    )
                                                }),
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
                SidebarAlert::UnverifiedDevices(count, handler) => div().p(px(4.)).child(
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
                                                .on_click(move |_, window, cx| {
                                                    if let Some(handler) = handler.clone() {
                                                        handler(
                                                            &SurfaceChangeEvent {
                                                                change: MainWindowSurface::AccountSettings(AccountSettingsPage::Devices).into(),
                                                            },
                                                            window,
                                                            cx,
                                                        );
                                                    }
                                                }),
                                        ),
                                ),
                        ),
                ),
                SidebarAlert::ClientError(recoverable_client_error) => div().p(px(4.)).child(
                    admonition()
                        .severity(AdmonitionSeverity::Warning)
                        .title(recoverable_client_error.title())
                        .child(recoverable_client_error.description()),
                ),
            })
            .child(verification_popover_clone.clone().into_any_element())
            .child(recovery_passphrase_popover_clone.clone().into_any_element())
            .child(recovery_key_reset_popover_clone.clone().into_any_element())
    }
}
