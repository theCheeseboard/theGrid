use crate::call_manager::LivekitCallManager;
use crate::call_surface::call_page::webcam_start_dialog::WebcamStartDialog;
use crate::{CallMember, CallState, LivekitCall, StreamState, TrackType};
use cntp_i18n::tr;
use contemporary::components::anchorer::WithAnchorer;
use contemporary::components::button::button;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon::icon;
use contemporary::components::icon_text::icon_text;
use contemporary::components::interstitial::interstitial;
use contemporary::components::layer::layer;
use contemporary::components::spinner::spinner;
use contemporary::easing::ease_out_cubic;
use contemporary::lerp::Lerpable;
use contemporary::styling::theme::ThemeStorage;
use gpui::prelude::FluentBuilder;
use gpui::{
    Along, App, AppContext, Axis, BorrowAppContext, Bounds, Context, Corner, ElementId, Entity,
    InteractiveElement, IntoElement, ObjectFit, ParentElement, Pixels, Point, Render, RenderOnce,
    StatefulInteractiveElement, Styled, StyledImage, Window, anchored, div, img, px, rgb,
};
use matrix_sdk::ruma::{OwnedDeviceId, OwnedRoomId, OwnedUserId, user_id};
use std::collections::HashMap;
use std::iter;
use std::rc::Rc;
use std::time::Instant;
use thegrid_common::mxc_image::{SizePolicy, mxc_image};
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::surfaces::{SurfaceChange, SurfaceChangeEvent, SurfaceChangeHandler};
use thegrid_screen_share::{PickerRequired, ScreenShareManager, ScreenShareStartEvent};

mod webcam_start_dialog;

#[derive(Clone)]
enum Focus {
    Overview,
    Focus(OwnedUserId, Option<OwnedDeviceId>),
}

#[derive(Clone)]
enum CallMemberAction {
    Focus,
}

pub struct CallPage {
    call: Entity<LivekitCall>,
    room_id: OwnedRoomId,
    on_surface_change: Rc<Box<SurfaceChangeHandler>>,

    webcam_start_dialog: Entity<WebcamStartDialog>,

    animation_start: Instant,
    old_coordinates: HashMap<usize, Bounds<Pixels>>,
    overview_coordinates: HashMap<usize, Bounds<Pixels>>,
    focus_coordinates: HashMap<usize, Bounds<Pixels>>,
    big_focus_coordinates: Bounds<Pixels>,
    old_big_focus_coordinates: Bounds<Pixels>,
    focus: Focus,
}

impl CallPage {
    pub fn new(
        room_id: OwnedRoomId,
        on_surface_change: Rc<Box<SurfaceChangeHandler>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let call_manager = cx.global::<LivekitCallManager>();
        let call = call_manager
            .calls()
            .iter()
            .find(|call| call.read(cx).room == room_id)
            .unwrap()
            .clone();

        let webcam_start_dialog = cx.new(|cx| WebcamStartDialog::new(cx));

        Self {
            call,
            room_id,
            on_surface_change,
            webcam_start_dialog,
            animation_start: Instant::now(),
            old_coordinates: HashMap::new(),
            overview_coordinates: HashMap::new(),
            focus_coordinates: HashMap::new(),
            big_focus_coordinates: Default::default(),
            old_big_focus_coordinates: Default::default(),
            focus: Focus::Overview,
        }
    }

    fn call_member_action(
        &mut self,
        action: &CallMemberAction,
        call_member: CallMember,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match action {
            CallMemberAction::Focus => {
                if matches!(self.focus, Focus::Overview) {
                    self.old_coordinates = self.overview_coordinates.clone();
                    self.old_big_focus_coordinates = Bounds {
                        origin: self
                            .big_focus_coordinates
                            .origin
                            .apply_along(Axis::Vertical, |y| y + window.bounds().size.height),
                        size: self.big_focus_coordinates.size,
                    };
                    self.animation_start = Instant::now();
                }
                self.focus = Focus::Focus(
                    call_member.room_member.user_id().to_owned(),
                    call_member.device_id,
                );
                cx.notify();
            }
        }
    }

    fn return_to_overview(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.focus, Focus::Focus(_, _)) {
            self.old_coordinates = self.focus_coordinates.clone();
            self.old_big_focus_coordinates = self.big_focus_coordinates;
            self.animation_start = Instant::now();
        }
        self.focus = Focus::Overview;
        cx.notify();
    }

    fn screenshare(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.call.read(cx).active_screenshare.is_some() {
            self.call.update(cx, |call, cx| {
                call.set_active_screenshare(None, cx);
            });
        } else {
            cx.update_global::<ScreenShareManager, _>(|screen_share_manager, cx| {
                let listener = cx.listener(|this, event: &ScreenShareStartEvent, _, cx| {
                    this.call.update(cx, |call, cx| {
                        call.set_active_screenshare(Some(event.frames.clone()), cx)
                    });
                });
                screen_share_manager.start_screen_share_session(listener, window, cx);
            });
        }
    }
}

