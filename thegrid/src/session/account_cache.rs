use crate::tokio_helper::TokioHelper;
use gpui::{App, AppContext, AsyncApp, Entity, WeakEntity};
use matrix_sdk::ruma::OwnedMxcUri;
use matrix_sdk::ruma::events::room::member::SyncRoomMemberEvent;
use matrix_sdk::{Client, Room};

pub struct AccountCache {
    display_name: Option<String>,
    avatar_url: Option<OwnedMxcUri>,
}

enum CacheMutation {
    SetDisplayName(Option<String>),
    SetAvatarUrl(Option<OwnedMxcUri>),
}

impl AccountCache {
    pub fn new(client: &Client, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let (tx, rx) = async_channel::bounded(1);

            let client_clone = client.clone();
            cx.spawn(
                async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                    let client = client_clone.clone();

                    let display_name = cx
                        .spawn_tokio(async move { client.account().get_display_name().await })
                        .await
                        .ok()
                        .flatten();

                    let client = client_clone.clone();

                    let avatar_url = cx
                        .spawn_tokio(async move { client.account().get_avatar_url().await })
                        .await
                        .ok()
                        .flatten();

                    let _ = weak_this.update(cx, |this, cx| {
                        this.display_name = display_name;
                        this.avatar_url = avatar_url;
                        cx.notify()
                    });
                },
            )
            .detach();

            let tx_clone = tx.clone();
            client.add_event_handler(|event: SyncRoomMemberEvent, room: Room| async move {
                let own_user_id = room.own_user_id();
                if *event.state_key() == *own_user_id
                    && let Some(original) = event.as_original()
                {
                    tx_clone
                        .send(CacheMutation::SetDisplayName(
                            original.content.displayname.clone(),
                        ))
                        .await
                        .unwrap();
                    tx_clone
                        .send(CacheMutation::SetAvatarUrl(
                            original.content.avatar_url.clone(),
                        ))
                        .await
                        .unwrap();
                }
            });

            cx.spawn(
                async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                    loop {
                        let mutation = rx.recv().await.unwrap();
                        weak_this
                            .update(cx, |this, cx| {
                                match mutation {
                                    CacheMutation::SetDisplayName(display_name) => {
                                        this.display_name = display_name;
                                    }
                                    CacheMutation::SetAvatarUrl(avatar_url) => {
                                        this.avatar_url = avatar_url;
                                    }
                                };
                                cx.notify();
                            })
                            .unwrap()
                    }
                },
            )
            .detach();

            AccountCache {
                display_name: None,
                avatar_url: None,
            }
        })
    }

    pub fn display_name(&self) -> Option<String> {
        self.display_name.clone()
    }

    pub fn avatar_url(&self) -> Option<OwnedMxcUri> {
        self.avatar_url.clone()
    }
}
