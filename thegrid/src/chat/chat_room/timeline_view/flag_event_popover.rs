use crate::chat::chat_room::open_room::OpenRoom;
use cntp_i18n::{tr, I18nString};
use contemporary::components::admonition::{admonition, AdmonitionSeverity};
use contemporary::components::button::button;
use contemporary::components::checkbox::radio_button;
use contemporary::components::constrainer::constrainer;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::pager::pager;
use contemporary::components::pager::slide_horizontal_animation::SlideHorizontalAnimation;
use contemporary::components::popover::popover;
use contemporary::components::spinner::spinner;
use contemporary::components::subtitle::subtitle;
use contemporary::components::text_field::TextField;
use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, AppContext, AsyncApp, Context, ElementId, Entity, IntoElement, ParentElement,
    Render, Styled, WeakEntity, Window,
};
use matrix_sdk_ui::timeline::EventTimelineItem;
use thegrid_common::tokio_helper::TokioHelper;

pub struct FlagEventPopover {
    visible: bool,
    state: FlagEventPopoverState,
    reason: Option<FlagReason>,

    room: Entity<OpenRoom>,
    event: Option<EventTimelineItem>,

    custom_reason_field: Entity<TextField>,
}

enum FlagEventPopoverState {
    Reason,
    Confirm,
    Loading,
    Complete,
}

#[derive(Clone, PartialEq, Eq)]
enum FlagReason {
    Spam,
    Violent,
    SexuallyExplicit,
    Custom(String),
}

impl FlagReason {
    pub fn id(&self) -> ElementId {
        match self {
            FlagReason::Spam => "flag-reason-spam",
            FlagReason::Violent => "flag-reason-violent",
            FlagReason::SexuallyExplicit => "flag-reason-sexually-explicit",
            FlagReason::Custom(_) => "flag-reason-custom",
        }
        .into()
    }

    pub fn human_readable_string(&self) -> I18nString {
        match self {
            FlagReason::Spam => {
                tr!("FLAG_REASON_SPAM", "Spam")
            }
            FlagReason::Violent => {
                tr!("FLAG_REASON_VIOLENT", "Violent")
            }
            FlagReason::SexuallyExplicit => {
                tr!("FLAG_REASON_SEXUALLY_EXPLICIT", "Sexually Explicit")
            }
            FlagReason::Custom(_) => {
                tr!("FLAG_REASON_CUSTOM", "Other reason")
            }
        }
    }

    pub fn report_string(&self) -> &str {
        match self {
            FlagReason::Spam => "This message contains spam",
            FlagReason::Violent => "This message contains violence",
            FlagReason::SexuallyExplicit => "This message contains sexually explicit content",
            FlagReason::Custom(reason) => reason,
        }
    }
}

impl FlagEventPopoverState {
    pub fn page(&self) -> usize {
        match self {
            FlagEventPopoverState::Reason => 0,
            FlagEventPopoverState::Confirm => 1,
            FlagEventPopoverState::Loading => 2,
            FlagEventPopoverState::Complete => 3,
        }
    }
}

impl FlagEventPopover {
    pub fn new(room: Entity<OpenRoom>, cx: &mut Context<Self>) -> Self {
        let text_changed_listener = cx.listener(|_, _, window, cx| {
            cx.defer_in(window, |this, _, cx| {
                let text = this.custom_reason_field.read(cx).text();
                this.reason = Some(FlagReason::Custom(text.to_string()));
                cx.notify();
            });
        });
        cx.observe_self(|this, cx| {
            let reason = this
                .reason
                .as_ref()
                .map(|reason| match reason {
                    FlagReason::Custom(reason) => reason,
                    _ => "",
                })
                .unwrap_or_default();

            this.custom_reason_field.update(cx, |field, cx| {
                if field.text() != reason {
                    field.set_text(reason);
                    cx.notify();
                }
            });
        })
        .detach();

        Self {
            visible: false,
            state: FlagEventPopoverState::Reason,
            reason: None,
            custom_reason_field: cx.new(|cx| {
                let mut text_field = TextField::new("custom-resaon", cx);
                text_field.set_placeholder(&tr!("FLAG_REASON_CUSTOM_PLACEHOLDER", "Reason"));
                text_field.on_text_changed(text_changed_listener);
                text_field
            }),
            room,
            event: None,
        }
    }

