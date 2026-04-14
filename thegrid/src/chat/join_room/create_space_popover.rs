use crate::chat::displayed_room::DisplayedRoom;
use cntp_i18n::tr;
use contemporary::components::admonition::AdmonitionSeverity;
use contemporary::components::button::{button, ButtonMenuOpenPolicy};
use contemporary::components::checkbox::{radio_button, CheckState, CheckedChangeEvent};
use contemporary::components::constrainer::constrainer;
use contemporary::components::context_menu::ContextMenuItem;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon::icon;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::pager::pager;
use contemporary::components::pager::slide_horizontal_animation::SlideHorizontalAnimation;
use contemporary::components::popover::popover;
use contemporary::components::spinner::spinner;
use contemporary::components::subtitle::subtitle;
use contemporary::components::text_field::TextField;
use contemporary::components::toast::Toast;
use contemporary::styling::theme::ThemeStorage;
use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, AppContext, AsyncWindowContext, Context, Entity, IntoElement,
    ParentElement, Render, Styled, WeakEntity, Window,
};
use matrix_sdk::ruma::api::client::room::create_room::v3::{CreationContent, Request};
use matrix_sdk::ruma::room::{JoinRule, RoomType};
use matrix_sdk::ruma::serde::Raw;
use matrix_sdk_ui::spaces::SpaceRoom;
use thegrid_common::mxc_image::{mxc_image, SizePolicy};
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::tokio_helper::TokioHelper;

pub struct CreateSpacePopover {
    visible: bool,
    processing: bool,

    name_field: Entity<TextField>,
    is_private_room: bool,
    federation: bool,

    editable_spaces: Entity<Option<Vec<SpaceRoom>>>,
    create_in_space: Option<SpaceRoom>,

    displayed_room: Entity<DisplayedRoom>,
}

impl CreateSpacePopover {
    pub fn new(displayed_room: Entity<DisplayedRoom>, cx: &mut Context<Self>) -> Self {
        Self {
            visible: false,
            processing: false,
            name_field: cx.new(|cx| {
                let mut text_field = TextField::new("name", cx);
                text_field.set_placeholder(tr!("SPACE_NAME", "Space Name").to_string().as_str());
                text_field
            }),
            is_private_room: true,
            federation: true,
            editable_spaces: cx.new(|cx| None),
            create_in_space: None,
            displayed_room,
        }
    }

    pub fn open(&mut self, create_in_space: Option<SpaceRoom>, cx: &mut Context<Self>) {
        self.visible = true;
        self.is_private_room = true;
        self.federation = true;
        self.processing = false;
        self.create_in_space = create_in_space;

        let session_manager = cx.global::<SessionManager>();
        self.editable_spaces = session_manager
            .spaces()
            .update(cx, |spaces, cx| spaces.get_editable_spaces(cx));

        cx.notify()
    }

    pub fn create_room(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let room_name = self.name_field.read(cx).text();
        if room_name.trim().is_empty() {
            self.name_field.update(cx, |name_field, cx| {
                name_field.flash_error(window, cx);
            });
            return;
        }

        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx).clone();
        let space_service = session_manager.spaces().read(cx).space_service();

        let mut request = Request::new();
        request.name = Some(room_name.to_string());
        request.creation_content = Some(
            Raw::new(&{
                let mut creation_content = CreationContent::new();
                creation_content.federate = self.federation;
                creation_content.room_type = Some(RoomType::Space);
                creation_content
            })
            .unwrap(),
        );

        self.processing = true;
        cx.notify();

        let is_private_room = self.is_private_room;
        let create_in_space = self.create_in_space.clone();

