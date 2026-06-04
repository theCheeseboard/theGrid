use crate::actions::{
    AccountSettings, AccountSwitcher, CreateRoom, CreateSpace, DirectJoinRoom, LogOut,
};
use crate::auth::recovery_passphrase_popover::RecoveryPassphrasePopover;
use crate::auth::verification_popover::VerificationPopover;
use crate::chat::displayed_room::DisplayedRoom;
use crate::chat::forced_device_verification::forced_device_verification;
use crate::chat::main_chat_surface::MainChatSurface;
use crate::chat::new_account_onboarding::new_account_onboarding;
use cntp_i18n::tr;
use contemporary::application::Details;
use contemporary::components::application_menu::ApplicationMenu;
use contemporary::components::button::button;
use contemporary::components::dialog_box::{dialog_box, StandardButton};
use contemporary::components::icon_text::icon_text;
use contemporary::components::interstitial::interstitial;
use contemporary::components::pager::fade_animation::FadeAnimation;
use contemporary::components::pager::pager;
use contemporary::components::spinner::spinner;
use contemporary::styling::theme::Theme;
use contemporary::surface::surface;
use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, App, AppContext, BorrowAppContext, Context, Entity, InteractiveElement,
    IntoElement, Menu, MenuItem, ParentElement, Render, Styled, Window,
};
use matrix_sdk::encryption::VerificationState;
use std::fs::remove_dir_all;
use std::rc::Rc;
use thegrid_common::session::error_handling::ClientError;
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::surfaces::{MainWindowSurface, SurfaceChangeEvent, SurfaceChangeHandler};

pub type ChangeRoomHandler = dyn Fn(&ChangeRoomEvent, &mut Window, &mut App) + 'static;
pub type RequestCryptographicResetHandler =
    dyn Fn(&RequestCryptographicResetEvent, &mut Window, &mut App) + 'static;

#[derive(Clone)]
pub struct ChangeRoomEvent {
    pub new_room: DisplayedRoom,
}

pub struct RequestCryptographicResetEvent;

pub struct ChatSurface {
    application_menu: Entity<ApplicationMenu>,
    main_chat_surface: Entity<MainChatSurface>,
    displayed_room: Entity<DisplayedRoom>,

    verification_ui: SelfVerificationUi,
    request_cryptographic_reset_dialog_open: bool,

    on_surface_change: Rc<Box<SurfaceChangeHandler>>,
}

#[derive(Clone)]
pub struct SelfVerificationUi {
    pub verification_popover: Entity<VerificationPopover>,
    pub recovery_passphrase_popover: Entity<RecoveryPassphrasePopover>,
    pub on_request_cryptographic_reset: Rc<Box<RequestCryptographicResetHandler>>,
}

impl ChatSurface {
    pub fn new(
        cx: &mut Context<Self>,
        on_surface_change: impl Fn(&SurfaceChangeEvent, &mut Window, &mut App) + 'static,
    ) -> ChatSurface {
        let on_surface_change: Rc<Box<SurfaceChangeHandler>> = Rc::new(Box::new(on_surface_change));
        let displayed_room = cx.new(|_| DisplayedRoom::None);

        let verification_popover = cx.new(|cx| VerificationPopover::new(cx));
        let recovery_passphrase_popover = cx.new(|cx| RecoveryPassphrasePopover::new(cx));
        let verification_ui = SelfVerificationUi {
            verification_popover: verification_popover.clone(),
            recovery_passphrase_popover: recovery_passphrase_popover.clone(),
            on_request_cryptographic_reset: Rc::new(Box::new(cx.listener(|this, _, _, cx| {
                this.request_cryptographic_reset_dialog_open = true;
                cx.notify();
            }))),
        };

        ChatSurface {
            application_menu: ApplicationMenu::new(
                cx,
                Menu {
                    name: "Application Menu".into(),
                    items: vec![
                        MenuItem::action(tr!("ROOMS_CREATE"), CreateRoom),
                        MenuItem::action(tr!("ROOMS_CREATE_SPACE"), CreateSpace),
                        MenuItem::action(tr!("ROOMS_DIRECT_JOIN"), DirectJoinRoom),
                        MenuItem::separator(),
                        MenuItem::action(
                            tr!("ACCOUNT_ACCOUNT_SETTINGS", "Account Settings"),
                            AccountSettings,
                        ),
                        MenuItem::separator(),
                        MenuItem::action(tr!("ACCOUNT_ACCOUNT_SWITCHER"), AccountSwitcher),
                        MenuItem::action(tr!("ACCOUNT_LOG_OUT"), LogOut),
                    ],
                    disabled: false,
                },
            ),
            main_chat_surface: MainChatSurface::new(
                cx,
                displayed_room.clone(),
                verification_ui.clone(),
                on_surface_change.clone(),
            ),
            verification_ui,
            request_cryptographic_reset_dialog_open: false,
            displayed_room,
            on_surface_change,
        }
    }
}

impl Render for ChatSurface {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>();
        let session_manager = cx.global::<SessionManager>();

        let Some(current_session) = session_manager.current_session() else {
            return div();
        };
        let verified = if session_manager.client().is_some() {
            let account = session_manager.current_account().read(cx);
            account.verification_state() == VerificationState::Verified
        } else {
            true
        };

