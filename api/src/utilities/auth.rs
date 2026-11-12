use std::time::Instant;

use chrono::{DateTime, Duration, TimeDelta, Utc};
use dashmap::DashMap;

use crate::utilities::token::TokenType;

#[derive(Clone, Debug)]
pub struct CachedAuthUser {
    pub id: i64,
    pub token_type: TokenType,
    pub expires_at: Option<DateTime<Utc>>,
    pub cached_at: Instant,
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
