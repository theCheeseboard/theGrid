use crate::call_manager::LivekitCallManager;
use cntp_i18n::tr;
use contemporary::components::button::button;
use contemporary::components::dialog_box::{StandardButton, dialog_box};
use contemporary::components::icon_text::icon_text;
use gpui::{App, Context, IntoElement, ParentElement, Render, Window};

pub struct CallDisconnectionCompleteEvent;

pub struct CallDisconnectConfirmationDialog {
    visible: bool,
    callback: Option<Box<dyn Fn(&CallDisconnectionCompleteEvent, &mut Window, &mut App) + 'static>>,
}

impl CallDisconnectConfirmationDialog {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            visible: false,
            callback: None,
        }
    }

    pub fn ensure_calls_disconnected(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        callback: impl Fn(&CallDisconnectionCompleteEvent, &mut Window, &mut App) + 'static,
    ) {
        if self.visible {
            return;
        }

        let call_manager = cx.global::<LivekitCallManager>();
        if call_manager.calls().is_empty() {
            window.defer(cx, move |window, cx| {
                callback(&CallDisconnectionCompleteEvent, window, cx);
            });
            return;
        }

        self.callback = Some(Box::new(callback));
        self.visible = true;
        cx.notify();
    }
}

impl Render for CallDisconnectConfirmationDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        dialog_box("call-disconnection-confirmation")
            .visible(self.visible)
            .content(tr!(
                "CALL_DISCONNECT_CONFIRMATION_DIALOG",
                "To continue, you will need to hang up your active calls."
            ))
            .standard_button(
                StandardButton::Cancel,
                cx.listener(|this, _, _, cx| {
                    this.visible = false;
                    cx.notify()
                }),
            )
            .button(
                button("end-call")
                    .destructive()
                    .child(icon_text(
                        "call-stop".into(),
                        tr!("CALL_DISCONNECT_HANG_UP", "Hang up and continue").into(),
                    ))
                    .on_click(cx.listener(|this, _, window, cx| {
                        let call_manager = cx.global::<LivekitCallManager>();
                        for call in call_manager.calls().clone() {
                            call.update(cx, |call, cx| call.end_call(cx));
                        }

                        let callback = this.callback.take();
                        window.defer(cx, |window, cx| {
                            callback.unwrap()(&CallDisconnectionCompleteEvent, window, cx);
                        });
                        this.visible = false;
                        cx.notify()
                    })),
            )
    }
}
