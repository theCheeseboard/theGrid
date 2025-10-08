use std::time::Duration;
use crate::tokio_helper::TokioHelper;
use gpui::{App, AppContext, AsyncApp, Entity, WeakEntity};
use matrix_sdk::Client;
use matrix_sdk::ruma::api::client::device::Device;

pub struct DevicesCache {
    devices: Vec<CachedDevice>,
}

enum CacheMutation {}

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

                        if weak_this
                            .update(cx, |this, cx| {
                                this.devices = cached_devices;
                                cx.notify();
                            })
                            .is_err()
                        {
                            return;
                        }
                        
                        cx.background_executor().timer(Duration::from_secs(5)).await;
                    }
                },
            )
            .detach();

            DevicesCache {
                devices: Vec::new(),
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
}
