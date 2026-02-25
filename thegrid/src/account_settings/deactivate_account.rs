use crate::main_window::{SurfaceChange, SurfaceChangeEvent, SurfaceChangeHandler};
use crate::uiaa_client::{CancelAuthenticationEvent, SendAuthDataEvent, UiaaClient};
use cntp_i18n::tr;
use contemporary::application::Details;
use contemporary::components::button::button;
use contemporary::components::constrainer::constrainer;
use contemporary::components::dialog_box::{StandardButton, dialog_box};
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::pager::pager;
use contemporary::components::pager::pager_animation::PagerAnimationDirection;
use contemporary::components::pager::slide_horizontal_animation::SlideHorizontalAnimation;
use contemporary::components::spinner::spinner;
use contemporary::components::subtitle::subtitle;
use contemporary::components::text_field::TextField;
use contemporary::styling::theme::Theme;
use contemporary::surface::surface;
use gpui::http_client::anyhow;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, AsyncApp, AsyncWindowContext, BorrowAppContext, Context, Entity, IntoElement,
    ParentElement, Render, Styled, WeakEntity, Window, div, px,
};
use gpui_tokio::Tokio;
use matrix_sdk::encryption::CrossSigningResetAuthType;
use matrix_sdk::encryption::recovery::{IdentityResetHandle, RecoveryError};
use matrix_sdk::ruma::OwnedUserId;
use matrix_sdk::ruma::api::client::uiaa::AuthData;
use std::fs::remove_dir_all;
use std::rc::Rc;
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::tokio_helper::TokioHelper;
use tracing::{Id, error};

pub struct DeactivateSurface {
    state: DeactivateState,
    error: Option<RecoveryError>,

    uiaa_client: Entity<UiaaClient>,
    deactivate_with_delete: bool,

    matrix_id_field: Entity<TextField>,

    on_surface_change: Rc<Box<SurfaceChangeHandler>>,
}

enum DeactivateState {
    TypeSelect,
    Confirm,
    Processing,
    Complete,
}

impl DeactivateSurface {
    pub fn new(
        cx: &mut App,
        on_surface_change: impl Fn(&SurfaceChangeEvent, &mut Window, &mut App) + 'static,
    ) -> Entity<Self> {
        cx.new(|cx| {
            let send_auth_data =
                cx.listener(|this: &mut Self, event: &SendAuthDataEvent, _, cx| {
                    this.confirm_deactivate(event.auth_data.clone(), cx);
                });
            let cancel_auth_listener = cx.listener(|this, _: &CancelAuthenticationEvent, _, cx| {
                this.state = DeactivateState::Confirm;
                cx.notify();
            });

            let uiaa_client =
                cx.new(|cx| UiaaClient::new(send_auth_data, cancel_auth_listener, cx));

            Self {
                state: DeactivateState::TypeSelect,
                error: None,

                uiaa_client,
                deactivate_with_delete: false,

                matrix_id_field: cx.new(|cx| {
                    let mut text_field = TextField::new("matrix-id", cx);
                    text_field.set_placeholder(tr!("AUTH_MATRIX_ID_EXAMPLE").to_string().as_str());
                    text_field
                }),
                on_surface_change: Rc::new(Box::new(on_surface_change)),
            }
        })
    }

    pub fn select_deactivate_method(&mut self, delete: bool, cx: &mut Context<Self>) {
        self.deactivate_with_delete = delete;
        self.state = DeactivateState::Confirm;
        self.matrix_id_field.update(cx, |field, cx| {
            let session_manager = cx.global::<SessionManager>();
            field.set_placeholder(
                session_manager
                    .client()
                    .unwrap()
                    .read(cx)
                    .user_id()
                    .unwrap()
                    .as_str(),
            );
            field.set_text("");
        });
        cx.notify();
    }

    pub fn confirm_deactivate(&mut self, auth_data: Option<AuthData>, cx: &mut Context<Self>) {
        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx).clone();

        let uiaa_client_entity = self.uiaa_client.clone();
        let delete_data = self.deactivate_with_delete;

