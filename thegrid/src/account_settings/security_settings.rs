use cntp_i18n::tr;
use contemporary::components::constrainer::constrainer;
use contemporary::components::grandstand::grandstand;
use contemporary::components::layer::layer;
use contemporary::components::subtitle::subtitle;
use contemporary::styling::theme::Theme;
use gpui::{
    App, AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div, px,
};

pub struct SecuritySettings {}

impl SecuritySettings {
    pub fn new(cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self {})
    }
}

impl Render for SecuritySettings {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>();

        div()
            .bg(theme.background)
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .child(
                grandstand("security-grandstand")
                    .text(tr!("ACCOUNT_SETTINGS_SECURITY"))
                    .pt(px(36.)),
            )
            .child(
                constrainer("security")
                    .flex()
                    .flex_col()
                    .w_full()
                    .p(px(8.))
                    .gap(px(8.))
                    .child(
                        layer()
                            .flex()
                            .flex_col()
                            .p(px(8.))
                            .w_full()
                            .child(subtitle(tr!("SECURITY_PROFILE", "Security Profile")))
                            .child(div().child("Coming soon")),
                    ),
            )
    }
}
