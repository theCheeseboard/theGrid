use matrix_sdk::ruma::api::client::rtc::RtcTransport;

pub enum FocusUrlError {
    RoomError,
    NoRtcFocus,
}

pub async fn get_focus_url(rtc_foci: Vec<RtcTransport>) -> Result<String, FocusUrlError> {
    let Some(RtcTransport::LiveKit(livekit_focus)) = rtc_foci
        .iter()
        .find(|focus| matches!(focus, RtcTransport::LiveKit(_)))
    else {
        return Err(FocusUrlError::NoRtcFocus);
    };

    Ok(livekit_focus.service_url.clone())
}
