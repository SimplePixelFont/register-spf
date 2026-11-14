use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Instant;

use chrono::{DateTime, Duration, TimeDelta, Utc};
use dashmap::DashMap;

use crate::utilities::token::TokenType;

#[derive(Clone, Debug)]
pub struct CachedAuthUser {
    pub token_id: i64,
    pub id: i64,
    pub token_type: TokenType,
    pub expires_at: Option<DateTime<Utc>>,
    pub cached_at: Instant,
    pub last_synced_timestamp: Arc<AtomicI64>,
}

impl CachedAuthUser {
    fn is_expired(&self, ttl: Duration) -> bool {
        TimeDelta::from_std(Instant::now().duration_since(self.cached_at)).unwrap() > ttl
    }
}

pub struct AuthCache {
    cache: DashMap<String, CachedAuthUser>,
    ttl: Duration,
}

impl AuthCache {
    pub fn new(ttl_seconds: i64) -> Self {
        Self {
            cache: DashMap::new(),
            ttl: Duration::seconds(ttl_seconds),
        }
    }

    pub fn get(&self, token_hash: &str) -> Option<CachedAuthUser> {
        if let Some(cached) = self.cache.get(token_hash) {
            if cached.is_expired(self.ttl) {
                drop(cached);
                self.invalidate(token_hash);
                return None;
            }
            if let Some(expires_at) = cached.expires_at {
                if Utc::now() > expires_at {
                    drop(cached);
                    self.invalidate(token_hash);
                    return None;
                }
            }
            return Some(cached.clone());
        }

        None
    }

    pub fn set(&self, token_hash: String, user: CachedAuthUser) {
        self.cache.insert(token_hash, user);
    }

    pub fn invalidate(&self, token_hash: &str) {
        self.cache.remove(token_hash);
    }

    pub fn invalidate_user(&self, user_id: i64) {
        self.cache.retain(|_, cached| cached.id != user_id);
    }

    pub fn try_record_token_use(&self, token_hash: &str, throttle_secs: i64) -> Option<i64> {
        if let Some(entry) = self.cache.get(token_hash) {
            let now_ts = Utc::now().timestamp();
            let prev = entry.last_synced_timestamp.load(Ordering::Relaxed);
            if now_ts - prev >= throttle_secs {
                if entry
                    .last_synced_timestamp
                    .compare_exchange(prev, now_ts, Ordering::SeqCst, Ordering::Relaxed)
                    .is_ok()
                {
                    return Some(entry.token_id);
                }
            }
        }
        None
    }

    pub fn cleanup(&self) {
        self.cache.retain(|_, cached| {
            if cached.is_expired(self.ttl) {
                return false;
            }
            if let Some(expires_at) = cached.expires_at {
                if Utc::now() > expires_at {
                    return false;
                }
            }
            true
        });
    }
}