    pub fn show(&mut self, event: EventTimelineItem, cx: &mut Context<Self>) {
        self.event = Some(event);
        self.visible = true;
        cx.notify();
    }

    fn render_flag_reason(
        &mut self,
        reason: FlagReason,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        radio_button(reason.id())
            .label(reason.human_readable_string())
            .when(
                self.reason
                    .as_ref()
                    .is_some_and(|this_reason| this_reason == &reason),
                |radio| radio.checked(),
            )
            .on_checked_changed(cx.listener(move |this, _, _, cx| {
                this.reason = Some(reason.clone());
                cx.notify();
            }))
    }

    fn flag_message(&mut self, cx: &mut Context<Self>) {
        self.state = FlagEventPopoverState::Loading;
        cx.notify();

        let reason = self.reason.as_ref().unwrap().report_string().to_string();
        let room = self.room.read(cx).room.clone().unwrap();
        let event = self.event.as_ref().unwrap().event_id().unwrap().to_owned();

        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                if let Err(e) = cx
                    .spawn_tokio(async move { room.report_content(event, Some(reason)).await })
                    .await
                {
                    let _ = weak_this.update(cx, |this, cx| {
                        this.state = FlagEventPopoverState::Confirm;
                        cx.notify();
                    });
                } else {
                    let _ = weak_this.update(cx, |this, cx| {
                        this.state = FlagEventPopoverState::Complete;
                        cx.notify();
                    });
                }
            },
        )
        .detach();
    }
}

impl Render for FlagEventPopover {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let have_reason = match &self.reason {
            None => false,
            Some(FlagReason::Custom(reason)) if reason.is_empty() => false,
            _ => true,
        };
        let is_event_encrypted = self
            .event
            .as_ref()
            .is_some_and(|event| event.encryption_info().is_some());

