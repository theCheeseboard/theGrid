use cntp_i18n::tr;
use contemporary::notification::Notification;
use gpui::{App, AppContext, AsyncApp, Context, Entity, WeakEntity};
use log::{error, info};
use matrix_sdk::Client;
use matrix_sdk::encryption::verification::{
    SasVerification, Verification, VerificationRequest, VerificationRequestState,
};
use matrix_sdk::ruma::events::key::verification::VerificationMethod;
use matrix_sdk::ruma::events::key::verification::accept::ToDeviceKeyVerificationAcceptEvent;
use matrix_sdk::ruma::events::key::verification::cancel::ToDeviceKeyVerificationCancelEvent;
use matrix_sdk::ruma::events::key::verification::done::ToDeviceKeyVerificationDoneEvent;
use matrix_sdk::ruma::events::key::verification::ready::ToDeviceKeyVerificationReadyEvent;
use matrix_sdk::ruma::events::key::verification::request::ToDeviceKeyVerificationRequestEvent;
use matrix_sdk::ruma::events::key::verification::start::{
    StartMethod, ToDeviceKeyVerificationStartEvent,
};
use matrix_sdk::ruma::{OwnedDeviceId, OwnedTransactionId};

pub struct VerificationRequestsCache {
    pub pending_verification_requests: Vec<VerificationRequestDetails>,
}

#[derive(Clone)]
pub struct VerificationRequestDetails {
    pub inner: VerificationRequest,
    pub sas_state: Option<SasVerification>,
    pub device_id: Option<OwnedDeviceId>,
}

enum CacheMutation {
    Push(VerificationRequestDetails),
    Remove(OwnedTransactionId),
    Replace(OwnedTransactionId, VerificationRequestDetails),
}

