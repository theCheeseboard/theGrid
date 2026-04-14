use crate::background_rgb_yuv_thread::BackgroundRgbYuvThread;
use crate::{PickerRequired, ScreenShareStartEvent};
use async_channel::Sender;
use gpui::http_client::anyhow;
use gpui::{App, AppContext, AsyncApp, AsyncWindowContext, Entity, RenderImage, Window};
use image::{Frame, RgbaImage, imageops};
use libwebrtc::prelude::I422Buffer;
use log::{error, info};
use objc2::__macro_helpers::NoneFamily;
use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{
    AnyThread, ClassType, DeclaredClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    msg_send_id,
};
use objc2_avf_audio::{AVAudioFormat, AVAudioPCMBuffer};
use objc2_core_audio_types::AudioBufferList;
use objc2_core_media::{
    CMAudioFormatDescriptionGetStreamBasicDescription, CMBlockBuffer, CMSampleBuffer,
    CMSampleBufferGetSampleAttachmentsArray, kCMSampleBufferError_ArrayTooSmall,
    kCMSampleBufferFlag_AudioBufferList_Assure16ByteAlignment,
};
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
use std::ptr::{NonNull, null, null_mut};
use std::sync::{Arc, Mutex};
use std::{slice, thread};
use thegrid_common::outbound_track::{OutboundTrack, RawVideoFrame};
use yuv::{
    BufferStoreMut, YuvConversionMode, YuvPlanarImageMut, YuvRange, YuvStandardMatrix,
    bgra_to_yuv422, rgb_to_yuv422,
};

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
    AudioFrame {
        samples: Vec<i16>,
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
                SCStreamOutputType::Audio => {
                    info!("Got audio frame");
                    let samples = unsafe {
                        let Some(format_description) = buffer.format_description() else {
                            return;
                        };

                        let format_description =
                            *CMAudioFormatDescriptionGetStreamBasicDescription(&format_description);
                        let mut required_size: usize = 0;
                        buffer.audio_buffer_list_with_retained_block_buffer(
                            &mut required_size,
                            null_mut(),
                            0,
                            None,
                            None,
                            0,
                            &mut null_mut(),
                        );

                        let mut audio_buffer_list_slice =
                            Box::<[u8]>::new_uninit_slice(required_size).assume_init();
                        let mut block_buffer = null_mut();
                        let status = buffer.audio_buffer_list_with_retained_block_buffer(
                            null_mut(),
                            audio_buffer_list_slice.as_mut_ptr() as *mut _,
                            required_size,
                            None,
                            None,
                            kCMSampleBufferFlag_AudioBufferList_Assure16ByteAlignment,
                            &mut block_buffer,
                        );

                        if status != 0 {
                            return;
                        }

                        let Some(format) = AVAudioFormat::initStandardFormatWithSampleRate_channels(
                            AVAudioFormat::alloc(),
                            format_description.mSampleRate,
                            format_description.mChannelsPerFrame,
                        ) else {
                            return;
                        };

                        let Some(buffer) =
                            AVAudioPCMBuffer::initWithPCMFormat_bufferListNoCopy_deallocator(
                                AVAudioPCMBuffer::alloc(),
                                &format,
                                NonNull::new_unchecked(std::mem::transmute::<
                                    *mut u8,
                                    *mut AudioBufferList,
                                >(
                                    audio_buffer_list_slice.as_mut_ptr()
                                )),
                                None,
                            )
                        else {
                            return;
                        };

                        if !buffer.floatChannelData().is_null() {
                            let float_channels = slice::from_raw_parts(
                                buffer.floatChannelData(),
                                buffer.format().channelCount() as usize,
                            );

                            let stride = buffer.stride();
                            (0_usize..buffer.frameLength() as usize)
                                .flat_map(|sample_index| {
                                    float_channels.iter().map(move |channel| {
                                        *channel.as_ptr().add(sample_index * stride)
                                    })
                                })
                                .map(|sample| (sample * i16::MAX as f32) as i16)
                                .collect::<Vec<_>>()
                        } else {
                            return;
                        }
                    };

                    let _ = smol::block_on(
                        self.ivars()
                            .tx
                            .send(MacScreenShareMessage::AudioFrame { samples }),
                    );
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
                stream_config.setSampleRate(48000);
                stream_config.setChannelCount(2);

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
                stream.addStreamOutput_type_sampleHandlerQueue_error(
                    ProtocolObject::from_ref(&*self),
                    SCStreamOutputType::Audio,
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
        unsafe fn content_sharing_picker_start_did_fail_with_error(&self, error: &NSError) {}
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
    let (tx, rx) = async_channel::unbounded();
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

    let bg_thread = BackgroundRgbYuvThread::new(move |frame, render_image| {
        let _ = smol::block_on(tx_clone.send(MacScreenShareMessage::RenderedFrame {
            frame,
            render_image,
        }));
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
                                let frames = cx.new(|cx| {
                                    OutboundTrack::new_combined(
                                        (width as u32, height as u32),
                                        48000,
                                        2,
                                        cx,
                                    )
                                });

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
                        bg_thread.queue_render(frame, width as u32, height as u32, render_image);
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
                    MacScreenShareMessage::AudioFrame { samples } => {
                        if weak_frames
                            .clone()
                            .unwrap()
                            .update(cx, |frames, cx| {
                                let buffer = frames.audio_sample_buffer();
                                buffer.extend(samples);

                                cx.notify();
                            })
                            .is_err()
                        {
                            return;
                        }
                    }
                    MacScreenShareMessage::Quit => {
                        if let Some(weak_frames) = weak_frames.as_ref() {
                            let _ = weak_frames.update(cx, |frames, cx| {
                                frames.set_terminated(cx);
                            });
                        }
                        return;
                    }
                }
            }

            drop(active_stream);
        })
        .detach();
}
