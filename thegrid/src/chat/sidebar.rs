mod directory_sidebar_page;
mod root_sidebar_page;
mod sidebar_list;
mod space_sidebar_page;
mod standard_room_element;

use crate::account_settings::security_settings::recovery_key_reset_popover::RecoveryKeyResetPopover;
use crate::auth::recovery_passphrase_popover::RecoveryPassphrasePopover;
use crate::auth::verification_popover::VerificationPopover;
use crate::chat::displayed_room::DisplayedRoom;
use crate::chat::sidebar::directory_sidebar_page::DirectorySidebarPage;
use crate::chat::sidebar::root_sidebar_page::RootSidebarPage;
use crate::chat::sidebar::space_sidebar_page::SpaceSidebarPage;
use cntp_i18n::{tr, trn};
use contemporary::components::admonition::{admonition, AdmonitionSeverity};
use contemporary::components::button::button;
use contemporary::components::dialog_box::{dialog_box, StandardButton};
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::pager::pager;
use contemporary::components::pager::slide_horizontal_animation::SlideHorizontalAnimation;
use contemporary::styling::theme::{Theme, VariableColor};
use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, App, AppContext, BorrowAppContext, Context,
    Entity, InteractiveElement, IntoElement, ListAlignment, ListState, ParentElement,
    Render, RenderOnce, StatefulInteractiveElement, Styled, Window,
};
use matrix_sdk::encryption::recovery::RecoveryState;
use matrix_sdk::encryption::VerificationState;
use matrix_sdk::ruma::room_id;
use std::rc::Rc;
use thegrid_common::mxc_image::{mxc_image, SizePolicy};
use thegrid_common::session::error_handling::{ClientError, RecoverableClientError};
use thegrid_common::session::room_cache::RoomCategory;
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::session::verification_requests_cache::VerificationRequestDetails;
use thegrid_common::surfaces::{
    AccountSettingsDeepLink, MainWindowSurface, SurfaceChangeEvent, SurfaceChangeHandler,
};
use thegrid_common::tokio_helper::TokioHelper;
use thegrid_rtc_livekit::active_call_sidebar_alert::active_call_sidebar_alert;
use thegrid_rtc_livekit::call_manager::LivekitCallManager;

pub struct Sidebar {
    displayed_room: Entity<DisplayedRoom>,
    on_surface_change: Option<Rc<Box<SurfaceChangeHandler>>>,

    current_page: usize,
    pages: Vec<SidebarPage>,
}

pub enum SidebarPage {
    Root(Entity<RootSidebarPage>),
    Space(Entity<SpaceSidebarPage>),
    Directory(Entity<DirectorySidebarPage>),
}

#[derive(IntoElement)]
enum SidebarAlert {
    None,
    IncomingVerificationRequest(Entity<VerificationRequestDetails>),
    VerifySession(bool, Option<Rc<Box<SurfaceChangeHandler>>>),
    SetupRecovery,
    RecoverRecovery,
    UnverifiedDevices(usize, Option<Rc<Box<SurfaceChangeHandler>>>),
    ClientError(RecoverableClientError),
    ActiveCall(Option<Rc<Box<SurfaceChangeHandler>>>),
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
            .filter(|request| request.read(cx).is_active())
            .collect();

        if !shown_verification_requests.is_empty() {
            return SidebarAlert::IncomingVerificationRequest(
                shown_verification_requests[0].clone(),
            );
        }

        let call_manager = cx.global::<LivekitCallManager>();
        if call_manager.current_call().is_some() {
            return SidebarAlert::ActiveCall(self.on_surface_change.clone());
        }

        let account = session_manager.current_account().read(cx);
        let devices = session_manager.devices().read(cx);
        if account.verification_state() != VerificationState::Verified {
            return SidebarAlert::VerifySession(
                devices.is_last_device(),
                self.on_surface_change.clone(),
            );
        }