impl VerificationRequestsCache {
    pub fn new(client: &Client, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let (tx, rx) = async_channel::bounded(1);

            let client_clone = client.clone();
            let tx_clone = tx.clone();
            client.add_event_handler(|event: ToDeviceKeyVerificationRequestEvent| async move {
                let verification_request = client_clone
                    .encryption()
                    .get_verification_request(&event.sender, &event.content.transaction_id)
                    .await;
                match verification_request {
                    None => {}
                    Some(verification_request) => {
                        let _ = tx_clone
                            .send(CacheMutation::Push(VerificationRequestDetails {
                                inner: verification_request,
                                sas_state: None,
                                device_id: Some(event.content.from_device),
                            }))
                            .await;
                    }
                }
            });

            let client_clone = client.clone();
            let tx_clone = tx.clone();
            client.add_event_handler(|event: ToDeviceKeyVerificationStartEvent| async move {
                let verification_request = client_clone
                    .encryption()
                    .get_verification_request(&event.sender, &event.content.transaction_id)
                    .await;
                match verification_request {
                    None => {}
                    Some(verification_request) => {
                        let sas_state = {
                            match verification_request.state() {
                                VerificationRequestState::Transitioned {
                                    verification: Verification::SasV1(sas),
                                    ..
                                } if matches!(event.content.method, StartMethod::SasV1(_)) => {
                                    match sas.accept().await {
                                        Ok(_) => Some(sas),
                                        Err(_) => None,
                                    }
                                }
                                _ => None,
                            }
                        };

                        let _ = tx_clone
                            .send(CacheMutation::Replace(
                                event.content.transaction_id,
                                VerificationRequestDetails {
                                    inner: verification_request,
                                    sas_state,
                                    device_id: Some(event.content.from_device),
                                },
                            ))
                            .await;
                    }
                }
            });

            let client_clone = client.clone();
            let tx_clone = tx.clone();
            client.add_event_handler(|event: ToDeviceKeyVerificationReadyEvent| async move {
                let verification_request = client_clone
                    .encryption()
                    .get_verification_request(&event.sender, &event.content.transaction_id)
                    .await;
                match verification_request {
                    None => {}
                    Some(verification_request) => {
                        let sas_state = if verification_request.we_started() {
                            if event
                                .content
                                .methods
                                .iter()
                                .any(|method| matches!(method, VerificationMethod::SasV1))
                            {
                                verification_request.start_sas().await.unwrap_or_else(|e| {
                                    error!("Unable to start SAS: {e}");
                                    None
                                })
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        let _ = tx_clone
                            .send(CacheMutation::Replace(
                                event.content.transaction_id,
                                VerificationRequestDetails {
                                    inner: verification_request,
                                    sas_state,
                                    device_id: None,
                                },
                            ))
                            .await;
                    }
                }
            });

            let client_clone = client.clone();
            let tx_clone = tx.clone();
            client.add_event_handler(|event: ToDeviceKeyVerificationAcceptEvent| async move {
                let verification_request = client_clone
                    .encryption()
                    .get_verification_request(&event.sender, &event.content.transaction_id)
                    .await;
                match verification_request {
                    None => {}
                    Some(verification_request) => {
                        let _ = tx_clone
                            .send(CacheMutation::Replace(
                                event.content.transaction_id,
                                VerificationRequestDetails {
                                    inner: verification_request,
                                    sas_state: None,
                                    device_id: None,
                                },
                            ))
                            .await;
                    }
                }
            });

            let client_clone = client.clone();
            let tx_clone = tx.clone();
            client.add_event_handler(|event: ToDeviceKeyVerificationDoneEvent| async move {
                let verification_request = client_clone
                    .encryption()
                    .get_verification_request(&event.sender, &event.content.transaction_id)
                    .await;
                match verification_request {
                    None => {}
                    Some(verification_request) => {
                        let _ = tx_clone
                            .send(CacheMutation::Replace(
                                event.content.transaction_id,
                                VerificationRequestDetails {
                                    inner: verification_request,
                                    sas_state: None,
                                    device_id: None,
                                },
                            ))
                            .await;
                    }
                }
            });

            let client_clone = client.clone();
            let tx_clone = tx.clone();
            client.add_event_handler(|event: ToDeviceKeyVerificationCancelEvent| async move {
                let verification_request = client_clone
                    .encryption()
                    .get_verification_request(&event.sender, &event.content.transaction_id)
                    .await;
                match verification_request {
                    None => {}
                    Some(verification_request) => {
                        let _ = tx_clone
                            .send(CacheMutation::Replace(
                                event.content.transaction_id,
                                VerificationRequestDetails {
                                    inner: verification_request,
                                    sas_state: None,
                                    device_id: None,
                                },
                            ))
                            .await;
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
                                    CacheMutation::Push(verification_request) => {
                                        if !verification_request.inner.we_started() {
                                            // Trigger a notification
                                            let _ = Notification::new()
                                                .summary(
                                                    tr!("INCOMING_VERIFICATION")
                                                        .to_string()
                                                        .as_str(),
                                                )
                                                .body(
                                                    tr!(
                                                        "INCOMING_SELF_VERIFICATION_DESCRIPTION",
                                                        device_id = verification_request
                                                            .device_id
                                                            .clone()
                                                            .map(|id| id.to_string())
                                                            .unwrap_or_else(|| tr!(
                                                                "UNKNOWN_DEVICE",
                                                                "Unknown Device"
                                                            )
                                                            .to_string())
                                                    )
                                                    .to_string()
                                                    .as_str(),
                                                )
                                                .post(cx);
                                        }

                                        this.pending_verification_requests
                                            .push(verification_request);
                                    }
                                    CacheMutation::Remove(transaction_id) => {
                                        this.pending_verification_requests.retain(|request| {
                                            request.inner.flow_id() != transaction_id
                                        })
                                    }
                                    CacheMutation::Replace(transaction_id, new_request) => {
                                        for request in this.pending_verification_requests.iter_mut()
                                        {
                                            if request.inner.flow_id() == transaction_id {
                                                *request = VerificationRequestDetails {
                                                    inner: new_request.inner,
                                                    sas_state: new_request
                                                        .sas_state
                                                        .or(request.sas_state.clone()),
                                                    device_id: new_request.device_id,
                                                };
                                                break;
                                            }
                                        }
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

            Self {
                pending_verification_requests: Vec::new(),
            }
        })
    }

    pub fn notify_new_verification_request(
        &mut self,
        verification_request: VerificationRequest,
        cx: &mut Context<Self>,
    ) {
        self.pending_verification_requests
            .push(VerificationRequestDetails {
                inner: verification_request,
                sas_state: None,
                device_id: None,
            });
        cx.notify()
    }

    pub fn verification_request(&self, flow_id: &str) -> Option<&VerificationRequestDetails> {
        self.pending_verification_requests
            .iter()
            .find(|request| request.inner.flow_id() == flow_id)
    }
}
