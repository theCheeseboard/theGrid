use crate::ScreenShareStartEvent;
use crate::background_rgb_yuv_thread::BackgroundRgbYuvThread;
use ashpd::desktop::screencast::{
    CursorMode, Screencast, SelectSourcesOptions, SourceType, Stream, Streams,
};
use ashpd::desktop::{PersistMode, Request, Session};
use ashpd::{WindowIdentifier, WindowIdentifierType};
use cancellation_token::CancellationTokenSource;
use gpui::private::anyhow;
use gpui::{
    App, AppContext, AsyncApp, AsyncWindowContext, Context, RenderImage, WeakEntity, Window,
};
use image::{Frame, RgbaImage};
use libc::flock;
use libwebrtc::prelude::I422Buffer;
use log::{error, info};
use pipewire::context::{ContextBox, ContextRc};
use pipewire::main_loop::{MainLoopBox, MainLoopRc};
use pipewire::properties::PropertiesBox;
use pipewire::spa::param::ParamType;
use pipewire::spa::param::audio::{AudioFormat, AudioInfoRawFlags};
use pipewire::spa::param::format::{FormatProperties, MediaSubtype, MediaType};
use pipewire::spa::param::video::{VideoFormat, VideoInfoRaw};
use pipewire::spa::pod::serialize::PodSerializer;
use pipewire::spa::pod::{Pod, Property, Value, ValueArray};
use pipewire::spa::sys::spa_hook;
use pipewire::spa::utils::{Direction, Fraction, Rectangle, SpaTypes};
use pipewire::spa::{pod, utils};
use pipewire::stream::{StreamBox, StreamFlags, StreamRc, StreamState};
use pipewire::sys::{
    pw_buffer, pw_context, pw_core, pw_properties, pw_proxy, pw_stream, pw_stream_events,
};
use pipewire::types::ObjectType;
use smallvec::smallvec;
use std::cell::RefCell;
use std::mem::ManuallyDrop;
use std::os::fd::OwnedFd;
use std::ptr::null_mut;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{ptr, slice, thread};
use thegrid_common::outbound_track::{OutboundTrack, RawVideoFrame};
use yuv::{
    BufferStoreMut, YuvConversionMode, YuvPlanarImageMut, YuvRange, YuvStandardMatrix,
    bgr_to_yuv422, bgra_to_yuv422,
};

pub struct XdgPortalScreenshareManager {
    tx: async_channel::Sender<XdgPortalScreenshareMessage>,
    is_available: bool,
}

enum XdgPortalScreenshareMessage {
    StartScreenshare {
        response_tx: async_channel::Sender<XdgPortalScreenshareResponse>,
        window_id: u64,
    },
    CloseScreenshare {
        session: Session<Screencast>,
    },
}

enum XdgPortalScreenshareResponse {
    ScreenshareStarted {
        pw_fd: OwnedFd,
        streams: Vec<Stream>,
        session: Session<Screencast>,
    },
}

enum InternalPipewireMessage {
    StreamMeta {
        resolution: (u32, u32),
    },
    StreamData {
        rgb_data: Vec<u8>,
        render_image: Arc<RenderImage>,
    },
    RenderedStreamData {
        frame: RawVideoFrame,
        render_image: Arc<RenderImage>,
    },
    StreamTerminated,
}

enum PipewireMessage {}

