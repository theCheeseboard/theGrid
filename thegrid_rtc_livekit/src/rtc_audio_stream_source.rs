use crate::call_manager::VolumeKey;
use async_ringbuf::producer::AsyncProducer;
use async_ringbuf::traits::{Consumer, Observer, Split};
use async_ringbuf::{AsyncHeapCons, AsyncHeapRb};
use cancellation_token::{CancellationToken, CancellationTokenSource};
use gpui::private::anyhow;
use gpui::{App, AsyncApp, Entity};
use livekit::track::TrackSource;
use livekit::webrtc::audio_stream::native::NativeAudioStream;
use log::{debug, info};
use matrix_sdk::ruma::{OwnedDeviceId, OwnedUserId};
use rodio::{ChannelCount, Sample, SampleRate, Source};
use smol::stream::StreamExt;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use thegrid_common::tokio_helper::TokioHelper;

pub struct RtcAudioStreamSource {
    channels: ChannelCount,
    sample_rate: SampleRate,
    pub consumer: AsyncHeapCons<i16>,
    deaf: Arc<RwLock<bool>>,
    volume: Arc<RwLock<f32>>,
    cancellation_token: CancellationToken,
    user_id: OwnedUserId,
    device_id: OwnedDeviceId,
    track_source: TrackSource,
}

impl RtcAudioStreamSource {
    pub fn new(
        mut stream: NativeAudioStream,
        channels: ChannelCount,
        sample_rate: SampleRate,
        deafen: Entity<bool>,
        volumes_entity: Entity<HashMap<VolumeKey, f32>>,
        user_id: OwnedUserId,
        device_id: OwnedDeviceId,
        track_source: TrackSource,
        cancellation_token_source: CancellationTokenSource,
        cx: &mut App,
    ) -> Self {
        let (mut producer, consumer) = AsyncHeapRb::<i16>::new(16384).split();
        cx.spawn({
            let cancellation_token_source = cancellation_token_source.clone();
            async move |cx: &mut AsyncApp| {
                let _ = cx
                    .spawn_tokio({
                        let cancellation_token_source = cancellation_token_source.clone();
                        async move {
                            // Receive the audio frames in a new task
                            while let Some(audio_frame) = stream.next().await {
                                if producer.push_exact(&audio_frame.data).await.is_err() {
                                    cancellation_token_source.cancel();
                                    return Ok(());
                                };
                            }

                            cancellation_token_source.cancel();
                            Ok::<_, anyhow::Error>(())
                        }
                    })
                    .await;
            }
        })
        .detach();

        let deaf = Arc::new(RwLock::new(*deafen.read(cx)));

        cx.observe(&deafen, {
            let deaf = deaf.clone();
            move |deafen, cx| {
                *deaf.write().unwrap() = *deafen.read(cx);
            }
        })
        .detach();

        let volume_key = VolumeKey::new(user_id.clone(), device_id.clone(), track_source);

        let volume = Arc::new(RwLock::new(logarithmic_attenuation_factor(
            *volumes_entity.read(cx).get(&volume_key).unwrap_or(&1_f32),
        )));
        cx.observe(&volumes_entity, {
            let volume = volume.clone();
            move |volumes_entity, cx| {
                let new_volume = logarithmic_attenuation_factor(
                    *volumes_entity.read(cx).get(&volume_key).unwrap_or(&1_f32),
                );
                debug!("Stream source volume: {new_volume} -> {:?}", volume_key);
                *volume.write().unwrap() = new_volume;
            }
        })
        .detach();

        debug!("New stream source");
        debug!(
            "Volume key: {:?}",
            VolumeKey::new(user_id.clone(), device_id.clone(), track_source)
        );

        Self {
            consumer,
            channels,
            sample_rate,
            deaf,
            volume,
            cancellation_token: cancellation_token_source.token(),
            user_id,
            device_id,
            track_source,
        }
    }
}

impl Iterator for RtcAudioStreamSource {
    type Item = Sample;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cancellation_token.is_canceled() {
            None
        } else if *self.deaf.read().unwrap() {
            Some(0.)
        } else {
            let volume = *self.volume.read().unwrap();
            Some(
                self.consumer
                    .try_pop()
                    .map(reformat)
                    .map(|v| v * volume)
                    .unwrap_or_default()
                    .clamp(-1., 1.),
            )
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.consumer.occupied_len(), None)
    }
}

impl Source for RtcAudioStreamSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        self.channels
    }

    fn sample_rate(&self) -> SampleRate {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

fn reformat(i16: i16) -> Sample {
    i16 as f32 / i16::MAX as f32
}

fn logarithmic_attenuation_factor(volume: f32) -> f32 {
    if volume > 0.99 {
        1.
    } else if volume < 0.01 {
        0.
    } else {
        -(1. - volume).log(100.)
    }
}
