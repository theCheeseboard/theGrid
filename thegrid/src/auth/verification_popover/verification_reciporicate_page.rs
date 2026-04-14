use cntp_i18n::tr;
use contemporary::components::button::button;
use contemporary::components::constrainer::constrainer;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::subtitle::subtitle;
use gpui::{
    App, AsyncApp, ClickEvent, Entity, IntoElement, ParentElement, RenderOnce, Styled, Window, div,
    px,
};
use std::rc::Rc;
use thegrid_common::session::verification_requests_cache::VerificationRequestDetails;
use thegrid_common::tokio_helper::TokioHelper;

#[derive(IntoElement)]
pub struct VerificationReciporicatePage {
    pub verification_request: Option<Entity<VerificationRequestDetails>>,
    pub on_back_click: Rc<Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
}

impl RenderOnce for VerificationReciporicatePage {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let verification_request = self
            .verification_request
            .as_ref()
            .map(|verification_request| verification_request.read(cx).clone());
        let on_back_click = self.on_back_click;

        if let Some(verification_request) = &verification_request
            && let Some(qr_state) = verification_request.qr_state.as_ref()
        {
            div()
                .flex()
                .flex_col()
                .gap(px(9.))
                .child(
                    grandstand("verify-popover-grandstand")
                        .text(tr!("POPOVER_VERIFY"))
                        .on_back_click(move |event, window, cx| on_back_click(event, window, cx)),
                )
                .child(
                    constrainer("verify-popover-constrainer").child(
                        layer()
                            .flex()
                            .flex_col()
                            .p(px(8.))
                            .w_full()
                            .child(subtitle(tr!(
                                "VERIFICATION_POPOVER_RECIPORICATE",
                                "Confirm Verification"
                            )))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(8.))
                                    .child(tr!(
                                        "VERIFICATION_POPOVER_RECIPORICATE_TEXT",
                                        "Was the QR code scanned successfully on the other device?"
                                    ))
                                    .child(
                                        button("verification-popover-ack")
                                            .child(icon_text(
                                                "dialog-ok",
                                                tr!(
                                                    "VERIFICATION_POPOVER_RECIPORICATE_ACK",
                                                    "The QR code was scanned successfully"
                                                ),
                                            ))
                                            .on_click({
                                                let qr_state = qr_state.clone();
                                                move |_, _, cx| {
                                                    let qr_state = qr_state.clone();
                                                    cx.spawn(async move |cx: &mut AsyncApp| {
                                                        let _ = cx
                                                            .spawn_tokio(async move {
                                                                qr_state.confirm().await
                                                            })
                                                            .await;
                                                    })
                                                    .detach();
                                                }
                                            }),
                                    )
                                    .child(
                                        button("verification-popover-nak")
                                            .child(icon_text(
                                                "dialog-cancel",
                                                tr!(
                                                    "VERIFICATION_POPOVER_RECIPORICATE_NAK",
                                                    "The QR code was not scanned successfully"
                                                ),
                                            ))
                                            .on_click({
                                                let qr_state = qr_state.clone();
                                                move |_, _, cx| {
                                                    let qr_state = qr_state.clone();
                                                    cx.spawn(async move |cx: &mut AsyncApp| {
                                                        let _ = cx
                                                            .spawn_tokio(async move {
                                                                qr_state.cancel().await
                                                            })
                                                            .await;
                                                    })
                                                    .detach();
                                                }
                                            }),
                                    ),
                            ),
                    ),
                )
        } else {
            div()
        }
    }
}
