use cntp_i18n::tr;
use contemporary::components::button::button;
use contemporary::components::constrainer::constrainer;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::subtitle::subtitle;
use gpui::{
    App, AsyncApp, ClickEvent, Entity, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use std::rc::Rc;
use thegrid_common::sas_emoji::SasEmoji;
use thegrid_common::session::verification_requests_cache::VerificationRequestDetails;
use thegrid_common::tokio_helper::TokioHelper;

#[derive(IntoElement)]
pub struct VerificationSasPage {
    pub verification_request: Option<Entity<VerificationRequestDetails>>,
    pub on_back_click: Rc<Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
}

impl RenderOnce for VerificationSasPage {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let verification_request = self
            .verification_request
            .as_ref()
            .map(|verification_request| verification_request.read(cx).clone());
        let on_back_click = self.on_back_click;

        if let Some(verification_request) = &verification_request
            && let Some(sas_state) = verification_request.sas_state.as_ref()
        {
            let sas_state_clone = sas_state.clone();
            let sas_state_clone_2 = sas_state.clone();

            let peer = verification_request.peer_id.clone();
            let is_self = verification_request.inner.is_self_verification();

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
                    div()
                        .id("verify-popover-constrainer-container")
                        .overflow_y_scroll()
                        .child(
                            constrainer("verify-popover-constrainer").child(
                                if sas_state.supports_emoji()
                                    && let Some(emoji) = sas_state.emoji()
                                {
                                    layer()
                                        .flex()
                                        .flex_col()
                                        .p(px(8.))
                                        .w_full()
                                        .child(subtitle(tr!(
                                            "VERIFICATION_SAS_EMOJI",
                                            "Compare these emoji"
                                        )))
                                        .child(if is_self {
                                            tr!(
                                                "VERIFICATION_SAS_EMOJI_DESCRIPTION",
                                                "Check on the other device and ensure that these \
                                                emoji are displayed, in the same order."
                                            )
                                        } else {
                                            tr!(
                                                "VERIFICATION_SAS_PEER_EMOJI_DESCRIPTION",
                                                "Using a different, trusted form of communication, \
                                                check with {{peer}} and ensure that they see these \
                                                emoji displayed, in the same order.",
                                                peer:quote = peer
                                            )
                                        })
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap(px(8.))
                                                .child(emoji.iter().fold(
                                                    div().grid().grid_cols(7).gap(px(4.)),
                                                    |david, emoji| {
                                                        david.child(
                                                            layer()
                                                                .flex()
                                                                .flex_col()
                                                                .justify_start()
                                                                .items_center()
                                                                .p(px(2.))
                                                                .child(
                                                                    div()
                                                                        .text_size(px(25.))
                                                                        .child(emoji.symbol),
                                                                )
                                                                .child(
                                                                    emoji.translated_description(),
                                                                ),
                                                        )
                                                    },
                                                ))
                                                .child(
                                                    button("verification-popover-ok")
                                                        .child(icon_text(
                                                            "dialog-ok",
                                                            tr!("EMOJI_MATCH", "The emoji match"),
                                                        ))
                                                        .on_click(move |_, _, cx| {
                                                            let sas_state = sas_state_clone.clone();
                                                            cx.spawn(async move |cx| {
                                                                let _ = cx
                                                                    .spawn_tokio(async move {
                                                                        sas_state.confirm().await
                                                                    })
                                                                    .await;
                                                            })
                                                            .detach();
                                                        }),
                                                )
                                                .child(
                                                    button("verification-popover-not-ok")
                                                        .child(icon_text(
                                                            "dialog-cancel",
                                                            tr!(
                                                                "EMOJI_NO_MATCH",
                                                                "The emoji do not match"
                                                            ),
                                                        ))
                                                        .on_click(move |_, _, cx| {
                                                            let sas_state =
                                                                sas_state_clone_2.clone();
                                                            cx.spawn(
                                                                async move |cx: &mut AsyncApp| {
                                                                    let _ = cx
                                                                        .spawn_tokio(async move {
                                                                            sas_state
                                                                                .mismatch()
                                                                                .await
                                                                        })
                                                                        .await;
                                                                },
                                                            )
                                                            .detach();
                                                        }),
                                                ),
                                        )
                                } else if let Some(decimals) = sas_state.decimals() {
                                    layer()
                                        .flex()
                                        .flex_col()
                                        .p(px(8.))
                                        .w_full()
                                        .child(subtitle(tr!(
                                            "VERIFICATION_SAS_DECIMAL",
                                            "Compare these numbers"
                                        )))
                                        .child(if is_self {
                                            tr!(
                                                "VERIFICATION_SAS_DECIMAL_DESCRIPTION",
                                                "Check on the other device and ensure that these \
                                                numbers are displayed, in the same order."
                                            )
                                        } else {
                                            tr!(
                                                "VERIFICATION_SAS_PEER_DECIMAL_DESCRIPTION",
                                                "Using a different, trusted form of communication, \
                                                check with {{peer}} and ensure that they see these \
                                                numbers displayed, in the same order.",
                                                peer:quote = peer
                                            )
                                        })
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap(px(8.))
                                                .child(
                                                    layer()
                                                        .flex()
                                                        .justify_center()
                                                        .text_size(px(35.))
                                                        .p(px(4.))
                                                        .child(format!(
                                                            "{} - {} - {}",
                                                            decimals.0, decimals.1, decimals.2
                                                        )),
                                                )
                                                .child(
                                                    button("verification-popover-ok")
                                                        .child(icon_text(
                                                            "dialog-ok",
                                                            tr!(
                                                                "NUMBERS_MATCH",
                                                                "The numbers match"
                                                            ),
                                                        ))
                                                        .on_click(move |_, _, cx| {
                                                            let sas_state = sas_state_clone.clone();
                                                            cx.spawn(async move |cx| {
                                                                let _ = cx
                                                                    .spawn_tokio(async move {
                                                                        sas_state.confirm().await
                                                                    })
                                                                    .await;
                                                            })
                                                            .detach();
                                                        }),
                                                )
                                                .child(
                                                    button("verification-popover-not-ok")
                                                        .child(icon_text(
                                                            "dialog-cancel",
                                                            tr!(
                                                                "NUMBERS_NO_MATCH",
                                                                "The numbers do not match"
                                                            ),
                                                        ))
                                                        .on_click(move |_, _, cx| {
                                                            let sas_state =
                                                                sas_state_clone_2.clone();
                                                            cx.spawn(
                                                                async move |cx: &mut AsyncApp| {
                                                                    let _ = cx
                                                                        .spawn_tokio(async move {
                                                                            sas_state
                                                                                .mismatch()
                                                                                .await
                                                                        })
                                                                        .await;
                                                                },
                                                            )
                                                            .detach();
                                                        }),
                                                ),
                                        )
                                } else {
                                    layer()
                                }
                                .into_any_element(),
                            ),
                        ),
                )
        } else {
            div()
        }
    }
}