impl Render for CallPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let session_manager = cx.global::<SessionManager>();
        let room = session_manager
            .rooms()
            .read(cx)
            .room(&self.room_id)
            .unwrap()
            .read(cx);
        let room_name = room.display_name().clone();
        let call = self.call.read(cx);
        let call_members = call.call_members().read(cx);

        let call_manager = cx.global::<LivekitCallManager>();
        let mute = call_manager.mute();
        let deaf = call_manager.deaf();

        let (rows, cols) = match call_members.len() {
            1 => (1, 1),
            2 => (1, 2),
            3..=4 => (2, 2),
            5..=6 => (3, 2),
            7..=9 => (3, 3),
            10..=12 => (3, 4),
            // If there are more than 16 people, arrange in a grid of 4 columns
            _ => ((call_members.len() / 4 + 1) as u16, 4),
        };

        let screenshare_manager = cx.global::<ScreenShareManager>();
        let can_screenshare = matches!(
            screenshare_manager.picker_required(),
            PickerRequired::SystemPicker
        );

        let theme = cx.theme();

        div()
            .size_full()
            .bg(rgb(0x000000))
            .flex()
            .flex_col()
            .flex_grow()
            .child(
                grandstand("call-join")
                    .text(room_name)
                    .pt(px(36.))
                    .on_back_click(cx.listener(move |this, _, window, cx| {
                        (this.on_surface_change)(
                            &SurfaceChangeEvent {
                                change: SurfaceChange::Pop,
                            },
                            window,
                            cx,
                        )
                    })),
            )
            .child(
                div()
                    .flex_grow()
                    .flex()
                    .flex_col()
                    .when_else(
                        call.on_hold,
                        |david| {
                            david.child(
                                interstitial()
                                    .flex_grow()
                                    .icon("media-playback-pause".into())
                                    .title(tr!("CALL_ON_HOLD", "This call is on hold").into())
                                    .message(
                                        tr!(
                                            "CALL_ON_HOLD_MESSAGE",
                                            "Take the call off hold to continue talking"
                                        )
                                        .into(),
                                    )
                                    .child(
                                        button("resume-call")
                                            .child(icon_text(
                                                "call-start".into(),
                                                tr!("CALL_TAKE_OFF_HOLD", "Take off hold").into(),
                                            ))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                let call = this.call.clone();
                                                cx.update_global::<LivekitCallManager, _>(
                                                    |call_manager, cx| {
                                                        if call_manager.current_call()
                                                            == Some(call.clone())
                                                        {
                                                            call.update(cx, |call, cx| {
                                                                call.set_on_hold(false, cx);
                                                            })
                                                        } else {
                                                            call_manager
                                                                .switch_to_call(call.clone(), cx);
                                                        }
                                                    },
                                                );
                                            })),
                                    ),
                            )
                        },
                        |david| {
                            david.child(match call.state {
                                CallState::Connecting => div()
                                    .flex_grow()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(spinner().size(px(32.)))
                                    .into_any_element(),
                                CallState::Active { .. } => div()
                                    .flex_grow()
                                    .child(
                                        call_members.iter().enumerate().fold(
                                            div()
                                                .id("overview-area")
                                                .absolute()
                                                .size_full()
                                                .left_0()
                                                .top_0()
                                                .grid()
                                                .grid_rows(rows)
                                                .grid_cols(cols)
                                                .p(px(16.))
                                                .gap(px(16.)),
                                            |david, (i, call_member)| {
                                                let this = cx.entity();
                                                david.child(div().id(i).with_anchorer(
                                                    move |div, bounds, _, cx| {
                                                        this.update(cx, |this, cx| {
                                                            this.overview_coordinates
                                                                .insert(i, bounds);
                                                        });

                                                        div
                                                    },
                                                ))
                                            },
                                        ),
                                    )
                                    .child(
                                        div()
                                            .id("focus-area")
                                            .absolute()
                                            .size_full()
                                            .left_0()
                                            .top_0()
                                            .p(px(16.))
                                            .flex()
                                            .flex_col()
                                            .gap(px(16.))
                                            .child(call_members.iter().enumerate().fold(
                                                div().flex().justify_center().gap(px(16.)),
                                                |david, (i, call_member)| {
                                                    let this = cx.entity();
                                                    david.child(
                                                        div()
                                                            .id(i)
                                                            .w(px(150.))
                                                            .h(px(100.))
                                                            .with_anchorer(
                                                                move |div, bounds, _, cx| {
                                                                    this.update(cx, |this, cx| {
                                                                        this.focus_coordinates
                                                                            .insert(i, bounds);
                                                                    });

                                                                    div
                                                                },
                                                            ),
                                                    )
                                                },
                                            ))
                                            .child({
                                                let this = cx.entity();
                                                div().flex_grow().with_anchorer(
                                                    move |div, bounds, _, cx| {
                                                        this.update(cx, |this, cx| {
                                                            this.big_focus_coordinates = bounds;
                                                        });

                                                        div
                                                    },
                                                )
                                            }),
                                    )
                                    .child(
                                        call_members
                                            .iter()
                                            .cloned()
                                            .enumerate()
                                            .map(|(i, call_member)| (Some(i), Some(call_member)))
                                            .chain(iter::once((
                                                None,
                                                if let Focus::Focus(user_id, device_id) =
                                                    &self.focus
                                                {
                                                    call_members
                                                        .iter()
                                                        .find(|member| {
                                                            member.room_member.user_id() == user_id
                                                                && &member.device_id == device_id
                                                        })
                                                        .cloned()
                                                } else {
                                                    None
                                                },
                                            )))
                                            .fold(
                                                div().absolute().size_full().left_0().top_0(),
                                                |david, (i, call_member)| {
                                                    david.child(CallMemberDisplay {
                                                        animation_start: self.animation_start,
                                                        old_coordinates: self
                                                            .old_coordinates
                                                            .clone(),
                                                        overview_coordinates: self
                                                            .overview_coordinates
                                                            .clone(),
                                                        focus_coordinates: self
                                                            .focus_coordinates
                                                            .clone(),
                                                        call: self.call.clone(),
                                                        call_member: call_member.clone(),
                                                        focus: self.focus.clone(),
                                                        slot: i,
                                                        big_focus_coordinates: self
                                                            .big_focus_coordinates,
                                                        old_big_focus_coordinates: self
                                                            .old_big_focus_coordinates,

                                                        on_action: Box::new(cx.listener(
                                                            move |this, event, window, cx| {
                                                                if let Some(call_member) =
                                                                    call_member.clone()
                                                                {
                                                                    this.call_member_action(
                                                                        &event,
                                                                        call_member,
                                                                        window,
                                                                        cx,
                                                                    );
                                                                }
                                                            },
                                                        )),
                                                    })
                                                },
                                            ),
                                    )
                                    .into_any_element(),
                                CallState::Ended => div().flex_grow().into_any_element(),
                                CallState::Error(error) => interstitial()
                                    .flex_grow()
                                    .icon("call-start".into())
                                    .title(
                                        tr!("CALL_CONNECTION_ERROR", "Unable to connect the call")
                                            .into(),
                                    )
                                    .message(error.to_string().into())
                                    .into_any_element(),
                            })
                        },
                    )
                    .child(
                        div()
                            .flex()
                            .p(px(16.))
                            .child(div().flex_grow())
                            .child(
                                layer()
                                    .border(px(1.))
                                    .border_color(theme.border_color)
                                    .p(px(8.))
                                    .gap(px(8.))
                                    .flex()
                                    .child(
                                        div()
                                            .flex()
                                            .bg(theme.button_background)
                                            .rounded(theme.border_radius)
                                            .when(can_screenshare, |david| {
                                                david.child(
                                                    button("screenshare")
                                                        .p(px(16.))
                                                        .child(icon("display".into()).size(24.))
                                                        .checked_when(
                                                            call.active_screenshare().is_some(),
                                                        )
                                                        .on_click(cx.listener(
                                                            move |this, _, window, cx| {
                                                                this.screenshare(window, cx)
                                                            },
                                                        )),
                                                )
                                            })
                                            .child(
                                                button("camera")
                                                    .p(px(16.))
                                                    .child(icon("camera-photo".into()).size(24.))
                                                    .checked_when(call.active_camera().is_some())
                                                    .on_click(cx.listener(
                                                        move |this, _, window, cx| {
                                                            let call = this.call.read(cx);
                                                            if call.active_camera().is_some() {
                                                                this.call.update(cx, |call, cx| {
                                                                    call.set_active_camera(None, cx)
                                                                });
                                                            } else {
                                                                let call = this.call.clone();
                                                                this.webcam_start_dialog.update(
                                                                    cx,
                                                                    |webcam_start_dialog, cx| {
                                                                        webcam_start_dialog
                                                                            .open(call, window, cx)
                                                                    },
                                                                )
                                                            }
                                                        },
                                                    )),
                                            )
                                            .child(
                                                button("deaf")
                                                    .p(px(16.))
                                                    .child(
                                                        icon(
                                                            if *deaf.read(cx) {
                                                                "headphones"
                                                            } else {
                                                                "headphones"
                                                            }
                                                            .into(),
                                                        )
                                                        .size(24.),
                                                    )
                                                    .checked_when(*deaf.read(cx))
                                                    .on_click(move |_, _, cx| {
                                                        let deafened = *deaf.read(cx);
                                                        deaf.write(cx, !deafened);
                                                    }),
                                            )
                                            .child(
                                                button("mute")
                                                    .p(px(16.))
                                                    .child(
                                                        icon(
                                                            if *mute.read(cx) {
                                                                "mic-off"
                                                            } else {
                                                                "mic-on"
                                                            }
                                                            .into(),
                                                        )
                                                        .size(24.),
                                                    )
                                                    .checked_when(*mute.read(cx))
                                                    .on_click(move |_, _, cx| {
                                                        let muted = *mute.read(cx);
                                                        mute.write(cx, !muted);
                                                    }),
                                            ),
                                    )
                                    .child(
                                        button("hangup-call")
                                            .p(px(16.))
                                            .destructive()
                                            .child(icon("call-stop".into()).size(24.))
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.call.update(cx, |call, cx| {
                                                    call.end_call(cx);
                                                });
                                                (this.on_surface_change)(
                                                    &SurfaceChangeEvent {
                                                        change: SurfaceChange::Pop,
                                                    },
                                                    window,
                                                    cx,
                                                )
                                            })),
                                    ),
                            )
                            .child(div().flex().flex_grow().justify_end().items_center().when(
                                matches!(self.focus, Focus::Focus(_, _)),
                                |david| {
                                    david.child(
                                        button("overview-button")
                                            .child(icon_text(
                                                "view-grid".into(),
                                                tr!("CALL_OVERVIEW", "Back to Overview").into(),
                                            ))
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.return_to_overview(window, cx)
                                            })),
                                    )
                                },
                            )),
                    ),
            )
            .child(self.webcam_start_dialog.clone())
    }
}

