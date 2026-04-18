use cntp_i18n::tr;
use contemporary::components::button::button;
use contemporary::components::constrainer::constrainer;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::subtitle::subtitle;
use contemporary::styling::theme::ThemeStorage;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, ClickEvent, Entity, InteractiveElement, IntoElement, ParentElement, RenderImage,
    RenderOnce, StatefulInteractiveElement, Styled, Window, div, img, px,
};
use image::{Frame, Rgba};
use matrix_sdk::encryption::verification::QrVerification;
use smallvec::smallvec;
use std::rc::Rc;
use std::sync::Arc;
use thegrid_common::session::verification_requests_cache::VerificationRequestDetails;

#[derive(IntoElement)]
pub struct VerificationSelectPage {
    pub verification_request: Option<Entity<VerificationRequestDetails>>,
    pub on_back_click: Rc<Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
}

impl RenderOnce for VerificationSelectPage {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let verification_request = self
            .verification_request
            .as_ref()
            .map(|verification_request| verification_request.read(cx).clone());
        let on_back_click = self.on_back_click;

        div()
            .flex()
            .flex_col()
            .gap(px(9.))
            .size_full()
            .overflow_hidden()
            .child(
                grandstand("verify-popover-grandstand")
                    .text(tr!("POPOVER_VERIFY"))
                    .on_back_click(move |event, window, cx| on_back_click(event, window, cx)),
            )
            .child(
                div()
                    .id("verify-popover-constrainer-container")
                    .overflow_y_scroll()
                    .child(
                        constrainer("verify-popover-constrainer").child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                .when_some(
                                    verification_request
                                        .as_ref()
                                        .and_then(|request| request.qr_state.clone()),
                                    |david, qr_state| david.child(QrShowLayer { qr_state }),
                                )
                                .child(SasLayer {
                                    verification_request: self.verification_request.clone(),
                                    is_only_verification_method: verification_request
                                        .as_ref()
                                        .is_none_or(|request| request.qr_state.is_none()),
                                }),
                        ),
                    ),
            )
    }
}

#[derive(IntoElement)]
struct QrShowLayer {
    qr_state: QrVerification,
}

impl RenderOnce for QrShowLayer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let qrcode_image = window.use_state(cx, |_, cx| {
            let image = self
                .qr_state
                .to_qr_code()
                .unwrap()
                .render::<Rgba<u8>>()
                .min_dimensions(250, 250)
                .max_dimensions(300, 300)
                .build();

            Arc::new(RenderImage::new(smallvec![Frame::new(image)]))
        });
        let image = qrcode_image.read(cx).clone();

        let theme = cx.theme();

        layer()
            .flex()
            .flex_col()
            .p(px(8.))
            .w_full()
            .child(subtitle(tr!(
                "VERIFICATION_POPOVER_QR_SCAN",
                "Verify with QR Code"
            )))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(tr!(
                        "VERIFICATION_POPOVER_QR_SCAN_TEXT",
                        "Scan this QR code with the other device to continue."
                    ))
                    .child(
                        div()
                            .flex()
                            .justify_center()
                            .child(img(image).rounded(theme.border_radius)),
                    ),
            )
    }
}

#[derive(IntoElement)]
struct SasLayer {
    verification_request: Option<Entity<VerificationRequestDetails>>,
    is_only_verification_method: bool,
}

impl RenderOnce for SasLayer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        layer()
            .flex()
            .flex_col()
            .p(px(8.))
            .w_full()
            .child(subtitle(tr!(
                "VERIFICATION_POPOVER_EMOJI",
                "Verify with Emoji"
            )))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(if self.is_only_verification_method {
                        tr!(
                            "VERIFICATION_POPOVER_EMOJI_ONLY_TEXT",
                            "Verify by comparing emoji on both devices."
                        )
                    } else {
                        tr!(
                            "VERIFICATION_POPOVER_EMOJI_TEXT",
                            "If you can't verify using another method, you can complete \
                            verification by comparing emoji on both devices."
                        )
                    })
                    .child(
                        button("verification-popover-ok")
                            .child(icon_text(
                                "arrow-right",
                                tr!("VERIFICATION_POPOVER_SAS", "Compare Emoji"),
                            ))
                            .on_click({
                                let verification_request = self.verification_request;
                                move |_, _, cx| {
                                    verification_request.clone().unwrap().update(
                                        cx,
                                        |verification_request, cx| {
                                            verification_request.start_sas(cx);
                                        },
                                    );
                                }
                            }),
                    ),
            )
    }
}
