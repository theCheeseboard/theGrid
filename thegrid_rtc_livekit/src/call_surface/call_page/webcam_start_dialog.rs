use crate::LivekitCall;
use crate::webcam::Webcam;
use cntp_i18n::tr;
use contemporary::components::button::{ButtonMenuOpenPolicy, button};
use contemporary::components::context_menu::ContextMenuItem;
use contemporary::components::dialog_box::{StandardButton, dialog_box};
use contemporary::components::icon::icon;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::permissions::{
    GrantStatus, PermissionRequestCompleteEvent, PermissionType, Permissions,
};
use gpui::prelude::FluentBuilder;
use gpui::{
    AppContext, Context, Entity, IntoElement, ObjectFit, ParentElement, Render, Styled,
    StyledImage, Window, div, img, px,
};
use nokhwa::utils::CameraInfo;
use nokhwa::{native_api_backend, query};

pub struct WebcamStartDialog {
    visible: bool,
    active_camera: Option<Entity<Webcam>>,
    camera_info: Option<Vec<CameraInfo>>,
    call: Option<Entity<LivekitCall>>,
}

impl WebcamStartDialog {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            visible: false,
            active_camera: None,
            camera_info: None,
            call: None,
        }
    }

    pub fn open(&mut self, call: Entity<LivekitCall>, window: &mut Window, cx: &mut Context<Self>) {
        self.visible = true;
        self.call = Some(call);
        cx.notify();

        match Permissions::grant_status(PermissionType::Camera) {
            GrantStatus::Granted | GrantStatus::PlatformUnsupported => {
                self.update_cameras(cx);
            }
            GrantStatus::Denied => {
                // Noop
            }
            GrantStatus::NotDetermined => {
                // Request permission
                Permissions::request_permission(
                    PermissionType::Camera,
                    cx.listener(|this, event: &PermissionRequestCompleteEvent, _, cx| {
                        if event.grant_status == GrantStatus::Granted {
                            this.update_cameras(cx);
                        }
                    }),
                    window,
                    cx,
                );
            }
        }
    }

    fn update_cameras(&mut self, cx: &mut Context<Self>) {
        self.camera_info = native_api_backend().and_then(|backend| query(backend).ok());

        if self.active_camera.is_none()
            && let Some(camera_info) = self
                .camera_info
                .as_ref()
                .and_then(|camera_info| camera_info.first())
        {
            self.set_active_camera(camera_info.clone(), cx);
        }

        cx.notify();
    }

    fn set_active_camera(&mut self, camera_info: CameraInfo, cx: &mut Context<Self>) {
        let webcam = cx.new(|cx| Webcam::new(camera_info.clone(), cx));
        self.active_camera = Some(webcam);
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.visible = false;
        self.active_camera = None;
        self.camera_info = None;
        self.call = None;
        cx.notify()
    }
}

impl Render for WebcamStartDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let grant_status = Permissions::grant_status(PermissionType::Camera);
        dialog_box("webcam-start")
            .visible(self.visible)
            .title(tr!("WEBCAM_START_TITLE", "Turn on Camera").into())
            .content(
                div()
                    .flex()
                    .flex_col()
                    .flex_grow()
                    .gap(px(4.))
                    .child(
                        layer()
                            .w(px(600.))
                            .h(px(400.))
                            .when(matches!(grant_status, GrantStatus::Denied), |david| {
                                david
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .p(px(8.))
                                    .child(tr!("AUDIO_SETUP_CAMERA_UNAVAILABLE"))
                            })
                            .when(
                                matches!(
                                    grant_status,
                                    GrantStatus::Granted | GrantStatus::PlatformUnsupported
                                ),
                                |david| {
                                    david.when_else(
                                        self.camera_info
                                            .as_ref()
                                            .map(|camera_info| !camera_info.is_empty())
                                            .unwrap_or(false),
                                        |david| {
                                            david.when_some(
                                                self.active_camera.as_ref(),
                                                |david, camera| {
                                                    let webcam = camera.read(cx);

                                                    david.flex().flex_col().flex_grow().child(
                                                        div()
                                                            .flex()
                                                            .overflow_hidden()
                                                            .when_some(
                                                                webcam.latest_frame().clone(),
                                                                |david, frame| {
                                                                    david.child(
                                                                        img(frame.clone())
                                                                            .object_fit(
                                                                                ObjectFit::Contain,
                                                                            )
                                                                            .size_full(),
                                                                    )
                                                                },
                                                            )
                                                            .when_some(
                                                                webcam.error(),
                                                                |david, error| {
                                                                    david.flex_grow().child(
                                                                        div()
                                                                            .flex()
                                                                            .items_center()
                                                                            .justify_center()
                                                                            .flex_grow()
                                                                            .size_full()
                                                                            .child(icon_text(
                                                                                "exception".into(),
                                                                                tr!(
                                                                                    "CAMERA_SETUP_\
                                                                                    CAMERA_ERROR",
                                                                                )
                                                                                .into(),
                                                                            )),
                                                                    )
                                                                },
                                                            ),
                                                    )
                                                },
                                            )
                                        },
                                        |david| {
                                            david
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .p(px(8.))
                                                .child(tr!("CAMERA_SETUP_NO_CAMERA"))
                                        },
                                    )
                                },
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .child(tr!("WEBCAM_START_CAMERA_SELECTION", "Camera"))
                            .child(div().flex_grow())
                            .when_some(self.active_camera.as_ref(), |david, camera| {
                                let camera = camera.read(cx);
                                david.child(camera.camera_info().human_name())
                            })
                            .when_some(self.camera_info.as_ref(), |david, camera_info| {
                                let camera_menu = camera_info
                                    .iter()
                                    .map(|camera| {
                                        let camera = camera.clone();
                                        ContextMenuItem::menu_item()
                                            .label(camera.human_name())
                                            .on_triggered(cx.listener(move |this, _, _, cx| {
                                                this.set_active_camera(camera.clone(), cx)
                                            }))
                                            .build()
                                    })
                                    .collect::<Vec<_>>();

                                david.child(
                                    button("camera-selection-button")
                                        .child(icon("arrow-down".into()))
                                        .with_menu_open_policy(ButtonMenuOpenPolicy::AnyClick)
                                        .with_menu(camera_menu),
                                )
                            }),
                    ),
            )
            .standard_button(
                StandardButton::Cancel,
                cx.listener(|this, _, _, cx| this.close(cx)),
            )
            .button(
                button("start-button")
                    .child(icon_text(
                        "camera-photo".into(),
                        tr!("WEBCAM_START_BUTTON", "Turn on Camera").into(),
                    ))
                    .when(
                        self.active_camera
                            .as_ref()
                            .is_none_or(|camera| camera.read(cx).error().is_some()),
                        |david| david.disabled(),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        let active_camera = this.active_camera.clone();
                        this.call.as_ref().unwrap().update(cx, |call, cx| {
                            call.set_active_camera(active_camera, cx);
                        });
                        this.close(cx);
                    })),
            )
    }
}