#[derive(IntoElement)]
struct CallMemberDisplay {
    call: Entity<LivekitCall>,
    call_member: Option<CallMember>,
    animation_start: Instant,
    old_coordinates: HashMap<usize, Bounds<Pixels>>,
    overview_coordinates: HashMap<usize, Bounds<Pixels>>,
    slot: Option<usize>,
    focus_coordinates: HashMap<usize, Bounds<Pixels>>,
    focus: Focus,

    on_action: Box<dyn Fn(&CallMemberAction, &mut Window, &mut App) + Send + Sync>,
    big_focus_coordinates: Bounds<Pixels>,
    old_big_focus_coordinates: Bounds<Pixels>,
}

impl RenderOnce for CallMemberDisplay {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let retained_call_member = window.use_state(cx, |_, _| self.call_member.clone());
        let call_member = retained_call_member.update(cx, |call_member, _| {
            if self.call_member.is_some() {
                *call_member = self.call_member.clone();
            }

            call_member.clone()
        });

        let Some(call_member) = call_member else {
            return div().absolute().into_any_element();
        };

        let Some(target_bounds) = (match self.slot {
            Some(slot) => match self.focus {
                Focus::Overview => self.overview_coordinates.get(&slot),
                Focus::Focus(_, _) => self.focus_coordinates.get(&slot),
            },
            None => match self.focus {
                Focus::Overview => Some(&Bounds {
                    origin: self
                        .big_focus_coordinates
                        .origin
                        .apply_along(Axis::Vertical, |y| y + window.bounds().size.height),
                    size: self.big_focus_coordinates.size,
                }),
                Focus::Focus(_, _) => Some(&self.big_focus_coordinates),
            },
        }) else {
            return div().absolute().into_any_element();
        };

