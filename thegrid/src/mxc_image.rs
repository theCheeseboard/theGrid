use contemporary::components::icon::icon;
use contemporary::styling::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, BorrowAppContext, IntoElement, ParentElement, Refineable, RenderOnce, StyleRefinement,
    Styled, Window, div, img, px, rgb, rgba,
};
use gpui::http_client::anyhow;
use thegrid::session::media_cache::{MediaCacheEntry, MediaState};
use thegrid::session::session_manager::SessionManager;

#[derive(IntoElement)]
pub struct MxcImage {
    style: StyleRefinement,
    mxc: MediaCacheEntry,
    size_policy: SizePolicy,
}

#[derive(Clone, Copy, PartialEq)]
pub enum SizePolicy {
    Auto,
    Fit,
    Constrain(f32, f32),
}

pub fn mxc_image(mxc: impl Into<MediaCacheEntry>) -> MxcImage {
    MxcImage {
        style: StyleRefinement::default(),
        mxc: mxc.into(),
        size_policy: SizePolicy::Auto,
    }
}

impl MxcImage {
    pub fn size_policy(mut self, size_policy: SizePolicy) -> Self {
        self.size_policy = size_policy;
        self
    }
}

impl RenderOnce for MxcImage {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // To avoid dropping the Arc<RenderImage> while the image is still on the screen,
        // we store it in state. Once a frame passes without this element being rendered,
        // its refcount will decrement and the image will be dropped if there are no other
        // references to it.
        let read_image_store = window.use_state(cx, |_, _| Err(anyhow!("No image")));

        let image = cx.update_global::<SessionManager, _>(|session_manager, cx| {
            session_manager.media().media_file(self.mxc, cx)
        });

        let image_file = image.read(cx);
        let is_failed = matches!(image_file.media_state, MediaState::Failed);
        let is_loading = matches!(image_file.media_state, MediaState::Loading);
        let is_loaded = matches!(image_file.media_state, MediaState::Loaded(_));
        let read_image_to_store = image_file.read_image();
        let read_image = image_file.read_image();
        read_image_store.write(cx, read_image_to_store);

        let mut david = div()
            .when(
                is_failed,
                |david| {
                    david
                        .bg(rgba(0x00000064))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon("exception".into()))
                },
            )
            .when(
                is_loading,
                |david| david.bg(rgba(0x00000064)),
            )
            .when(
                is_loaded,
                |david| {
                    if let Ok(image) = read_image {
                        david.child(
                            img(image.clone())
                                .when(self.size_policy == SizePolicy::Fit, |img| img.size_full())
                                .when_some(
                                    match self.size_policy {
                                        SizePolicy::Constrain(width, height) => {
                                            Some((width, height))
                                        }
                                        _ => None,
                                    },
                                    |img, dimensions| {
                                        let image_dimensions = image.size(0);

                                        let mut width = image_dimensions.width.0 as f32;
                                        let mut height = image_dimensions.height.0 as f32;
                                        if width > dimensions.0 {
                                            height = height * dimensions.0 / width;
                                            width = dimensions.0;
                                        }
                                        if height > dimensions.1 {
                                            width = width * dimensions.1 / height;
                                            height = dimensions.1;
                                        }
                                        img.w(px(width)).h(px(height))
                                    },
                                ),
                        )
                    } else {
                        david
                            .bg(rgba(0x00000064))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(icon("exception".into()))
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