        let client = session_manager.client().unwrap().read(cx);
        let recovery = client.encryption().recovery();
        if recovery.state() == RecoveryState::Disabled {
            return SidebarAlert::SetupRecovery;
        } else if recovery.state() == RecoveryState::Incomplete {
            return SidebarAlert::RecoverRecovery;
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

        let client = session_manager.client().unwrap().read(cx);
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
                        SidebarPage::Directory(directory_sidebar_page) => {
                            pager.page(directory_sidebar_page.clone().into_any_element())
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
                            .fallback_image(client.user_id().unwrap())
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
                            .child(div().text_color(theme.foreground.disabled()).child(
                                session.secrets.session_meta().unwrap().user_id.to_string(),
                            )),
                    ),
            )
    }
}

impl RenderOnce for SidebarAlert {
    fn render<'a>(self, window: &mut Window, cx: &'a mut App) -> impl IntoElement {
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
                SidebarAlert::IncomingVerificationRequest(verification_request_entity) => {
                    let verification_request = verification_request_entity.read(cx);

                    let session_manager = cx.global::<SessionManager>();
                    let devices = session_manager.devices().read(cx);
                    let device_list = devices.devices();

                    let device = verification_request.device_id.as_ref().and_then(|device_id| {
                        device_list.iter().find(|device| &device.inner.device_id == device_id)
                    });

                    let device_name =
                        device.map(
                            |device|
                                device
                                    .inner
                                    .display_name
                                    .clone()
                                    .map(|display_name| format!("{display_name} ({})", device.inner.device_id))
                                    .unwrap_or_else(
                                        || device.inner.device_id.to_string()
                                    )
                        ).unwrap_or_else(|| tr!(
                            "UNKNOWN_DEVICE",
                            "Unknown Device"
                        ).into());

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
                                            "Verify {{device_name}} to trust \
                                            it and share encryption keys. The other device will \
                                            be able to decrypt your messages.",
                                            device_name:quote = device_name
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
                                                        "dialog-ok",
                                                        tr!(
                                                            "INCOMING_VERIFICATION_ACCEPT",
                                                            "Verify Now"
                                                        )
                                                            ,
                                                    ))
                                                    .on_click({
                                                        let verification_request_entity = verification_request_entity.clone();
                                                        move |_, _, cx| {
                                                            let verification_request_entity = verification_request_entity.clone();
                                                            verification_request_entity.update(cx, |verification_request, cx| {
                                                                verification_request.accept(cx);
                                                            });

                                                            verification_popover.update(
                                                                cx,
                                                                |verification_popover, cx| {
                                                                    verification_popover
                                                                        .set_verification_request(
                                                                            verification_request_entity,
                                                                            cx,
                                                                        );
                                                                },
                                                            );
                                                        }
                                                    }),
                                            )
                                            .child(
                                                button("verification-request-decline")
                                                    .child(icon_text(
                                                        "dialog-cancel",
                                                        tr!(
                                                            "INCOMING_VERIFICATION_DECLINE",
                                                            "Don't Verify"
                                                        )
                                                            ,
                                                    ))
                                                    .on_click({
                                                        let verification_request_entity = verification_request_entity.clone();
                                                        move |_, _, cx: &mut App| {
                                                            verification_request_entity.update(cx, |verification_request, cx| {
                                                                verification_request.cancel(cx);
                                                            });
                                                        }
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
                                                    "configure",
                                                    tr!("SETUP_RECOVERY_NOW", "Set up now"),
                                                ))
                                                .on_click(move |_, _, cx| {
                                                    recovery_key_reset_popover.update(
                                                        cx,
                                                        |recovery_key_reset_popover, cx| {
                                                            recovery_key_reset_popover.open(cx);
                                                            cx.notify();
                                                        },
                                                    )
                                                }),
                                        ),
                                ),
                        ),
                ),
                SidebarAlert::RecoverRecovery => div().p(px(4.)).child(
                    admonition()
                        .severity(AdmonitionSeverity::Warning)
                        .title(tr!("FIX_RECOVERY", "Recovery data corrupt"))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                .child(tr!(
                                    "FIX_RECOVERY_DESCRIPTION",
                                    "Your local recovery data is corrupt. Your recovery key is \
                                    required to continue backing up your encryption keys.",
                                ))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .rounded(theme.border_radius)
                                        .bg(theme.button_background)
                                        .child(
                                            button("verify-recovery")
                                                .child(icon_text(
                                                    "visibility",
                                                    tr!(
                                                        "VERIFY_SESSION_RECOVERY_KEY",
                                                    )
                                                        ,
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
                                ),
                        ),
                ),
                SidebarAlert::VerifySession(is_last_device, handler) => {
                    let theme = theme.clone();
                    let reset_dialog_open = window.use_state(cx, |_, _| false);

                    div().p(px(4.)).child(
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
                                                            "edit-copy",
                                                            tr!(
                                                            "VERIFY_SESSION_OTHER_DEVICE",
                                                            "Verify with another verified device"
                                                        )
                                                                ,
                                                        ))
                                                        .on_click(move |_, _, cx| {
                                                            verification_popover.update(
                                                                cx,
                                                                |verification_popover, cx| {
                                                                    verification_popover
                                                                        .trigger_outgoing_verification(
                                                                            cx,
                                                                        )
                                                                },
                                                            );
                                                        }),
                                                )
                                            })
                                            .child(
                                                button("verify-recovery")
                                                    .child(icon_text(
                                                        "visibility",
                                                        tr!(
                                                        "VERIFY_SESSION_RECOVERY_KEY",
                                                        "Enter Recovery Key"
                                                    )
                                                            ,
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
                                                        "help-contents",
                                                        tr!(
                                                            "VERIFY_SESSION_RESET_CRYPTO",
                                                            "I lost my verification methods"
                                                        ),
                                                    ))
                                                    .on_click({
                                                        let reset_dialog_open = reset_dialog_open.clone();
                                                        move |_, window, cx| {
                                                            reset_dialog_open.write(cx, true);
                                                        }
                                                    }),
                                            ),
                                    ),
                            ),
                    ).child(
                        dialog_box("crypto-reset-dialog")
                            .visible(*reset_dialog_open.read(cx))
                            .title(
                                tr!(
                                    "VERIFY_SESSION_RESET_CRYPTO_DIALOG_TITLE",
                                    "Encryption Setup Recovery"
                                )
                            )
                            .content(
                                tr!(
                                    "VERIFY_SESSION_RESET_CRYPTO_DIALOG_MESSAGE",
                                    "If you can't verify this session because you've lost access \
                                    to all your other verification methods, you can reset your \
                                    cryptographic identity to start over. You will lose access to \
                                    your existing encrypted messages, and all of your devices \
                                    will become unverified."
                                )
                            )
                            .standard_button(StandardButton::Cancel, {
                                let reset_dialog_open = reset_dialog_open.clone();
                                move |_, _, cx| {
                                    reset_dialog_open.write(cx, false);
                                }
                            }).button(
                            button("do-reset-crypto")
                                .destructive()
                                .child(icon_text(
                                    "view-refresh",
                                    tr!("SECURITY_IDENTITY_RESET"),
                                ))
                                .on_click(move |_, window, cx| {
                                    reset_dialog_open.write(cx, false);
                                    if let Some(handler) = handler.clone() {
                                        handler(
                                            &SurfaceChangeEvent {
                                                change: MainWindowSurface::IdentityReset.into(),
                                            },
                                            window,
                                            cx,
                                        );
                                    }
                                }),
                        )
                    )
                }
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
                                                    "phone",
                                                    tr!(
                                                        "UNVERIFIED_DEVICES_VIEW_DEVICES",
                                                        "View Devices"
                                                    )
                                                        ,
                                                ))
                                                .on_click(move |_, window, cx| {
                                                    if let Some(handler) = handler.clone() {
                                                        handler(
                                                            &SurfaceChangeEvent {
                                                                change: MainWindowSurface::AccountSettings(AccountSettingsDeepLink::Devices).into(),
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
                SidebarAlert::ActiveCall(handler) => {
                    div()
                        .p(px(4.))
                        .child(active_call_sidebar_alert(Rc::new(Box::new(
                            move |event, window, cx| {
                                if let Some(handler) = handler.clone() {
                                    handler(event, window, cx);
                                }
                            },
                        ))))
                }
            })
            .child(verification_popover_clone.clone().into_any_element())
            .child(recovery_passphrase_popover_clone.clone().into_any_element())
            .child(recovery_key_reset_popover_clone.clone().into_any_element())
    }
}
