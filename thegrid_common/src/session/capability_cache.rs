use crate::tokio_helper::TokioHelper;
use gpui::{AsyncApp, Context, WeakEntity};
use matrix_sdk::Client;
use matrix_sdk::ruma::api::client::discovery::get_capabilities::v3::{
    Capabilities, RoomVersionsCapability,
};
use std::time::Duration;

pub struct CapabilityCache {
    room_versions: RoomVersionsCapability,
}

impl CapabilityCache {
    pub fn new(client: &Client, cx: &mut Context<Self>) -> Self {
        cx.spawn({
            let client = client.clone();
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                loop {
                    let client = client.clone();
                    let capabilities = client.homeserver_capabilities();
                    if let Ok(capabilities) = cx
                        .spawn_tokio(async move { capabilities.room_versions().await })
                        .await
                    {
                        if weak_this
                            .update(cx, |this, cx| {
                                this.room_versions = capabilities;
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
        }
    }

    pub fn supported_room_versions(&self) -> &RoomVersionsCapability {
        &self.room_versions
    }
}
