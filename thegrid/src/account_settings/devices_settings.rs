use crate::account_settings::security_settings::recovery_key_reset_popover::RecoveryKeyResetPopover;
use crate::auth::verification_popover::VerificationPopover;
use chrono::{DateTime, Local};
use cntp_i18n::tr;
use contemporary::components::button::button;
use contemporary::components::constrainer::constrainer;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon::icon;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::subtitle::subtitle;
use contemporary::styling::theme::{Theme, VariableColor};
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, Context, ElementId, Entity, InteractiveElement, IntoElement, ParentElement,
    Render, RenderOnce, Styled, Window, div, px, rgba,
};
use matrix_sdk::encryption::identities::Device;
use matrix_sdk::encryption::recovery::RecoveryState;
use matrix_sdk::ruma::OwnedDeviceId;
use std::rc::Rc;
use thegrid::admonition::{AdmonitionSeverity, admonition};
use thegrid::session::devices_cache::CachedDevice;
use thegrid::session::session_manager::SessionManager;

pub struct DevicesSettings {
    recovery_key_reset_popover: Entity<RecoveryKeyResetPopover>,
    verification_popover: Entity<VerificationPopover>,
}

impl DevicesSettings {
    pub fn new(cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self {
            recovery_key_reset_popover: cx.new(|cx| RecoveryKeyResetPopover::new(cx)),
            verification_popover: cx.new(|cx| VerificationPopover::new(cx)),
        })
    }

    pub fn trigger_outgoing_verification(&mut self, cx: &mut Context<Self>) {
        self.verification_popover
            .update(cx, |verification_popover, cx| {
                verification_popover.trigger_outgoing_verification(cx)
            });
    }

    pub fn request_device_verification(&mut self, device: Device, cx: &mut Context<Self>) {
        self.verification_popover
            .update(cx, |verification_popover, cx| {
                verification_popover.request_device_verification(device, cx)
            });
    }
}

impl Render for DevicesSettings {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>();
        let session_manager = cx.global::<SessionManager>();

        let account = session_manager.current_account().read(cx);
        let verified = account.we_are_verified();

        let client = session_manager.client().unwrap().read(cx).clone();
        let recovery_not_set_up = client.encryption().recovery().state() == RecoveryState::Disabled;

        let devices = session_manager.devices().read(cx);
        let mut device_list = devices.devices();
        let this_device = device_list
            .iter()
            .position(|device| device.inner.device_id == client.device_id().unwrap())
            .unwrap();
        let this_device = device_list.swap_remove(this_device).clone();

        div()
            .bg(theme.background)
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .child(
                grandstand("devices-grandstand")
                    .text(tr!("ACCOUNT_SETTINGS_DEVICES"))
                    .pt(px(36.)),
            )
            .child(
                constrainer("devices")
                    .flex()
                    .flex_col()
                    .w_full()
                    .p(px(8.))
                    .gap(px(8.))
                    .when(recovery_not_set_up, |david| {
                        david.child(
                            admonition()
                                .severity(AdmonitionSeverity::Warning)
                                .title(tr!("SETUP_RECOVERY"))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(4.))
                                        .child(tr!("SETUP_RECOVERY_DESCRIPTION"))
                                        .child(
                                            div().flex().child(div().flex_grow()).child(
                                                button("setup-now")
                                                    .child(icon_text(
                                                        "configure".into(),
                                                        tr!("SETUP_RECOVERY_NOW").into(),
                                                    ))
                                                    .on_click(cx.listener(
                                                        move |this, _, _, cx| {
                                                            this.recovery_key_reset_popover.update(
                                                                cx,
                                                                |popover, cx| {
                                                                    popover.open(cx);
                                                                    cx.notify();
                                                                },
                                                            )
                                                        },
                                                    )),
                                            ),
                                        ),
                                ),
                        )
                    })
                    .when(!verified && !recovery_not_set_up, |david| {
                        david.child(
                            admonition()
                                .severity(AdmonitionSeverity::Warning)
                                .title(tr!("VERIFY_SESSION"))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(4.))
                                        .child(tr!("VERIFY_SESSION_DESCRIPTION"))
                                        .child(tr!(
                                            "VERIFY_SESSION_DESCRIPTION_ADDITIONAL",
                                            "Until you verify this device, you can't verify any \
                                            other devices. If you don't have another device to \
                                            verify with, head to the Security settings for other \
                                            options."
                                        ))
                                        .child(
                                            div().flex().child(div().flex_grow()).child(
                                                button("verify-now")
                                                    .child(icon_text(
                                                        "edit-copy".into(),
                                                        tr!("VERIFY_SESSION_OTHER_DEVICE").into(),
                                                    ))
                                                    .on_click(cx.listener(
                                                        move |this, _, _, cx| {
                                                            this.trigger_outgoing_verification(cx)
                                                        },
                                                    )),
                                            ),
                                        ),
                                ),
                        )
                    })
                    .child(
                        layer()
                            .flex()
                            .flex_col()
                            .p(px(8.))
                            .w_full()
                            .child(subtitle(tr!("DEVICES_THIS_DEVICE", "This Device")))
                            .child(DeviceItem {
                                device: this_device,
                                verify_device: Rc::new(Box::new(|_, _, _| {})),
                            }),
                    )
                    .when(!device_list.is_empty(), |david| {
                        david.child(
                            device_list.into_iter().cloned().fold(
                                layer()
                                    .flex()
                                    .flex_col()
                                    .p(px(8.))
                                    .gap(px(4.))
                                    .w_full()
                                    .child(subtitle(tr!("DEVICES_OTHER_DEVICES", "Other Devices"))),
                                |david, item| {
                                    let device = item.encryption_status.clone().unwrap();
                                    david.child(
                                        div()
                                            .id(ElementId::Name(
                                                item.inner.device_id.to_string().into(),
                                            ))
                                            .child(DeviceItem {
                                                device: item,
                                                verify_device: Rc::new(Box::new(cx.listener(
                                                    move |this, _, _, cx| {
                                                        this.request_device_verification(
                                                            device.clone(),
                                                            cx,
                                                        )
                                                    },
                                                ))),
                                            }),
                                    )
                                },
                            ),
                        )
                    }),
            )
            .child(self.verification_popover.clone().into_any_element())
    }
}

