pub mod fallback_image;

use crate::mxc_image::fallback_image::{FallbackImage, IntoFallbackImage};
use crate::session::media_cache::{MediaCacheEntry, MediaState};
use crate::session::session_manager::SessionManager;
use contemporary::components::icon::icon;
use contemporary::components::skeleton::{SkeletonExt, skeleton};
use gpui::http_client::anyhow;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, BorrowAppContext, ElementId, IntoElement, ObjectFit, ParentElement, Refineable,
    RenderOnce, StyleRefinement, Styled, StyledImage, Window, div, img, px, rgba,
};

#[derive(IntoElement)]
pub struct MxcImage {
    style: StyleRefinement,
    mxc: MediaCacheEntry,
    size_policy: SizePolicy,
    fallback_image: Option<FallbackImage>,
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
        fallback_image: None,
    }
}

impl MxcImage {
    pub fn size_policy(mut self, size_policy: SizePolicy) -> Self {
        self.size_policy = size_policy;
        self
    }

    pub fn fallback_image(mut self, fallback_image: impl IntoFallbackImage) -> Self {
        self.fallback_image = Some(fallback_image.fallback_image());
        self
    }
}

impl RenderOnce for MxcImage {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // To avoid dropping the Arc<RenderImage> while the image is still on the screen,
        // we store it in state. Once a frame passes without this element being rendered,
        // its refcount will decrement and the image will be dropped if there are no other
        // references to it.
        let mxc_url_string = self.mxc.to_string();
        let read_image_store =
            window.use_keyed_state(ElementId::Name(mxc_url_string.into()), cx, |_, _| {
                Err(anyhow!("No image"))
            });

        let (image, is_loading) = match &self.mxc {
            MediaCacheEntry::MediaSource(_) => {
                let image = cx.update_global::<SessionManager, _>(|session_manager, cx| {
                    session_manager.media().media_file(self.mxc.clone(), cx)
                });

                let image_file = image.read(cx);
                let is_failed = matches!(image_file.media_state, MediaState::Failed);
                let is_loading = matches!(image_file.media_state, MediaState::Loading);
                let is_loaded = matches!(image_file.media_state, MediaState::Loaded(_));
                let read_image_to_store = image_file.read_image();
                let read_image = image_file.read_image();
                read_image_store.write(cx, read_image_to_store);

                if is_failed || is_loading {
                    (None, is_loading)
                } else if is_loaded {
                    (read_image.ok(), false)
                } else {
                    (None, false)
                }
            }
            MediaCacheEntry::None => (None, false),
        };

        let mut david = div()
            .when_none(&image, |david| {
                if is_loading {
                    david
                        .bg(rgba(0x00000064))
                        .child(skeleton("image-loading").absolute().size_full())
                } else if let Some(fallback_image) = self.fallback_image {
                    let fallback_image = fallback_image.fallback_image();
                    david.bg(fallback_image.color).child(fallback_image.content)
                } else {
                    david.bg(rgba(0x00000064)).child(icon("exception"))
                }
                .flex()
                .items_center()
                .justify_center()
                .when_some(
                    match self.size_policy {
                        SizePolicy::Constrain(width, height) => Some((width, height)),
                        _ => None,
                    },
                    |david, (width, height)| david.w(px(width)).h(px(height)),
                )
            })
            .when_some(image, |david, image| {
                david
                    .child(
                        img(image.clone())
                            .when_some(self.style.corner_radii.top_left, |david, radius| {
                                david.rounded_tl(radius)
                            })
                            .when_some(self.style.corner_radii.top_right, |david, radius| {
                                david.rounded_tr(radius)
                            })
                            .when_some(self.style.corner_radii.bottom_left, |david, radius| {
                                david.rounded_bl(radius)
                            })
                            .when_some(self.style.corner_radii.bottom_right, |david, radius| {
                                david.rounded_br(radius)
                            })
                            .when(self.size_policy == SizePolicy::Fit, |img| img.size_full())
                            .when_some(
                                match self.size_policy {
                                    SizePolicy::Constrain(width, height) => Some((width, height)),
                                    _ => None,
                                },
                                |img, (width, height)| {
                                    img.w(px(width))
                                        .max_w(px(width))
                                        .h(px(height))
                                        .max_h(px(height))
                                        .object_fit(ObjectFit::Contain)
                                },
                            ),
                    )
                    .when_some(
                        match self.size_policy {
                            SizePolicy::Constrain(width, height) => Some((width, height)),
                            _ => None,
                        },
                        |david, (width, height)| {
                            david
                                .w(px(width))
                                .max_w(px(width))
                                .h(px(height))
                                .max_h(px(height))
                                .overflow_hidden()
                        },
                    )
            });
        david.style().refine(&self.style);
        david
    }
}

impl Styled for MxcImage {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}
