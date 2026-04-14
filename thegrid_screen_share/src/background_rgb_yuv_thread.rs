use gpui::{App, RenderImage};
use libwebrtc::prelude::I422Buffer;
use log::error;
use smallvec::smallvec;
use std::sync::{Arc, Mutex};
use std::thread;
use thegrid_common::outbound_track::RawVideoFrame;
use yuv::{
    BufferStoreMut, YuvConversionMode, YuvPlanarImageMut, YuvRange, YuvStandardMatrix,
    bgra_to_yuv422,
};

pub struct BackgroundRgbYuvThread {
    tx_render_thread: async_channel::Sender<()>,
    rgb_data: Arc<Mutex<(Vec<u8>, u32, u32, Arc<RenderImage>)>>,
}

impl BackgroundRgbYuvThread {
    pub fn new(callback: impl Fn(RawVideoFrame, Arc<RenderImage>) + 'static + Send) -> Self {
        let rgb_data = Arc::new(Mutex::new((
            Vec::new(),
            0,
            0,
            Arc::new(RenderImage::new(smallvec![])),
        )));
        let rgb_data_clone = rgb_data.clone();
        let (tx_render_thread, rx_render_thread) = async_channel::bounded(1);

        thread::spawn(move || {
            let old_resolution = (0, 0);
            let mut buffer = None;
            while smol::block_on(rx_render_thread.recv()).is_ok() {
                let rgb_data_lock = rgb_data.lock().unwrap();
                let (rgb_data, width, height, render_image) = rgb_data_lock.clone();
                drop(rgb_data_lock);

                if old_resolution != (width, height) {
                    buffer = Some(I422Buffer::new(width, height));
                }

                let buffer = buffer.as_mut().unwrap();
                let (stride_y, stride_u, stride_v) = buffer.strides();
                let (buffer_y, buffer_u, buffer_v) = buffer.data_mut();

                let mut planar_image = YuvPlanarImageMut {
                    y_plane: BufferStoreMut::Borrowed(buffer_y),
                    y_stride: stride_y,
                    u_plane: BufferStoreMut::Borrowed(buffer_u),
                    u_stride: stride_u,
                    v_plane: BufferStoreMut::Borrowed(buffer_v),
                    v_stride: stride_v,
                    width,
                    height,
                };

                if let Err(e) = bgra_to_yuv422(
                    &mut planar_image,
                    &rgb_data,
                    width * 4,
                    YuvRange::Limited,
                    YuvStandardMatrix::Bt2020,
                    YuvConversionMode::Balanced,
                ) {
                    error!("Failed to convert RGB to YUV422: {:?}", e);
                } else {
                    callback(
                        RawVideoFrame::YUV422Planar(
                            buffer_y.to_vec(),
                            buffer_u.to_vec(),
                            buffer_v.to_vec(),
                        ),
                        render_image,
                    );
                }
            }
        });

        Self {
            tx_render_thread,
            rgb_data: rgb_data_clone,
        }
    }

    pub fn queue_render(
        &self,
        rgb_data: Vec<u8>,
        width: u32,
        height: u32,
        image: Arc<RenderImage>,
    ) {
        *self.rgb_data.lock().unwrap() = (rgb_data, width, height, image);
        let _ = self.tx_render_thread.try_send(());
    }
}