#[derive(IntoElement)]
struct DeviceItem {
    device: CachedDevice,
    verify_device: Rc<Box<dyn Fn(&(), &mut Window, &mut App)>>,
}

impl RenderOnce for DeviceItem {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.global::<Theme>();
        let session_manager = cx.global::<SessionManager>();

        let account = session_manager.current_account().read(cx);

        let device_verified = if account.we_are_verified() {
            match self.device.encryption_status {
                None => true,
                Some(device_encryption) => device_encryption.is_verified(),
            }
        } else {
            // True because we ourselves aren't verified, so we can't verify this device.
            true
        };

        let mut supplementary_text = Vec::new();
        if let Some(ip) = self.device.inner.last_seen_ip {
            supplementary_text.push(ip);
        }
        if let Some(last_seen_ts) = self.device.inner.last_seen_ts {
            let last_seen_date = DateTime::from_timestamp_secs(last_seen_ts.as_secs().into())
                .unwrap()
                .with_timezone(&Local);

            supplementary_text.push(
                tr!(
                    "DEVICE_LAST_ACTIVITY",
                    "Last activity {{last_activity_timestamp}}",
                    last_activity_timestamp:date("YMDT", length="medium")=last_seen_date
                )
                .into(),
            );
        }

        let verify_device = self.verify_device.clone();

        layer()
            .p(px(4.))
            .flex()
            .items_center()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(2.))
                            .child(self.device.inner.display_name.unwrap_or_default())
                            .child(
                                div()
                                    .flex()
                                    .text_color(theme.foreground.disabled())
                                    .child("•")
                                    .child(self.device.inner.device_id.to_string()),
                            )
                            .when(!device_verified, |david| {
                                david.child(
                                    div()
                                        .rounded(theme.border_radius)
                                        .bg(rgba(0xFFC80010))
                                        .p(px(2.))
                                        .child(tr!("UNVERIFIED_DEVICE_BADGE", "Unverified")),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_color(theme.foreground.disabled())
                            .child(supplementary_text.join(" • ")),
                    ),
            )
            .child(div().flex_grow())
            .child(
                div()
                    .flex()
                    .rounded(theme.border_radius)
                    .bg(theme.button_background)
                    .when(!device_verified, |david| {
                        david.child(
                            button("verify-device-button")
                                .child(icon_text(
                                    "dialog-ok".into(),
                                    tr!("DEVICE_VERIFY", "Verify").into(),
                                ))
                                .on_click(move |_, window, cx| verify_device(&(), window, cx)),
                        )
                    })
                    .child(
                        button("log-out-device-button")
                            .destructive()
                            .child(icon("system-log-out".into())),
                    ),
            )
    }
}