impl XdgPortalScreenshareManager {
    pub fn new(cx: &mut Context<Self>) -> Self {
        pipewire::init();

        let (tx, rx) = async_channel::bounded(1);
        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                loop {
                    let Ok(proxy) = Screencast::new().await else {
                        cx.background_executor()
                            .timer(Duration::from_secs(10))
                            .await;
                        continue;
                    };

                    if weak_this
                        .update(cx, |this, cx| {
                            this.is_available = true;
                        })
                        .is_err()
                    {
                        return;
                    }

                    while let Ok(message) = rx.recv().await {
                        match message {
                            XdgPortalScreenshareMessage::StartScreenshare {
                                response_tx,
                                window_id,
                            } => {
                                // let window_id =
                                //     WindowIdentifier::Raw(WindowIdentifierType::Wayland());

                                let session =
                                    proxy.create_session(Default::default()).await.unwrap();
                                if proxy
                                    .select_sources(
                                        &session,
                                        SelectSourcesOptions::default()
                                            .set_cursor_mode(CursorMode::Embedded)
                                            .set_sources(SourceType::Monitor | SourceType::Window)
                                            .set_multiple(false)
                                            .set_persist_mode(PersistMode::DoNot),
                                    )
                                    .await
                                    .is_err()
                                {
                                    error!("Call to select_sources failed");
                                    continue;
                                };

                                // TODO: Provide the parent window
                                let Ok(start_response) =
                                    proxy.start(&session, None, Default::default()).await
                                else {
                                    error!("Call to start screencast failed");
                                    continue;
                                };

                                let Ok(response) = start_response.response() else {
                                    error!("Call to screencast response failed");
                                    continue;
                                };

                                let Ok(pw_fd) = proxy
                                    .open_pipe_wire_remote(&session, Default::default())
                                    .await
                                else {
                                    error!("Call to open pipewire remote failed");
                                    continue;
                                };

                                // TODO: Listen for session closed and use that to determine if the screenshare was stopped by the system

                                let _ = response_tx
                                    .send(XdgPortalScreenshareResponse::ScreenshareStarted {
                                        pw_fd,
                                        streams: response.streams().to_vec(),
                                        session,
                                    })
                                    .await;
                            }
                            XdgPortalScreenshareMessage::CloseScreenshare { session } => {
                                let _ = session.close().await;
                            }
                        }
                    }
                }
            },
        )
        .detach();

        Self {
            tx,
            is_available: false,
        }
    }

    pub fn is_available(&self) -> bool {
        self.is_available
    }

    pub fn start_screen_share_session(
        &mut self,
        callback: impl Fn(&ScreenShareStartEvent, &mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let callback = Rc::new(callback);
        let window_id = window.window_handle().window_id().as_u64();

        let (tx, rx) = async_channel::bounded(1);
        let dbus_tx = self.tx.clone();
        cx.spawn_in(
            window,
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncWindowContext| {
                if dbus_tx
                    .send(XdgPortalScreenshareMessage::StartScreenshare {
                        response_tx: tx,
                        window_id,
                    })
                    .await
                    .is_err()
                {
                    return;
                };

                while let Ok(message) = rx.recv().await {
                    match message {
                        XdgPortalScreenshareResponse::ScreenshareStarted { pw_fd, streams, session } => {
                            let (pw_tx, pw_rx) = pipewire::channel::channel();
                            let (tx_internal, rx_internal) = async_channel::bounded(1);

                            let tx_internal_clone = tx_internal.clone();
                            let bg_thread =
                                BackgroundRgbYuvThread::new(move |frame, render_image| {
                                    let _ = smol::block_on(tx_internal_clone.send(
                                        InternalPipewireMessage::RenderedStreamData {
                                            frame,
                                            render_image,
                                        },
                                    ));
                                });

                            let dbus_tx = dbus_tx.clone();
                            let callback = callback.clone();
                            cx.spawn(async move |cx: &mut AsyncWindowContext| {
                                let mut weak_stream = None;
                                while let Ok(message) = rx_internal.recv().await {
                                    match message {
                                        InternalPipewireMessage::StreamMeta { resolution } => {
                                            if weak_stream.is_some() {
                                                continue;
                                            }

                                            let stream = cx
                                                .update(|window, cx| {
                                                    let stream = cx.new(|cx| {
                                                        OutboundTrack::new_combined(
                                                            resolution, 48000, 2, cx,
                                                        )
                                                    });
                                                    callback(
                                                        &ScreenShareStartEvent {
                                                            frames: stream.clone(),
                                                        },
                                                        window,
                                                        cx,
                                                    );
                                                    stream
                                                })
                                                .unwrap();
                                            weak_stream = Some(stream.downgrade());
                                        }
                                        InternalPipewireMessage::StreamData {
                                            render_image,
                                            rgb_data,
                                        } => {
                                            let Some(weak_stream) = &weak_stream else {
                                                continue;
                                            };

                                            let Ok((width, height)) = weak_stream
                                                .read_with(cx, |stream, cx| stream.resolution()) else {
                                                break;
                                            };

                                            bg_thread.queue_render(
                                                rgb_data,
                                                width,
                                                height,
                                                render_image,
                                            );
                                        }
                                        InternalPipewireMessage::RenderedStreamData {
                                            frame,
                                            render_image,
                                        } => {
                                            let Some(weak_stream) = &weak_stream else {
                                                continue;
                                            };

                                            if weak_stream
                                                .update(cx, |stream, cx| {
                                                    stream.set_frame(render_image, frame, cx);
                                                })
                                                .is_err()
                                            {
                                                break;
                                            }
                                        }
                                        InternalPipewireMessage::StreamTerminated => {
                                            // Kill the stream
                                            break;
                                        }
                                    }
                                }

                                // TODO: Stop pipewire streaming
                                let _ = pw_tx.send(());
                                if let Some(stream) = weak_stream {
                                    let _ = stream.update(cx, |stream, cx| {
                                        stream.set_terminated(cx);
                                    });
                                }

                                let _ = dbus_tx
                                    .send(XdgPortalScreenshareMessage::CloseScreenshare {
                                        session
                                    })
                                    .await;
                            })
                            .detach();

                            thread::spawn(move || {
                                let mainloop = MainLoopRc::new(None).unwrap();
                                let Ok(context) = ContextRc::new(&mainloop, None) else {
                                    error!("Failed to create pipewire context");
                                    return;
                                };

                                let Ok(core) = context.connect_fd_rc(pw_fd, None) else {
                                    error!("Failed to connect pipewire context");
                                    return;
                                };

                                let core_clone = core.clone();
                                let Ok(registry) = core_clone.get_registry() else {
                                    error!("Failed to get pipewire registry");
                                    return;
                                };

                                let streams_holder = RefCell::new(Vec::new());
                                let stream_listeners_holder = RefCell::new(Vec::new());

                                let core_clone = core.clone();
                                let _listener = registry
                                    .add_listener_local()
                                    .global(move |global| {
                                        let resolution = Rc::new(Mutex::new((0, 0)));

                                        if global.type_ == ObjectType::Node
                                            && let Some(stream) = streams.iter().find(|stream| {
                                                stream.pipe_wire_node_id() == global.id
                                            })
                                        {
                                            stream.size();
                                            if let Ok(stream) = StreamRc::new(
                                                core_clone.clone(),
                                                "screenshare",
                                                PropertiesBox::new(),
                                            ) {
                                                let tx_internal_clone = tx_internal.clone();
                                                let tx_internal_clone_2 = tx_internal.clone();
                                                let tx_internal_clone_3 = tx_internal.clone();

                                                let resolution_clone = resolution.clone();
                                                let listener = stream
                                                    .add_local_listener_with_user_data(())
                                                    .param_changed(move |stream, _, id, param| {
                                                        if id != ParamType::Format.as_raw() {
                                                            return;
                                                        }

                                                        let Some(param) = param else {
                                                            return;
                                                        };

                                                        let mut info = VideoInfoRaw::new();
                                                        if info.parse(param).is_err() {
                                                            return;
                                                        }

                                                        *resolution.lock().unwrap() = (info.size().width, info.size().height);

                                                        let _ = smol::block_on(tx_internal_clone.send(InternalPipewireMessage::StreamMeta {
                                                            resolution: (info.size().width, info.size().height),
                                                        }));
                                                    })
                                                    .state_changed(move |stream, _, old, new| {
                                                        if new == StreamState::Paused && old == StreamState::Streaming {
                                                            let _ = smol::block_on(tx_internal_clone_2.send(InternalPipewireMessage::StreamTerminated));
                                                        }
                                                    })
                                                    .process(move |stream, _| {
                                                        while let Some(mut buffer) =
                                                            stream.dequeue_buffer()
                                                        {
                                                            for data in buffer.datas_mut() {
                                                                let Some(buf) = data.data() else {
                                                                    continue;
                                                                };

                                                                let (width, height) = *resolution_clone.lock().unwrap();
                                                                let Some(image) =
                                                                    RgbaImage::from_vec(width, height, buf.to_vec())
                                                                else {
                                                                    continue;
                                                                };

                                                                let render_image = Arc::new(RenderImage::new(smallvec![Frame::new(image)]));

                                                                let _ = smol::block_on(tx_internal_clone_3.send(InternalPipewireMessage::StreamData {
                                                                    rgb_data: buf.to_vec(),
                                                                    render_image,
                                                                }));
                                                            }
                                                        }
                                                    })
                                                    .register()
                                                    .unwrap();

                                                let yuy2_format = build_bgrx_format().unwrap();
                                                let _ = stream.connect(
                                                    Direction::Input,
                                                    Some(global.id),
                                                    StreamFlags::AUTOCONNECT
                                                        | StreamFlags::MAP_BUFFERS
                                                        | StreamFlags::RT_PROCESS,
                                                    &mut [Pod::from_bytes(&yuy2_format).unwrap()],
                                                );

                                                streams_holder.borrow_mut().push(stream);
                                                stream_listeners_holder.borrow_mut().push(listener);
                                            }
                                        }
                                    })
                                    .register();

                                let _receiver = pw_rx.attach(mainloop.loop_(), {
                                    let mainloop = mainloop.clone();
                                    move |_| mainloop.quit()
                                });

                                mainloop.run();
                            });
                        }
                    }
                }
            },
        )
        .detach();
    }
}

