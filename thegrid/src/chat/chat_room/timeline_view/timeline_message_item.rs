use crate::chat::chat_room::open_room::OpenRoom;
use crate::chat::chat_room::timeline_view::author_flyout::{
    AuthorFlyoutUserActionListener, author_flyout,
};
use crate::chat::chat_room::timeline_view::message_error_item::message_error_item;
use crate::chat::chat_room::timeline_view::reply_fragment::reply_fragment_in_reply_to;
use crate::chat::displayed_room::DisplayedRoom;
use cntp_i18n::{I18N_MANAGER, Quote, i18n_manager, tr};
use contemporary::components::admonition::{AdmonitionSeverity, admonition};
use contemporary::components::anchorer::WithAnchorer;
use contemporary::components::button::button;
use contemporary::components::context_menu::ContextMenuItem;
use contemporary::components::dialog_box::{StandardButton, dialog_box};
use contemporary::components::icon::icon;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::spinner::spinner;
use contemporary::styling::theme::{Theme, ThemeStorage, VariableColor};
use directories::UserDirs;
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, AppContext, AsyncApp, BorrowAppContext, Bounds, ClipboardItem, Entity,
    IntoElement, ParentElement, Path, Pixels, RenderOnce, Styled, Window, canvas, div, point, px,
    rgba,
};
use matrix_sdk::room::RoomMember;
use matrix_sdk::ruma::events::room::message::{
    FileMessageEventContent, FormattedBody, MessageFormat, MessageType,
};
use matrix_sdk::ruma::matrix_uri::MatrixId;
use matrix_sdk::ruma::{MatrixToUri, OwnedUserId, UserId};
use matrix_sdk_ui::timeline::{MsgLikeContent, MsgLikeKind, Profile, TimelineDetails};
use std::fs::copy;
use std::rc::Rc;
use thegrid_common::mxc_image::{SizePolicy, mxc_image};
use thegrid_common::session::media_cache::{MediaCacheEntry, MediaFile, MediaState};
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::tokio_helper::TokioHelper;
use thegrid_text_rendering::TextView;
use tracing::info;

#[derive(IntoElement)]
pub struct TimelineMessageItem {
    content: MsgLikeContent,
    sender_profile: TimelineDetails<Profile>,
    sender: OwnedUserId,
    room: Entity<OpenRoom>,
    displayed_room: Entity<DisplayedRoom>,
    on_user_action: Rc<Box<AuthorFlyoutUserActionListener>>,
}

pub fn timeline_message_item(
    content: MsgLikeContent,
    sender_profile: TimelineDetails<Profile>,
    sender: OwnedUserId,
    room: Entity<OpenRoom>,
    displayed_room: Entity<DisplayedRoom>,
    on_user_action: Rc<Box<AuthorFlyoutUserActionListener>>,
) -> TimelineMessageItem {
    TimelineMessageItem {
        content,
        sender_profile,
        sender,
        room,
        displayed_room,
        on_user_action,
    }
}

impl RenderOnce for TimelineMessageItem {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();

        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx).clone();

        let reactions = self.content.reactions;
        div()
            .flex()
            .flex_col()
            .when_some(self.content.in_reply_to, |david, reply_details| {
                david.child(reply_fragment_in_reply_to(
                    reply_details,
                    self.room.clone(),
                    self.displayed_room.clone(),
                    self.on_user_action.clone(),
                ))
            })
            .child(match self.content.kind {
                MsgLikeKind::Message(message) => div().child(msgtype_to_message_line(
                    message.msgtype(),
                    self.sender,
                    self.sender_profile,
                    false,
                    self.room,
                    self.displayed_room,
                    self.on_user_action,
                    window,
                    cx,
                )),
                MsgLikeKind::Redacted => div().child(message_error_item(
                    "edit-delete",
                    tr!("MESSAGE_REDACTED", "Removed"),
                    cx,
                )),
                MsgLikeKind::UnableToDecrypt(_) => div().child(message_error_item(
                    "exception",
                    tr!("MESSAGE_UNABLE_TO_DECRYPT", "Unable to decrypt"),
                    cx,
                )),
                _ => div().child(message_error_item(
                    "dialog-warning",
                    tr!("MESSAGE_UNSUPPORTED"),
                    cx,
                )),
            })
            .when(!reactions.is_empty(), |david| {
                david.child(reactions.iter().fold(
                    div().flex().mt(px(4.)).gap(px(4.)),
                    |david, (reaction, reactees)| {
                        david.child(
                            div()
                                .flex()
                                .p(px(2.))
                                .gap(px(2.))
                                .border(px(1.))
                                .border_color(theme.border_color)
                                .when_else(
                                    reactees.contains_key(&client.user_id().unwrap().to_owned()),
                                    |david| david.bg(theme.info_accent_color),
                                    |david| david.bg(theme.layer_background),
                                )
                                .rounded(theme.border_radius)
                                .child(reaction.clone())
                                .child(i18n_manager!().locale.format_decimal(reactees.len())),
                        )
                    },
                ))
            })
    }
}

