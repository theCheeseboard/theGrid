use cntp_i18n::{Quote, tr};
use contemporary::components::button::button;
use contemporary::components::context_menu::{ContextMenuExt, ContextMenuItem};
use contemporary::components::dialog_box::{StandardButton, dialog_box};
use contemporary::components::icon_text::icon_text;
use contemporary::styling::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AsyncApp, ClickEvent, ClipboardItem, ElementId, Entity, FontWeight, InteractiveElement,
    IntoElement, ParentElement, RenderOnce, StatefulInteractiveElement, Styled, Window, div, px,
};
use matrix_sdk::RoomMemberships;
use matrix_sdk::ruma::OwnedRoomId;
use std::collections::HashMap;
use std::rc::Rc;
use thegrid::session::room_cache::CachedRoom;
use thegrid::session::session_manager::SessionManager;
use thegrid::tokio_helper::TokioHelper;
use url::Url;

pub struct InviteEvent {
    pub room_id: OwnedRoomId,
}

#[derive(IntoElement)]
pub struct StandardRoomElement {
    pub room: Entity<CachedRoom>,
    pub current_room: Option<OwnedRoomId>,
    pub on_click: Rc<Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
    pub on_invite: Rc<Box<dyn Fn(&InviteEvent, &mut Window, &mut App)>>,
}

#[derive(PartialEq, Clone, Copy)]
enum CurrentDialogBox {
    None,
    LeaveRoom,
}

impl RenderOnce for StandardRoomElement {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let current_dialog_box = window.use_state(cx, |_, _| CurrentDialogBox::None);
        let current_dialog_box_value = *current_dialog_box.read(cx);
        let theme = cx.global::<Theme>();
        let room = self.room.read(cx);
        let room_id = room.inner.room_id().to_owned();
        let room_id_2 = room_id.clone();
        let on_click = self.on_click;
        let on_invite = self.on_invite;
        let matrix_room = room.inner.clone();
        let matrix_room_2 = room.inner.clone();

        let current_dialog_box_2 = current_dialog_box.clone();
        let current_dialog_box_3 = current_dialog_box.clone();

        let display_name = room
            .inner
            .cached_display_name()
            .map(|name| name.to_string())
            .or_else(|| room.inner.name())
            .unwrap_or_default();