fn build_bgrx_format() -> anyhow::Result<Vec<u8>> {
    let mut props = Vec::with_capacity(6);
    props.push(Property::new(
        FormatProperties::MediaType.as_raw(),
        Value::Id(utils::Id(MediaType::Video.as_raw())),
    ));
    props.push(Property::new(
        FormatProperties::MediaSubtype.as_raw(),
        Value::Id(utils::Id(MediaSubtype::Raw.as_raw())),
    ));
    props.push(Property::new(
        FormatProperties::VideoFormat.as_raw(),
        Value::Id(utils::Id(VideoFormat::BGRx.as_raw())),
    ));

    let bytes = PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &Value::Object(pod::Object {
            type_: SpaTypes::ObjectParamFormat.as_raw(),
            id: ParamType::EnumFormat.as_raw(),
            properties: props,
        }),
    )?
    .0
    .into_inner();

    Ok(bytes)
}

macro_rules! declare_forwarded_c_function {
    (
        $lib:literal,
        fn $name:ident ( $( $arg:ident : $arg_ty:ty ),* $(,)? ) $( -> $ret:ty )? ;
    ) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $name( $( $arg : $arg_ty ),* ) $( -> $ret )? {
            unsafe {
                let handle = libc::dlopen(
                    concat!($lib, "\0").as_ptr().cast(),
                    libc::RTLD_NOW | libc::RTLD_NOLOAD,
                );

                if handle.is_null() {
                    panic!(concat!("failed to open already-loaded library: ", $lib));
                }

                let addr = libc::dlsym(
                    handle,
                    concat!(stringify!($name), "\0").as_ptr().cast(),
                );

                if addr.is_null() {
                    libc::dlclose(handle);
                    panic!(concat!("failed to resolve symbol: ", stringify!($name)));
                }

                let func: extern "C" fn( $( $arg_ty ),* ) $( -> $ret )? =
                    std::mem::transmute(addr);

                let result = func( $( $arg ),* );

                libc::dlclose(handle);

                result
            }
        }
    };
}

