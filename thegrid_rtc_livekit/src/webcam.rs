use gpui::http_client::anyhow;
use gpui::private::anyhow;
use gpui::{AppContext, AsyncApp, Context, Entity, RenderImage, WeakEntity};
use image::{Frame, RgbaImage, imageops};
use log::error;
use nokhwa::pixel_format::YuyvFormat;
use nokhwa::utils::{CameraInfo, FrameFormat, RequestedFormat, RequestedFormatType, Resolution};
use nokhwa::{Buffer, Camera, NokhwaError};
use smallvec::smallvec;
use std::sync::Arc;
use std::thread;
use thegrid_common::outbound_track::{OutboundTrack, RawVideoFrame};
use yuv::{YuvPackedImage, YuvRange, YuvStandardMatrix, yuyv422_to_bgra};

pub struct Webcam {
    camera_info: CameraInfo,
    error: Option<anyhow::Error>,
    output_frame: Entity<OutboundTrack>,
}

enum WebcamMessage {
    Frame {
        render_image: Arc<RenderImage>,
        buffer: Buffer,
    },
    Error(anyhow::Error),
}

impl Webcam {
    pub fn new(camera_info: CameraInfo, cx: &mut Context<Self>) -> Self {
        let format =
            RequestedFormat::new::<YuyvFormat>(RequestedFormatType::AbsoluteHighestResolution);

        let (tx, rx) = async_channel::bounded(1);

        let camera = Camera::new(camera_info.index().clone(), format)
            .and_then(|mut camera| camera.open_stream().map(|_| camera));

        let mut camera = match camera {
            Ok(camera) => camera,
            Err(e) => {
                return Self {
                    camera_info,
                    output_frame: cx.new(|cx| OutboundTrack::new_error(anyhow!(e.clone()), cx)),
                    error: Some(anyhow!(e)),
                };
            }
        };

        let resolution = camera.resolution();

        let output_frame =
            cx.new(|cx| OutboundTrack::new_video((resolution.width(), resolution.height()), cx));

        thread::spawn(move || {
            loop {
                let buffer = match camera.frame() {
                    Ok(buffer) => buffer,
                    Err(e) => {
                        let _ = smol::block_on(tx.send(WebcamMessage::Error(anyhow!(e))));
                        return;
                    }
                };

                let frame_data = match buffer.source_frame_format() {
                    FrameFormat::YUYV => {
                        let rgb_buf_size =
                            buffer.resolution().height() * buffer.resolution().width() * 4;
                        let mut dest = vec![0; rgb_buf_size as usize];

                        if let Err(e) = yuyv422_to_bgra(
                            &YuvPackedImage {
                                height: buffer.resolution().height(),
                                width: buffer.resolution().width(),
                                yuy: buffer.buffer(),
                                yuy_stride: buffer.resolution().width() * 2,
                            },
                            &mut dest,
                            buffer.resolution().width() * 4,
                            YuvRange::Full,
                            YuvStandardMatrix::Bt2020,
                        ) {
                            error!("Unable to convert frame from webcam: yuyv to bgra: {:?}", e);
                            let _ = smol::block_on(tx.send(WebcamMessage::Error(anyhow!(e))));
                            return;
                        }

                        dest
                    }
                    _ => {
                        return;
                    }
                };

                let Some(mut image) = RgbaImage::from_vec(
                    buffer.resolution().width(),
                    buffer.resolution().height(),
                    frame_data,
                ) else {
                    let _ = smol::block_on(tx.send(WebcamMessage::Error(anyhow!(
                        "Unable to create ImageBuffer"
                    ))));
                    return;
                };

                // Flip the RenderImage horizontally for display
                imageops::flip_horizontal_in_place(&mut image);
                let render_image = Arc::new(RenderImage::new(smallvec![Frame::new(image)]));
                if smol::block_on(tx.send(WebcamMessage::Frame {
                    render_image,
                    buffer,
                }))
                .is_err()
                {
                    return;
                };
            }
        });

        let weak_output_frame = output_frame.downgrade();
        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                while let Ok(message) = rx.recv().await {
                    match message {
                        WebcamMessage::Frame {
                            render_image,
                            buffer,
                        } => {
                            if weak_output_frame
                                .update(cx, |frame, cx| {
                                    frame.set_frame(
                                        render_image,
                                        match buffer.source_frame_format() {
                                            FrameFormat::YUYV => {
                                                RawVideoFrame::YUYV422(buffer.buffer().to_vec())
                                            }
                                            _ => todo!(),
                                        },
                                        cx,
                                    );
                                })
                                .is_err()
                            {
                                return;
                            };
                        }
                        WebcamMessage::Error(e) => {
                            if weak_output_frame
                                .update(cx, |frame, cx| {
                                    frame.set_error(e, cx);
                                })
                                .is_err()
                            {
                                return;
                            };

                            if weak_this
                                .update(cx, move |this, cx| {
                                    this.error = Some(anyhow!("Error"));
                                    cx.notify();
                                })
                                .is_err()
                            {
                                return;
                            }
                        }
                    }
                }
            },
        )
        .detach();

        Self {
            camera_info,
            output_frame,
            error: None,
        }
    }

    pub fn camera_info(&self) -> &CameraInfo {
        &self.camera_info
    }

    pub fn output_frame(&self) -> Entity<OutboundTrack> {
        self.output_frame.clone()
    }

    pub fn error(&self) -> Option<&anyhow::Error> {
        self.error.as_ref()
    }
}