pub fn msgtype_to_message_line<'a>(
    msgtype: &MessageType,
    sender: OwnedUserId,
    sender_profile: TimelineDetails<Profile>,
    as_reply: bool,
    room: Entity<OpenRoom>,
    displayed_room: Entity<DisplayedRoom>,
    on_user_action: Rc<Box<AuthorFlyoutUserActionListener>>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement + 'a {
    let theme = cx.theme();
    match msgtype {
        MessageType::Emote(emote) => div()
            .flex()
            .items_center()
            .when_some(
                match sender_profile {
                    TimelineDetails::Ready(profile) => Some(profile),
                    _ => None,
                },
                |david, profile| {
                    david.child(
                        mxc_image(profile.avatar_url.clone())
                            .fixed_square(px(24.))
                            .size_policy(SizePolicy::Fit)
                            .rounded(theme.border_radius)
                            .fallback_image(sender)
                            .mr(px(2.)),
                    )
                },
            )
            .child(
                canvas(
                    |bounds, _, _| {
                        // TODO: RTL?
                        let mut path = Path::new(bounds.top_right());
                        path.line_to(point(bounds.left(), bounds.center().y));
                        path.line_to(bounds.bottom_right());
                        path
                    },
                    |_, path, window, cx| {
                        let theme = cx.theme();
                        window.paint_path(path, theme.layer_background)
                    },
                )
                .w(px(12.))
                .h(px(24.)),
            )
            .child(
                div()
                    .min_h(px(24.))
                    .bg(theme.layer_background)
                    .rounded_tr(theme.border_radius)
                    .rounded_br(theme.border_radius)
                    .flex()
                    .child(div().p(px(2.)).italic().child(emote.body.clone())),
            )
            .into_any_element(),
        MessageType::Image(image) if as_reply => div()
            .child(icon_text("image-png", image.body.clone()))
            .into_any_element(),
        MessageType::Image(image) => {
            let aspect_ratio = if let Some(image_info) = image.info.as_ref()
                && let Some(width) = image_info.width
                && let Some(height) = image_info.height
            {
                Some(i64::from(width) as f32 / i64::from(height) as f32)
            } else {
                None
            };

            let bounds = window.use_state(cx, |_, _| None);

            div()
                .with_anchorer({
                    let bounds_entity = bounds.clone();
                    move |david, bounds, _, cx| {
                        bounds_entity.write(cx, Some(bounds));

                        david
                    }
                })
                .when_some(bounds.read(cx).clone(), |david, bounds| {
                    let width = bounds.size.width.as_f32().min(500.);
                    let height = width / aspect_ratio.unwrap_or(1.);

                    david.child(
                        div()
                            .child(
                                mxc_image(image.source.clone())
                                    .size_policy(SizePolicy::Constrain(width, height)),
                            )
                            .when_else(
                                aspect_ratio.is_some(),
                                |david| david.w(px(width)).h(px(height)),
                                |david| david.min_w(px(100.)).min_h(px(30.)),
                            ),
                    )
                })
                .into_any_element()
        }
        MessageType::Text(text) => div()
            .child(text_message(
                as_reply,
                room,
                displayed_room,
                on_user_action,
                &text.body,
                &text.formatted,
                window,
                cx,
            ))
            .into_any_element(),
        MessageType::Notice(notice) => {
            let theme = cx.theme();

            div()
                .font_family(theme.monospaced_font_family.clone())
                .child(text_message(
                    as_reply,
                    room,
                    displayed_room,
                    on_user_action,
                    &notice.body,
                    &notice.formatted,
                    window,
                    cx,
                ))
                .into_any_element()
        }
        MessageType::File(file) => {
            let file = file.clone();
            let media_file_entity = cx.update_global::<SessionManager, _>(|session_manager, cx| {
                let media_cache = session_manager.media();
                let media_cache_entry = MediaCacheEntry::from(file.source.clone());
                media_cache.media_file_lazy(media_cache_entry, cx)
            });
            let media_file_entity_2 = media_file_entity.clone();
            let media_file_entity_3 = media_file_entity.clone();
            let media_file = media_file_entity.read(cx);
            let theme = cx.global::<Theme>();

            let file_name = file.filename();

            div()
                .p(px(2.))
                .bg(rgba(0x00C8FF10))
                .rounded(theme.border_radius)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .child(
                            icon(
                                file.info
                                    .as_ref()
                                    .and_then(|file_info| file_info.mimetype.clone())
                                    .map(|mimetype| mimetype.replace("/", "-"))
                                    .unwrap_or("application-octet-stream".into()),
                            )
                            .size(24.),
                        )
                        .child(file.filename().to_string())
                        .child(match media_file.media_state {
                            MediaState::Idle | MediaState::Failed => button("download-button")
                                .child(icon("cloud-download"))
                                .on_click(move |_, _, cx| {
                                    download_file(file.clone(), media_file_entity.clone(), cx);
                                })
                                .into_any_element(),
                            MediaState::Loading => spinner().size(px(16.)).into_any_element(),
                            MediaState::Loaded(_) => button("open-button")
                                .child(icon("document-open"))
                                .with_menu(vec![
                                    ContextMenuItem::separator()
                                        .label(tr!(
                                            "FILE_OPEN_MENU_HEADER",
                                            "For downloaded file {{filename}}",
                                            filename:Quote = file_name
                                        ))
                                        .build(),
                                    ContextMenuItem::menu_item()
                                        .label(tr!("FILE_OPEN", "Open"))
                                        .icon("document-open")
                                        .on_triggered(move |_, _, cx| {
                                            let media_file = media_file_entity_2.read(cx);

                                            let MediaState::Loaded(media_file) =
                                                &media_file.media_state
                                            else {
                                                return;
                                            };

                                            cx.open_with_system(media_file.path())
                                        })
                                        .build(),
                                    ContextMenuItem::menu_item()
                                        .label(tr!("FILE_SAVE_AS", "Save As..."))
                                        .icon("document-save-as")
                                        .on_triggered(move |_, _, cx| {
                                            save_file(
                                                file.clone(),
                                                media_file_entity_3.clone(),
                                                cx,
                                            );
                                        })
                                        .build(),
                                ])
                                .into_any_element(),
                        }),
                )
                .into_any_element()
        }
        MessageType::VerificationRequest(verification_request) => {
            "Key Verification Request".into_any_element()
        }
        _ => message_error_item(
            "dialog-warning",
            tr!("MESSAGE_UNSUPPORTED", "Unsupported Message"),
            cx,
        )
        .into_any_element(),
    }
}

