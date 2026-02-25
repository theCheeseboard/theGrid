use crate::session::account_cache::AccountCache;
use crate::session::devices_cache::DevicesCache;
use crate::session::media_cache::MediaCache;
use crate::session::room_cache::RoomCache;
use crate::session::verification_requests_cache::VerificationRequestsCache;
use gpui::{App, Entity};
use matrix_sdk::Client;

pub struct Caches {
    pub verification_requests: Entity<VerificationRequestsCache>,
    pub account_cache: Entity<AccountCache>,
    pub devices_cache: Entity<DevicesCache>,
    pub media_cache: MediaCache,
    pub room_cache: Entity<RoomCache>,
}

impl Caches {
    pub fn new(client: &Client, cx: &mut App) -> Self {
        Self {
            verification_requests: VerificationRequestsCache::new(client, cx),
            account_cache: AccountCache::new(client, cx),
            devices_cache: DevicesCache::new(client, cx),
            media_cache: MediaCache::new(client),
            room_cache: RoomCache::new(client, cx),
        }
    }
}
