// On Windows do NOT show a console window when opening the app
#![cfg_attr(all(not(test), target_os = "windows"), windows_subsystem = "windows")]

mod actions;
pub mod auth;
pub mod register;
mod chat;
mod main_window;
mod utilities;

mod account_settings;
mod mxc_image;
mod uiaa_client;

use crate::actions::{AccountSettings, AccountSwitcher, CreateRoom, LogOut, register_actions};
use crate::chat::chat_input::bind_chat_input_keys;
use crate::main_window::MainWindow;
use cntp_i18n::{I18N_MANAGER, tr, tr_load};
use cntp_icon_tool_macros::application_icon;
use contemporary::application::{ApplicationLink, Details, License, new_contemporary_application};
use contemporary::macros::application_details;
use contemporary::setup::{Contemporary, ContemporaryMenus, setup_contemporary};
use contemporary::window::contemporary_window_options;
use gpui::{App, Bounds, Menu, MenuItem, WindowBounds, WindowOptions, px, size};
use smol_macros::main;
use std::any::TypeId;
use std::rc::Rc;
use thegrid::session::session_manager::setup_session_manager;

fn mane() {
    application_icon!("../dist/baseicon.svg");

    new_contemporary_application().run(|cx: &mut App| {
        gpui_tokio::init(cx);
        thegrid_text_rendering::init(cx);
        I18N_MANAGER.write().unwrap().load_source(tr_load!());
        let bounds = Bounds::centered(None, size(px(800.0), px(600.0)), cx);

        setup_session_manager(cx);
        bind_chat_input_keys(cx);

        let default_window_options = contemporary_window_options(cx, "theGrid".into());
        register_actions(cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..default_window_options
            },
            |_, cx| {
                let window = MainWindow::new(cx);
                let weak_window = window.downgrade();
                let weak_windew = window.downgrade();
                let weak_windaw = window.downgrade();

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
                                            tr!("ACCOUNT_ACCOUNT_SETTINGS", "Account Settings..."),
                                            AccountSettings,
                                        ),
                                        MenuItem::separator(),
                                        MenuItem::action(
                                            tr!("ACCOUNT_ACCOUNT_SWITCHER", "Switch Accounts..."),
                                            AccountSwitcher,
                                        ),
                                        MenuItem::action(tr!("ACCOUNT_LOG_OUT", "Log Out"), LogOut),
                                    ],
                                },
                                Menu {
                                    name: tr!("MENU_ROOMS", "Rooms").into(),
                                    items: vec![MenuItem::action(
                                        tr!("ROOMS_CREATE", "Create Room..."),
                                        CreateRoom,
                                    ), MenuItem::action(
                                        tr!("ROOMS_DIRECT_JOIN", "Join a room..."),
                                        CreateRoom,
                                    )],
                                },
                            ],
                            on_about: Rc::new(move |cx| {
                                weak_window.upgrade().unwrap().update(cx, |window, cx| {
                                    window.about_surface_open(true);
                                    cx.notify()
                                })
                            }),
                            on_settings: None,
                        },
                    },
                );

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