#[derive(Clone)]
struct AuthorFlyoutInformation {
    bounds: Bounds<Pixels>,
    author: Entity<Option<RoomMember>>,
}

fn text_message(
    as_reply: bool,
    room: Entity<OpenRoom>,
    displayed_room: Entity<DisplayedRoom>,
    on_user_action: Rc<Box<AuthorFlyoutUserActionListener>>,
    body: &String,
    formatted: &Option<FormattedBody>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let current_link_confirmation = window.use_state(cx, |_, _| None);
    let author_flyout_information_entity = window.use_state(cx, |_, _| None);

    let body = match &formatted {
        Some(FormattedBody {
            format: MessageFormat::Html,
            body,
        }) => TextView::html("html-text", format!("<body>{body}</body>"), window, cx)
            .on_link_clicked({
                let current_link_confirmation = current_link_confirmation.clone();
                let author_flyout_information = author_flyout_information_entity.clone();
                let room = room.clone();
                move |event, _, cx| {
                    info!("Link clicked: {}", event.url);
                    if let Ok(uri) = MatrixToUri::parse(&event.url) {
                        match uri.id() {
                            MatrixId::User(user_id) => {
                                let author = cx.new(|_| None);

                                let room = room.read(cx).room.clone().unwrap();
                                cx.spawn({
                                    let author = author.clone();
                                    let user_id = user_id.clone();
                                    async move |cx: &mut AsyncApp| {
                                        if let Ok(room_member) = cx
                                            .spawn_tokio(
                                                async move { room.get_member(&user_id).await },
                                            )
                                            .await
                                        {
                                            author.write(cx, room_member)
                                        }
                                    }
                                })
                                .detach();

                                author_flyout_information.write(
                                    cx,
                                    Some(AuthorFlyoutInformation {
                                        bounds: event.bounds,
                                        author,
                                    }),
                                );
                            }
                            _ => {}
                        }
                    } else {
                        // Ask the user if they want to go to this link
                        current_link_confirmation.write(cx, Some(event.url.clone()));
                    }
                }
            })
            .into_any_element(),
        _ => body.clone().into_any_element(),
    };

    let theme = cx.global::<Theme>();
    let current_link = current_link_confirmation.read(cx).clone();
    let author_flyout_information = author_flyout_information_entity.read(cx).as_ref();
    div()
        .flex()
        .w_full()
        .max_w_full()
        .child(
            div()
                .max_w_full()
                .p(px(6.))
                .when_else(
                    as_reply,
                    |david| david.bg(rgba(0x00C8FF05)),
                    |david| david.bg(rgba(0x00C8FF10)),
                )
                .rounded(theme.border_radius)
                .child(body),
        )
        .child(
            dialog_box("link-open-confirmation")
                .render_as_deferred(true)
                .visible(current_link.is_some())
                .title(tr!("LINK_OPEN_CONFIRMATION", "Visit Link"))
                .content(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .child(tr!(
                            "LINK_OPEN_CONFIRMATION_PROMPT",
                            "Visit the following link?"
                        ))
                        .child(
                            layer()
                                .flex()
                                .items_center()
                                .p(px(4.))
                                .gap(px(4.))
                                .child(
                                    div()
                                        .flex_grow()
                                        .child(current_link.unwrap_or_default().to_string()),
                                )
                                .child(
                                    button("copy-link")
                                        .flat()
                                        .child(icon("edit-copy"))
                                        .on_click({
                                            let current_link_confirmation =
                                                current_link_confirmation.clone();
                                            move |_, _, cx| {
                                                if let Some(link) =
                                                    current_link_confirmation.read(cx).clone()
                                                {
                                                    cx.write_to_clipboard(
                                                        ClipboardItem::new_string(link.to_string()),
                                                    )
                                                }
                                            }
                                        }),
                                ),
                        )
                        .child(div().text_color(theme.foreground.disabled()).child(tr!(
                            "LINK_OPEN_CONFIRMATION_WARNING",
                            "Make sure it's a place you trust; the web can be scary!"
                        ))),
                )
                .standard_button(StandardButton::Cancel, {
                    let current_link_confirmation = current_link_confirmation.clone();
                    move |_, _, cx| {
                        current_link_confirmation.write(cx, None);
                    }
                })
                .button(
                    button("link-open-button")
                        .child(icon_text("dialog-ok", tr!("LINK_OPEN", "Visit Link")))
                        .on_click({
                            let current_link_confirmation = current_link_confirmation.clone();
                            move |_, _, cx| {
                                current_link_confirmation.update(cx, |current_link, cx| {
                                    if let Some(link) = current_link {
                                        cx.open_url(link);
                                    }
                                    *current_link = None;
                                    cx.notify()
                                })
                            }
                        }),
                ),
        )
        .child(author_flyout(
            author_flyout_information
                .map(|author_flyout_information| author_flyout_information.bounds)
                .unwrap_or_default(),
            author_flyout_information.is_some(),
            author_flyout_information
                .map(|author_flyout_information| author_flyout_information.author.clone())
                .clone()
                .unwrap_or_else(|| cx.new(|_| None)),
            room,
            displayed_room,
            {
                let author_flyout_information_entity = author_flyout_information_entity.clone();
                move |_, _, cx| {
                    author_flyout_information_entity.write(cx, None);
                }
            },
            move |event, window, cx| on_user_action(event, window, cx),
        ))
        .into_any_element()
}

fn download_file(file: FileMessageEventContent, media_file: Entity<MediaFile>, cx: &mut App) {
    // Trigger a job
    media_file.update(cx, |media_file, cx| {
        media_file.request_media(
            Some(file.filename().into()),
            file.info.and_then(|info| info.mimetype),
            true,
            cx,
        );
    })
}

fn save_file(file: FileMessageEventContent, media_file: Entity<MediaFile>, cx: &mut App) {
    let user_dirs = UserDirs::new().unwrap();
    let prompt = cx.prompt_for_new_path(user_dirs.download_dir().unwrap(), Some(file.filename()));
    let media_file = media_file.read(cx);
    let MediaState::Loaded(media_file) = &media_file.media_state else {
        return;
    };

    let path = media_file.path().to_path_buf();
    cx.spawn(async move |_: &mut AsyncApp| {
        if let Some(new_path) = prompt.await.ok().and_then(|result| result.ok()).flatten() {
            // Save the file to the path
            copy(path, new_path).unwrap();
        };
    })
    .detach();
}
