use gpui::{App, AppContext, AsyncApp, Entity, WeakEntity};
use log::{error, info};
use matrix_sdk::Client;
use matrix_sdk::encryption::verification::{
    SasVerification, Verification, VerificationRequest, VerificationRequestState,
};
use matrix_sdk::ruma::OwnedTransactionId;
use matrix_sdk::ruma::events::key::verification::VerificationMethod;
use matrix_sdk::ruma::events::key::verification::accept::ToDeviceKeyVerificationAcceptEvent;
use matrix_sdk::ruma::events::key::verification::cancel::ToDeviceKeyVerificationCancelEvent;
use matrix_sdk::ruma::events::key::verification::done::ToDeviceKeyVerificationDoneEvent;
use matrix_sdk::ruma::events::key::verification::ready::ToDeviceKeyVerificationReadyEvent;
use matrix_sdk::ruma::events::key::verification::request::ToDeviceKeyVerificationRequestEvent;
use matrix_sdk::ruma::events::key::verification::start::{
    StartMethod, ToDeviceKeyVerificationStartEvent,
};

pub struct VerificationRequestsCache {
    pub pending_verification_requests: Vec<VerificationRequestDetails>,
}

#[derive(Clone)]
pub struct VerificationRequestDetails {
    pub inner: VerificationRequest,
    pub sas_state: Option<SasVerification>,
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
                        tx_clone
                            .send(CacheMutation::Push(VerificationRequestDetails {
                                inner: verification_request,
                                sas_state: None,
                            }))
                            .await
                            .unwrap();
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
                        let sas_state = if let StartMethod::SasV1(_) = event.content.method {
                            if let VerificationRequestState::Transitioned {
                                verification: Verification::SasV1(sas),
                                ..
                            } = verification_request.state()
                            {
                                match sas.accept().await {
                                    Ok(_) => Some(sas),
                                    Err(_) => None,
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        tx_clone
                            .send(CacheMutation::Replace(
                                event.content.transaction_id,
                                VerificationRequestDetails {
                                    inner: verification_request,
                                    sas_state,
                                },
                            ))
                            .await
                            .unwrap();
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
                        info!("Ready event! {:?}", verification_request);
                        let sas_state = if event
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
                        };

                        tx_clone
                            .send(CacheMutation::Replace(
                                event.content.transaction_id,
                                VerificationRequestDetails {
                                    inner: verification_request,
                                    sas_state,
                                },
                            ))
                            .await
                            .unwrap();
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
                        tx_clone
                            .send(CacheMutation::Replace(
                                event.content.transaction_id,
                                VerificationRequestDetails {
                                    inner: verification_request,
                                    sas_state: None,
                                },
                            ))
                            .await
                            .unwrap();
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
                        tx_clone
                            .send(CacheMutation::Replace(
                                event.content.transaction_id,
                                VerificationRequestDetails {
                                    inner: verification_request,
                                    sas_state: None,
                                },
                            ))
                            .await
                            .unwrap();
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
                        tx_clone
                            .send(CacheMutation::Replace(
                                event.content.transaction_id,
                                VerificationRequestDetails {
                                    inner: verification_request,
                                    sas_state: None,
                                },
                            ))
                            .await
                            .unwrap();
                    }
                }
            });

            cx.spawn(
                async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                    loop {
                        let mutation = rx.recv().await.unwrap();
                        weak_this
                            .update(cx, |this, cx| {
                                match mutation {
                                    CacheMutation::Push(verification_request) => {
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
                                                *request = new_request;
                                                break;
                                            }
                                        }
                                    }
                                }
                                cx.notify();
                            })
                            .unwrap();
                    }
                },
            )
            .detach();

            Self {
                pending_verification_requests: Vec::new(),
            }
        })
    }

    pub fn verification_request(&self, flow_id: &str) -> Option<&VerificationRequestDetails> {
        self.pending_verification_requests
            .iter()
            .find(|request| request.inner.flow_id() == flow_id)
    }
}
