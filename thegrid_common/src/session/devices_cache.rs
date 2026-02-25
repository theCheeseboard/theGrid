use crate::tokio_helper::TokioHelper;
use gpui::http_client::anyhow;
use gpui::{App, AppContext, AsyncApp, Entity, WeakEntity};
use matrix_sdk::Client;
use matrix_sdk::ruma::api::client::device::Device;
use matrix_sdk::stream::StreamExt;
use std::time::Duration;

pub struct DevicesCache {
    devices: Vec<CachedDevice>,
    pub is_last_device: bool,
}

enum CacheMutation {}

#[derive(Clone)]
pub struct CachedDevice {
    pub inner: Device,
    pub encryption_status: Option<matrix_sdk::encryption::identities::Device>,
}

impl DevicesCache {
    pub fn new(client: &Client, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let user_id = client.user_id().unwrap().to_owned();

            let client_clone = client.clone();
            cx.spawn(
                async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                    let client = client_clone.clone();
                    let mut devices_stream = cx
                        .spawn_tokio(async move { client.encryption().devices_stream().await })
                        .await
                        .unwrap();

                    loop {
                        let client = client_clone.clone();
                        let Ok(devices) =
                            cx.spawn_tokio(async move { client.devices().await }).await
                        else {
                            return;
                        };

                        let mut cached_devices = Vec::new();
                        for device in devices.devices {
                            let client = client_clone.clone();
                            let user_id = user_id.clone();
                            let device_id = device.device_id.clone();
                            let device_encryption_status = cx
                                .spawn_tokio(async move {
                                    client.encryption().get_device(&user_id, &device_id).await
                                })
                                .await
                                .ok()
                                .flatten();
                            cached_devices.push(CachedDevice {
                                inner: device,
                                encryption_status: device_encryption_status,
                            });
                        }

                        let client = client_clone.clone();
                        let is_last_device = cx
                            .spawn_tokio(async move {
                                client.encryption().recovery().is_last_device().await
                            })
                            .await
                            .unwrap_or(true);

                        if weak_this
                            .update(cx, |this, cx| {
                                this.devices = cached_devices;
                                this.is_last_device = is_last_device;
                                cx.notify();
                            })
                            .is_err()
                        {
                            return;
                        }

                        let _ = devices_stream.next().await;
                    }
                },
            )
            .detach();

            DevicesCache {
                devices: Vec::new(),
                is_last_device: true,
            }
        })
    }

    pub fn unverified_devices(&self) -> Vec<&CachedDevice> {
        self.devices
            .iter()
            .filter(|device| {
                if let Some(encryption_status) = &device.encryption_status {
                    !encryption_status.is_verified()
                } else {
                    false
                }
            })
            .collect()
    }

    pub fn devices(&self) -> Vec<&CachedDevice> {
        self.devices.iter().collect()
    }

    pub fn is_last_device(&self) -> bool {
        self.is_last_device
    }
}
