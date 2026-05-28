use cntp_i18n::tr;
use contemporary::components::grandstand::grandstand;
use contemporary::components::interstitial::interstitial;
use contemporary::components::layer::layer;
use contemporary::components::pager::lift_animation::LiftAnimation;
use contemporary::components::pager::pager;
use contemporary::components::pager::pager_animation::PagerAnimationDirection;
use contemporary::styling::theme::Theme;
use contemporary::surface::surface;
use gpui::{div, px, uniform_list, App, IntoElement, ParentElement, RenderOnce, Styled, Window};
use thegrid_common::surfaces::{NotReadyReason, SurfaceChange, SurfaceChangeEvent};

#[derive(IntoElement)]
pub struct NotReadySurface {
    reason: NotReadyReason,
}

pub fn not_ready_surface(reason: NotReadyReason) -> NotReadySurface {
    NotReadySurface { reason }
}

impl RenderOnce for NotReadySurface {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        surface()
            .child(
                interstitial()
                    .icon("exception")
                    .title(tr!("NOT_READY_TITLE", "theGrid cannot be used"))
                    .message(self.reason.reason())
                    .size_full(),
            )
            .into_any_element()
    }
}