        let old_bounds = self
            .slot
            .map(|slot| self.old_coordinates.get(&slot).cloned().unwrap_or_default())
            .unwrap_or(self.old_big_focus_coordinates);
        let bounds = old_bounds.lerp(
            target_bounds,
            ease_out_cubic((self.animation_start.elapsed().as_secs_f32() / 0.5).clamp(0., 1.)),
        );

        let profile_picture_size = (bounds.size.width.min(bounds.size.height) * 0.8).min(px(96.));

        let theme = cx.theme();

        let connecting = matches!(call_member.mic_state, StreamState::Unavailable)
            && matches!(call_member.camera_state, StreamState::Unavailable)
            && matches!(call_member.screenshare_state, StreamState::Unavailable);
        let is_muted = matches!(call_member.mic_state, StreamState::Off);

        let call = self.call.read(cx);
        let camera_image = match call_member.camera_state {
            StreamState::On(sid) => {
                if call.our_track_sids.get(&TrackType::Camera).as_ref() == Some(&&sid)
                    && let Some(camera) = call.active_camera()
                {
                    let camera = camera.read(cx);
                    camera.latest_render_frame().clone()
                } else {
                    call.video_stream_images.get(&sid).cloned()
                }
            }
            _ => None,
        };
        let screenshare_image = match call_member.screenshare_state {
            StreamState::On(sid) => {
                if call.our_track_sids.get(&TrackType::Screenshare).as_ref() == Some(&&sid)
                    && let Some(screenshare) = call.active_screenshare()
                {
                    let screenshare = screenshare.read(cx);
                    screenshare.latest_render_frame().clone()
                } else {
                    call.video_stream_images.get(&sid).cloned()
                }
            }
            _ => None,
        };

