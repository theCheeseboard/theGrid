use crate::ScreenShareStartEvent;
use crate::background_rgb_yuv_thread::BackgroundRgbYuvThread;
use gpui::{App, AppContext, AsyncWindowContext, RenderImage, Window};
use image::{Frame, RgbaImage};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use smallvec::smallvec;
use std::ffi::c_void;
use std::slice;
use std::sync::Arc;
use thegrid_common::outbound_track::{OutboundTrack, RawVideoFrame};
use windows::Foundation::TypedEventHandler;
use windows::Graphics::Capture::{
    Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCapturePicker,
};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BOX, D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
    D3D11_CREATE_DEVICE_VIDEO_SUPPORT, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING, D3D11CreateDevice, ID3D11Resource,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::{IDXGIDevice, IDXGISurface};
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::UI::Shell::IInitializeWithWindow;
use windows::core::{Interface, Ref};

enum InternalMessage {
    RenderedStreamData {
        frame: RawVideoFrame,
        render_image: Arc<RenderImage>,
    },
    StreamTerminated,
}

pub fn start_screen_share_session(
    callback: impl Fn(&ScreenShareStartEvent, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) {
    let hwnd = match HasWindowHandle::window_handle(window).unwrap().as_raw() {
        RawWindowHandle::Win32(handle) => handle,
        _ => panic!("Expeted Win32 window handle for Windows platform"),
    }
    .hwnd
    .get() as *mut c_void;

    let gp = GraphicsCapturePicker::new().unwrap();
    unsafe {
        gp.cast::<IInitializeWithWindow>()
            .unwrap()
            .Initialize(HWND(hwnd))
            .unwrap();
    }

    window
        .spawn(cx, async move |cx: &mut AsyncWindowContext| {
            let Ok(graphics_capture_item) = gp.PickSingleItemAsync().unwrap().await else {
                return;
            };

            let mut d3d11_device = None;
            let mut d3d11_context = None;
            unsafe {
                D3D11CreateDevice(
                    None,
                    D3D_DRIVER_TYPE_HARDWARE,
                    Default::default(),
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_VIDEO_SUPPORT,
                    None,
                    D3D11_SDK_VERSION,
                    Some(&mut d3d11_device),
                    None,
                    Some(&mut d3d11_context),
                )
            }
            .unwrap();

            let d3d11_context = d3d11_context.unwrap();
            let d3d11_device = d3d11_device.unwrap();
            let d3d_device = unsafe {
                CreateDirect3D11DeviceFromDXGIDevice(&d3d11_device.cast::<IDXGIDevice>().unwrap())
                    .unwrap()
                    .cast::<IDirect3DDevice>()
                    .unwrap()
            };

            let size = graphics_capture_item.Size().unwrap();

            let texture = unsafe {
                let mut texture = None;
                d3d11_device
                    .CreateTexture2D(
                        &D3D11_TEXTURE2D_DESC {
                            Width: size.Width as u32,
                            Height: size.Height as u32,
                            MipLevels: 0,
                            ArraySize: 1,
                            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                            SampleDesc: DXGI_SAMPLE_DESC {
                                Count: 1,
                                Quality: 0,
                            },
                            Usage: D3D11_USAGE_STAGING,
                            BindFlags: 0,
                            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                            MiscFlags: 0,
                        },
                        None,
                        Some(&mut texture),
                    )
                    .unwrap();
                texture.unwrap()
            };

            let (tx, rx) = async_channel::bounded(1);

            graphics_capture_item
                .Closed(&TypedEventHandler::new({
                    let tx = tx.clone();
                    move |sender: Ref<GraphicsCaptureItem>, _| {
                        let _ = smol::block_on(tx.send(InternalMessage::StreamTerminated));
                        Ok(())
                    }
                }))
                .unwrap();

            let bg_thread = BackgroundRgbYuvThread::new(move |frame, render_image| {
                let _ = smol::block_on(tx.send(InternalMessage::RenderedStreamData {
                    frame,
                    render_image,
                }));
            });

            let capture_frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
                &d3d_device,
                DirectXPixelFormat::B8G8R8A8UIntNormalized,
                2,
                size,
            )
            .unwrap();
            capture_frame_pool
                .FrameArrived(&TypedEventHandler::new(
                    move |sender: Ref<Direct3D11CaptureFramePool>, _| {
                        let next_frame = sender.unwrap().TryGetNextFrame()?;

                        let surface = unsafe {
                            next_frame
                                .Surface()?
                                .cast::<IDirect3DDxgiInterfaceAccess>()?
                                .GetInterface::<IDXGISurface>()?
                        };
                        let bytes = unsafe {
                            d3d11_context.CopySubresourceRegion(
                                &texture,
                                0,
                                0,
                                0,
                                0,
                                &surface.cast::<ID3D11Resource>()?,
                                0,
                                Some(&D3D11_BOX {
                                    left: 0,
                                    top: 0,
                                    front: 0,
                                    right: size.Width as u32,
                                    bottom: size.Height as u32,
                                    back: 1,
                                }),
                            );

                            let mut ptr = D3D11_MAPPED_SUBRESOURCE::default();
                            d3d11_context.Map(&texture, 0, D3D11_MAP_READ, 0, Some(&mut ptr))?;
                            let data_len = size.Width * size.Height * 4;
                            let data = ptr.pData.cast::<u8>();

                            let data_vec = slice::from_raw_parts(data, data_len as usize).to_vec();

                            d3d11_context.Unmap(&texture, 0);

                            data_vec
                        };

                        let Some(image) = RgbaImage::from_vec(
                            size.Width as u32,
                            size.Height as u32,
                            bytes.clone(),
                        ) else {
                            return Ok(());
                        };

                        let render_image = Arc::new(RenderImage::new(smallvec![Frame::new(image)]));
                        bg_thread.queue_render(
                            bytes,
                            size.Width as u32,
                            size.Height as u32,
                            render_image,
                        );

                        Ok(())
                    },
                ))
                .unwrap();

            let session = capture_frame_pool
                .CreateCaptureSession(&graphics_capture_item)
                .unwrap();
            session.SetIsCursorCaptureEnabled(true).unwrap();
            session.SetIsBorderRequired(true).unwrap();
            session.StartCapture().unwrap();

            let outbound_track = cx.new(|cx| {
                OutboundTrack::new_combined((size.Width as u32, size.Height as u32), 48000, 2, cx)
            });

            let weak_outbound_track = outbound_track.downgrade();
            if cx
                .update(|window, cx| {
                    callback(
                        &ScreenShareStartEvent {
                            frames: outbound_track,
                        },
                        window,
                        cx,
                    );
                })
                .is_err()
            {
                return;
            };

            while let Ok(message) = rx.recv().await {
                match message {
                    InternalMessage::RenderedStreamData {
                        frame,
                        render_image,
                    } => {
                        if weak_outbound_track
                            .update(cx, |outbound_track, cx| {
                                outbound_track.set_frame(render_image, frame, cx);
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                    InternalMessage::StreamTerminated => {
                        break;
                    }
                }
            }

            let _ = weak_outbound_track.update(cx, |outbound_track, cx| {
                outbound_track.set_terminated(cx);
            });
            let _ = session.Close();
        })
        .detach();
}
