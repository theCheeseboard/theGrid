use crate::tokio_helper::TokioHelper;
use gpui::http_client::anyhow;
use gpui::private::anyhow;
use gpui::{App, AppContext, AsyncApp, Context, Entity, RenderImage, WeakEntity};
use image::{Frame, ImageReader, Pixel, RgbaImage};
use matrix_sdk::media::{MediaFileHandle, MediaFormat, MediaRequestParameters};
use matrix_sdk::ruma::OwnedMxcUri;
use matrix_sdk::ruma::events::room::MediaSource;
use matrix_sdk::{Client, Error};
use smallvec::smallvec;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

pub struct MediaCache {
    client: Client,
    tracked_files: RefCell<HashMap<OwnedMxcUri, Entity<MediaFile>>>,
}

impl MediaCache {
    pub fn new(client: &Client) -> Self {
        Self {
            client: client.clone(),
            tracked_files: RefCell::new(HashMap::new()),
        }
    }

    pub fn media_file(&self, media_source: OwnedMxcUri, cx: &mut App) -> Entity<MediaFile> {
        self.tracked_files
            .borrow_mut()
            .entry(media_source.clone())
            .or_insert_with(|| MediaFile::new(self.client.clone(), media_source, cx))
            .to_owned()
    }
}

pub struct MediaFile {
    client: Client,
    mxc_uri: OwnedMxcUri,
    pub media_state: MediaState,
    pub read_image: Mutex<Weak<RenderImage>>,
}

pub enum MediaState {
    Loading,
    Loaded(MediaFileHandle),
    Failed,
}

impl MediaFile {
    pub fn new(client: Client, mxc_uri: OwnedMxcUri, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let mut media_file = Self {
                client,
                mxc_uri,
                media_state: MediaState::Failed,
                read_image: Mutex::new(Weak::new()),
            };
            media_file.request_media(cx);
            media_file
        })
    }

    pub fn request_media(&mut self, cx: &mut Context<Self>) {
        self.media_state = MediaState::Loading;

        let media_client = self.client.media();
        let mxc_uri = self.mxc_uri.clone();
        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let response = cx
                    .spawn_tokio(async move {
                        media_client
                            .get_media_file(
                                &MediaRequestParameters {
                                    source: MediaSource::Plain(mxc_uri),
                                    format: MediaFormat::File,
                                },
                                None,
                                &"application/octet-stream".parse().unwrap(),
                                true,
                                None,
                            )
                            .await
                    })
                    .await;

                let _ = weak_this.update(cx, |this, cx| {
                    match response {
                        Ok(media_file) => {
                            this.media_state = MediaState::Loaded(media_file);
                        }
                        Err(_) => {
                            this.media_state = MediaState::Failed;
                        }
                    }
                    cx.notify()
                });
            },
        )
        .detach();
        cx.notify()
    }

    pub fn read_image(&self) -> anyhow::Result<Arc<RenderImage>> {
        let mut read_image = self.read_image.lock().unwrap();
        match read_image.upgrade() {
            None => {
                let MediaState::Loaded(media_file) = &self.media_state else {
                    return Err(anyhow!("Media file not loaded"));
                };

                let mut image = ImageReader::open(media_file.path())?
                    .with_guessed_format()?
                    .decode()?
                    .into_rgba8();
                rgb_to_bgr(&mut image);
                let frame = Frame::new(image);

                let arc = Arc::new(RenderImage::new(smallvec![frame]));
                *read_image = Arc::downgrade(&arc);
                Ok(arc)
            }
            Some(image) => Ok(image),
        }
    }
}

fn rgb_to_bgr(image: &mut RgbaImage) {
    image.pixels_mut().for_each(|v| {
        let slice = v.channels();
        *v = *image::Rgba::from_slice(&[slice[2], slice[1], slice[0], slice[3]]);
    });
}