        let context_menu = vec![
            ContextMenuItem::separator()
                .label(tr!("FOR_ROOM", "For {{room}}", room:Quote=display_name))
                .build(),
            ContextMenuItem::menu_item()
                .label(tr!("ROOM_MARK_READ", "Mark as Read"))
                .icon("mail-mark-read")
                .disabled()
                .build(),
            ContextMenuItem::menu_item()
                .label(tr!("ROOM_NOTIFICATIONS", "Notification Settings..."))
                .icon("reminder")
                .disabled()
                .build(),
            ContextMenuItem::separator().build(),
            ContextMenuItem::menu_item()
                .label(tr!("ROOM_INVITE", "Invite Someone..."))
                .icon("user")
                .on_triggered(move |_, window, cx| {
                    on_invite.clone()(
                        &InviteEvent {
                            room_id: room_id_2.clone(),
                        },
                        window,
                        cx,
                    );
                })
                .build(),
            ContextMenuItem::menu_item()
                .label(tr!("ROOM_COPY_LINK", "Copy link to room"))
                .icon("edit-copy")
                .on_triggered(move |_, _, cx| {
                    let matrix_room = matrix_room_2.clone();
                    let room_id = matrix_room.room_id().to_owned();
                    cx.spawn(async move |cx: &mut AsyncApp| {
                        let mut vias = Vec::new();
                        // TODO: Pick the server of the highest power level user

                        // Pick the three most popular servers
                        let Ok(members) = cx
                            .spawn_tokio(async move {
                                matrix_room.members_no_sync(RoomMemberships::ACTIVE).await
                            })
                            .await
                        else {
                            // TODO: What to do?
                            return;
                        };

                        let mut popular_servers = HashMap::new();
                        for member in members {
                            let server = member.user_id().server_name().to_owned();
                            let count = popular_servers.entry(server).or_insert(0);
                            *count += 1;
                        }

                        while vias.len() < 3 && !popular_servers.is_empty() {
                            let popular_server = popular_servers
                                .iter()
                                .max_by_key(|(_, count)| *count)
                                .map(|(server, _)| server)
                                .unwrap()
                                .clone();
                            popular_servers.remove(&popular_server);

                            // Ban IP literals
                            if popular_server.is_ip_literal() {
                                continue;
                            }
                            vias.push(popular_server);
                        }

                        // Return at most 3 vias
                        vias.truncate(3);

                        cx.update(|cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(
                                room_id.matrix_to_uri_via(vias).to_string(),
                            ))
                        })
                        .unwrap();
                    })
                    .detach();
                })
                .build(),
            ContextMenuItem::separator().build(),
            ContextMenuItem::menu_item()
                .label(tr!("ROOM_SETTINGS", "Room Settings..."))
                .icon("configure")
                .disabled()
                .build(),
            ContextMenuItem::menu_item()
                .label(tr!("ROOM_LEAVE", "Leave Room"))
                .icon("system-log-out")
                .on_triggered(move |_, _, cx| {
                    current_dialog_box.write(cx, CurrentDialogBox::LeaveRoom);
                })
                .build(),
        ];

        div()
            .id("item")
            .flex()
            .w_full()
            .items_center()
            .m(px(2.))
            .p(px(2.))
            .rounded(theme.border_radius)
            .when(
                self.current_room
                    .is_some_and(|current_room| current_room == room_id),
                |david| david.bg(theme.button_background),
            )
            .child(display_name.clone())
            .child(div().flex_grow())
            .when_else(
                room.inner.unread_notification_counts().notification_count > 0,
                |david| {
                    david.font_weight(FontWeight::BOLD).child(
                        div()
                            .rounded(theme.border_radius)
                            .bg(theme.error_accent_color)
                            .p(px(2.))
                            .child(
                                room.inner
                                    .unread_notification_counts()
                                    .notification_count
                                    .to_string(),
                            ),
                    )
                },
                |david| {
                    david.when(room.inner.num_unread_messages() > 0, |david| {
                        david.child(div().bg(theme.foreground).size(px(8.)).rounded(px(4.)))
                    })
                },
            )
            .on_click(move |event, window, cx| {
                on_click(event, window, cx);
            })
            .with_context_menu(context_menu)
            .child(
                dialog_box("leave-room-dialog-box")
                    .visible(current_dialog_box_value == CurrentDialogBox::LeaveRoom)
                    .title(tr!("ROOM_LEAVE").into())
                    .content_text_informational(
                        tr!(
                            "LEAVE_ROOM_TEXT",
                            "Do you want to leave {{room}}?",
                            room:Quote=display_name
                        )
                        .into(),
                        if room.inner.is_public().unwrap_or(false) {
                            tr!(
                                "LEAVE_ROOM_INFORMATIONAL_PUBLIC",
                                "You can rejoin this room later."
                            )
                            .into()
                        } else {
                            tr!(
                                "LEAVE_ROOM_INFORMATIONAL_NOT_PUBLIC",
                                "You may not be able to rejoin this room unless you are re-invited."
                            )
                            .into()
                        },
                    )
                    .standard_button(StandardButton::Cancel, move |_, _, cx| {
                        current_dialog_box_2.write(cx, CurrentDialogBox::None);
                    })
                    .button(
                        button("leave-room-button")
                            .destructive()
                            .child(icon_text("system-log-out".into(), tr!("ROOM_LEAVE").into()))
                            .on_click(move |_, _, cx| {
                                let matrix_room = matrix_room.clone();
                                current_dialog_box_3.write(cx, CurrentDialogBox::LeaveRoom);

                                cx.spawn(async move |cx: &mut AsyncApp| {
                                    let _ = cx
                                        .spawn_tokio(async move { matrix_room.leave().await })
                                        .await;
                                })
                                .detach();
                            }),
                    ),
            )
            .into_any_element()
    }
}
