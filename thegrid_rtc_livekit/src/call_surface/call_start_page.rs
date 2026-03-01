use crate::call_manager::{FocusUrl, LivekitCallManager};
use cntp_i18n::tr;
use contemporary::components::admonition::{AdmonitionSeverity, admonition};
use contemporary::components::button::button;
use contemporary::components::context_menu::ContextMenuItem;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon::icon;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::subtitle::subtitle;
use contemporary::permissions::{GrantStatus, PermissionType, Permissions};
use contemporary::styling::theme::ThemeStorage;
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{Device, DeviceDescription};
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AsyncApp, BorrowAppContext, Context, IntoElement, ParentElement, Render, RenderOnce,
    Styled, WeakEntity, Window, div, px, rgb,
};
use matrix_sdk::ruma::OwnedRoomId;
use std::rc::Rc;
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::surfaces::SurfaceChangeHandler;

pub struct CallStartPage {
    room_id: OwnedRoomId,
    on_surface_change: Rc<Box<SurfaceChangeHandler>>,

    audio_output_devices: Vec<cpal::Device>,
    audio_input_devices: Vec<cpal::Device>,
    selected_output_device: Option<cpal::Device>,
    selected_input_device: Option<cpal::Device>,
}

impl CallStartPage {
    pub fn new(
        room_id: OwnedRoomId,
        on_surface_change: Rc<Box<SurfaceChangeHandler>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let call_manager = cx.global::<LivekitCallManager>();
        let muted = *call_manager.mute().read(cx);

        let (input_devices, selected_input_device) = Self::get_default_mic_settings();

        let host = cpal::default_host();
        let output_devices = host
            .output_devices()
            .map(|devices| devices.collect())
            .unwrap_or_default();
        let selected_output_device = host.default_output_device();

        Self {
            room_id,
            on_surface_change,
            audio_output_devices: output_devices,
            audio_input_devices: input_devices,
            selected_output_device,
            selected_input_device: if muted { None } else { selected_input_device },
        }
    }

    fn get_default_mic_settings() -> (Vec<Device>, Option<Device>) {
        match Permissions::grant_status(PermissionType::Microphone) {
            GrantStatus::Granted | GrantStatus::PlatformUnsupported => {
                let host = cpal::default_host();
                let input_devices = host
                    .input_devices()
                    .map(|devices| devices.collect())
                    .unwrap_or_default();
                let selected_input_device = host.default_input_device();

                (input_devices, selected_input_device)
            }
            GrantStatus::Denied | GrantStatus::NotDetermined => {
                // Avoid loading mic information
                Default::default()
            }
        }
    }

    fn render_camera_area(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .p(px(8.))
            .child(subtitle(tr!("CAMERA_SETUP", "Camera")))
            .child(
                layer()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(tr!("CAMERA_SETUP_NOT_SUPPORTED", "Coming Soon")),
            )
    }

    fn render_audio_setup(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .p(px(8.))
            .child(subtitle(tr!("AUDIO_SETUP", "Audio Output")))
            .child(
                div()
                    .flex()
                    .child(tr!("AUDIO_SETUP_OUTPUT", "Audio Output"))
                    .child(div().flex_grow())
                    .child(
                        button("audio-select-device")
                            .child(
                                self.selected_output_device
                                    .clone()
                                    .and_then(|device| device.description().ok())
                                    .map(|device| device.name().to_string())
                                    .unwrap_or_else(|| tr!("AUDIO_DEVICE_NONE", "Muted").into()),
                            )
                            .with_menu(
                                std::iter::once(None)
                                    .chain(self.audio_output_devices.iter().filter_map(|device| {
                                        device
                                            .description()
                                            .ok()
                                            .map(|description| Some((device.clone(), description)))
                                    }))
                                    .map(|device| {
                                        match device {
                                            None => ContextMenuItem::menu_item()
                                                .label(tr!("AUDIO_DEVICE_NONE", "Muted"))
                                                .on_triggered(cx.listener(
                                                    move |this, _, _, cx| {
                                                        this.selected_output_device = None;
                                                        cx.notify();
                                                    },
                                                )),
                                            Some((device, description)) => {
                                                ContextMenuItem::menu_item()
                                                    .label(description.name().to_string())
                                                    .on_triggered(cx.listener(
                                                        move |this, _, _, cx| {
                                                            this.selected_output_device =
                                                                Some(device.clone());
                                                            cx.notify();
                                                        },
                                                    ))
                                            }
                                        }
                                        .build()
                                    })
                                    .collect(),
                            ),
                    ),
            )
    }

