use cntp_i18n::{tr, trn};
use contemporary::components::button::button;
use contemporary::components::icon_text::icon_text;
use contemporary::components::subtitle::subtitle;
use contemporary::styling::theme::Theme;
use gpui::{
    App, ClickEvent, InteractiveElement, IntoElement, ParentElement, RenderOnce, Styled, Window,
    div, px, relative,
};
use matrix_sdk::room::RoomMember;
use thegrid_common::mxc_image::{SizePolicy, mxc_image};

#[derive(IntoElement)]
pub struct CallMembersView {
    pub members: Vec<RoomMember>,
    pub start_call: Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>,
}

impl RenderOnce for CallMembersView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.global::<Theme>();
        div()
            .absolute()
            .left_0()
            .top_0()
            .size_full()
            .flex()
            .items_start()
            .justify_end()
            .child(
                div()
                    .id("attachment-list")
                    .rounded(theme.border_radius)
                    .bg(theme.background)
                    .border(px(1.))
                    .border_color(theme.border_color)
                    .occlude()
                    .m(px(8.))
                    .p(px(4.))
                    .gap(px(4.))
                    .w(px(250.))
                    .max_h(relative(0.7))
                    .child(subtitle(tr!("ACTIVE_CALL_TITLE", "Active Call")))
                    .child(
                        self.members
                            .iter()
                            .take(3)
                            .fold(div().flex().gap(px(2.)).items_center(), |david, member| {
                                david.child(
                                    mxc_image(member.avatar_url())
                                        .fallback_image(member.user_id())
                                        .rounded(theme.border_radius)
                                        .size_policy(SizePolicy::Constrain(16., 16.)),
                                )
                            })
                            .child(div().pl(px(4.)).child(trn!(
                                "ACTIVE_CALL_CONTENT",
                                "{{count}} user in this room",
                                "{{count}} users in this room",
                                count = self.members.len() as isize
                            ))),
                    )
                    .child(
                        button("join-call")
                            .child(icon_text("call-start", tr!("JOIN_CALL", "Join Call")))
                            .on_click(self.start_call),
                    ),
            )
    }
}
