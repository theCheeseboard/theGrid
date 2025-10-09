use gpui::prelude::FluentBuilder;
use gpui::{
    App, BorrowAppContext, IntoElement, ParentElement, Refineable, RenderOnce, StyleRefinement,
    Styled, Window, div, img, rgb, rgba,
};
use matrix_sdk::ruma::OwnedMxcUri;
use thegrid::session::media_cache::MediaState;
use thegrid::session::session_manager::SessionManager;

#[derive(IntoElement)]
pub struct MxcImage {
    style: StyleRefinement,
    mxc: OwnedMxcUri,
}

pub fn mxc_image(mxc: OwnedMxcUri) -> MxcImage {
    MxcImage {
        style: StyleRefinement::default(),
        mxc,
    }
}

impl RenderOnce for MxcImage {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let image = cx.update_global::<SessionManager, _>(|session_manager, cx| {
            session_manager.media().media_file(self.mxc, cx)
        });

        let image_file = image.read(cx);

        let mut david = div()
            .when(
                matches!(image_file.media_state, MediaState::Failed),
                |david| david.bg(rgb(0xFF00FF)),
            )
            .when(
                matches!(image_file.media_state, MediaState::Loading),
                |david| david.bg(rgba(0x000000C0)),
            )
            .when(
                matches!(image_file.media_state, MediaState::Loaded(_)),
                |david| {
                    if let Ok(image) = image_file.read_image() {
                        david.child(img(image).size_full())
                    } else {
                        david.bg(rgb(0xFF00FF))
                    }
                },
            );
        david.style().refine(&self.style);
        david
    }
}

impl Styled for MxcImage {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}
