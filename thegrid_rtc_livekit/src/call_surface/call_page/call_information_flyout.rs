use crate::LivekitCall;
use crate::sfu_session::SfuSessionState;
use cntp_i18n::tr;
use contemporary::components::layer::layer;
use contemporary::components::subtitle::subtitle;
use contemporary::components::tooltip::simple_tooltip;
use contemporary::styling::theme::ThemeStorage;
use gpui::{
    App, Entity, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement, Styled, Window, div, px, rgb,
};
use livekit::ConnectionState;
use thegrid_common::mxc_image::{SizePolicy, mxc_image};
use thegrid_common::session::session_manager::SessionManager;

#[derive(IntoElement)]
pub struct CallInformationFlyout {
    pub call: Entity<LivekitCall>,
}

impl RenderOnce for CallInformationFlyout {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let call = self.call.read(cx);
        let active_call_participants = call.active_call_participants_state.read(cx);
        let master_sfu = call.master_sfu_session.clone();

        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx);
        let user_id = client.user_id().unwrap().to_owned();
        let device_id = client.device_id().unwrap().to_owned();

        div().occlude().child(
            layer().p(px(4.)).child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(subtitle(tr!(
                        "CALL_INFORMATION_FLYOUT_TITLE",
                        "Call Information"
                    )))
                    .child(call.connected_sfu_sessions.iter().fold(
                        div().flex().flex_col().gap(px(4.)),
                        |david, (sfu, session)| {
                            let session = session.read(cx);

                            let mut users_on_call = session
                                .subscribed_streams
                                .iter()
                                .map(|stream| (stream.user_id.clone(), stream.device_id.clone()))
                                .collect::<Vec<_>>();

                            // Always show us as part of the master SFU
                            if master_sfu
                                .as_ref()
                                .is_some_and(|master_sfu| master_sfu == sfu)
                            {
                                users_on_call.insert(0, (user_id.clone(), device_id.clone()))
                            }

                            users_on_call.dedup();

                            david.child(
                            users_on_call
                                .iter()
                                .map(|(user_id, device_id)| {
                                    (
                                        user_id,
                                        device_id,
                                        active_call_participants.iter().find(|participant| {
                                            participant.room_member.user_id() == user_id
                                        }),
                                    )
                                })
                                .fold(
                                    div().flex().flex_col().gap(px(4.)).child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(4.))
                                            .child(
                                                div()
                                                    .flex()
                                                    .size(px(16.))
                                                    .items_center()
                                                    .justify_center()
                                                    .child(div().rounded(px(4.)).size(px(8.)).bg(
                                                        match session.state() {
                                                            SfuSessionState::Connecting => {
                                                                rgb(0xFFC800)
                                                            }
                                                            SfuSessionState::Connected {
                                                                connection_state,
                                                                ..
                                                            } => match connection_state {
                                                                ConnectionState::Disconnected => {
                                                                    rgb(0xFF0000)
                                                                }
                                                                ConnectionState::Connected => {
                                                                    rgb(0x00FF00)
                                                                }
                                                                ConnectionState::Reconnecting => {
                                                                    rgb(0xFFC800)
                                                                }
                                                            },
                                                            SfuSessionState::Error(_) => {
                                                                rgb(0xFF0000)
                                                            }
                                                        },
                                                    )),
                                            )
                                            .child(sfu.clone()),
                                    ),
                                    |david, (user_id, device_id, participant)| {
                                        david.child(
                                            div()
                                                .id(format!(
                                                    "participant_{}_{}",
                                                    user_id, device_id
                                                ))
                                                .flex()
                                                .gap(px(4.))
                                                .items_center()
                                                .child(div().size(px(16.)))
                                                .child(match participant {
                                                    Some(participant) => mxc_image(
                                                        participant.room_member.avatar_url(),
                                                    )
                                                    .fallback_image(
                                                        participant.room_member.user_id(),
                                                    )
                                                    .rounded(theme.border_radius)
                                                    .fixed_square(px(16.))
                                                    .size_policy(SizePolicy::Fit)
                                                    .into_any_element(),
                                                    None => div().size(px(16.)).into_any_element(),
                                                })
                                                .child(match participant {
                                                    Some(participant) => participant
                                                        .room_member
                                                        .display_name()
                                                        .map(|display_name| {
                                                            display_name.to_string()
                                                        })
                                                        .unwrap_or_else(|| user_id.to_string()),
                                                    None => user_id.to_string(),
                                                })
                                                .tooltip(simple_tooltip(format!(
                                                    "{} ({})",
                                                    user_id, device_id
                                                ))),
                                        )
                                    },
                                ),
                        )
                        },
                    )),
            ),
        )
    }
}
