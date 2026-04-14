use cntp_i18n::tr;
use contemporary::components::button::button;
use contemporary::components::dialog_box::{StandardButton, dialog_box};
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::progress_bar::progress_bar;
use contemporary::styling::theme::ThemeStorage;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AsyncApp, AsyncWindowContext, ClickEvent, InteractiveElement, IntoElement, ObjectFit,
    ParentElement, PathPromptOptions, RenderImage, RenderOnce, SharedString, Styled, StyledImage,
    Window, div, img, px,
};
use image::{Frame, ImageReader, Pixel, RgbaImage};
use matrix_sdk::ruma::OwnedMxcUri;
use smallvec::smallvec;
use std::fs;
use std::io::Cursor;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::tokio_helper::TokioHelper;

#[derive(IntoElement)]
pub struct UploadMxcDialog {
    title: SharedString,
    visible: bool,

    accept_button_icon: SharedString,
    accept_button_text: SharedString,

    on_cancel: Rc<Box<dyn Fn(&UploadMxcRejectEvent, &mut Window, &mut App)>>,
    on_accept: Rc<Box<dyn Fn(&UploadMxcAcceptEvent, &mut Window, &mut App)>>,
}

pub struct UploadMxcRejectEvent;
pub struct UploadMxcAcceptEvent {
    pub mxc_url: OwnedMxcUri,
    pub height: u64,
    pub width: u64,
    pub mime_type: String,
    pub blur_hash: Option<String>,
    pub file_size: u64,
}

enum UploadMxcState {
    Idle,
    Selected {
        data: Vec<u8>,
        image: Arc<RenderImage>,
        mime_type: &'static str,
    },
    Uploading {
        image: Arc<RenderImage>,
        progress: usize,
        total_progress: usize,
    },
}

pub fn upload_mxc_dialog(
    title: impl Into<SharedString>,
    visible: bool,
    accept_button_icon: SharedString,
    accept_button_text: SharedString,
    on_cancel: impl Fn(&UploadMxcRejectEvent, &mut Window, &mut App) + 'static,
    on_accept: impl Fn(&UploadMxcAcceptEvent, &mut Window, &mut App) + 'static,
) -> UploadMxcDialog {
    UploadMxcDialog {
        title: title.into(),
        visible,
        accept_button_icon,
        accept_button_text,
        on_cancel: Rc::new(Box::new(on_cancel)),
        on_accept: Rc::new(Box::new(on_accept)),
    }
}

impl RenderOnce for UploadMxcDialog {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let on_cancel = self.on_cancel;
        let on_accept = self.on_accept;

        let state = window.use_state(cx, |_, _| UploadMxcState::Idle);

        let browse_handler = {
            let state = state.clone();
            move |event: &ClickEvent, window: &mut Window, cx: &mut App| {
                let state = state.clone();
                let prompt = cx.prompt_for_paths(PathPromptOptions {
                    files: true,
                    directories: false,
                    multiple: false,
                    prompt: Some(tr!("UPLOAD_MXC_BROWSE_PROMPT", "Upload").into()),
                });

                let session_manager = cx.global::<SessionManager>();
                let client = session_manager.client().unwrap().read(cx).clone();

                cx.spawn(async move |cx: &mut AsyncApp| {
                    let data = match prompt.await {
                        Ok(Ok(Some(paths))) if paths.len() == 1 => fs::read(paths.first().unwrap()),
                        _ => {
                            return;
                        }
                    };

                    let Ok(data) = data else {
                        return;
                    };

                    let Ok(image_reader) =
                        ImageReader::new(Cursor::new(&data)).with_guessed_format()
                    else {
                        return;
                    };

                    let mime_type = image_reader.format().unwrap().to_mime_type();

                    let Ok(image) = image_reader.decode() else {
                        return;
                    };

                    let mut image = image.into();
                    rgb_to_bgr(&mut image);

                    let _ = state.update(cx, |state, cx| {
                        *state = UploadMxcState::Selected {
                            data,
                            image: Arc::new(RenderImage::new(smallvec![Frame::new(image.into())])),
                            mime_type,
                        };
                        cx.notify();
                    });
                })
                .detach();
            }
        };

