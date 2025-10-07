use crate::session::account_cache::AccountCache;
use crate::session::verification_requests_cache::VerificationRequestsCache;
use gpui::{App, Entity};
use matrix_sdk::Client;

pub struct Caches {
    pub verification_requests: Entity<VerificationRequestsCache>,
    pub account_cache: Entity<AccountCache>,
}

impl Caches {
    pub fn new(client: &Client, cx: &mut App) -> Self {
        Self {
            verification_requests: VerificationRequestsCache::new(client, cx),
            account_cache: AccountCache::new(client, cx),
        }
    }
}