    fn render_mic_setup(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let mic_grant_status = Permissions::grant_status(PermissionType::Microphone);

        div()
            .flex()
            .flex_col()
            .size_full()
            .p(px(8.))
            .child(subtitle(tr!("MIC_SETUP", "Microphone")))
            .when(
                matches!(mic_grant_status, GrantStatus::NotDetermined),
                |david| {
                    david.child(
                        layer()
                            .p(px(8.))
                            .size_full()
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .child(
                                button("mic-request-permission")
                                    .child(icon_text(
                                        "audio-input-microphone".into(),
                                        tr!("AUDIO_SETUP_ENABLE_MIC", "Turn on mic").into(),
                                    ))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.request_permission(PermissionType::Microphone, cx);
                                    })),
                            ),
                    )
                },
            )
            .when(matches!(mic_grant_status, GrantStatus::Denied), |david| {
                david.child(
                    layer()
                        .p(px(8.))
                        .size_full()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .child(tr!(
                            "AUDIO_SETUP_MIC_UNAVAILABLE",
                            "Access to microphone prohibited by your device"
                        )),
                )
            })
            .when(
                matches!(
                    mic_grant_status,
                    GrantStatus::Granted | GrantStatus::PlatformUnsupported
                ),
                |david| {
                    david.child(
                        div()
                            .flex()
                            .child(tr!("AUDIO_SETUP_INPUT", "Audio Input"))
                            .child(div().flex_grow())
                            .child(
                                button("mic-select-device")
                                    .child(
                                        self.selected_input_device
                                            .clone()
                                            .and_then(|device| device.description().ok())
                                            .map(|device| device.name().to_string())
                                            .unwrap_or_else(|| tr!("AUDIO_DEVICE_NONE").into()),
                                    )
                                    .with_menu(
                                        std::iter::once(None)
                                            .chain(self.audio_input_devices.iter().filter_map(
                                                |device| {
                                                    device.description().ok().map(|description| {
                                                        Some((device.clone(), description))
                                                    })
                                                },
                                            ))
                                            .map(|device| {
                                                match device {
                                                    None => ContextMenuItem::menu_item()
                                                        .label(tr!("AUDIO_DEVICE_NONE", "Muted"))
                                                        .on_triggered(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.selected_input_device = None;
                                                                cx.notify();
                                                            },
                                                        )),
                                                    Some((device, description)) => {
                                                        ContextMenuItem::menu_item()
                                                            .label(description.name().to_string())
                                                            .on_triggered(cx.listener(
                                                                move |this, _, _, cx| {
                                                                    this.selected_input_device =
                                                                        Some(device.clone());
                                                                    cx.notify();
                                                                },
                                                            ))
                                                    }
                                                }
                                                .build()
                                            })
                                            .collect(),
                                    ),
                            ),
                    )
                },
            )
    }

    fn request_permission(&mut self, permission: PermissionType, cx: &mut Context<Self>) {
        let (tx, rx) = async_channel::bounded(1);
        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let Ok(_) = rx.recv().await else {
                    return;
                };

                let _ = weak_this.update(cx, |this, cx| {
                    let (input_devices, selected_input_device) = Self::get_default_mic_settings();
                    this.audio_input_devices = input_devices;
                    this.selected_input_device = selected_input_device;
                    cx.notify();
                });
            },
        )
        .detach();

        Permissions::request_permission(PermissionType::Microphone, move |success| {
            let _ = smol::block_on(tx.send(success));
        })
    }
}

