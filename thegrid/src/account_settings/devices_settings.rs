use crate::account_settings::security_settings::recovery_key_reset_popover::RecoveryKeyResetPopover;
use crate::auth::verification_popover::VerificationPopover;
use crate::uiaa_client::{SendAuthDataEvent, UiaaClient};
use chrono::{DateTime, Local};
use cntp_i18n::tr;
use contemporary::components::admonition::{AdmonitionSeverity, admonition};
use contemporary::components::button::button;
use contemporary::components::constrainer::constrainer;
use contemporary::components::dialog_box::{StandardButton, dialog_box};
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon::icon;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::subtitle::subtitle;
use contemporary::styling::theme::{Theme, VariableColor};
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, AsyncApp, Context, ElementId, Entity, InteractiveElement, IntoElement,
    ParentElement, Render, RenderOnce, Styled, WeakEntity, Window, div, px, rgba,
};
use matrix_sdk::encryption::identities::Device;
use matrix_sdk::encryption::recovery::RecoveryState;
use matrix_sdk::ruma::OwnedDeviceId;
use matrix_sdk::ruma::api::client::uiaa::AuthData;
use std::rc::Rc;
use thegrid_common::session::devices_cache::CachedDevice;
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::tokio_helper::TokioHelper;
use tracing::error;

pub struct DevicesSettings {
    recovery_key_reset_popover: Entity<RecoveryKeyResetPopover>,
    verification_popover: Entity<VerificationPopover>,
    log_out_device: Option<OwnedDeviceId>,
    log_out_confirm_dialog_visible: bool,
    uiaa_client: Entity<UiaaClient>,
}

impl DevicesSettings {
    pub fn new(cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let send_auth_data =
                cx.listener(|this: &mut Self, event: &SendAuthDataEvent, _, cx| {
                    this.confirm_log_out_device(event.auth_data.clone(), cx);
                });

            Self {
                recovery_key_reset_popover: cx.new(|cx| RecoveryKeyResetPopover::new(cx)),
                verification_popover: cx.new(VerificationPopover::new),
                log_out_device: None,
                log_out_confirm_dialog_visible: false,
                uiaa_client: cx.new(|cx| UiaaClient::new(send_auth_data, |_, _, _| {}, cx)),
            }
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

    pub fn log_out_device(&mut self, device: OwnedDeviceId, cx: &mut Context<Self>) {
        self.log_out_device = Some(device);
        self.log_out_confirm_dialog_visible = true;
        cx.notify();
    }

    pub fn confirm_log_out_device(&mut self, auth_data: Option<AuthData>, cx: &mut Context<Self>) {
        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx).clone();
        let device = self.log_out_device.clone().unwrap();

        let uiaa_client_entity = self.uiaa_client.clone();

        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                if let Err(e) = cx
                    .spawn_tokio(async move { client.delete_devices(&[device], auth_data).await })
                    .await
                {
                    if let Some(uiaa) = e.as_uiaa_response() {
                        uiaa_client_entity
                            .update(cx, |uiaa_client, cx| {
                                uiaa_client.set_uiaa_info(uiaa.clone(), cx);
                                cx.notify()
                            })
                            .unwrap();
                        return;
                    } else {
                        error!("Failed to log out device: {:?}", e);
                    }
                }
            },
        )
        .detach();
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
                            .child({
                                let device_id = this_device.inner.device_id.clone();
                                DeviceItem {
                                    device: this_device,
                                    verify_device: None,
                                    erase_device: Rc::new(Box::new(cx.listener(
                                        move |this, _, _, cx| {
                                            this.log_out_device(device_id.clone(), cx)
                                        },
                                    ))),
                                }
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
                                    let device = item.encryption_status.clone();
                                    let device_id = item.inner.device_id.clone();
                                    david.child(
                                        div()
                                            .id(ElementId::Name(
                                                device_id.clone().to_string().into(),
                                            ))
                                            .child(DeviceItem {
                                                device: item,
                                                verify_device: match device {
                                                    None => None,
                                                    Some(device) => Some(Rc::new(Box::new(
                                                        cx.listener(move |this, _, _, cx| {
                                                            this.request_device_verification(
                                                                device.clone(),
                                                                cx,
                                                            )
                                                        }),
                                                    ))),
                                                },
                                                erase_device: Rc::new(Box::new(cx.listener(
                                                    move |this, _, _, cx| {
                                                        this.log_out_device(device_id.clone(), cx)
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
            .child(
                dialog_box("log-out-confirm")
                    .visible(self.log_out_confirm_dialog_visible)
                    .title(tr!("DEVICES_LOG_OUT_TITLE", "Forcibly log device out?").into())
                    .content_text_informational(
                        tr!(
                            "DEVICES_LOG_OUT_TEXT",
                            "Do you want to forcibly log out from {{device}}?",
                            device = self
                                .log_out_device
                                .as_ref()
                                .map(|device_id| device_id.clone().to_string())
                                .unwrap_or_default()
                        )
                        .into(),
                        tr!(
                            "DEVICES_LOG_OUT_INFORMATION",
                            "The device won't be able to receive or send any messages, and if \
                            it was verified, it will no longer be verified."
                        )
                        .into(),
                    )
                    .standard_button(
                        StandardButton::Cancel,
                        cx.listener(|this, _, _, cx| {
                            this.log_out_confirm_dialog_visible = false;
                            cx.notify();
                        }),
                    )
                    .button(
                        button("log-out-force")
                            .destructive()
                            .child(icon_text(
                                "system-log-out".into(),
                                tr!("DEVICES_LOG_OUT_ACTION", "Forcibly log out").into(),
                            ))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.confirm_log_out_device(None, cx);
                                this.log_out_confirm_dialog_visible = false;
                                cx.notify();
                            })),
                    ),
            )
            .child(self.uiaa_client.clone())
    }
}

#[derive(IntoElement)]
struct DeviceItem {
    device: CachedDevice,
    verify_device: Option<Rc<Box<dyn Fn(&(), &mut Window, &mut App)>>>,
    erase_device: Rc<Box<dyn Fn(&(), &mut Window, &mut App)>>,
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
        let erase_device = self.erase_device.clone();

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
                        david.when_some(verify_device, |david, verify_device| {
                            david.child(
                                button("verify-device-button")
                                    .child(icon_text(
                                        "dialog-ok".into(),
                                        tr!("DEVICE_VERIFY", "Verify").into(),
                                    ))
                                    .on_click(move |_, window, cx| verify_device(&(), window, cx)),
                            )
                        })
                    })
                    .child(
                        button("log-out-device-button")
                            .destructive()
                            .child(icon("system-log-out".into()))
                            .on_click(move |_, window, cx| erase_device(&(), window, cx)),
                    ),
            )
    }
}