        let on_action = Rc::new(self.on_action);

        anchored()
            .position(Point::new(px(0.), px(0.)))
            .child(
                div()
                    .absolute()
                    .left(bounds.origin.x)
                    .top(bounds.origin.y)
                    .w(bounds.size.width)
                    .h(bounds.size.height)
                    .id(self
                        .slot
                        .map(|slot| ElementId::from(slot))
                        .unwrap_or(ElementId::Name("big_focus".into())))
                    .bg(if call_member.mic_active {
                        theme.info_accent_color
                    } else {
                        theme.layer_background
                    })
                    .rounded(theme.border_radius)
                    .border(px(1.))
                    .border_color(theme.border_color)
                    .p(px(8.))
                    .when(self.slot.is_some(), |david| david.cursor_pointer())
                    .child(
                        if let Some(camera_frame) =
                            screenshare_image.clone().or(camera_image.clone())
                        {
                            div().flex().size_full().overflow_hidden().child(
                                img(camera_frame.clone())
                                    .object_fit(ObjectFit::Contain)
                                    .size_full(),
                            )
                        } else {
                            div()
                                .flex()
                                .size_full()
                                .items_center()
                                .justify_center()
                                .child(
                                    mxc_image(call_member.room_member.avatar_url())
                                        .rounded(theme.border_radius)
                                        .size(profile_picture_size)
                                        .size_policy(SizePolicy::Fit)
                                        .when(connecting, |david| david.opacity(0.5)),
                                )
                        }
                        .when(screenshare_image.is_some(), |david| {
                            david.when(
                                bounds.size.width >= px(400.) && bounds.size.height >= px(400.),
                                |david| {
                                    david.when_some(camera_image, |david, camera_frame| {
                                        // TODO: Calculate aspect ratio and find out good size for the inside frame
                                        let width = 300.;
                                        let height = 200.;

                                        david.child(
                                            div()
                                                .absolute()
                                                .left_0()
                                                .top_0()
                                                .size_full()
                                                .flex()
                                                .items_start()
                                                .justify_end()
                                                .p(px(16.))
                                                .child(
                                                    layer()
                                                        .flex()
                                                        .border(px(1.))
                                                        .border_color(theme.border_color)
                                                        .w(px(width))
                                                        .h(px(height))
                                                        .child(
                                                            img(camera_frame)
                                                                .object_fit(ObjectFit::Contain)
                                                                .size_full(),
                                                        ),
                                                ),
                                        )
                                    })
                                },
                            )
                        })
                        .child(
                            div()
                                .absolute()
                                .left_0()
                                .top_0()
                                .size_full()
                                .flex()
                                .items_end()
                                .child(
                                    div()
                                        .flex()
                                        .flex_grow()
                                        .child(
                                            call_member
                                                .room_member
                                                .display_name()
                                                .unwrap_or_default()
                                                .to_string(),
                                        )
                                        .child(div().flex_grow())
                                        .when(is_muted, |david| {
                                            david.child(icon("mic-off".into()))
                                        }),
                                ),
                        ),
                    )
                    .on_click(move |_, window, app| {
                        on_action(&CallMemberAction::Focus, window, app);
                    }),
            )
            .into_any_element()
    }
}
