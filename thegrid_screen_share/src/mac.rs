use crate::{PickerRequired, ScreenShareStartEvent};
use async_channel::Sender;
use gpui::http_client::anyhow;
use gpui::{App, AppContext, AsyncApp, AsyncWindowContext, Entity, RenderImage, Window};
use image::{Frame, RgbaImage, imageops};
use libwebrtc::prelude::I422Buffer;
use log::{error, info};
use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{
    AnyThread, ClassType, DeclaredClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    msg_send_id,
};
use objc2_core_media::{CMSampleBuffer, CMSampleBufferGetSampleAttachmentsArray};
use objc2_core_video::{
    CVPixelBufferGetBaseAddress, CVPixelBufferGetBaseAddressOfPlane, CVPixelBufferGetBytesPerRow,
    CVPixelBufferGetBytesPerRowOfPlane, CVPixelBufferGetHeight, CVPixelBufferGetHeightOfPlane,
    CVPixelBufferGetPixelFormatType, CVPixelBufferGetTypeID, CVPixelBufferGetWidth,
    CVPixelBufferGetWidthOfPlane, CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags,
    CVPixelBufferUnlockBaseAddress, kCVPixelFormatType_32BGRA, kCVPixelFormatType_32RGBA,
    kCVPixelFormatType_420YpCbCr8BiPlanarFullRange,
};
use objc2_foundation::NSError;
use objc2_screen_capture_kit::{
    SCCaptureDynamicRange, SCContentFilter, SCContentSharingPicker,
    SCContentSharingPickerConfiguration, SCContentSharingPickerMode,
    SCContentSharingPickerObserver, SCStream, SCStreamConfiguration, SCStreamDelegate,
    SCStreamFrameInfo, SCStreamOutput, SCStreamOutputType,
};
use smallvec::smallvec;
use std::sync::{Arc, Mutex};
use std::{slice, thread};
use thegrid_common::video_frame::{RawVideoFrame, VideoFrame};
use yuv::{
    BufferStoreMut, YuvConversionMode, YuvPlanarImageMut, YuvRange, YuvStandardMatrix,
    bgra_to_yuv422, rgb_to_yuv422,
};

pub struct MacScreenShareSetup {}

impl MacScreenShareSetup {
    fn picker_required(&self) -> PickerRequired {
        PickerRequired::SystemPicker
    }

    fn start_screen_share_session(
        &self,
        callback: Box<dyn Fn(&ScreenShareStartEvent, &mut Window, &mut App)>,
    ) {
    }
}

pub enum MacScreenShareMessage {
    Start {
        stream: Retained<SCStream>,
    },
    Frame {
        frame: Vec<u8>,
        width: usize,
        height: usize,
    },
    RenderedFrame {
        frame: RawVideoFrame,
        render_image: Arc<RenderImage>,
    },
    Quit,
}

unsafe impl Send for MacScreenShareMessage {}

struct RustSCStreamDelegateFields {
    tx: Sender<MacScreenShareMessage>,
}

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = RustSCStreamDelegateFields]
    struct RustSCStreamDelegate;

    unsafe impl NSObjectProtocol for RustSCStreamDelegate {}

    unsafe impl SCStreamDelegate for RustSCStreamDelegate {
        #[unsafe(method(stream:didStopWithError:))]
        fn stream_did_stop_with_error(&self, _stream: &SCStream, error: &NSError) {
            let _ = smol::block_on(self.ivars().tx.send(MacScreenShareMessage::Quit));
        }
    }

    unsafe impl SCStreamOutput for RustSCStreamDelegate {
        #[unsafe(method(stream:didOutputSampleBuffer:ofType:))]
        fn stream_did_output_sample_buffer_of_type(
            &self,
            stream: &SCStream,
            buffer: &CMSampleBuffer,
            output_type: SCStreamOutputType,
        ) {
            match output_type {
                SCStreamOutputType::Screen => {
                    let (frame, width, height) = unsafe {
                        let Some(image_buffer) = buffer.image_buffer() else {
                            return;
                        };
                        CVPixelBufferLockBaseAddress(
                            &image_buffer,
                            CVPixelBufferLockFlags::empty(),
                        );

                        let width = CVPixelBufferGetWidth(&image_buffer);
                        let height = CVPixelBufferGetHeight(&image_buffer);

                        let format = CVPixelBufferGetPixelFormatType(&image_buffer);
                        let frame = match format {
                            kCVPixelFormatType_32BGRA => {
                                let bytes_per_row = CVPixelBufferGetBytesPerRow(&image_buffer);
                                let base_address = CVPixelBufferGetBaseAddress(&image_buffer);
                                let slice = slice::from_raw_parts(
                                    base_address as *const u8,
                                    bytes_per_row * height,
                                );

                                slice.to_vec()
                            }
                            _ => panic!("Unsupported pixel format: {:?}", format),
                        };

                        CVPixelBufferUnlockBaseAddress(
                            &image_buffer,
                            CVPixelBufferLockFlags::empty(),
                        );

                        (frame, width, height)
                    };

                    let _ = smol::block_on(self.ivars().tx.send(MacScreenShareMessage::Frame {
                        frame,
                        width,
                        height,
                    }));
                }
                _ => {
                    // ???
                }
            }
        }
    }

    unsafe impl SCContentSharingPickerObserver for RustSCStreamDelegate {
        #[unsafe(method(contentSharingPicker:didCancelForStream:))]
        unsafe fn content_sharing_picker_did_cancel_for_stream(
            &self,
            picker: &SCContentSharingPicker,
            stream: Option<&SCStream>,
        ) {
            error!("SCContentSharingPicker cancelled");
            let _ = smol::block_on(self.ivars().tx.send(MacScreenShareMessage::Quit));
        }

        #[unsafe(method(contentSharingPicker:didUpdateWithFilter:forStream:))]
        fn content_sharing_picker_did_update_with_filter_for_stream(
            &self,
            picker: &SCContentSharingPicker,
            filter: &SCContentFilter,
            stream: Option<&SCStream>,
        ) {
            info!("SCContentSharingPicker updated with filter: {:?}", filter);

            let stream = unsafe {
                let stream_config = SCStreamConfiguration::new();
                stream_config.setCapturesAudio(true);
                stream_config.setExcludesCurrentProcessAudio(true);
                stream_config.setCaptureMicrophone(false);
                stream_config.setCaptureDynamicRange(SCCaptureDynamicRange::SDR);
                stream_config.setPixelFormat(kCVPixelFormatType_32BGRA);

                let stream = SCStream::initWithFilter_configuration_delegate(
                    SCStream::alloc(),
                    filter,
                    &stream_config,
                    Some(ProtocolObject::from_ref(&*self)),
                );
                stream.addStreamOutput_type_sampleHandlerQueue_error(
                    ProtocolObject::from_ref(&*self),
                    SCStreamOutputType::Screen,
                    None,
                );

                stream.startCaptureWithCompletionHandler(None);

                stream
            };

            let _ = smol::block_on(
                self.ivars()
                    .tx
                    .send(MacScreenShareMessage::Start { stream }),
            );
        }

        #[unsafe(method(contentSharingPickerStartDidFailWithError:))]
        unsafe fn content_sharing_picker_start_did_fail_with_error(&self, error: &NSError) {
        }
    }
);

