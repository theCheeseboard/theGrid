use contemporary::components::icon::icon;
use gpui::prelude::FluentBuilder;
use gpui::{AnyElement, App, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px};

#[derive(IntoElement)]
pub struct StateChangeElement {
    icon: Option<String>,
    text: AnyElement,
}

pub fn state_change_element(icon: Option<String>, text: impl IntoElement) -> StateChangeElement {
    StateChangeElement {
        icon,
        text: text.into_any_element(),
    }
}

impl RenderOnce for StateChangeElement {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .w_full()
            .max_w_full()
            .gap(px(8.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .min_w(px(40.))
                    .mx(px(2.))
                    .child(div().flex_grow())
                    .when_some(self.icon, |david, icon_name| {
                        david.child(icon(icon_name.into()))
                    }),
            )
            .child(div().w_full().max_w_full().child(self.text))
            .into_any_element()
    }
}
