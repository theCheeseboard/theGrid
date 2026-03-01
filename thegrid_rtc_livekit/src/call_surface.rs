mod call_page;
mod call_start_page;

use crate::call_manager::LivekitCallManager;
use crate::call_surface::call_page::CallPage;
use crate::call_surface::call_start_page::CallStartPage;
use contemporary::components::pager::fade_animation::FadeAnimation;
use contemporary::components::pager::pager;
use contemporary::surface::surface;
use gpui::{
    App, AppContext, Context, Entity, InteractiveElement, IntoElement, Render, Styled, Window, div,
    rgb,
};
use matrix_sdk::ruma::OwnedRoomId;
use std::rc::Rc;
use thegrid_common::surfaces::{SurfaceChangeEvent, SurfaceChangeHandler};

pub struct CallSurface {
    room_id: OwnedRoomId,
    on_surface_change: Rc<Box<SurfaceChangeHandler>>,

    call_start_page: Entity<CallStartPage>,
}

impl CallSurface {
    pub fn new(
        cx: &mut Context<Self>,
        room_id: OwnedRoomId,
        on_surface_change: impl Fn(&SurfaceChangeEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        let on_surface_change: Rc<Box<SurfaceChangeHandler>> =
            Rc::new(Box::new(move |event, window, cx| {
                on_surface_change(event, window, cx)
            }));
        let start_page =
            cx.new(|cx| CallStartPage::new(room_id.clone(), on_surface_change.clone(), cx));
        Self {
            room_id,
            on_surface_change,
            call_start_page: start_page,
        }
    }
}

impl Render for CallSurface {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let call_manager = cx.global::<LivekitCallManager>();
        let selected_call = call_manager
            .calls()
            .iter()
            .find(|call| call.read(cx).room == self.room_id)
            .cloned();

        let call_page = selected_call.as_ref().map(|_| {
            window.use_state(cx, |window, cx| {
                CallPage::new(self.room_id.clone(), self.on_surface_change.clone(), cx)
            })
        });

        surface()
            .child(
                pager("call-pager", if selected_call.is_none() { 0 } else { 1 })
                    .bg(rgb(0x000000))
                    .size_full()
                    .animation(FadeAnimation::new())
                    .page(self.call_start_page.clone().into_any_element())
                    .page(if let Some(call_page) = call_page {
                        call_page.into_any_element()
                    } else {
                        div().into_any_element()
                    }),
            )
            .into_any_element()
    }
}
