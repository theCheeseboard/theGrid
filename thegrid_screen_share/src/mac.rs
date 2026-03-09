use crate::{PickerRequired, ScreenShareStartEvent};
use async_channel::Sender;
use gpui::http_client::anyhow;
use gpui::{App, AppContext, AsyncApp, AsyncWindowContext, Entity, RenderImage, Window};
use image::{Frame, RgbaImage, imageops};
use log::{error, info};
use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{
    AnyThread, ClassType, DeclaredClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    msg_send_id,
};
use objc2_core_media::{CMSampleBuffer, CMSampleBufferGetSampleAttachmentsArray};
use objc2_core_video::{
    CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow, CVPixelBufferGetHeight,
    CVPixelBufferGetPixelFormatType, CVPixelBufferGetTypeID, CVPixelBufferGetWidth,
    CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags, CVPixelBufferUnlockBaseAddress,
};
use objc2_foundation::NSError;
use objc2_screen_capture_kit::{
    SCContentFilter, SCContentSharingPicker, SCContentSharingPickerConfiguration,
    SCContentSharingPickerMode, SCContentSharingPickerObserver, SCStream, SCStreamConfiguration,
    SCStreamDelegate, SCStreamFrameInfo, SCStreamOutput, SCStreamOutputType,
};
use smallvec::smallvec;
use std::slice;
use std::sync::Arc;
use thegrid_common::video_frame::{RawVideoFrame, VideoFrame};
use yuv::rgb_to_yuv422;

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
        buffer: Vec<u8>,
        width: usize,
        height: usize,
    },
    Quit,
}

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
            info!("SCStream stopped with error: {:?}", error);
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
                    // let Some(attachments) = unsafe {
                    //     CMSampleBufferGetSampleAttachmentsArray(buffer, false)
                    // }.and_then(|array| array.get(0)) else {
                    //     return;
                    // }

                    let (slice, bytes_per_row, width, height, image_buffer) = unsafe {
                        let Some(image_buffer) = buffer.image_buffer() else {
                            return;
                        };
                        CVPixelBufferLockBaseAddress(
                            &image_buffer,
                            CVPixelBufferLockFlags::empty(),
                        );

                        let bytes_per_row = CVPixelBufferGetBytesPerRow(&image_buffer);
                        let width = CVPixelBufferGetWidth(&image_buffer);
                        let height = CVPixelBufferGetHeight(&image_buffer);
                        let base_address = CVPixelBufferGetBaseAddress(&image_buffer);

                        let slice = slice::from_raw_parts::<u8>(
                            std::mem::transmute(base_address),
                            bytes_per_row * height,
                        );

                        (slice, bytes_per_row, width, height, image_buffer)
                    };

                    let buffer = slice.to_vec();

                    unsafe {
                        CVPixelBufferUnlockBaseAddress(
                            &image_buffer,
                            CVPixelBufferLockFlags::empty(),
                        );
                    };

                    let _ = smol::block_on(self.ivars().tx.send(MacScreenShareMessage::Frame {
                        buffer,
                        width,
                        height,
                    }));
                }
                _ => {
                    // ???
                }
            }
            info!(
                "SCStream output sample buffer: {:?}, {:?}",
                buffer, output_type
            );
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

                let stream = SCStream::initWithFilter_configuration_delegate(
                    SCStream::alloc(),
                    &filter,
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
            error!(
                "SCContentSharingPicker start failed with error: {:?}",
                error
            );
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

    window
        .spawn(cx, async move |cx: &mut AsyncWindowContext| {
            let mut weak_frames = None;
            let mut active_stream = None;

            while let Ok(message) = rx.recv().await {
                match message {
                    MacScreenShareMessage::Start { stream } => {
                        let Ok(frames) = cx.update(|window, cx| {
                            let frames = cx.new(|cx| VideoFrame::new((0, 0), cx));

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
                        active_stream = Some(OwnedStream::new(stream));
                    }
                    MacScreenShareMessage::Frame {
                        buffer,
                        width,
                        height,
                    } => {
                        if weak_frames
                            .clone()
                            .unwrap()
                            .update(cx, |frames, cx| {
                                let Some(image) = RgbaImage::from_vec(
                                    width as u32,
                                    height as u32,
                                    buffer.clone(),
                                ) else {
                                    return;
                                };

                                let render_image =
                                    Arc::new(RenderImage::new(smallvec![Frame::new(image)]));
                                frames.set_frame(render_image, RawVideoFrame::RGB(buffer), cx);
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