        popover("flag-event-popover")
            .visible(self.visible)
            .size_neg(100.)
            .anchor_bottom()
            .content(
                pager("recovery-passphrase-pager", self.state.page())
                    .size_full()
                    .animation(SlideHorizontalAnimation::new())
                    .page(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(9.))
                            .child(
                                grandstand("message-flag-grandstand")
                                    .text(tr!("MESSAGE_FLAG"))
                                    .on_back_click(cx.listener(move |this, _, _, cx| {
                                        this.visible = false;
                                        this.reason = None;
                                        this.state = FlagEventPopoverState::Reason;
                                        cx.notify()
                                    })),
                            )
                            .child(
                                constrainer("message-flag-constrainer").child(
                                    layer()
                                        .flex()
                                        .flex_col()
                                        .p(px(8.))
                                        .w_full()
                                        .child(subtitle(tr!(
                                            "MESSAGE_FLAG_REASON_SUBTITLE",
                                            "Why do you want to flag this message?"
                                        )))
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap(px(8.))
                                                .child(self.render_flag_reason(
                                                    FlagReason::Spam,
                                                    window,
                                                    cx,
                                                ))
                                                .child(self.render_flag_reason(
                                                    FlagReason::Violent,
                                                    window,
                                                    cx,
                                                ))
                                                .child(self.render_flag_reason(
                                                    FlagReason::SexuallyExplicit,
                                                    window,
                                                    cx,
                                                ))
                                                .child(
                                                    radio_button("custom-reason")
                                                        .label(tr!("FLAG_REASON_CUSTOM"))
                                                        .when(
                                                            matches!(
                                                                self.reason,
                                                                Some(FlagReason::Custom(_))
                                                            ),
                                                            |radio| radio.checked(),
                                                        )
                                                        .on_checked_changed(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.reason =
                                                                    Some(FlagReason::Custom(
                                                                        "".to_string(),
                                                                    ));
                                                                cx.notify();
                                                            },
                                                        )),
                                                )
                                                .child(self.custom_reason_field.clone())
                                                .child(
                                                    button("flag-next")
                                                        .child(icon_text("go-next", tr!("NEXT")))
                                                        .when(!have_reason, |david| {
                                                            david.disabled()
                                                        })
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.state =
                                                                FlagEventPopoverState::Confirm;
                                                            cx.notify()
                                                        })),
                                                ),
                                        ),
                                ),
                            )
                            .into_any_element(),
                    )
                    .page(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(9.))
                            .child(
                                grandstand("message-flag-grandstand")
                                    .text(tr!("MESSAGE_FLAG"))
                                    .on_back_click(cx.listener(move |this, _, _, cx| {
                                        this.state = FlagEventPopoverState::Reason;
                                        cx.notify()
                                    })),
                            )
                            .child(
                                constrainer("message-flag-constrainer").child(
                                    layer()
                                        .flex()
                                        .flex_col()
                                        .p(px(8.))
                                        .w_full()
                                        .child(subtitle(tr!(
                                            "MESSAGE_FLAG_CONTINUE_SUBTITLE",
                                            "Flag this message?"
                                        )))
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap(px(8.))
                                                .child(tr!(
                                                    "MESSAGE_FLAG_CONTINUE",
                                                    "The details of this message will \
                                                    be sent to your homeserver administrator \
                                                    for review. The policies of your homeserver \
                                                    will dictate what action, if any, is to be \
                                                    taken."
                                                ))
                                                .when(is_event_encrypted, |david| {
                                                    david.child(
                                                        admonition()
                                                            .severity(AdmonitionSeverity::Warning)
                                                            .title(tr!(
                                                                "MESSAGE_FLAG_ENCRYPTED_EVENT\
                                                                _TITLE",
                                                                "This message is encrypted"
                                                            ))
                                                            .child(tr!(
                                                                "MESSAGE_FLAG_ENCRYPTED_EVENT\
                                                                _MESSAGE",
                                                                "Your homeserver administrator \
                                                                will not be able to view the \
                                                                contents of this message, but they \
                                                                will still be able to see the \
                                                                room and ID of this message."
                                                            )),
                                                    )
                                                })
                                                .child(
                                                    div().child(
                                                        button("flag-ok")
                                                            .child(icon_text(
                                                                "flag",
                                                                tr!("MESSAGE_FLAG"),
                                                            ))
                                                            .destructive()
                                                            .on_click(cx.listener(
                                                                |this, _, _, cx| {
                                                                    this.flag_message(cx)
                                                                },
                                                            )),
                                                    ),
                                                ),
                                        ),
                                ),
                            )
                            .into_any_element(),
                    )
                    .page(
                        div()
                            .size_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(spinner())
                            .into_any_element(),
                    )
                    .page(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(9.))
                            .child(
                                grandstand("message-flag-grandstand")
                                    .text(tr!("MESSAGE_FLAG"))
                                    .on_back_click(cx.listener(move |this, _, _, cx| {
                                        this.visible = false;
                                        this.reason = None;
                                        this.state = FlagEventPopoverState::Reason;
                                        cx.notify()
                                    })),
                            )
                            .child(
                                constrainer("message-flag-constrainer").child(
                                    layer()
                                        .flex()
                                        .flex_col()
                                        .p(px(8.))
                                        .w_full()
                                        .child(subtitle(tr!(
                                            "MESSAGE_FLAG_OK",
                                            "Message flagged for review"
                                        )))
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap(px(8.))
                                                .child(tr!(
                                                    "MESSAGE_FLAG_OK_MESSAGE",
                                                    "The details about this message were sent \
                                                    to your homeserver administrator for review."
                                                ))
                                                .child(
                                                    button("recovery-passphrase-popover-ok")
                                                        .child(icon_text("dialog-ok", tr!("DONE")))
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.visible = false;
                                                            this.reason = None;
                                                            this.state =
                                                                FlagEventPopoverState::Reason;
                                                        })),
                                                ),
                                        )
                                        .into_any_element(),
                                ),
                            )
                            .into_any_element(),
                    ),
            )
    }
}
