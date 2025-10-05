use contemporary::components::subtitle::subtitle;
use contemporary::styling::theme::Theme;
use gpui::{
    AnyElement, App, Div, IntoElement, ParentElement, RenderOnce, StyleRefinement, Styled, Window,
    div, px, rgb, rgba,
};

pub enum AdmonitionSeverity {
    Info,
    Warning,
    Error,
}
#[derive(IntoElement)]
pub struct Admonition {
    severity: AdmonitionSeverity,
    title: String,
    content: Div,
    style_refinement: StyleRefinement,
}

pub fn admonition() -> Admonition {
    Admonition {
        severity: AdmonitionSeverity::Info,
        title: String::new(),
        content: div(),
        style_refinement: StyleRefinement::default(),
    }
}

impl Admonition {
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn severity(mut self, severity: AdmonitionSeverity) -> Self {
        self.severity = severity;
        self
    }
}

impl RenderOnce for Admonition {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.global::<Theme>();

        div()
            .p(px(4.))
            .border(px(1.))
            .border_color(theme.border_color)
            .rounded(theme.border_radius)
            .flex()
            .flex_col()
            .gap(px(4.))
            .bg(match self.severity {
                AdmonitionSeverity::Info => rgba(0x00C8FF10),
                AdmonitionSeverity::Warning => rgba(0xFFC80010),
                AdmonitionSeverity::Error => rgba(0xC8000010),
            })
            .child(subtitle(self.title))
            .child(self.content)
    }
}

impl ParentElement for Admonition {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.content.extend(elements);
    }
}

impl Styled for Admonition {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style_refinement
    }
}
