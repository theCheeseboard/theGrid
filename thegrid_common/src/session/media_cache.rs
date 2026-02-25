use crate::tokio_helper::TokioHelper;
use gpui::http_client::anyhow;
use gpui::private::anyhow;
use gpui::{App, AppContext, AsyncApp, Context, Entity, RenderImage, WeakEntity};
use image::{Frame, ImageReader, Pixel, RgbaImage};
use matrix_sdk::Client;
use matrix_sdk::media::{MediaFileHandle, MediaFormat, MediaRequestParameters, UniqueKey};
use matrix_sdk::ruma::OwnedMxcUri;
use matrix_sdk::ruma::events::room::MediaSource;
use smallvec::smallvec;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Display;
use std::hash::Hash;
use std::sync::{Arc, Mutex, Weak};

pub struct MediaCache {
    client: Client,
    tracked_files: RefCell<HashMap<MediaCacheEntry, Entity<MediaFile>>>,
}

#[derive(Clone)]
pub enum MediaCacheEntry {
    MediaSource(MediaSource),
    None,
}

impl MediaCacheEntry {
    pub fn new(media_source: MediaSource) -> Self {
        MediaCacheEntry::MediaSource(media_source)
    }

    pub fn from_mxc(mxc: OwnedMxcUri) -> Self {
        MediaCacheEntry::MediaSource(MediaSource::Plain(mxc))
    }
}

impl Display for MediaCacheEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            MediaCacheEntry::MediaSource(source) => match source {
                MediaSource::Plain(plain) => plain.to_string(),
                MediaSource::Encrypted(encrypted) => encrypted.url.to_string(),
            },
            MediaCacheEntry::None => "none".into(),
        };
        write!(f, "{}", str)
    }
}

impl From<MediaSource> for MediaCacheEntry {
    fn from(media_source: MediaSource) -> Self {
        Self::new(media_source)
    }
}

impl From<OwnedMxcUri> for MediaCacheEntry {
    fn from(value: OwnedMxcUri) -> Self {
        Self::from_mxc(value)
    }
}

impl From<Option<OwnedMxcUri>> for MediaCacheEntry {
    fn from(value: Option<OwnedMxcUri>) -> Self {
        match value {
            None => MediaCacheEntry::None,
            Some(value) => Self::from_mxc(value),
        }
    }
}

impl PartialEq for MediaCacheEntry {
    fn eq(&self, other: &Self) -> bool {
        match self {
            MediaCacheEntry::MediaSource(media_source) => match other {
                MediaCacheEntry::MediaSource(other_media_source) => {
                    media_source.unique_key() == other_media_source.unique_key()
                }
                MediaCacheEntry::None => false,
            },
            MediaCacheEntry::None => match other {
                MediaCacheEntry::MediaSource(_) => false,
                MediaCacheEntry::None => true,
            },
        }
    }
}

impl Eq for MediaCacheEntry {}

impl Hash for MediaCacheEntry {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            MediaCacheEntry::MediaSource(media_source) => {
                media_source.unique_key().hash(state);
            }
            MediaCacheEntry::None => {}
        }
    }
}

impl MediaCache {
    pub fn new(client: &Client) -> Self {
        Self {
            client: client.clone(),
            tracked_files: RefCell::new(HashMap::new()),
        }
    }

    pub fn media_file(
        &self,
        media_cache_entry: MediaCacheEntry,
        cx: &mut App,
    ) -> Entity<MediaFile> {
        let media_source = match &media_cache_entry {
            MediaCacheEntry::MediaSource(media_source) => media_source,
            MediaCacheEntry::None => {
                panic!("Tried to get media file for None")
            }
        };

        self.tracked_files
            .borrow_mut()
            .entry(media_cache_entry.clone())
            .or_insert_with(|| MediaFile::new(self.client.clone(), media_source.clone(), true, cx))
            .to_owned()
    }

    pub fn media_file_lazy(
        &self,
        media_cache_entry: MediaCacheEntry,
        cx: &mut App,
    ) -> Entity<MediaFile> {
        let media_source = match &media_cache_entry {
            MediaCacheEntry::MediaSource(media_source) => media_source,
            MediaCacheEntry::None => {
                panic!("Tried to get media file for None")
            }
        };

        self.tracked_files
            .borrow_mut()
            .entry(media_cache_entry.clone())
            .or_insert_with(|| MediaFile::new(self.client.clone(), media_source.clone(), false, cx))
            .to_owned()
    }
}

pub struct MediaFile {
    client: Client,
    mxc_uri: MediaSource,
    pub media_state: MediaState,
    pub read_image: Mutex<Weak<RenderImage>>,
}

pub enum MediaState {
    Idle,
    Loading,
    Loaded(MediaFileHandle),
    Failed,
}

impl MediaFile {
    pub fn new(
        client: Client,
        mxc_uri: MediaSource,
        start_request_immediately: bool,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx| {
            let mut media_file = Self {
                client,
                mxc_uri,
                media_state: MediaState::Failed,
                read_image: Mutex::new(Weak::new()),
            };

            if start_request_immediately {
                media_file.request_media(None, None, false, cx);
            }
            media_file
        })
    }

    pub fn request_media(
        &mut self,
        filename: Option<String>,
        mime_type: Option<String>,
        spawn_job: bool,
        cx: &mut Context<Self>,
    ) {
        self.media_state = MediaState::Loading;

        // TODO: Spawn job

        let media_client = self.client.media();
        let mxc_uri = self.mxc_uri.clone();
        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let response = cx
                    .spawn_tokio(async move {
                        media_client
                            .get_media_file(
                                &MediaRequestParameters {
                                    source: mxc_uri,
                                    format: MediaFormat::File,
                                },
                                filename,
                                &mime_type
                                    .unwrap_or("application/octet-stream".into())
                                    .parse()
                                    .unwrap(),
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
