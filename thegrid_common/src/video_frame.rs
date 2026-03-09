use gpui::http_client::anyhow;
use gpui::{Context, RenderImage};
use image::{Frame, RgbaImage, imageops};
use libwebrtc::prelude::{I422Buffer, VideoRotation};
use log::error;
use smallvec::smallvec;
use std::cell::RefCell;
use std::sync::Arc;
use yuv::{BufferStoreMut, YuvPackedImage, YuvPlanarImageMut, YuvRange, YuvStandardMatrix, yuyv422_to_bgra, yuyv422_to_yuv422, rgb_to_yuv422, YuvConversionMode};

pub struct VideoFrame {
    latest_frame_render_image: Option<Arc<RenderImage>>,
    latest_frame_buffer: Option<RawVideoFrame>,
    resolution: (u32, u32),
}

pub enum RawVideoFrame {
    YUYV(Vec<u8>),
    RGB(Vec<u8>)
}

pub enum OutputFormat {
    YUV422,
}

impl VideoFrame {
    pub fn new(resolution: (u32, u32), cx: &mut Context<Self>) -> Self {
        Self {
            latest_frame_render_image: None,
            latest_frame_buffer: None,
            resolution,
        }
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.latest_frame_render_image = None;
        self.latest_frame_buffer = None;
        cx.notify()
    }

    pub fn set_resolution(&mut self, resolution: (u32, u32), cx: &mut Context<Self>) {
        self.resolution = resolution;
        self.clear(cx);
    }
    
    pub fn set_frame(
        &mut self,
        render_image: Arc<RenderImage>,
        buffer: RawVideoFrame,
        cx: &mut Context<Self>,
    ) {
        if let Some(old_image) = self.latest_frame_render_image.clone() {
            // Drop this image from all windows
            cx.defer(move |cx| {
                for window in cx.windows() {
                    let image = old_image.clone();
                    let _ = window.update(cx, move |_, window, _| {
                        let _ = window.drop_image(image);
                    });
                }
            });
        }

        self.latest_frame_render_image = Some(render_image);
        self.latest_frame_buffer = Some(buffer);
        cx.notify()
    }

    pub fn latest_render_frame(&self) -> Option<Arc<RenderImage>> {
        self.latest_frame_render_image.clone()
    }

    pub fn i422_frame_buffer(&self) -> Option<I422Buffer> {
        let Some(frame_buffer) = &self.latest_frame_buffer else {
            return None;
        };

        let (frame_width, frame_height) = self.resolution;

        let mut buffer = I422Buffer::new(frame_width, frame_height);
        let (stride_y, stride_u, stride_v) = buffer.strides();
        let (buffer_y, buffer_u, buffer_v) = buffer.data_mut();

        let mut planar_image = YuvPlanarImageMut {
            y_plane: BufferStoreMut::Borrowed(buffer_y),
            y_stride: stride_y,
            u_plane: BufferStoreMut::Borrowed(buffer_u),
            u_stride: stride_u,
            v_plane: BufferStoreMut::Borrowed(buffer_v),
            v_stride: stride_v,
            width: frame_width,
            height: frame_height,
        };

        match frame_buffer {
            RawVideoFrame::YUYV(yuyv_bytes) => {
                if let Err(e) = yuyv422_to_yuv422(
                    &mut planar_image,
                    &YuvPackedImage {
                        height: frame_height,
                        width: frame_width,
                        yuy: yuyv_bytes.as_slice(),
                        yuy_stride: frame_width * 2,
                    },
                ) {
                    error!("Failed to convert YUYV to YUV422: {:?}", e);
                    None
                } else {
                    Some(buffer)
                }
            }
            RawVideoFrame::RGB(rgb_bytes) => {
                if let Err(e) = rgb_to_yuv422(
                    &mut planar_image,
                    rgb_bytes,
                    frame_width * 4,
                    YuvRange::Limited,
                    YuvStandardMatrix::Bt2020,
                    YuvConversionMode::Balanced
                ) {
                    error!("Failed to convert RGB to YUV422: {:?}", e);
                    None
                } else {
                    Some(buffer)
                }
            }
        }
    }

    pub fn width(&self) -> u32 {
        self.resolution.0
    }

    pub fn height(&self) -> u32 {
        self.resolution.1
    }

    pub fn resolution(&self) -> (u32, u32) {
        self.resolution
    }
}
