use crate::tokio_helper::TokioHelper;
use gpui::{App, AppContext, AsyncApp, Entity, WeakEntity};
use matrix_sdk::encryption::VerificationState;
use matrix_sdk::ruma::OwnedMxcUri;
use matrix_sdk::ruma::api::client::discovery::get_authorization_server_metadata::v1::{
    AccountManagementAction, AuthorizationServerMetadata,
};
use matrix_sdk::ruma::events::room::member::SyncRoomMemberEvent;
use matrix_sdk::{AuthApi, Client, Room};

pub struct AccountCache {
    display_name: Option<String>,
    avatar_url: Option<OwnedMxcUri>,
    verification_state: VerificationState,
    oauth_metadata: Option<AuthorizationServerMetadata>,
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
                        let mut state_stream = client.encryption().verification_state();
                        state_stream.reset();

                        while let Some(state) = state_stream.next().await {
                            let _ = weak_this.update(cx, |this, cx| {
                                this.verification_state = state;
                                cx.notify()
                            });
                        }
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
                    if tx_clone
                        .send(CacheMutation::SetDisplayName(
                            original.content.displayname.clone(),
                        ))
                        .await
                        .is_err()
                    {
                        return;
                    }
                    if tx_clone
                        .send(CacheMutation::SetAvatarUrl(
                            original.content.avatar_url.clone(),
                        ))
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
            });

            cx.spawn(
                async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                    loop {
                        let mutation = rx.recv().await.unwrap();
                        if weak_this
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
                            .is_err()
                        {
                            return;
                        }
                    }
                },
            )
            .detach();

            cx.spawn({
                let client = client.clone();
                async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                    if let Some(AuthApi::OAuth(oauth_auth)) = client.auth_api() {
                        let Ok(metadata) = cx
                            .spawn_tokio(async move { oauth_auth.cached_server_metadata().await })
                            .await
                        else {
                            return;
                        };

                        let _ = weak_this.update(cx, |this, cx| {
                            this.oauth_metadata = Some(metadata);
                            cx.notify();
                        });
                    }
                }
            })
            .detach();

            AccountCache {
                display_name: None,
                avatar_url: None,
                verification_state: VerificationState::Unknown,
                oauth_metadata: None,
            }
        })
    }

    pub fn display_name(&self) -> Option<String> {
        self.display_name.clone()
    }

    pub fn avatar_url(&self) -> Option<OwnedMxcUri> {
        self.avatar_url.clone()
    }

    pub fn verification_state(&self) -> VerificationState {
        self.verification_state
    }

    pub fn oauth_metadata(&self) -> Option<&AuthorizationServerMetadata> {
        self.oauth_metadata.as_ref()
    }
}