        div()
            .size_full()
            .key_context("MainSurface")
            .child(
                surface()
                    .child(
                        pager("chat-surface-root-pager", {
                            match session_manager.error() {
                                ClientError::None | ClientError::Recoverable(_) => {
                                    if session_manager.client().is_some() {
                                        if session_manager.is_new_account() {
                                            3
                                        } else {
                                            if cfg!(feature = "force-device-verification")
                                                && !verified
                                            {
                                                4
                                            } else {
                                                1
                                            }
                                        }
                                    } else {
                                        0
                                    }
                                }
                                ClientError::Terminal(_) => 2,
                            }
                        })
                        .animation(FadeAnimation::new())
                        .page(
                            div()
                                .size_full()
                                .flex()
                                .flex_col()
                                .items_center()
                                .justify_center()
                                .gap(px(8.))
                                .child(spinner())
                                .child(div().text_size(theme.heading_font_size).child(tr!(
                                    "MAIN_CHAT_WELCOME",
                                    "Welcome back, {{user}}!",
                                    user = current_session
                                        .secrets
                                        .session_meta()
                                        .unwrap()
                                        .user_id
                                        .localpart()
                                )))
                                .into_any_element(),
                        )
                        .page(self.main_chat_surface.clone().into_any_element())
                        .page(match session_manager.error() {
                            ClientError::None => div().into_any_element(),
                            ClientError::Terminal(terminal_error) => interstitial()
                                .size_full()
                                .icon("network-disconnect")
                                .title(tr!("MAIN_CHAT_ERROR_TERMINAL", "Disconnected from Matrix"))
                                .message(terminal_error.description())
                                .when_else(
                                    terminal_error.should_logout(),
                                    |david| {
                                        david.child(
                                            button("log-out-button")
                                                .child(icon_text(
                                                    "system-log-out",
                                                    tr!("ACCOUNT_LOG_OUT"),
                                                ))
                                                .on_click(cx.listener(|_, _, _, cx| {
                                                    cx.update_global::<SessionManager, ()>(
                                                        |session_manager, cx| {
                                                            let details = cx.global::<Details>();
                                                            let directories =
                                                                details.standard_dirs().unwrap();
                                                            let data_dir = directories.data_dir();
                                                            let session_dir =
                                                                data_dir.join("sessions");
                                                            let this_session_dir = session_dir
                                                                .join(
                                                                    session_manager
                                                                        .current_session()
                                                                        .as_ref()
                                                                        .unwrap()
                                                                        .uuid
                                                                        .to_string(),
                                                                );

                                                            // Delete the session
                                                            remove_dir_all(this_session_dir)
                                                                .unwrap();

                                                            session_manager.clear_session()
                                                        },
                                                    );
                                                })),
                                        )
                                    },
                                    |david| {
                                        david.child(
                                            button("log-out-button")
                                                .child(icon_text(
                                                    "system-switch-user",
                                                    tr!(
                                                        "ACCOUNT_SWITCHER_ERROR",
                                                        "Account Switcher"
                                                    ),
                                                ))
                                                .on_click(cx.listener(|_, _, _, cx| {
                                                    cx.update_global::<SessionManager, ()>(
                                                        |session_manager, cx| {
                                                            session_manager.clear_session()
                                                        },
                                                    );
                                                })),
                                        )
                                    },
                                )
                                .into_any_element(),
                            ClientError::Recoverable(_) => div().into_any_element(),
                        })
                        .page(new_account_onboarding().into_any_element())
                        .page(
                            forced_device_verification(self.verification_ui.clone())
                                .into_any_element(),
                        )
                        .size_full(),
                    )
                    .application_menu(self.application_menu.clone()),
            )
            .child(self.verification_ui.verification_popover.clone())
            .child(self.verification_ui.recovery_passphrase_popover.clone())
            .child(
                dialog_box("crypto-reset-dialog")
                    .visible(self.request_cryptographic_reset_dialog_open)
                    .title(tr!(
                        "VERIFY_SESSION_RESET_CRYPTO_DIALOG_TITLE",
                        "Encryption Setup Recovery"
                    ))
                    .content(tr!(
                        "VERIFY_SESSION_RESET_CRYPTO_DIALOG_MESSAGE",
                        "If you can't verify this session because you've lost access \
                                    to all your other verification methods, you can reset your \
                                    cryptographic identity to start over. You will lose access to \
                                    your existing encrypted messages, and all of your devices \
                                    will become unverified."
                    ))
                    .standard_button(
                        StandardButton::Cancel,
                        cx.listener(|this, _, _, cx| {
                            this.request_cryptographic_reset_dialog_open = false;
                            cx.notify();
                        }),
                    )
                    .button(
                        button("do-reset-crypto")
                            .destructive()
                            .child(icon_text("view-refresh", tr!("SECURITY_IDENTITY_RESET")))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.request_cryptographic_reset_dialog_open = false;
                                (this.on_surface_change)(
                                    &SurfaceChangeEvent {
                                        change: MainWindowSurface::IdentityReset.into(),
                                    },
                                    window,
                                    cx,
                                )
                            })),
                    ),
            )
    }
}
