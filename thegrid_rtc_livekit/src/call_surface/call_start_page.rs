use cntp_i18n::tr;
use contemporary::components::button::button;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon::icon;
use contemporary::components::icon_text::icon_text;
use gpui::{App, Context, IntoElement, ParentElement, Render, RenderOnce, Styled, Window, div, px, rgb, AsyncApp, WeakEntity, BorrowAppContext};
use matrix_sdk::ruma::OwnedRoomId;
use std::rc::Rc;
use contemporary::permissions::{GrantStatus, PermissionType, Permissions};
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::surfaces::SurfaceChangeHandler;
use crate::call_manager::LivekitCallManager;

#[derive(IntoElement)]
pub struct CallStartPage {
    room_id: OwnedRoomId,
    on_surface_change: Rc<Box<SurfaceChangeHandler>>,
}

pub fn call_start_page(
    room_id: OwnedRoomId,
    on_surface_change: Rc<Box<SurfaceChangeHandler>>,
) -> CallStartPage {
    CallStartPage {
        room_id,
        on_surface_change,
    }
}

impl RenderOnce for CallStartPage {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let session_manager = cx.global::<SessionManager>();
        let room = session_manager
            .rooms()
            .read(cx)
            .room(&self.room_id)
            .unwrap()
            .read(cx);
        let room_name = room.display_name().clone();

        div()
            .size_full()
            .bg(rgb(0x000000))
            .flex()
            .flex_col()
            .flex_grow()
            .child(
                grandstand("call-join")
                    .text(
                        tr!("CALL_JOIN_GRANDSTAND", "Join call in {{room}}", room:quote=room_name),
                    )
                    .pt(px(36.))
                    .on_back_click(move |_, window, cx| {
                        (self.on_surface_change)(
                            &thegrid_common::surfaces::SurfaceChangeEvent {
                                change: thegrid_common::surfaces::SurfaceChange::Pop,
                            },
                            window,
                            cx,
                        )
                    }),
            )
            .child(
                div()
                    .flex_grow()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        button("join-call")
                            .child(icon_text(
                                "call-start".into(),
                                tr!("CALL_JOIN_BUTTON", "Join Call").into(),
                            ))
                            .on_click(move |_, _, cx| {
                                start_call(self.room_id.clone(), cx);
                            }),
                    ),
            )
    }
}

fn start_call(room_id: OwnedRoomId, cx: &mut App) {
    cx
        .update_global::<LivekitCallManager, _>(|call_manager, cx| {
            call_manager.start_call(room_id, cx);
        });
    
    // match Permissions::grant_status(PermissionType::Microphone) {
    //     GrantStatus::Granted | GrantStatus::PlatformUnsupported => ,
    //     GrantStatus::Denied => {
    //         self.microphone_access_dialog = true;
    //         cx.notify();
    //     }
    //     GrantStatus::NotDetermined => {
    //         let (tx, rx) = async_channel::bounded(1);
    //         cx.spawn(
    //             async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
    //                 let Ok(success) = rx.recv().await else {
    //                     return;
    //                 };
    // 
    //                 let _ = weak_this.update(cx, |this, cx| {
    //                     if success {
    //                         // Try to start the call again
    //                         this.start_call(cx);
    //                     } else {
    //                         this.microphone_access_dialog = true;
    //                         cx.notify();
    //                     }
    //                 });
    //             },
    //         )
    //         .detach();
    // 
    //         Permissions::request_permission(PermissionType::Microphone, move |success| {
    //             let _ = smol::block_on(tx.send(success));
    //         })
    //     }
    // }
}