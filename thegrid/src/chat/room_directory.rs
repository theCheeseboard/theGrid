use crate::chat::displayed_room::DisplayedRoom;
use crate::mxc_image::{SizePolicy, mxc_image};
use async_channel::Sender;
use cntp_i18n::tr;
use contemporary::components::grandstand::grandstand;
use contemporary::components::layer::layer;
use contemporary::components::pager::pager;
use contemporary::components::spinner::spinner;
use contemporary::components::subtitle::subtitle;
use contemporary::styling::theme::{Theme, ThemeStorage, VariableColor};
use gpui::{
    AnyElement, AppContext, AsyncApp, Context, Entity, InteractiveElement, IntoElement,
    ListAlignment, ListState, ParentElement, Render, Styled, WeakEntity, Window, div, list, px,
    rgb,
};
use imbl::Vector;
use matrix_sdk::room_directory_search::{RoomDescription, RoomDirectorySearch};
use matrix_sdk::stream::StreamExt;
use matrix_sdk::{Error, OwnedServerName};
use thegrid::session::session_manager::SessionManager;
use thegrid::tokio_helper::TokioHelper;

mod directory_view;

pub struct RoomDirectory {
    server_name: OwnedServerName,
    state: DirectorySearchState,
    query: String,
    busy: bool,

    current_results: Vector<RoomDescription>,
    list_state: ListState,
    next_page_channel: Sender<()>,
}

enum DirectorySearchState {
    Searching,
    ResultsReady,
    Error(String),
}

enum Feedback {
    ClearBusy,
    SetReady,
}

impl RoomDirectory {
    pub fn new(
        server_name: OwnedServerName,
        displayed_room: Entity<DisplayedRoom>,
        cx: &mut Context<Self>,
    ) -> Self {
        let (tx, _) = async_channel::bounded(1);

        let list_state = ListState::new(0, ListAlignment::Top, px(200.));

        let mut this = Self {
            server_name,
            state: DirectorySearchState::Searching,
            query: "".to_string(),
            busy: false,

            current_results: Vector::new(),
            list_state,
            next_page_channel: tx,
        };

        this.run_search(cx);

        this
    }

    pub fn run_search(&mut self, cx: &mut Context<Self>) {
        self.state = DirectorySearchState::Searching;
        cx.notify();

        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx).clone();

        let filter = if self.query.is_empty() {
            None
        } else {
            Some(self.query.clone())
        };
        let server = self.server_name.clone();

        let (tx_next_page, rx_next_page) = async_channel::bounded(1);
        let (tx_feedback, rx_feedback) = async_channel::bounded(1);
        self.next_page_channel = tx_next_page;

        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                loop {
                    let Ok(feedback) = rx_feedback.recv().await else {
                        return;
                    };

                    if weak_this
                        .update(cx, |this, cx| {
                            match feedback {
                                Feedback::ClearBusy => {
                                    this.busy = false;
                                }
                                Feedback::SetReady => {
                                    this.state = DirectorySearchState::ResultsReady;
                                }
                            }
                            cx.notify();
                        })
                        .is_err()
                    {
                        return;
                    };
                }
            },
        )
        .detach();

        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let mut room_directory_search = RoomDirectorySearch::new(client);

                let (og_vector, mut updates) = room_directory_search.results();
                if weak_this
                    .update(cx, |this, cx| {
                        this.current_results = og_vector;
                    })
                    .is_err()
                {
                    return;
                };

                let weak_this_2 = weak_this.clone();
                cx.spawn(async move |cx: &mut AsyncApp| {
                    while let Some(diffs) = updates.next().await {
                        if weak_this_2
                            .update(cx, |this, cx| {
                                for diff in diffs {
                                    diff.apply(&mut this.current_results);
                                }
                                this.list_state.reset(this.current_results.len());
                                cx.notify();
                            })
                            .is_err()
                        {
                            return;
                        }
                    }
                })
                .detach();

                if let Err(e) = cx
                    .spawn_tokio::<_, (), Error>(async move {
                        room_directory_search
                            .search(filter, 10, Some(server))
                            .await?;

                        if tx_feedback.send(Feedback::SetReady).await.is_err() {
                            return Ok(());
                        };

                        loop {
                            if rx_next_page.recv().await.is_err() {
                                return Ok(());
                            };

                            room_directory_search.next_page().await?;

                            if tx_feedback.send(Feedback::ClearBusy).await.is_err() {
                                return Ok(());
                            };
                        }
                    })
                    .await
                {
                    let _ = weak_this.update(cx, |this, cx| {
                        this.state = DirectorySearchState::Error(e.to_string());
                        cx.notify();
                    });
                }
            },
        )
        .detach();
    }

    fn render_list_item(
        &mut self,
        i: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let room_description = &self.current_results[i];

        div()
            .overflow_x_hidden()
            .py(px(2.))
            .child(
                layer()
                    .overflow_x_hidden()
                    .flex()
                    .gap(px(4.))
                    .p(px(4.))
                    .child(
                        mxc_image(room_description.avatar_url.clone())
                            .rounded(theme.border_radius)
                            .size(px(40.))
                            .size_policy(SizePolicy::Fit),
                    )
                    .child(
                        div()
                            .overflow_x_hidden()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(div().child(room_description.name.clone().unwrap_or("".into())))
                            .child(
                                div()
                                    .overflow_x_hidden()
                                    .text_color(theme.foreground.disabled())
                                    .child(room_description.topic.clone().unwrap_or("".into())),
                            ),
                    ),
            )
            .into_any_element()
    }
}

impl Render for RoomDirectory {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>();
        let server_name_string = self.server_name.to_string();

        div()
            .bg(theme.background)
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .child(
                grandstand("directory-grandstand")
                    .text(tr!(
                        "ROOM_DIRECTORY_TITLE",
                        "Room Directory of {{server}}",
                        server:quote = server_name_string
                    ))
                    .pt(px(36.)),
            )
            .child(
                div()
                    .flex()
                    .justify_center()
                    .size_full()
                    .gap(px(8.))
                    .child(
                        div().flex().flex_col().p(px(4.)).gap(px(4.)).child(
                            layer()
                                .p(px(8.))
                                .gap(px(8.))
                                .child(subtitle(tr!("MEMBER_LIST_FILTERS", "Filters")))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .bg(theme.button_background)
                                        .rounded(theme.border_radius)
                                        .child("something goes here"),
                                ),
                        ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .max_w(px(600.))
                            .size_full()
                            .px(px(8.))
                            .gap(px(8.))
                            .child(
                                pager(
                                    "directory-pager",
                                    match self.state {
                                        DirectorySearchState::Searching => 0,
                                        DirectorySearchState::ResultsReady => 1,
                                        DirectorySearchState::Error(_) => 1,
                                    },
                                )
                                .page(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_around()
                                        .size_full()
                                        .child(spinner())
                                        .into_any_element(),
                                )
                                .page(
                                    list(
                                        self.list_state.clone(),
                                        cx.processor(Self::render_list_item),
                                    )
                                    .size_full()
                                    .into_any_element(),
                                )
                                .size_full(),
                            ),
                    ),
            )
    }
}
