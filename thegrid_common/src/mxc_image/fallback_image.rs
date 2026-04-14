use contemporary::components::icon::icon;
use gpui::{black, rgb, white, AnyElement, Hsla, IntoElement};
use image::EncodableLayout;
use matrix_sdk::ruma::{OwnedRoomId, OwnedUserId, RoomId, UserId};
use rustc_hash::FxHasher;
use std::hash::Hasher;

pub struct FallbackImageData {
    pub color: Hsla,
    pub content: AnyElement,
}

pub enum FallbackImage {
    User(OwnedUserId),
    Room(OwnedRoomId),
}

impl FallbackImage {
    pub fn fallback_image(&self) -> FallbackImageData {
        let (rand, icon_name) = match self {
            FallbackImage::User(user_id) => (user_id.as_bytes(), "user"),
            FallbackImage::Room(room_id) => (room_id.as_bytes(), "im-room"),
        };

        let mut hasher = FxHasher::default();
        hasher.write(rand);
        let color = Hsla::from(rgb(hasher.finish() as u32));
        let foreground = if color.l < 0.5 { white() } else { black() };

        FallbackImageData {
            color,
            content: icon(icon_name)
                .foreground(foreground.into())
                .into_any_element(),
        }
    }
}

pub trait IntoFallbackImage {
    fn fallback_image(&self) -> FallbackImage;
}

impl IntoFallbackImage for OwnedUserId {
    fn fallback_image(&self) -> FallbackImage {
        FallbackImage::User(self.clone())
    }
}

impl IntoFallbackImage for &OwnedUserId {
    fn fallback_image(&self) -> FallbackImage {
        self.clone().fallback_image()
    }
}

impl IntoFallbackImage for &UserId {
    fn fallback_image(&self) -> FallbackImage {
        FallbackImage::User(OwnedUserId::from(self.to_owned()))
    }
}

impl IntoFallbackImage for OwnedRoomId {
    fn fallback_image(&self) -> FallbackImage {
        FallbackImage::Room(self.clone())
    }
}

impl IntoFallbackImage for &OwnedRoomId {
    fn fallback_image(&self) -> FallbackImage {
        self.clone().fallback_image()
    }
}

impl IntoFallbackImage for &RoomId {
    fn fallback_image(&self) -> FallbackImage {
        FallbackImage::Room(OwnedRoomId::from(self.to_owned()))
    }
}
