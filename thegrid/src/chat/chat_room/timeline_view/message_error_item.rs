use contemporary::components::icon::icon;
use contemporary::styling::theme::{ThemeStorage, VariableColor};
use gpui::{App, IntoElement, ParentElement, SharedString, Styled, div, px};

pub fn message_error_item(
    icon_name: impl Into<SharedString>,
    message: impl Into<String>,
    cx: &mut App,
) -> impl IntoElement {
    let theme = cx.theme();

    div()
        .text_color(theme.foreground.disabled())
        .italic()
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(icon(icon_name.into()).foreground(theme.foreground.disabled()))
                .child(message.into()),
        )
}