        let current_state = state.read(cx);
        dialog_box("upload-mxc")
            .visible(self.visible)
            .title(self.title)
            .content(
                div().w(px(300.)).h(px(300.)).child(match current_state {
                    UploadMxcState::Idle => div()
                        .flex()
                        .size_full()
                        .items_center()
                        .justify_center()
                        .child(
                            layer()
                                .flex()
                                .flex_col()
                                .items_center()
                                .p(px(4.))
                                .child(tr!("UPLOAD_MXC_PROMPT", "Choose an image to upload"))
                                .child(
                                    button("upload-button")
                                        .child(icon_text(
                                            "document-open",
                                            tr!("UPLOAD_MXC_BROWSE_BUTTON", "Browse..."),
                                        ))
                                        .on_click(browse_handler),
                                ),
                        ),
                    UploadMxcState::Selected { image, .. }
                    | UploadMxcState::Uploading { image, .. } => div()
                        .flex()
                        .flex_col()
                        .size_full()
                        .items_center()
                        .justify_center()
                        .gap(px(4.))
                        .child(
                            img(image.clone())
                                .size_full()
                                .object_fit(ObjectFit::Contain),
                        )
                        .child(
                            button("upload-button")
                                .child(icon_text(
                                    "document-open",
                                    tr!("UPLOAD_MXC_CHANGE_BUTTON", "Pick another image..."),
                                ))
                                .on_click(browse_handler),
                        )
                        .when_some(
                            match current_state {
                                UploadMxcState::Uploading {
                                    progress,
                                    total_progress,
                                    ..
                                } => Some((progress, total_progress)),
                                _ => None,
                            },
                            |david, (progress, total_progress)| {
                                let theme = cx.theme();

                                let mut color = theme.background;
                                color.a = 0.7;

                                david.child(
                                    div()
                                        .absolute()
                                        .size_full()
                                        .top_0()
                                        .left_0()
                                        .occlude()
                                        .bg(color)
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap(px(4.))
                                                .items_center()
                                                .justify_center()
                                                .size_full()
                                                .child(tr!("UPLOAD_MXC_UPLOADING", "Uploading..."))
                                                .child(progress_bar().w(px(200.)).value(
                                                    *progress as f32 / *total_progress as f32,
                                                )),
                                        ),
                                )
                            },
                        ),
                }),
            )
            .standard_button(StandardButton::Cancel, move |_, window, cx| {
                on_cancel(&UploadMxcRejectEvent, window, cx)
            })
            .button(
                button("upload")
                    .when(
                        matches!(
                            state.read(cx),
                            UploadMxcState::Idle | UploadMxcState::Uploading { .. }
                        ),
                        |david| david.disabled(),
                    )
                    .child(icon_text(self.accept_button_icon, self.accept_button_text))
                    .on_click({
                        let state = state.clone();
                        move |_, window, cx| {
                            let (data, image, mime_type) = match state.read(cx) {
                                UploadMxcState::Selected {
                                    data,
                                    image,
                                    mime_type,
                                } => (data, image, mime_type),
                                _ => panic!("Unexpected state"),
                            };

                            let size = image.size(0);
                            let file_size = data.iter().len();

                            let session_manager = cx.global::<SessionManager>();
                            let client = session_manager.client().unwrap().read(cx).clone();

                            window
                                .spawn(cx, {
                                    let data = data.clone();
                                    let image = image.clone();
                                    let mime_type = *mime_type;
                                    let state = state.clone();
                                    let on_accept = on_accept.clone();
                                    async move |cx: &mut AsyncWindowContext| {
                                        let fut = client.media().upload(
                                            &FromStr::from_str(mime_type).unwrap(),
                                            data.clone(),
                                            None,
                                        );

                                        let mut progress = fut.subscribe_to_send_progress();
                                        cx.spawn({
                                            let state = state.downgrade();
                                            let image = image.clone();
                                            async move |cx: &mut AsyncWindowContext| {
                                                while let Some(progress) = progress.next().await {
                                                    if state
                                                        .update(cx, |state, cx| {
                                                            *state = UploadMxcState::Uploading {
                                                                image: image.clone(),
                                                                progress: 0,
                                                                total_progress: 1,
                                                            }
                                                        })
                                                        .is_err()
                                                    {
                                                        return;
                                                    }
                                                }
                                            }
                                        })
                                        .detach();

                                        match cx.spawn_tokio(async move { fut.await }).await {
                                            Ok(result) => {
                                                let _ = cx.update(|window, cx| {
                                                    on_accept(
                                                        &UploadMxcAcceptEvent {
                                                            mxc_url: result.content_uri,
                                                            blur_hash: result.blurhash,
                                                            width: size.width.0 as u64,
                                                            height: size.width.0 as u64,
                                                            mime_type: mime_type.to_string(),
                                                            file_size: file_size as u64,
                                                        },
                                                        window,
                                                        cx,
                                                    );
                                                });
                                            }
                                            Err(e) => {
                                                // TODO: error
                                                let _ = state.write(
                                                    cx,
                                                    UploadMxcState::Selected {
                                                        data: data.clone(),
                                                        image: image.clone(),
                                                        mime_type,
                                                    },
                                                );
                                            }
                                        }
                                    }
                                })
                                .detach();

                            state.write(
                                cx,
                                UploadMxcState::Uploading {
                                    image: image.clone(),
                                    progress: 0,
                                    total_progress: 1,
                                },
                            );
                        }
                    }),
            )
    }
}

fn rgb_to_bgr(image: &mut RgbaImage) {
    image.pixels_mut().for_each(|v| {
        let slice = v.channels();
        *v = *image::Rgba::from_slice(&[slice[2], slice[1], slice[0], slice[3]]);
    });
}
