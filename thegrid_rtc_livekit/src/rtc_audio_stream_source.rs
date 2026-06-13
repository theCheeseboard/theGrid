use async_ringbuf::producer::AsyncProducer;
use async_ringbuf::traits::{Consumer, Observer, Split};
use async_ringbuf::{AsyncHeapCons, AsyncHeapRb};
use cancellation_token::{CancellationToken, CancellationTokenSource};
use gpui::private::anyhow;
use gpui::{App, AsyncApp, Entity};
use livekit::webrtc::audio_stream::native::NativeAudioStream;
use rodio::{ChannelCount, Sample, SampleRate, Source};
use smol::stream::StreamExt;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use thegrid_common::tokio_helper::TokioHelper;

pub struct RtcAudioStreamSource {
    channels: ChannelCount,
    sample_rate: SampleRate,
    pub consumer: AsyncHeapCons<i16>,
    deaf: Arc<RwLock<bool>>,
    cancellation_token: CancellationToken,
}

impl RtcAudioStreamSource {
    pub fn new(
        mut stream: NativeAudioStream,
        channels: ChannelCount,
        sample_rate: SampleRate,
        deafen: Entity<bool>,
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

        Self {
            consumer,
            channels,
            sample_rate,
            deaf,
            cancellation_token: cancellation_token_source.token(),
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
            Some(self.consumer.try_pop().map(reformat).unwrap_or_default())
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
