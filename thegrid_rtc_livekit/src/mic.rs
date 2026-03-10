use cpal::traits::{DeviceTrait, StreamTrait};
use gpui::{App, AppContext, AsyncApp, Entity};
use log::error;
use thegrid_common::outbound_track::OutboundTrack;

pub fn open_mic(device: &cpal::Device, cx: &mut App) -> Entity<OutboundTrack> {
    let device = device.clone();

    let (tx, rx) = async_channel::unbounded();

    let mut supported_device_configs = device.supported_input_configs().unwrap();
    let supported_config = supported_device_configs
        .next()
        .unwrap()
        .with_sample_rate(48000);

    let outbound_track = cx.new(|cx| {
        OutboundTrack::new_audio(
            supported_config.sample_rate(),
            supported_config.channels(),
            cx,
        )
    });

    let input_stream = device
        .build_input_stream(
            &supported_config.config(),
            move |data: &[i16], _: &cpal::InputCallbackInfo| {
                let _ = smol::block_on(tx.send(data.to_vec()));
            },
            move |err| {
                // Errors? What errors!?
                error!("cpal: error in input stream: {:?}", err)
            },
            None,
        )
        .unwrap();

    let weak_outbound_track = outbound_track.downgrade();
    cx.spawn(async move |cx: &mut AsyncApp| {
        while let Ok(samples) = rx.recv().await {
            if weak_outbound_track
                .update(cx, |outbound_track, cx| {
                    let buffer = outbound_track.audio_sample_buffer();
                    buffer.extend(samples);

                    cx.notify();
                })
                .is_err()
            {
                return;
            }
        }

        drop(input_stream);
    })
    .detach();

    outbound_track
}