        cx.spawn_in(
            window,
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncWindowContext| match cx
                .spawn_tokio(async move { client.create_room(request).await })
                .await
            {
                Ok(room) => {
                    let room_id = room.room_id().to_owned();

                    let _ = cx
                        .spawn_tokio({
                            let create_in_space = create_in_space.clone();
                            async move {
                                room.privacy_settings()
                                    .update_join_rule(match is_private_room {
                                        false => JoinRule::Public,
                                        true => JoinRule::Private,
                                    })
                                    .await
                            }
                        })
                        .await;

                    if let Some(create_in_space) = create_in_space {
                        let child = room_id.clone();
                        let parent = create_in_space.room_id;
                        let _ = cx
                            .spawn_tokio(async move {
                                space_service.add_child_to_space(child, parent).await
                            })
                            .await;
                    }

                    let _ = weak_this.update(cx, |this, cx| {
                        this.displayed_room.write(cx, DisplayedRoom::Room(room_id));
                        this.visible = false;
                        cx.notify();
                    });
                }
                Err(e) => {
                    let _ = cx.update(|window, cx| {
                        weak_this.update(cx, |this, cx| {
                            Toast::new()
                                .title(
                                    tr!("SPACE_CREATE_ERROR_TITLE", "Unable to create the space")
                                        .as_ref(),
                                )
                                .body(
                                    tr!(
                                        "SPACE_CREATE_ERROR_TEXT",
                                        "The space could not be created",
                                    )
                                    .as_ref(),
                                )
                                .severity(AdmonitionSeverity::Error)
                                .post(window, cx);

                            this.processing = false;
                            cx.notify();
                        })
                    });
                }
            },
        )
        .detach();
    }

    fn create_space_page_contents(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        constrainer("create-room-constrainer").child(
            layer()
                .flex()
                .flex_col()
                .p(px(8.))
                .w_full()
                .child(subtitle(tr!("CREATE_SPACE_OPTIONS", "Space Options")))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(tr!(
                            "CREATE_SPACE_DESCRIPTION",
                            "Create a space to collect related rooms into a group"
                        ))
                        .child(self.name_field.clone())
                        .child(
                            layer().p(px(8.)).flex().flex_col().child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(4.))
                                    .child(tr!("CREATE_ROOM_SPACE"))
                                    .child(div().flex_grow())
                                    .when_some(self.create_in_space.as_ref(), |david, space| {
                                        david.child(
                                            mxc_image(space.avatar_url.clone())
                                                .fallback_image(&space.room_id)
                                                .fixed_square(px(32.))
                                                .size_policy(SizePolicy::Fit)
                                                .rounded(theme.border_radius),
                                        )
                                    })
                                    .child(
                                        self.create_in_space
                                            .as_ref()
                                            .map(|space| space.display_name.clone())
                                            .unwrap_or(tr!("CREATE_ROOM_SPACE_NONE").into()),
                                    )
                                    .child({
                                        let mut space_menu = vec![
                                            ContextMenuItem::menu_item()
                                                .label(tr!("CREATE_ROOM_SPACE_NONE"))
                                                .on_triggered(cx.listener(move |this, _, _, cx| {
                                                    this.create_in_space = None;
                                                    cx.notify()
                                                }))
                                                .build(),
                                        ];
                                        space_menu.extend(
                                            self.editable_spaces
                                                .read(cx)
                                                .clone()
                                                .unwrap_or_default()
                                                .into_iter()
                                                .map(|space| {
                                                    ContextMenuItem::menu_item()
                                                        .label(space.display_name.clone())
                                                        .on_triggered(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.create_in_space =
                                                                    Some(space.clone());
                                                                cx.notify()
                                                            },
                                                        ))
                                                        .build()
                                                }),
                                        );

                                        button("space-selection-button")
                                            .child(icon("arrow-down"))
                                            .with_menu_open_policy(ButtonMenuOpenPolicy::AnyClick)
                                            .with_menu(space_menu)
                                    }),
                            ),
                        )
                        .child(
                            layer()
                                .p(px(8.))
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                .child(
                                    radio_button("space-visibility-private")
                                        .label(tr!(
                                            "CREATE_SPACE_VISIBILITY_PRIVATE",
                                            "Create Private Space"
                                        ))
                                        .when(self.is_private_room, |david| david.checked())
                                        .on_checked_changed(cx.listener(
                                            |this, event: &CheckedChangeEvent, _, cx| {
                                                if event.check_state == CheckState::On {
                                                    this.is_private_room = true;
                                                    cx.notify()
                                                }
                                            },
                                        )),
                                )
                                .child(
                                    radio_button("space-visibility-public")
                                        .label(tr!(
                                            "CREATE_SPACE_VISIBILITY_PUBLIC",
                                            "Create Public Space"
                                        ))
                                        .when(!self.is_private_room, |david| david.checked())
                                        .on_checked_changed(cx.listener(
                                            |this, event: &CheckedChangeEvent, _, cx| {
                                                if event.check_state == CheckState::On {
                                                    this.is_private_room = false;
                                                    cx.notify()
                                                }
                                            },
                                        )),
                                ),
                        )
                        .child(
                            button("do-create")
                                .child(icon_text("list-add", tr!("CREATE_SPACE", "Create Space")))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.create_room(window, cx)
                                })),
                        ),
                ),
        )
    }
}

impl Render for CreateSpacePopover {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        popover("create-space-popover")
            .visible(self.visible)
            .size_neg(100.)
            .anchor_bottom()
            .content(
                pager("create-space-pager", if self.processing { 1 } else { 0 })
                    .animation(SlideHorizontalAnimation::new())
                    .size_full()
                    .page(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(9.))
                            .child(
                                grandstand("create-space-grandstand")
                                    .text(tr!("CREATE_SPACE_TITLE", "Create Space"))
                                    .on_back_click(cx.listener(move |this, _, _, cx| {
                                        this.visible = false;
                                        cx.notify()
                                    })),
                            )
                            .child(self.create_space_page_contents(cx))
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
                    ),
            )
    }
}
