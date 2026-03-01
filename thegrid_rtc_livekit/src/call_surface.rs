mod call_page;
mod call_start_page;

use crate::call_manager::LivekitCallManager;
use crate::call_surface::call_page::CallPage;
use crate::call_surface::call_start_page::call_start_page;
use cntp_i18n::tr;
use contemporary::components::grandstand::grandstand;
use contemporary::components::layer::layer;
use contemporary::components::pager::fade_animation::FadeAnimation;
use contemporary::components::pager::lift_animation::LiftAnimation;
use contemporary::components::pager::pager;
use contemporary::components::pager::pager_animation::PagerAnimationDirection;
use contemporary::styling::theme::Theme;
use contemporary::surface::surface;
use gpui::{
    App, Context, Entity, InteractiveElement, IntoElement, Render, Styled, Window, div, px, rgb,
    uniform_list,
};
use matrix_sdk::ruma::OwnedRoomId;
use std::rc::Rc;
use thegrid_common::surfaces::{SurfaceChange, SurfaceChangeEvent, SurfaceChangeHandler};

pub struct CallSurface {
    room_id: OwnedRoomId,
    on_surface_change: Rc<Box<SurfaceChangeHandler>>,
}

impl CallSurface {
    pub fn new(
        cx: &mut Context<Self>,
        room_id: OwnedRoomId,
        on_surface_change: impl Fn(&SurfaceChangeEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        let on_surface_change = Rc::new(Box::new(on_surface_change));
        Self {
            room_id,
            on_surface_change: Rc::new(Box::new(move |event, window, cx| {
                on_surface_change(event, window, cx)
            })),
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
                    .page(
                        call_start_page(self.room_id.clone(), self.on_surface_change.clone())
                            .into_any_element(),
                    )
                    .page(if let Some(call_page) = call_page {
                        call_page.into_any_element()
                    } else {
                        div().into_any_element()
                    }),
            )
            .into_any_element()
    }
}
