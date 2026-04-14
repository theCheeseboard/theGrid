use crate::session::account_cache::AccountCache;
use crate::session::devices_cache::DevicesCache;
use crate::session::ignored_users_cache::IgnoredUsersCache;
use crate::session::media_cache::MediaCache;
use crate::session::room_cache::RoomCache;
use crate::session::spaces_cache::SpacesCache;
use crate::session::verification_requests_cache::VerificationRequestsCache;
use gpui::{App, AppContext, Entity};
use matrix_sdk::ruma::api::client::discovery::discover_homeserver::RtcFocusInfo;
use matrix_sdk::Client;
use crate::session::capability_cache::CapabilityCache;

pub struct Caches {
    pub verification_requests: Entity<VerificationRequestsCache>,
    pub account_cache: Entity<AccountCache>,
    pub capability_cache: Entity<CapabilityCache>,
    pub devices_cache: Entity<DevicesCache>,
    pub media_cache: MediaCache,
    pub room_cache: Entity<RoomCache>,
    pub spaces_cache: Entity<SpacesCache>,
    pub ignored_users_cache: Entity<IgnoredUsersCache>,

    pub rtc_foci: Vec<RtcFocusInfo>,
}

impl Caches {
    pub fn new(client: &Client, cx: &mut App) -> Self {
        Self {
            verification_requests: VerificationRequestsCache::new(client, cx),
            account_cache: AccountCache::new(client, cx),
            devices_cache: DevicesCache::new(client, cx),
            capability_cache: cx.new(|cx| CapabilityCache::new(client, cx)),
            media_cache: MediaCache::new(client),
            room_cache: RoomCache::new(client, cx),
            spaces_cache: cx.new(|cx| SpacesCache::new(client, cx)),
            ignored_users_cache: cx.new(|cx| IgnoredUsersCache::new(client, cx)),
            rtc_foci: Vec::new(),
        }
    }
}
