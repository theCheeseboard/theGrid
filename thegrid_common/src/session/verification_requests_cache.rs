use crate::tokio_helper::TokioHelper;
use cntp_i18n::tr;
use contemporary::notification::Notification;
use gpui::{App, AppContext, AsyncApp, Context, Entity, WeakEntity};
use log::error;
use matrix_sdk::encryption::verification::{
    AcceptSettings, QrVerification, SasVerification, Verification, VerificationRequest,
    VerificationRequestState,
};
use matrix_sdk::ruma::events::key::verification::accept::{
    OriginalSyncKeyVerificationAcceptEvent, ToDeviceKeyVerificationAcceptEvent,
};
use matrix_sdk::ruma::events::key::verification::cancel::{
    OriginalSyncKeyVerificationCancelEvent, ToDeviceKeyVerificationCancelEvent,
};
use matrix_sdk::ruma::events::key::verification::done::{
    OriginalSyncKeyVerificationDoneEvent, ToDeviceKeyVerificationDoneEvent,
};
use matrix_sdk::ruma::events::key::verification::ready::{
    OriginalSyncKeyVerificationReadyEvent, ToDeviceKeyVerificationReadyEvent,
};
use matrix_sdk::ruma::events::key::verification::request::ToDeviceKeyVerificationRequestEvent;
use matrix_sdk::ruma::events::key::verification::start::{
    OriginalSyncKeyVerificationStartEvent, StartMethod, ToDeviceKeyVerificationStartEvent,
};
use matrix_sdk::ruma::events::key::verification::{ShortAuthenticationString, VerificationMethod};
use matrix_sdk::ruma::events::message::MessageEvent;
use matrix_sdk::ruma::events::room::message::{
    MessageType, OriginalSyncRoomMessageEvent, RoomMessageEvent,
};
use matrix_sdk::ruma::events::{MessageLikeEventContent, MessageLikeEventType};
use matrix_sdk::ruma::{OwnedDeviceId, OwnedTransactionId, OwnedUserId};
use matrix_sdk::{Client, Error};

pub static SUPPORTED_VERIFICATION_METHODS: &[VerificationMethod] = &[
    VerificationMethod::SasV1,
    VerificationMethod::QrCodeShowV1,
    VerificationMethod::ReciprocateV1,
];

pub struct VerificationRequestsCache {
    pub pending_verification_requests: Vec<Entity<VerificationRequestDetails>>,
}

#[derive(Clone)]
pub struct VerificationRequestDetails {
    pub inner: VerificationRequest,
    pub sas_state: Option<SasVerification>,
    pub qr_state: Option<QrVerification>,
    pub device_id: Option<OwnedDeviceId>,
    pub peer_id: OwnedUserId,

