use cancellation_token::CancellationTokenSource;
use gpui::http_client::anyhow;
use gpui::private::anyhow;
use gpui::{AsyncApp, Context, RenderImage, WeakEntity};
use image::{Frame, RgbaImage, imageops};
use log::error;
use nokhwa::pixel_format::{RgbAFormat, YuyvFormat};
use nokhwa::utils::{
    CameraIndex, CameraInfo, FrameFormat, RequestedFormat, RequestedFormatType, Resolution,
};
use nokhwa::{Buffer, CallbackCamera, Camera, NokhwaError};
use smallvec::smallvec;
use std::sync::Arc;
use std::thread;
use yuv::{YuvPackedImage, YuvRange, YuvStandardMatrix, yuyv422_to_bgra};

pub struct Webcam {
    camera_info: CameraInfo,
    latest_frame_render_image: Option<Arc<RenderImage>>,
    latest_frame_buffer: Option<Buffer>,
    error: Option<anyhow::Error>,
    cancellation_source: CancellationTokenSource,
    resolution: Resolution,
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
        let cancellation_source = CancellationTokenSource::new();
        let cancellation_token = cancellation_source.token();

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
                    latest_frame_render_image: None,
                    latest_frame_buffer: None,
                    error: Some(anyhow!(e)),
                    cancellation_source,
                    resolution: Default::default(),
                };
            }
        };

        let resolution = camera.resolution().clone();

        thread::spawn(move || {
            loop {
                if cancellation_token.is_canceled() {
                    return;
                }

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
                let _ = smol::block_on(tx.send(WebcamMessage::Frame {
                    render_image,
                    buffer,
                }));
            }
        });

        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                while let Ok(message) = rx.recv().await {
                    match message {
                        WebcamMessage::Frame {
                            render_image,
                            buffer,
                        } => {
                            if weak_this
                                .update(cx, move |this, cx| {
                                    this.latest_frame_render_image = Some(render_image);
                                    this.latest_frame_buffer = Some(buffer);
                                    cx.notify();
                                })
                                .is_err()
                            {
                                return;
                            }
                        }
                        WebcamMessage::Error(e) => {
                            if weak_this
                                .update(cx, move |this, cx| {
                                    this.latest_frame_render_image = None;
                                    this.latest_frame_buffer = None;
                                    this.error = Some(e);
                                    cx.notify();
                                })
                                .is_err()
                            {}
                        }
                    }
                }
            },
        )
        .detach();

        Self {
            camera_info,
            latest_frame_render_image: None,
            latest_frame_buffer: None,
            error: None,
            cancellation_source,
            resolution,
        }
    }

    pub fn camera_info(&self) -> &CameraInfo {
        &self.camera_info
    }

    pub fn latest_frame(&self) -> Option<Arc<RenderImage>> {
        self.latest_frame_render_image.clone()
    }

    pub fn latest_frame_buffer(&self) -> Option<&Buffer> {
        self.latest_frame_buffer.as_ref()
    }

    pub fn error(&self) -> Option<&anyhow::Error> {
        self.error.as_ref()
    }

    pub fn resolution(&self) -> Resolution {
        self.resolution
    }
}

impl Drop for Webcam {
    fn drop(&mut self) {
        self.cancellation_source.cancel();
    }
}