impl Render for CallStartPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let session_manager = cx.global::<SessionManager>();
        let room = session_manager
            .rooms()
            .read(cx)
            .room(&self.room_id)
            .unwrap()
            .read(cx);
        let room_name = room.display_name().clone();

        let room_id = self.room_id.clone();
        let focus_url = window.use_state(cx, |window, cx| {
            cx.update_global::<LivekitCallManager, _>(|call_manager, cx| {
                call_manager.best_focus_url_for_room(room_id, cx)
            })
        });
        let focus_url = focus_url.read(cx).clone();

        let theme = cx.theme().clone();

        div()
            .size_full()
            .bg(rgb(0x000000))
            .flex()
            .flex_col()
            .flex_grow()
            .child(
                grandstand("call-join")
                    .text(
                        tr!("CALL_JOIN_GRANDSTAND", "Join call in {{room}}", room:quote=room_name),
                    )
                    .pt(px(36.))
                    .on_back_click(cx.listener(move |this, _, window, cx| {
                        (this.on_surface_change)(
                            &thegrid_common::surfaces::SurfaceChangeEvent {
                                change: thegrid_common::surfaces::SurfaceChange::Pop,
                            },
                            window,
                            cx,
                        )
                    })),
            )
            .child(
                div()
                    .flex_grow()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .child(div().child(subtitle(tr!("JOIN_CALL", "Join Call"))))
                            .child(
                                div()
                                    .p(px(4.))
                                    .border(px(1.))
                                    .border_color(theme.border_color)
                                    .rounded(theme.border_radius)
                                    .flex()
                                    .flex_col()
                                    .justify_center()
                                    .gap(px(4.))
                                    .bg(match &focus_url {
                                        FocusUrl::Url(_) | FocusUrl::Processing => {
                                            theme.info_accent_color
                                        }
                                        FocusUrl::NoAvailableFocus => theme.error_accent_color,
                                    })
                                    .child(match &focus_url {
                                        FocusUrl::Url(url) => {
                                            tr!(
                                                "CALL_JOIN_LIVEKIT_SERVER_ADVICE",
                                                "This call will be joined through {{url}}",
                                                url = url
                                            )
                                        }
                                        FocusUrl::Processing => {
                                            tr!(
                                                "CALL_JOIN_PROCESSING",
                                                "Please wait while the details of this call are \
                                                checked..."
                                            )
                                        }
                                        FocusUrl::NoAvailableFocus => {
                                            tr!(
                                                "CALL_JOIN_NO_FOCUS",
                                                "This call cannot be connected because your \
                                                homeserver is not configured to support calling."
                                            )
                                        }
                                    }),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap(px(8.))
                                    .child(
                                        layer()
                                            .border(px(1.))
                                            .border_color(theme.border_color)
                                            .child(self.render_camera_area(window, cx))
                                            .w(px(300.))
                                            .h(px(200.)),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(8.))
                                            .child(
                                                layer()
                                                    .size_full()
                                                    .border(px(1.))
                                                    .border_color(theme.border_color)
                                                    .child(self.render_audio_setup(window, cx)),
                                            )
                                            .child(
                                                layer()
                                                    .size_full()
                                                    .border(px(1.))
                                                    .border_color(theme.border_color)
                                                    .child(self.render_mic_setup(window, cx)),
                                            )
                                            .w(px(300.)),
                                    ),
                            )
                            .child(
                                div().flex().gap(px(8.)).child(div().w(px(300.))).child(
                                    button("join-call")
                                        .child(icon_text(
                                            "call-start".into(),
                                            tr!("CALL_JOIN_BUTTON", "Join Call").into(),
                                        ))
                                        .when(!matches!(focus_url, FocusUrl::Url(_)), |button| {
                                            button.disabled()
                                        })
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            start_call(this.room_id.clone(), cx);
                                        }))
                                        .w(px(300.)),
                                ),
                            ),
                    ),
            )
    }
}

fn start_call(room_id: OwnedRoomId, cx: &mut App) {
    cx.update_global::<LivekitCallManager, _>(|call_manager, cx| {
        call_manager.start_call(room_id, cx);
    });
}
