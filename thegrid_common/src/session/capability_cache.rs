use crate::tokio_helper::TokioHelper;
use gpui::{AsyncApp, Context, WeakEntity};
use matrix_sdk::ruma::api::client::discovery::get_capabilities::v3::RoomVersionsCapability;
use matrix_sdk::Client;
use std::time::Duration;

pub struct CapabilityCache {
    room_versions: RoomVersionsCapability,
    can_change_password: bool,
}

impl CapabilityCache {
    pub fn new(client: &Client, cx: &mut Context<Self>) -> Self {
        cx.spawn({
            let client = client.clone();
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                loop {
                    let client = client.clone();
                    let capabilities = client.homeserver_capabilities();
                    if let Ok(room_versions) = cx
                        .spawn_tokio({
                            let capabilities = capabilities.clone();
                            async move { capabilities.room_versions().await }
                        })
                        .await
                    {
                        if weak_this
                            .update(cx, |this, cx| {
                                this.room_versions = room_versions;
                                cx.notify();
                            })
                            .is_err()
                        {
                            return;
                        }
                    }

                    if let Ok(can_change_password) = cx
                        .spawn_tokio({
                            let capabilities = capabilities.clone();
                            async move { capabilities.can_change_password().await }
                        })
                        .await
                    {
                        if weak_this
                            .update(cx, |this, cx| {
                                this.can_change_password = can_change_password;
                                cx.notify();
                            })
                            .is_err()
                        {
                            return;
                        }
                    }

                    // Refresh capabilities once an hour
                    cx.background_executor()
                        .timer(Duration::from_hours(1))
                        .await;
                }
            }
        })
        .detach();

        Self {
            room_versions: Default::default(),
            can_change_password: true,
        }
    }

    pub fn supported_room_versions(&self) -> &RoomVersionsCapability {
        &self.room_versions
    }

    pub fn can_change_password(&self) -> bool {
        self.can_change_password
    }
}