declare_forwarded_c_function!(
    "libpipewire-0.3.so",
    fn pw_init(argc: *mut ::std::os::raw::c_int, argv: *mut *mut *mut ::std::os::raw::c_char);
);
declare_forwarded_c_function!(
    "libpipewire-0.3.so",
    fn pw_context_new(
        main_loop: *mut pipewire::sys::pw_loop,
        props: *mut pipewire::sys::pw_properties,
        user_data_size: usize,
    ) -> *mut pipewire::sys::pw_context;
);
declare_forwarded_c_function!(
    "libpipewire-0.3.so",
    fn pw_context_connect_fd(
        context: *mut pipewire::sys::pw_context,
        fd: ::std::os::raw::c_int,
        properties: *mut pipewire::sys::pw_properties,
        user_data_size: usize,
    ) -> *mut pw_core;
);
declare_forwarded_c_function!(
    "libpipewire-0.3.so",
    fn pw_context_destroy(context: *mut pipewire::sys::pw_context);
);
declare_forwarded_c_function!(
    "libpipewire-0.3.so",
    fn pw_stream_new(
        core: *mut pipewire::sys::pw_core,
        name: *const ::std::os::raw::c_char,
        props: *mut pipewire::sys::pw_properties,
    ) -> *mut pipewire::sys::pw_stream;
);
declare_forwarded_c_function!(
    "libpipewire-0.3.so",
    fn pw_stream_add_listener(
        stream: *mut pw_stream,
        listener: *mut spa_hook,
        events: *const pw_stream_events,
        data: *mut ::std::os::raw::c_void,
    );
);
declare_forwarded_c_function!(
    "libpipewire-0.3.so",
    fn pw_stream_connect(
        stream: *mut pw_stream,
        direction: pipewire::spa::sys::spa_direction,
        target_id: u32,
        flags: pipewire::sys::pw_stream_flags,
        params: *mut *const pipewire::spa::sys::spa_pod,
        n_params: u32,
    ) -> ::std::os::raw::c_int;
);
declare_forwarded_c_function!(
    "libpipewire-0.3.so",
    fn pw_stream_destroy(stream: *mut pw_stream);
);
declare_forwarded_c_function!(
    "libpipewire-0.3.so",
    fn pw_stream_dequeue_buffer(stream: *mut pw_stream) -> *mut pw_buffer;
);
declare_forwarded_c_function!(
    "libpipewire-0.3.so",
    fn pw_stream_queue_buffer(
        stream: *mut pw_stream,
        buffer: *mut pw_buffer,
    ) -> ::std::os::raw::c_int;
);
declare_forwarded_c_function!(
    "libpipewire-0.3.so",
    fn pw_proxy_destroy(proxy: *mut pw_proxy);
);
declare_forwarded_c_function!(
    "libpipewire-0.3.so",
    fn pw_core_disconnect(core: *mut pw_core) -> ::std::os::raw::c_int;
);