    // Used to immediately hide the QR code show flow after the user manually starts SAS
    pub sas_manually_started: bool,
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
                if let Some(verification_request) = client_clone
                    .encryption()
                    .get_verification_request(&event.sender, &event.content.transaction_id)
                    .await
                {
                    let _ = tx_clone
                        .send(CacheMutation::Push(VerificationRequestDetails {
                            inner: verification_request,
                            sas_state: None,
                            qr_state: None,
                            device_id: Some(event.content.from_device),
                            peer_id: event.sender.clone(),
                            sas_manually_started: false,
                        }))
                        .await;
                }
            });

            let client_clone = client.clone();
            let tx_clone = tx.clone();
            client.add_event_handler(|event: ToDeviceKeyVerificationStartEvent| async move {
                if let Some(verification_request) = client_clone
                    .encryption()
                    .get_verification_request(&event.sender, &event.content.transaction_id)
                    .await
                {
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
                                qr_state: None,
                                device_id: Some(event.content.from_device),
                                peer_id: event.sender.clone(),
                                sas_manually_started: false,
                            },
                        ))
                        .await;
                }
            });

            let client_clone = client.clone();
            let tx_clone = tx.clone();
            client.add_event_handler(|event: ToDeviceKeyVerificationReadyEvent| async move {
                if let Some(verification_request) = client_clone
                    .encryption()
                    .get_verification_request(&event.sender, &event.content.transaction_id)
                    .await
                {
                    let methods = event.content.methods;

                    // Start QR Show if QR Scan is supported by the peer
                    let qr_state = if methods.contains(&VerificationMethod::QrCodeScanV1)
                        && methods.contains(&VerificationMethod::ReciprocateV1)
                    {
                        verification_request
                            .generate_qr_code()
                            .await
                            .unwrap_or_else(|e| {
                                error!("Unable to generate QR code: {e}");
                                None
                            })
                    } else {
                        None
                    };

                    let _ = tx_clone
                        .send(CacheMutation::Replace(
                            event.content.transaction_id,
                            VerificationRequestDetails {
                                inner: verification_request,
                                sas_state: None,
                                qr_state,
                                device_id: None,
                                peer_id: event.sender.clone(),
                                sas_manually_started: false,
                            },
                        ))
                        .await;
                }
            });

            let client_clone = client.clone();
            let tx_clone = tx.clone();
            client.add_event_handler(|event: ToDeviceKeyVerificationAcceptEvent| async move {
                if let Some(verification_request) = client_clone
                    .encryption()
                    .get_verification_request(&event.sender, &event.content.transaction_id)
                    .await
                {
                    let _ = tx_clone
                        .send(CacheMutation::Replace(
                            event.content.transaction_id,
                            VerificationRequestDetails {
                                inner: verification_request,
                                sas_state: None,
                                qr_state: None,
                                device_id: None,
                                peer_id: event.sender.clone(),
                                sas_manually_started: false,
                            },
                        ))
                        .await;
                }
            });

            let client_clone = client.clone();
            let tx_clone = tx.clone();
            client.add_event_handler(|event: ToDeviceKeyVerificationDoneEvent| async move {
                if let Some(verification_request) = client_clone
                    .encryption()
                    .get_verification_request(&event.sender, &event.content.transaction_id)
                    .await
                {
                    let _ = tx_clone
                        .send(CacheMutation::Replace(
                            event.content.transaction_id,
                            VerificationRequestDetails {
                                inner: verification_request,
                                sas_state: None,
                                qr_state: None,
                                device_id: None,
                                peer_id: event.sender.clone(),
                                sas_manually_started: false,
                            },
                        ))
                        .await;
                }
            });

            let client_clone = client.clone();
            let tx_clone = tx.clone();
            client.add_event_handler(|event: ToDeviceKeyVerificationCancelEvent| async move {
                if let Some(verification_request) = client_clone
                    .encryption()
                    .get_verification_request(&event.sender, &event.content.transaction_id)
                    .await
                {
                    let _ = tx_clone
                        .send(CacheMutation::Replace(
                            event.content.transaction_id,
                            VerificationRequestDetails {
                                inner: verification_request,
                                sas_state: None,
                                qr_state: None,
                                device_id: None,
                                peer_id: event.sender.clone(),
                                sas_manually_started: false,
                            },
                        ))
                        .await;
                }
            });

            let client_clone = client.clone();
            let tx_clone = tx.clone();
            client.add_event_handler(|event: OriginalSyncRoomMessageEvent| async move {
                if let MessageType::VerificationRequest(verification_request_event) =
                    event.content.msgtype
                {
                    if let Some(verification_request) = client_clone
                        .encryption()
                        .get_verification_request(&event.sender, &event.event_id)
                        .await
                    {
                        let _ = tx_clone
                            .send(CacheMutation::Push(VerificationRequestDetails {
                                inner: verification_request,
                                sas_state: None,
                                qr_state: None,
                                device_id: Some(verification_request_event.from_device),
                                peer_id: event.sender.clone(),
                                sas_manually_started: false,
                            }))
                            .await;
                    }
                };
            });

            let client_clone = client.clone();
            let tx_clone = tx.clone();
            client.add_event_handler(|event: OriginalSyncKeyVerificationStartEvent| async move {
                if let Some(verification_request) = client_clone
                    .encryption()
                    .get_verification_request(&event.sender, &event.content.relates_to.event_id)
                    .await
                {
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
                            event.content.relates_to.event_id.as_str().into(),
                            VerificationRequestDetails {
                                inner: verification_request,
                                sas_state,
                                qr_state: None,
                                device_id: Some(event.content.from_device),
                                peer_id: event.sender.clone(),
                                sas_manually_started: false,
                            },
                        ))
                        .await;
                }
            });

            let client_clone = client.clone();
            let tx_clone = tx.clone();
            client.add_event_handler(|event: OriginalSyncKeyVerificationReadyEvent| async move {
                if let Some(verification_request) = client_clone
                    .encryption()
                    .get_verification_request(&event.sender, &event.content.relates_to.event_id)
                    .await
                {
                    let methods = event.content.methods;

                    // Start QR Show if QR Scan is supported by the peer
                    let qr_state = if methods.contains(&VerificationMethod::QrCodeScanV1)
                        && methods.contains(&VerificationMethod::ReciprocateV1)
                    {
                        verification_request
                            .generate_qr_code()
                            .await
                            .unwrap_or_else(|e| {
                                error!("Unable to generate QR code: {e}");
                                None
                            })
                    } else {
                        None
                    };

                    let _ = tx_clone
                        .send(CacheMutation::Replace(
                            event.content.relates_to.event_id.as_str().into(),
                            VerificationRequestDetails {
                                inner: verification_request,
                                sas_state: None,
                                qr_state,
                                device_id: None,
                                peer_id: event.sender.clone(),
                                sas_manually_started: false,
                            },
                        ))
                        .await;
                }
            });

            let client_clone = client.clone();
            let tx_clone = tx.clone();
            client.add_event_handler(|event: OriginalSyncKeyVerificationAcceptEvent| async move {
                if let Some(verification_request) = client_clone
                    .encryption()
                    .get_verification_request(&event.sender, &event.content.relates_to.event_id)
                    .await
                {
                    let _ = tx_clone
                        .send(CacheMutation::Replace(
                            event.content.relates_to.event_id.as_str().into(),
                            VerificationRequestDetails {
                                inner: verification_request,
                                sas_state: None,
                                qr_state: None,
                                device_id: None,
                                peer_id: event.sender.clone(),
                                sas_manually_started: false,
                            },
                        ))
                        .await;
                }
            });

            let client_clone = client.clone();
            let tx_clone = tx.clone();
            client.add_event_handler(|event: OriginalSyncKeyVerificationDoneEvent| async move {
                if let Some(verification_request) = client_clone
                    .encryption()
                    .get_verification_request(&event.sender, &event.content.relates_to.event_id)
                    .await
                {
                    let _ = tx_clone
                        .send(CacheMutation::Replace(
                            event.content.relates_to.event_id.as_str().into(),
                            VerificationRequestDetails {
                                inner: verification_request,
                                sas_state: None,
                                qr_state: None,
                                device_id: None,
                                peer_id: event.sender.clone(),
                                sas_manually_started: false,
                            },
                        ))
                        .await;
                }
            });

            let client_clone = client.clone();
            let tx_clone = tx.clone();
            client.add_event_handler(|event: OriginalSyncKeyVerificationCancelEvent| async move {
                if let Some(verification_request) = client_clone
                    .encryption()
                    .get_verification_request(&event.sender, &event.content.relates_to.event_id)
                    .await
                {
                    let _ = tx_clone
                        .send(CacheMutation::Replace(
                            event.content.relates_to.event_id.as_str().into(),
                            VerificationRequestDetails {
                                inner: verification_request,
                                sas_state: None,
                                qr_state: None,
                                device_id: None,
                                peer_id: event.sender.clone(),
                                sas_manually_started: false,
                            },
                        ))
                        .await;
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
                                                    tr!(
                                                        "INCOMING_VERIFICATION",
                                                        "Incoming Verification Request"
                                                    )
                                                    .to_string()
                                                    .as_str(),
                                                )
                                                .body(
                                                    tr!(
                                                        "INCOMING_SELF_VERIFICATION_DESCRIPTION",
                                                        "Verify your other device ({{device_id}}) \
                                                        to share encryption keys. The other device \
                                                        will be able to decrypt your messages.",
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
                                            .push(cx.new(|_| verification_request));
                                    }
                                    CacheMutation::Remove(transaction_id) => {
                                        this.pending_verification_requests.retain(|request| {
                                            request.read(cx).inner.flow_id() != transaction_id
                                        })
                                    }
                                    CacheMutation::Replace(transaction_id, new_request) => {
                                        for request in this.pending_verification_requests.iter_mut()
                                        {
                                            let new_request = new_request.clone();
                                            if request.update(cx, |request, cx| {
                                                if request.inner.flow_id() == transaction_id {
                                                    *request = VerificationRequestDetails {
                                                        inner: new_request.inner,
                                                        sas_state: new_request
                                                            .sas_state
                                                            .or(request.sas_state.clone()),
                                                        qr_state: new_request
                                                            .qr_state
                                                            .or(request.qr_state.clone()),
                                                        device_id: new_request.device_id,
                                                        peer_id: new_request.peer_id,
                                                        sas_manually_started: request
                                                            .sas_manually_started,
                                                    };
                                                    cx.notify();
                                                    true
                                                } else {
                                                    false
                                                }
                                            }) {
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
        peer_id: OwnedUserId,
        cx: &mut Context<Self>,
    ) -> Entity<VerificationRequestDetails> {
        let details = cx.new(|_| VerificationRequestDetails {
            inner: verification_request,
            sas_state: None,
            qr_state: None,
            device_id: None,
            peer_id,
            sas_manually_started: false,
        });
        self.pending_verification_requests.push(details.clone());
        cx.notify();

        details
    }

    pub fn verification_request(
        &self,
        flow_id: &str,
        cx: &App,
    ) -> Option<Entity<VerificationRequestDetails>> {
        self.pending_verification_requests
            .iter()
            .find(|request| request.read(cx).inner.flow_id() == flow_id)
            .cloned()
    }
}

impl VerificationRequestDetails {
    pub fn cancel(&self, cx: &mut Context<Self>) {
        let verification_request = self.inner.clone();
        cx.spawn(async move |_, cx: &mut AsyncApp| {
            let _ = cx
                .spawn_tokio(async move { verification_request.cancel().await })
                .await;
        })
        .detach();
    }

    pub fn start_sas(&mut self, cx: &mut Context<Self>) {
        self.sas_manually_started = true;
        cx.notify();

        let verification_request = self.inner.clone();
        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| match cx
                .spawn_tokio(async move { verification_request.start_sas().await })
                .await
            {
                Ok(sas_state) => {
                    let _ = weak_this.update(cx, |this, cx| {
                        this.sas_state = sas_state;
                        cx.notify();
                    });
                }
                Err(e) => {
                    error!("Unable to start SAS: {e}");
                }
            },
        )
        .detach();
    }

    pub fn start_qr_show(&self, cx: &mut Context<Self>) {
        let verification_request = self.inner.clone();
        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| match cx
                .spawn_tokio(async move { verification_request.generate_qr_code().await })
                .await
            {
                Ok(qr_state) => {
                    let _ = weak_this.update(cx, |this, cx| {
                        this.qr_state = qr_state;
                        cx.notify();
                    });
                }
                Err(e) => {
                    error!("Unable to start QR: {e}");
                }
            },
        )
        .detach();
    }

    pub fn is_active(&self) -> bool {
        !self.inner.is_done() && !self.inner.is_cancelled()
    }

    pub fn accept(&self, cx: &mut Context<Self>) {
        let verification_request = self.inner.clone();
        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let should_show_qr =
                    verification_request
                        .their_supported_methods()
                        .is_some_and(|other_methods| {
                            other_methods.contains(&VerificationMethod::QrCodeScanV1)
                                && other_methods.contains(&VerificationMethod::ReciprocateV1)
                        });

                let _ = cx
                    .spawn_tokio(async move {
                        verification_request
                            .accept_with_methods(SUPPORTED_VERIFICATION_METHODS.to_vec())
                            .await
                    })
                    .await;

                if should_show_qr {
                    let _ = weak_this.update(cx, |this, cx| {
                        this.start_qr_show(cx);
                        cx.notify();
                    });
                }
            },
        )
        .detach();
    }
}
