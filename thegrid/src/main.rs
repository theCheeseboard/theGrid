// On Windows do NOT show a console window when opening the app
#![cfg_attr(all(not(test), target_os = "windows"), windows_subsystem = "windows")]
#![recursion_limit = "256"]

mod actions;
pub mod auth;
mod chat;
mod main_window;
pub mod register;
mod utilities;

mod account_settings;
mod uiaa_client;
mod upload_mxc_dialog;

use crate::actions::{
    register_actions, AccountSettings, AccountSwitcher, CreateRoom, CreateSpace, DirectJoinRoom,
    LogOut,
};
use crate::chat::chat_input::bind_chat_input_keys;
use crate::main_window::MainWindow;
use cntp_i18n::{tr, tr_load, I18N_MANAGER};
use cntp_icon_tool_macros::application_icon;
use contemporary::application::{new_contemporary_application, ApplicationLink, Details, License};
use contemporary::macros::application_details;
use contemporary::self_update::init_self_update;
use contemporary::setup::{setup_contemporary, Contemporary, ContemporaryMenus};
use contemporary::window::contemporary_window_options;
use gpui::{
    px, size, App, AsyncApp, Bounds, Menu, MenuItem, WeakEntity, WindowBounds, WindowOptions,
};
use smol_macros::main;
use std::any::TypeId;
use std::cell::RefCell;
use std::ptr;
use std::rc::Rc;
use std::str::FromStr;
use thegrid_common::session::session_manager::{setup_session_manager, SessionManager};
use thegrid_common::session::sso_login::SsoLogin;
use thegrid_common::setup_thegrid_common;
use thegrid_rtc_livekit::call_manager::setup_call_manager;
use thegrid_rtc_livekit::setup_thegrid_rtc_livekit;
use url::Url;

fn mane() {
    application_icon!("../dist/baseicon.svg");

    let (open_urls_tx, open_urls_rx) = async_channel::bounded(1);

    let application = new_contemporary_application();
    application.on_open_urls(move |urls| {
        let open_urls_tx = open_urls_tx.clone();
        smol::spawn(async move {
            open_urls_tx.send(urls).await.unwrap();
        })
        .detach();
    });

    application.run(|cx: &mut App| {
        gpui_tokio::init(cx);
        thegrid_text_rendering::init(cx);
        I18N_MANAGER.write().unwrap().load_source(tr_load!());
        setup_thegrid_common();
        setup_thegrid_rtc_livekit();
        let bounds = Bounds::centered(None, size(px(800.0), px(600.0)), cx);

        let outer_window: Rc<RefCell<WeakEntity<MainWindow>>> =
            Rc::new(RefCell::new(WeakEntity::new_invalid()));

        setup_contemporary(
            cx,
            Contemporary {
                details: Details {
                    generatable: application_details!(),
                    copyright_holder: "Victor Tran",
                    copyright_year: "2026",
                    application_version: "1.0",
                    license: License::Gpl3OrLater,
                    links: [
                        (
                            ApplicationLink::FileBug,
                            "https://github.com/theCheeseboard/thegrid/issues",
                        ),
                        (
                            ApplicationLink::SourceCode,
                            "https://github.com/theCheeseboard/thegrid",
                        ),
                    ]
                    .into(),
                },
                menus: ContemporaryMenus {
                    menus: vec![
                        Menu {
                            name: tr!("MENU_ACCOUNT", "Account").into(),
                            items: vec![
                                MenuItem::action(
                                    tr!("ACCOUNT_ACCOUNT_SWITCHER", "Switch Accounts..."),
                                    AccountSwitcher,
                                ),
                                MenuItem::action(tr!("ACCOUNT_LOG_OUT", "Log Out"), LogOut),
                            ],
                            disabled: false,
                        },
                        Menu {
                            name: tr!("MENU_ROOMS", "Rooms").into(),
                            items: vec![
                                MenuItem::action(tr!("ROOMS_CREATE", "Create Room..."), CreateRoom),
                                MenuItem::action(
                                    tr!("ROOMS_CREATE_SPACE", "Create Space..."),
                                    CreateSpace,
                                ),
                                MenuItem::action(
                                    tr!("ROOMS_DIRECT_JOIN", "Join a room..."),
                                    DirectJoinRoom,
                                ),
                            ],
                            disabled: false,
                        },
                    ],
                    on_about: Rc::new({
                        let outer_window = outer_window.clone();
                        move |cx| {
                            outer_window
                                .borrow()
                                .upgrade()
                                .unwrap()
                                .update(cx, |window, cx| {
                                    window.about_surface_open(true);
                                    cx.notify()
                                })
                        }
                    }),
                    on_settings: Some(Rc::new({
                        let outer_window = outer_window.clone();
                        move |cx| {
                            outer_window
                                .borrow()
                                .upgrade()
                                .unwrap()
                                .update(cx, |window, cx| {
                                    window.open_settings();
                                    cx.notify()
                                })
                        }
                    })),
                },
            },
        );

        init_self_update(
            Url::from_str("https://binchicken.vicr123.com").unwrap(),
            "thegrid",
            option_env!("BIN_CHICKEN_UUID"),
            option_env!("BIN_CHICKEN_SIGNATURE_PUBLIC_KEY"),
            cx,
        );

        setup_session_manager(cx);
        setup_call_manager(cx);
        bind_chat_input_keys(cx);

        cx.spawn(async move |cx: &mut AsyncApp| {
            while let Ok(urls) = open_urls_rx.recv().await {
                for url in urls {
                    let Ok(url) = Url::parse(&url) else {
                        continue;
                    };

                    if url.scheme() == "thegrid"
                        && url.path() == "/token-callback"
                        && let Some((_, token)) = url.query_pairs().find(|(key, _)| key == "token")
                    {
                        let _ = cx.update_global::<SessionManager, _>(|session_manager, cx| {
                            session_manager.insert_sso_login(
                                SsoLogin {
                                    token: token.to_string(),
                                },
                                cx,
                            );
                        });
                    }
                }
            }
        })
        .detach();

        let default_window_options = contemporary_window_options(cx, "theGrid");
        register_actions(cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..default_window_options
            },
            |window, cx| {
                let window = MainWindow::new(cx);
                *outer_window.borrow_mut() = window.downgrade();

                window
            },
        )
        .unwrap();
        cx.activate(true);
    });
}

#[tokio::main]
async fn main() {
    mane()
}
