use crate::call_manager::{FocusUrl, LivekitCallManager};
use crate::webcam::Webcam;
use crate::TrackType;
use cntp_i18n::{tr, trn};
use contemporary::components::button::{button, ButtonMenuOpenPolicy};
use contemporary::components::context_menu::ContextMenuItem;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon::icon;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::subtitle::subtitle;
use contemporary::permissions::{
    GrantStatus, PermissionRequestCompleteEvent, PermissionType, Permissions,
};
use contemporary::styling::theme::ThemeStorage;
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::Device;
use gpui::prelude::FluentBuilder;
use gpui::{
    div, img, px, rgb, AppContext, BorrowAppContext, Context, Entity,
    IntoElement, ObjectFit, ParentElement, Render, Styled, StyledImage, Window,
};
use matrix_sdk::room::RoomMember;
use matrix_sdk::ruma::OwnedRoomId;
use nokhwa::utils::CameraInfo;
use nokhwa::{native_api_backend, query};
use std::collections::HashMap;
use std::rc::Rc;
use thegrid_common::mxc_image::{mxc_image, SizePolicy};
use thegrid_common::room::active_call_participants::track_active_call_participants;
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::surfaces::SurfaceChangeHandler;

pub struct CallStartPage {
    room_id: OwnedRoomId,
    on_surface_change: Rc<Box<SurfaceChangeHandler>>,

    audio_output_devices: Vec<cpal::Device>,
    audio_input_devices: Vec<cpal::Device>,
    selected_output_device: Option<cpal::Device>,
    selected_input_device: Option<cpal::Device>,
    active_call_users: Entity<Vec<RoomMember>>,

    active_camera: Option<Entity<Webcam>>,
    camera_info: Option<Vec<CameraInfo>>,
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

        let active_call_users = track_active_call_participants(room_id.clone(), cx);

        let mut this = Self {
            room_id,
            on_surface_change,
            audio_output_devices: output_devices,
            audio_input_devices: input_devices,
            selected_output_device,
            selected_input_device: if muted { None } else { selected_input_device },
            active_call_users,
            active_camera: None,
            camera_info: None,
        };
        this.fetch_camera_info(cx);
        this
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

    fn fetch_camera_info(&mut self, cx: &mut Context<Self>) {
        match Permissions::grant_status(PermissionType::Camera) {
            GrantStatus::Granted | GrantStatus::PlatformUnsupported => {
                self.camera_info = native_api_backend().and_then(|backend| query(backend).ok());
                cx.notify();
            }
            GrantStatus::Denied | GrantStatus::NotDetermined => {
                self.camera_info = None;
                cx.notify();
            }
        }
    }

