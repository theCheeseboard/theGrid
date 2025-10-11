use crate::tokio_helper::TokioHelper;
use gpui::{App, AppContext, AsyncApp, Entity, WeakEntity};
use matrix_sdk::encryption::identities::UserIdentity;
use matrix_sdk::ruma::OwnedMxcUri;
use matrix_sdk::ruma::events::room::member::SyncRoomMemberEvent;
use matrix_sdk::{Client, Room};
use std::time::Duration;

pub struct AccountCache {
    display_name: Option<String>,
    avatar_url: Option<OwnedMxcUri>,
    identity: Option<UserIdentity>,
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

            let client_clone = client.clone();
            cx.spawn(
                async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                    loop {
                        let client = client_clone.clone();
                        let user_id = client.user_id().unwrap().to_owned();
                        if let Ok(identity) = cx
                            .spawn_tokio(async move {
                                client.encryption().request_user_identity(&user_id).await
                            })
                            .await
                        {
                            let _ = weak_this.update(cx, |this, cx| {
                                this.identity = identity;
                                cx.notify()
                            });
                        }

                        cx.background_executor()
                            .timer(Duration::from_secs(10))
                            .await;
                    }
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
                identity: None,
            }
        })
    }

    pub fn display_name(&self) -> Option<String> {
        self.display_name.clone()
    }

    pub fn avatar_url(&self) -> Option<OwnedMxcUri> {
        self.avatar_url.clone()
    }

    pub fn identity(&self) -> Option<UserIdentity> {
        self.identity.clone()
    }

    pub fn we_are_verified(&self) -> bool {
        if let Some(identity) = self.identity()
            && identity.is_verified()
        {
            true
        } else {
            false
        }
    }
}
