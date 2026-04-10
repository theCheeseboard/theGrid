use crate::tokio_helper::TokioHelper;
use gpui::{AsyncApp, Context, WeakEntity};
use matrix_sdk::Client;
use matrix_sdk::ruma::api::client::discovery::get_capabilities::v3::Capabilities;
use std::time::Duration;

pub struct CapabilityCache {
    capabilities: Capabilities,
}

impl CapabilityCache {
    pub fn new(client: &Client, cx: &mut Context<Self>) -> Self {
        cx.spawn({
            let client = client.clone();
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                loop {
                    let client = client.clone();
                    if let Ok(capabilities) = cx
                        .spawn_tokio(async move { client.get_capabilities().await })
                        .await
                    {
                        if weak_this
                            .update(cx, |this, cx| {
                                this.capabilities = capabilities;
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
            capabilities: Default::default(),
        }
    }

    pub fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }
}
