use gpui::http_client::anyhow;
use gpui::private::anyhow;
use gpui::{Context, RenderImage};
use image::{imageops, Frame, RgbaImage};
use libwebrtc::prelude::{I422Buffer, VideoRotation};
use log::error;
use ringbuffer::{AllocRingBuffer, RingBuffer};
use smallvec::smallvec;
use std::cell::RefCell;
use std::sync::Arc;
use yuv::{
    bgra_to_yuv422, rgb_to_yuv422, yuyv422_to_bgra, yuyv422_to_yuv422, BufferStoreMut, YuvConversionMode,
    YuvPackedImage, YuvPlanarImage, YuvPlanarImageMut, YuvRange, YuvStandardMatrix,
};

pub struct OutboundTrack {
    video: Option<OutboundTrackVideo>,
    audio: Option<OutboundTrackAudio>,
    status: OutboundTrackStatus,
}

struct OutboundTrackVideo {
    latest_frame_render_image: Option<Arc<RenderImage>>,
    latest_frame_buffer: Option<RawVideoFrame>,
    resolution: (u32, u32),
}

struct OutboundTrackAudio {
    sample_rate: u32,
    channels: u16,
    audio_samples: AllocRingBuffer<i16>,
}

pub enum OutboundTrackStatus {
    Ready,
    Error(anyhow::Error),
    Terminated,
}

pub enum RawVideoFrame {
    YUYV422(Vec<u8>),
    YUV422Planar(Vec<u8>, Vec<u8>, Vec<u8>),
    BGRA(Vec<u8>),
}

pub enum OutputFormat {
    YUV422,
}

impl OutboundTrack {
    pub fn new_video(resolution: (u32, u32), cx: &mut Context<Self>) -> Self {
        Self {
            video: Some(OutboundTrackVideo {
                resolution,
                latest_frame_buffer: None,
                latest_frame_render_image: None,
            }),
            audio: None,
            status: OutboundTrackStatus::Ready,
        }
    }

    pub fn new_audio(sample_rate: u32, channels: u16, cx: &mut Context<Self>) -> Self {
        Self {
            video: None,
            audio: Some(OutboundTrackAudio {
                sample_rate,
                channels,
                audio_samples: AllocRingBuffer::new(16384),
            }),
            status: OutboundTrackStatus::Ready,
        }
    }

    pub fn new_combined(
        resolution: (u32, u32),
        sample_rate: u32,
        channels: u16,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            video: Some(OutboundTrackVideo {
                resolution,
                latest_frame_buffer: None,
                latest_frame_render_image: None,
            }),
            audio: Some(OutboundTrackAudio {
                sample_rate,
                channels,
                audio_samples: AllocRingBuffer::new(16384),
            }),
            status: OutboundTrackStatus::Ready,
        }
    }

    pub fn new_error(e: anyhow::Error, cx: &mut Context<Self>) -> Self {
        Self {
            video: None,
            audio: None,
            status: OutboundTrackStatus::Error(e),
        }
    }

    pub fn set_error(&mut self, e: anyhow::Error, cx: &mut Context<Self>) {
        self.status = OutboundTrackStatus::Error(e);
        self.clear(cx);
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        if let Some(video) = &mut self.video {
            video.latest_frame_render_image = None;
            video.latest_frame_buffer = None;
        }
        cx.notify()
    }

    pub fn set_resolution(&mut self, resolution: (u32, u32), cx: &mut Context<Self>) {
        let video = self
            .video
            .as_mut()
            .expect("Tried to set the resolution of a non-video outbound track");
        video.resolution = resolution;
        self.clear(cx);
    }

    pub fn set_frame(
        &mut self,
        render_image: Arc<RenderImage>,
        buffer: RawVideoFrame,
        cx: &mut Context<Self>,
    ) {
        let video = self
            .video
            .as_mut()
            .expect("Tried to set the video of a non-video outbound track");

        if let Some(old_image) = video.latest_frame_render_image.clone() {
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

        video.latest_frame_render_image = Some(render_image);
        video.latest_frame_buffer = Some(buffer);
        cx.notify()
    }

    pub fn set_terminated(&mut self, cx: &mut Context<Self>) {
        self.status = OutboundTrackStatus::Terminated;
        self.clear(cx);
    }

    pub fn audio_sample_buffer(&mut self) -> &mut AllocRingBuffer<i16> {
        &mut self
            .audio
            .as_mut()
            .expect("Tried to access audio samples of non-audio outbound track")
            .audio_samples
    }

    pub fn latest_render_frame(&self) -> Option<Arc<RenderImage>> {
        self.video
            .as_ref()
            .and_then(|video| video.latest_frame_render_image.clone())
    }

    pub fn i422_frame_buffer(&self) -> Option<I422Buffer> {
        let video = self
            .video
            .as_ref()
            .expect("Tried to set the video of a non-video outbound track");

        let Some(frame_buffer) = &video.latest_frame_buffer else {
            return None;
        };

        let (frame_width, frame_height) = video.resolution;

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
            RawVideoFrame::YUYV422(yuyv_bytes) => {
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
            RawVideoFrame::YUV422Planar(y_bytes, u_bytes, v_bytes) => {
                drop(planar_image);
                buffer_y.copy_from_slice(y_bytes);
                buffer_u.copy_from_slice(u_bytes);
                buffer_v.copy_from_slice(v_bytes);
                Some(buffer)
            }
            RawVideoFrame::BGRA(rgb_bytes) => {
                if let Err(e) = bgra_to_yuv422(
                    &mut planar_image,
                    rgb_bytes,
                    frame_width * 4,
                    YuvRange::Limited,
                    YuvStandardMatrix::Bt2020,
                    YuvConversionMode::Balanced,
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
        let video = self
            .video
            .as_ref()
            .expect("Tried to get resolution of a non-video outbound track");
        video.resolution.0
    }

    pub fn height(&self) -> u32 {
        let video = self
            .video
            .as_ref()
            .expect("Tried to get resolution of a non-video outbound track");
        video.resolution.1
    }

    pub fn resolution(&self) -> (u32, u32) {
        let video = self
            .video
            .as_ref()
            .expect("Tried to get resolution of a non-video outbound track");
        video.resolution
    }

    pub fn sample_rate(&self) -> u32 {
        let audio = self
            .audio
            .as_ref()
            .expect("Tried to get sample rate of a non-audio outbound track");
        audio.sample_rate
    }

    pub fn channels(&self) -> u16 {
        let audio = self
            .audio
            .as_ref()
            .expect("Tried to get channels of a non-audio outbound track");
        audio.channels
    }

    pub fn has_video(&self) -> bool {
        self.video.is_some()
    }

    pub fn has_audio(&self) -> bool {
        self.audio.is_some()
    }

    pub fn status(&self) -> &OutboundTrackStatus {
        &self.status
    }
}