        self.state = DeactivateState::Processing;
        cx.notify();

        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                if let Err(e) = cx
                    .spawn_tokio(async move {
                        client
                            .account()
                            .deactivate(None, auth_data, delete_data)
                            .await
                    })
                    .await
                {
                    if let Some(uiaa) = e.as_uiaa_response() {
                        uiaa_client_entity
                            .update(cx, |uiaa_client, cx| {
                                uiaa_client.set_uiaa_info(uiaa.clone(), cx);
                                cx.notify()
                            })
                            .unwrap();
                        return;
                    } else {
                        error!("Failed to deactivate account: {:?}", e);
                        weak_this
                            .update(cx, |this, cx| {
                                this.state = DeactivateState::Confirm;
                                cx.notify();
                            })
                            .unwrap();
                    }
                } else {
                    weak_this
                        .update(cx, |this, cx| {
                            this.complete_deactivate(cx);
                        })
                        .unwrap();
                }
            },
        )
        .detach();
    }

    fn complete_deactivate(&mut self, cx: &mut Context<Self>) {
        cx.update_global::<SessionManager, ()>(|session_manager, cx| {
            let details = cx.global::<Details>();
            let directories = details.standard_dirs().unwrap();
            let data_dir = directories.data_dir();
            let session_dir = data_dir.join("sessions");
            let this_session_dir = session_dir.join(
                session_manager
                    .current_session()
                    .as_ref()
                    .unwrap()
                    .uuid
                    .to_string(),
            );

            // Delete the session
            remove_dir_all(this_session_dir).unwrap();
            session_manager.clear_session();
        });

        self.state = DeactivateState::Complete;
        cx.notify();
    }

    fn deactivate_selection_page(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.global::<Theme>();
        div()
            .bg(theme.background)
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                grandstand("devices-grandstand")
                    .text(tr!("ACCOUNT_DEACTIVATE", "Deactivate Account"))
                    .pt(px(36.))
                    .on_back_click(cx.listener(|this, _, window, cx| {
                        (this.on_surface_change)(
                            &SurfaceChangeEvent {
                                change: SurfaceChange::Pop,
                            },
                            window,
                            cx,
                        )
                    })),
            )
            .child(
                constrainer("devices")
                    .flex()
                    .flex_col()
                    .w_full()
                    .p(px(8.))
                    .child(
                        layer()
                            .flex()
                            .flex_col()
                            .p(px(8.))
                            .gap(px(8.))
                            .w_full()
                            .child(subtitle(tr!("ACCOUNT_DEACTIVATE")))
                            .child(tr!(
                                "ACCOUNT_DEACTIVATE_CHOOSE_TITLE",
                                "Choose an option to deactivate your account:"
                            ))
                            .child(
                                layer()
                                    .p(px(8.))
                                    .gap(px(8.))
                                    .flex_col()
                                    .child(tr!(
                                        "ACCOUNT_DEACTIVATE_ONLY_DESCRIPTION",
                                        "Deactivate your account without \
                                                        hiding your existing messages"
                                    ))
                                    .child(
                                        button("deactivate-with-no-delete")
                                            .child(icon_text(
                                                "arrow-right".into(),
                                                tr!(
                                                    "ACCOUNT_DEACTIVATE_ONLY",
                                                    "Deactivate Account Only"
                                                )
                                                .into(),
                                            ))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.select_deactivate_method(false, cx);
                                            })),
                                    ),
                            )
                            .child(
                                layer()
                                    .p(px(8.))
                                    .gap(px(8.))
                                    .flex_col()
                                    .child(tr!(
                                        "ACCOUNT_DEACTIVATE_AND_DELETE_DESCRIPTION",
                                        "Deactivate your account and hide \
                                                        messages from people that join \
                                                        rooms in future"
                                    ))
                                    .child(
                                        button("deactivate-with-delete")
                                            .child(icon_text(
                                                "arrow-right".into(),
                                                tr!(
                                                    "ACCOUNT_DEACTIVATE_AND_DELETE",
                                                    "Deactivate Account \
                                                                    and Hide Messages"
                                                )
                                                .into(),
                                            ))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.select_deactivate_method(true, cx);
                                            })),
                                    ),
                            ),
                    ),
            )
    }

    fn deactivate_confirm_page(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.global::<Theme>();
        div()
            .bg(theme.background)
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                grandstand("devices-grandstand")
                    .text(tr!("ACCOUNT_DEACTIVATE"))
                    .pt(px(36.))
                    .on_back_click(cx.listener(|this, _, window, cx| {
                        this.state = DeactivateState::TypeSelect;
                        cx.notify();
                    })),
            )
            .child(
                constrainer("devices")
                    .flex()
                    .flex_col()
                    .w_full()
                    .p(px(8.))
                    .child(
                        layer()
                            .flex()
                            .flex_col()
                            .p(px(8.))
                            .gap(px(8.))
                            .w_full()
                            .child(subtitle(tr!("ACCOUNT_DEACTIVATE")))
                            .child(tr!(
                                "ACCOUNT_DEACTIVATE_TITLE",
                                "Deactivating your account will make \
                                                it unavailable for use. If you continue,"
                            ))
                            .child(layer().p(px(4.)).child(tr!(
                                "ACCOUNT_DEACTIVATE_UPSHOT_1",
                                "You will not be able to log into this account."
                            )))
                            .child(layer().p(px(4.)).child(tr!(
                                "ACCOUNT_DEACTIVATE_UPSHOT_2",
                                "You will not be able to reactivate \
                                                the account, as deactivation is permanent"
                            )))
                            .child(layer().p(px(4.)).child(tr!(
                                "ACCOUNT_DEACTIVATE_UPSHOT_3",
                                "Your Matrix ID will become unavailable \
                                                for use by everyone - including yourself"
                            )))
                            .child(layer().p(px(4.)).child(tr!(
                                "ACCOUNT_DEACTIVATE_UPSHOT_4",
                                "You will leave all rooms that you have joined"
                            )))
                            .child(tr!(
                                "ACCOUNT_DEACTIVATE_DESCRIPTION_2",
                                "To continue to deactivate your account, \
                                                confirm your Matrix ID below."
                            ))
                            .child(self.matrix_id_field.clone())
                            .child(
                                button("deactivate-button")
                                    .destructive()
                                    .child(icon_text(
                                        "list-remove".into(),
                                        tr!("ACCOUNT_DEACTIVATE").into(),
                                    ))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        let matrix_id = OwnedUserId::try_from(
                                            this.matrix_id_field.read(cx).text(),
                                        );

                                        if let Ok(matrix_id) = matrix_id {
                                            let session_manager = cx.global::<SessionManager>();

                                            if session_manager
                                                .client()
                                                .unwrap()
                                                .read(cx)
                                                .user_id()
                                                .unwrap()
                                                .eq(&matrix_id)
                                            {
                                                this.confirm_deactivate(None, cx);
                                                return;
                                            }
                                        }

                                        // TODO: Flash error
                                    })),
                            ),
                    ),
            )
    }

    fn deactivate_complete_page(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.global::<Theme>();
        div()
            .bg(theme.background)
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                grandstand("devices-grandstand")
                    .text(tr!("ACCOUNT_DEACTIVATE"))
                    .pt(px(36.)),
            )
            .child(
                constrainer("devices")
                    .flex()
                    .flex_col()
                    .w_full()
                    .p(px(8.))
                    .child(
                        layer()
                            .flex()
                            .flex_col()
                            .p(px(8.))
                            .gap(px(8.))
                            .w_full()
                            .child(subtitle(tr!(
                                "ACCOUNT_DEACTIVATE_COMPLETE",
                                "Account Deactivated"
                            )))
                            .child(tr!(
                                "ACCOUNT_DEACTIVATE_COMPLETE_DESCRIPTION",
                                "Your account was deactivated."
                            ))
                            .child(
                                button("deactivate-ok")
                                    .child(icon_text("dialog-ok".into(), tr!("DONE").into()))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        // Pop twice to get back to
                                        // home page
                                        (this.on_surface_change)(
                                            &SurfaceChangeEvent {
                                                change: SurfaceChange::Pop,
                                            },
                                            window,
                                            cx,
                                        );
                                        (this.on_surface_change)(
                                            &SurfaceChangeEvent {
                                                change: SurfaceChange::Pop,
                                            },
                                            window,
                                            cx,
                                        );
                                        this.state = DeactivateState::Confirm;
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
    }
}

impl Render for DeactivateSurface {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let session_manager = cx.global::<SessionManager>();

        // Stop rendering here because we shouldn't get to see this page
        let has_no_session = session_manager.current_session().is_none();

        surface()
            .child(
                div()
                    .size_full()
                    .child(
                        pager(
                            "identity-reset-pager",
                            match self.state {
                                DeactivateState::TypeSelect => 0,
                                DeactivateState::Confirm => 1,
                                DeactivateState::Processing => 2,
                                DeactivateState::Complete => 3,
                            },
                        )
                        .size_full()
                        .flex_grow()
                        .animation(SlideHorizontalAnimation::new())
                        .when_else(
                            has_no_session,
                            |pager| {
                                // Render empty pages because the client is not available
                                pager
                                    .page(div().into_any_element())
                                    .page(div().into_any_element())
                            },
                            |pager| {
                                pager
                                    .page(
                                        self.deactivate_selection_page(window, cx)
                                            .into_any_element(),
                                    )
                                    .page(
                                        self.deactivate_confirm_page(window, cx).into_any_element(),
                                    )
                            },
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
                        .page(self.deactivate_complete_page(window, cx).into_any_element()),
                    )
                    .child(self.uiaa_client.clone()),
            )
            .into_any_element()
    }
}