impl RustSCStreamDelegate {
    fn new(ivars: RustSCStreamDelegateFields) -> Retained<Self> {
        let this = Self::alloc(MainThreadMarker::new().unwrap()).set_ivars(ivars);
        unsafe { msg_send![super(this), init] }
    }
}

struct OwnedStream {
    stream: Retained<SCStream>,
}

impl OwnedStream {
    pub fn new(stream: Retained<SCStream>) -> Self {
        Self { stream }
    }
}

impl Drop for OwnedStream {
    fn drop(&mut self) {
        unsafe {
            self.stream.stopCaptureWithCompletionHandler(None);
        }
    }
}

pub fn start_screen_share_session(
    callback: impl Fn(&ScreenShareStartEvent, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) {
    let (tx, rx) = async_channel::bounded(1);
    let tx_clone = tx.clone();

    unsafe {
        let delegate = RustSCStreamDelegate::new(RustSCStreamDelegateFields { tx });

        let configuration = SCContentSharingPickerConfiguration::new();
        configuration.setAllowedPickerModes(
            SCContentSharingPickerMode::SingleDisplay
                .union(SCContentSharingPickerMode::SingleWindow),
        );

        let picker = SCContentSharingPicker::sharedPicker();
        picker.setDefaultConfiguration(&configuration);
        picker.setActive(true);
        picker.present();
        picker.addObserver(ProtocolObject::from_ref(&*delegate));
    };

    let rgb_data = Arc::new(Mutex::new((
        Vec::new(),
        0,
        0,
        Arc::new(RenderImage::new(smallvec![])),
    )));
    let rgb_data_clone = rgb_data.clone();
    let (tx_render_thread, rx_render_thread) = async_channel::bounded(1);
    thread::spawn(move || {
        while smol::block_on(rx_render_thread.recv()).is_ok() {
            let rgb_data_lock = rgb_data.lock().unwrap();
            let (rgb_data, width, height, render_image) = rgb_data_lock.clone();
            drop(rgb_data_lock);

            let mut buffer = I422Buffer::new(width, height);
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
                if smol::block_on(tx_clone.send(MacScreenShareMessage::RenderedFrame {
                    frame: RawVideoFrame::YUV422Planar(
                        buffer_y.to_vec(),
                        buffer_u.to_vec(),
                        buffer_v.to_vec(),
                    ),
                    render_image,
                }))
                .is_err()
                {
                    return;
                }
            }
        }
    });

    window
        .spawn(cx, async move |cx: &mut AsyncWindowContext| {
            let mut weak_frames = None;
            let mut active_stream = None;

            while let Ok(message) = rx.recv().await {
                match message {
                    MacScreenShareMessage::Start { stream } => {
                        active_stream = Some(OwnedStream::new(stream));
                    }
                    MacScreenShareMessage::Frame {
                        frame,
                        width,
                        height,
                    } => {
                        if weak_frames.is_none() {
                            let Ok(frames) = cx.update(|window, cx| {
                                let frames =
                                    cx.new(|cx| VideoFrame::new((width as u32, height as u32), cx));

                                callback(
                                    &ScreenShareStartEvent {
                                        frames: frames.clone(),
                                    },
                                    window,
                                    cx,
                                );
                                frames.downgrade()
                            }) else {
                                return;
                            };

                            weak_frames = Some(frames);
                        }

                        let Some(image) =
                            RgbaImage::from_vec(width as u32, height as u32, frame.clone())
                        else {
                            return;
                        };

                        let render_image = Arc::new(RenderImage::new(smallvec![Frame::new(image)]));
                        *rgb_data_clone.lock().unwrap() =
                            (frame.clone(), width as u32, height as u32, render_image);
                        let _ = tx_render_thread.try_send(());
                    }
                    MacScreenShareMessage::RenderedFrame {
                        frame,
                        render_image,
                    } => {
                        if weak_frames
                            .clone()
                            .unwrap()
                            .update(cx, |frames, cx| {
                                frames.set_frame(render_image, frame, cx);
                            })
                            .is_err()
                        {
                            return;
                        }
                    }
                    MacScreenShareMessage::Quit => {
                        return;
                    }
                }
            }

            drop(active_stream);
        })
        .detach();
}