    fn render_camera_setup(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let camera_grant_status = Permissions::grant_status(PermissionType::Camera);

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
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .overflow_hidden()
                    .when(
                        matches!(camera_grant_status, GrantStatus::NotDetermined),
                        |david| {
                            david.p(px(8.)).child(
                                button("camera-on")
                                    .child(icon_text(
                                        "camera-photo",
                                        tr!("CAMERA_SETUP_ENABLE", "Turn on camera"),
                                    ))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.turn_on_camera(None, window, cx);
                                    })),
                            )
                        },
                    )
                    .when(
                        matches!(camera_grant_status, GrantStatus::Denied),
                        |david| {
                            david.p(px(8.)).child(tr!(
                                "AUDIO_SETUP_CAMERA_UNAVAILABLE",
                                "Access to camera prohibited by your device"
                            ))
                        },
                    )
                    .when(
                        matches!(
                            camera_grant_status,
                            GrantStatus::Granted | GrantStatus::PlatformUnsupported
                        ),
                        |david| {
                            david
                                .when_some(self.active_camera.as_ref(), |david, webcam| {
                                    let webcam = webcam.read(cx);

                                    let camera_menu = self
                                        .camera_info
                                        .as_ref()
                                        .map(|camera_info| {
                                            camera_info
                                                .iter()
                                                .map(|camera| {
                                                    let camera = camera.clone();
                                                    ContextMenuItem::menu_item()
                                                        .label(camera.human_name())
                                                        .on_triggered(cx.listener(
                                                            move |this, _, window, cx| {
                                                                this.turn_on_camera(
                                                                    Some(camera.clone()),
                                                                    window,
                                                                    cx,
                                                                )
                                                            },
                                                        ))
                                                        .build()
                                                })
                                                .collect::<Vec<_>>()
                                        })
                                        .unwrap_or_default();

                                    david.child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .flex_grow()
                                            .child(
                                                div()
                                                    .flex()
                                                    .overflow_hidden()
                                                    .when_some(
                                                        webcam
                                                            .output_frame()
                                                            .read(cx)
                                                            .latest_render_frame()
                                                            .clone(),
                                                        |david, frame| {
                                                            david.child(
                                                                img(frame.clone())
                                                                    .object_fit(ObjectFit::Contain)
                                                                    .size_full(),
                                                            )
                                                        },
                                                    )
                                                    .when_some(webcam.error(), |david, error| {
                                                        david.flex_grow().child(
                                                            div()
                                                                .flex()
                                                                .items_center()
                                                                .justify_center()
                                                                .flex_grow()
                                                                .size_full()
                                                                .child(icon_text(
                                                                    "exception",
                                                                    tr!(
                                                                        "CAMERA_SETUP_\
                                                                        CAMERA_ERROR",
                                                                        "Unable to access \
                                                                        the camera"
                                                                    ),
                                                                )),
                                                        )
                                                    }),
                                            )
                                            .child(
                                                layer()
                                                    .flex()
                                                    .child(
                                                        button("camera-selection-button")
                                                            .flex_grow()
                                                            .child(
                                                                webcam.camera_info().human_name(),
                                                            )
                                                            .with_menu_open_policy(
                                                                ButtonMenuOpenPolicy::AnyClick,
                                                            )
                                                            .with_menu(camera_menu),
                                                    )
                                                    .child(
                                                        button("camera-off-button")
                                                            .child(icon("window-close"))
                                                            .destructive()
                                                            .on_click(cx.listener(
                                                                |this, _, _, cx| {
                                                                    this.turn_off_camera(cx)
                                                                },
                                                            )),
                                                    ),
                                            ),
                                    )
                                })
                                .when_none(&self.active_camera, |david| {
                                    david.when_else(
                                        self.camera_info
                                            .as_ref()
                                            .map(|camera_info| !camera_info.is_empty())
                                            .unwrap_or(false),
                                        |david| {
                                            david.p(px(8.)).child(
                                                button("camera-on")
                                                    .child(icon_text(
                                                        "camera-photo",
                                                        tr!("CAMERA_SETUP_ENABLE"),
                                                    ))
                                                    .on_click(cx.listener(
                                                        |this, _, window, cx| {
                                                            this.turn_on_camera(None, window, cx);
                                                        },
                                                    )),
                                            )
                                        },
                                        |david| {
                                            david.p(px(8.)).child(tr!(
                                                "CAMERA_SETUP_NO_CAMERA",
                                                "No camera available on this device"
                                            ))
                                        },
                                    )
                                })
                        },
                    ),
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
                                        "audio-input-microphone",
                                        tr!("AUDIO_SETUP_ENABLE_MIC", "Turn on mic"),
                                    ))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.request_permission(
                                            PermissionType::Microphone,
                                            window,
                                            cx,
                                        );
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

    fn turn_on_camera(
        &mut self,
        camera_info: Option<CameraInfo>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match Permissions::grant_status(PermissionType::Camera) {
            GrantStatus::Granted | GrantStatus::PlatformUnsupported => {
                let Some(camera) = camera_info.or_else(|| {
                    self.camera_info
                        .as_ref()
                        .and_then(|camera_info| camera_info.first().cloned())
                }) else {
                    return;
                };

                let webcam = cx.new(|cx| Webcam::new(camera.clone(), cx));
                self.active_camera = Some(webcam);
            }
            GrantStatus::NotDetermined => {
                self.request_permission(PermissionType::Camera, window, cx);
            }
            GrantStatus::Denied => {}
        }
    }

    fn turn_off_camera(&mut self, cx: &mut Context<Self>) {
        self.active_camera = None;
        cx.notify()
    }

    fn request_permission(
        &mut self,
        permission: PermissionType,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        Permissions::request_permission(
            permission,
            cx.listener(
                move |this, event: &PermissionRequestCompleteEvent, window, cx| {
                    if permission == PermissionType::Camera {
                        if event.grant_status == GrantStatus::Granted {
                            this.fetch_camera_info(cx);
                            this.turn_on_camera(None, window, cx);
                        }
                    } else {
                        let (input_devices, selected_input_device) =
                            Self::get_default_mic_settings();
                        this.audio_input_devices = input_devices;
                        this.selected_input_device = selected_input_device;
                    }
                    cx.notify();
                },
            ),
            window,
            cx,
        );
    }

    fn start_call(&mut self, cx: &mut Context<Self>) {
        let room_id = self.room_id.clone();
        let output_device = self.selected_output_device.clone();
        let input_device = self.selected_input_device.clone();

        cx.update_global::<LivekitCallManager, _>(|call_manager, cx| {
            call_manager.active_output_device().write(cx, output_device);
            call_manager.active_input_device().write(cx, input_device);

            let mut initial_streams = HashMap::new();
            if let Some(active_camera) = &self.active_camera {
                let output_frame = active_camera.read(cx).output_frame();
                initial_streams.insert(TrackType::Camera, output_frame);
            }

            if call_manager
                .start_call(room_id, initial_streams, cx)
                .is_some()
            {
                self.turn_off_camera(cx);
            }
        });
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
        let active_call_users = self.active_call_users.read(cx).clone();

        let in_other_call = cx
            .global::<LivekitCallManager>()
            .current_call()
            .is_some_and(|call| call.read(cx).room() != self.room_id);

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
                        this.turn_off_camera(cx);
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
                                            .child(self.render_camera_setup(window, cx))
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
                                div()
                                    .flex()
                                    .gap(px(8.))
                                    .child(div().flex().items_center().w(px(300.)).when(
                                        active_call_users.len() > 0,
                                        |david| {
                                            david.child(
                                                active_call_users
                                                    .iter()
                                                    .take(3)
                                                    .fold(
                                                        div().flex().gap(px(2.)).items_center(),
                                                        |david, member| {
                                                            david.child(
                                                                mxc_image(member.avatar_url())
                                                                    .fallback_image(
                                                                        member.user_id(),
                                                                    )
                                                                    .rounded(theme.border_radius)
                                                                    .size(px(16.))
                                                                    .size_policy(SizePolicy::Fit),
                                                            )
                                                        },
                                                    )
                                                    .child(div().pl(px(4.)).child(trn!(
                                                        "ACTIVE_CALL_CONTENT",
                                                        "{{count}} user in this room",
                                                        "{{count}} users in this room",
                                                        count = active_call_users.len() as isize
                                                    ))),
                                            )
                                        },
                                    ))
                                    .child(
                                        button("join-call")
                                            .child(icon_text(
                                                "call-start",
                                                tr!("CALL_JOIN_BUTTON", "Join Call"),
                                            ))
                                            .when(
                                                !matches!(focus_url, FocusUrl::Url(_)),
                                                |button| button.disabled(),
                                            )
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.start_call(cx);
                                            }))
                                            .w(px(300.)),
                                    ),
                            )
                            .when(in_other_call, |david| {
                                david.child(div().flex().justify_end().child(icon_text(
                                    "media-playback-pause",
                                    tr!(
                                        "CALL_HOLD_NOTIFICATION",
                                        "Your current call will be placed on hold"
                                    ),
                                )))
                            }),
                    ),
            )
    }
}
