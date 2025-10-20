use crate::chat::chat_room::open_room::OpenRoom;
use cntp_i18n::tr;
use contemporary::components::admonition::{AdmonitionSeverity, admonition};
use contemporary::components::button::button;
use contemporary::components::icon::icon;
use contemporary::components::layer::layer;
use contemporary::components::subtitle::subtitle;
use contemporary::styling::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, Entity, InteractiveElement, IntoElement, ParentElement, Render, RenderOnce,
    StatefulInteractiveElement, Styled, Window, div, px, relative,
};

#[derive(IntoElement)]
pub struct AttachmentsView {
    pub(crate) open_room: Entity<OpenRoom>,
}

impl RenderOnce for AttachmentsView {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.global::<Theme>();
        let open_room = self.open_room.clone();
        let pending_attachments = &self.open_room.read(cx).pending_attachments;

        div()
            .absolute()
            .left_0()
            .top_0()
            .size_full()
            .flex()
            .items_end()
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
                    .w(px(400.))
                    .max_h(relative(0.7))
                    .overflow_y_scroll()
                    .child(subtitle(tr!("ATTACHMENTS_TITLE", "Attachments")))
                    .child(pending_attachments.iter().enumerate().fold(
                        div().flex().flex_col().gap(px(4.)),
                        |david, (i, attachment)| {
                            let open_room = open_room.clone();
                            david.child(
                                div().id(i).child(
                                    layer()
                                        .flex()
                                        .flex_col()
                                        .p(px(2.))
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .child(attachment.filename.clone())
                                                .child(div().flex_grow())
                                                .child(
                                                    button("delete-button")
                                                        .flat()
                                                        .child(icon("edit-delete".into()))
                                                        .on_click(move |_, _, cx| {
                                                            open_room.update(
                                                                cx,
                                                                |open_room, cx| {
                                                                    open_room
                                                                        .remove_pending_attachment(
                                                                            i, cx,
                                                                        );
                                                                },
                                                            );
                                                        }),
                                                ),
                                        )
                                        .when(attachment.data.is_err(), |david| {
                                            david.child(
                                                admonition()
                                                    .severity(AdmonitionSeverity::Error)
                                                    .title(tr!("ATTACH_ERROR", "Attachment Error"))
                                                    .child(
                                                        attachment
                                                            .data
                                                            .as_ref()
                                                            .unwrap_err()
                                                            .to_string(),
                                                    ),
                                            )
                                        }),
                                ),
                            )
                        },
                    )),
            )
    }
}
