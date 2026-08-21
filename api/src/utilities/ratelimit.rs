use crate::{AppState, PUBLIC_API_KEY, error::AppError, utilities::token::validate_token_cached};
use axum::{
    extract::{ConnectInfo, FromRequestParts, Request, State},
    http::HeaderMap,
    middleware::Next,
    response::Response,
};
use dashmap::DashMap;
use std::time::Duration;
use std::{net::SocketAddr, time::Instant};

#[derive(Clone)]
pub struct AuthUser {
    pub id: i64,
    // pub token_type: TokenType,
    pub rate_limit_tier: RateLimitTier,
}

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
    AppState: axum::extract::FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        // Check if user was attached by middleware
        if let Some(user) = parts.extensions.get::<AuthUser>().cloned() {
            return Ok(user);
        }

        Err(AppError::Unauthorized(
            "Authentication required".to_string(),
        ))
    }
}

// #[derive(Clone)]
// pub struct OptionalAuth(pub Option<AuthUser>);

// impl<S> FromRequestParts<S> for OptionalAuth
// where
//     S: Send + Sync,
//     AppState: axum::extract::FromRef<S>,
// {
//     type Rejection = std::convert::Infallible; // Never fails!

//     async fn from_request_parts(
//         parts: &mut axum::http::request::Parts,
//         _state: &S,
//     ) -> Result<Self, Self::Rejection> {
//         let auth = parts.extensions.get::<AuthUser>().cloned();

//         Ok(OptionalAuth(auth))
//     }
// }

// impl OptionalAuth {
//     pub fn is_authenticated(&self) -> bool {
//         self.0.is_some()
//     }

//     pub fn user_id(&self) -> Option<i64> {
//         self.0.as_ref().map(|u| u.id)
//     }

//     pub fn require(self) -> Result<AuthUser, AppError> {
//         self.0
//             .ok_or_else(|| AppError::Unauthorized("Authentication required".to_string()))
//     }
// }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateLimitTier {
    FrontendPublic,
    ApiKey,
    Authenticated,
    Anonymous,
    Comment,
    FontUpload,
    SnippetSubmit,
}

impl RateLimitTier {
    fn max_requests(&self) -> usize {
        match self {
            RateLimitTier::FrontendPublic => 40,
            RateLimitTier::ApiKey => 30,
            RateLimitTier::Authenticated => 40,
            RateLimitTier::Anonymous => 10,
            RateLimitTier::Comment => 10,
            RateLimitTier::FontUpload => 1,
            RateLimitTier::SnippetSubmit => 3,
        }
    }

    fn window_duration(&self) -> std::time::Duration {
        match self {
            RateLimitTier::SnippetSubmit => std::time::Duration::from_secs(3600),
            _ => std::time::Duration::from_secs(60),
        }
    }
}

#[derive(Clone)]
struct RateLimitEntry {
    count: usize,
    window_start: Instant,
}

pub struct RateLimiter {
    ip_limits: DashMap<String, RateLimitEntry>,
    token_limits: DashMap<String, RateLimitEntry>,
    font_upload_limits: DashMap<String, RateLimitEntry>,
    comment_limits: DashMap<String, RateLimitEntry>,
    snippet_submit_limits: DashMap<String, RateLimitEntry>,
    trusted_domains: Vec<String>,
}

impl RateLimiter {
    pub fn new(trusted_domains: Vec<String>) -> Self {
        Self {
            ip_limits: DashMap::new(),
            token_limits: DashMap::new(),
            font_upload_limits: DashMap::new(),
            comment_limits: DashMap::new(),
            snippet_submit_limits: DashMap::new(),
            trusted_domains,
        }
    }

    pub fn is_trusted_domain(&self, origin: &str) -> bool {
        self.trusted_domains
            .iter()
            .any(|domain| origin.contains(domain))
    }

    pub fn check_ip(&self, ip: &str, tier: RateLimitTier) -> bool {
        Self::check_limit(&self.ip_limits, ip, tier)
    }

    pub fn check_token(&self, token: &str, tier: RateLimitTier) -> bool {
        Self::check_limit(&self.token_limits, token, tier)
    }

    pub fn check_font_upload(&self, ip: &str) -> bool {
        Self::check_limit(&self.font_upload_limits, ip, RateLimitTier::FontUpload)
    }

    pub fn check_comment_limit(&self, user_id: i64) -> bool {
        Self::check_limit(
            &self.comment_limits,
            &user_id.to_string(),
            RateLimitTier::Comment,
        )
    }

    pub fn check_snippet_submit(&self, key: &str) -> bool {
        Self::check_limit(
            &self.snippet_submit_limits,
            key,
            RateLimitTier::SnippetSubmit,
        )
    }

    fn check_limit(
        limits: &DashMap<String, RateLimitEntry>,
        key: &str,
        tier: RateLimitTier,
    ) -> bool {
        let now = Instant::now();

        let mut entry = limits.entry(key.to_string()).or_insert(RateLimitEntry {
            count: 0,
            window_start: now,
        });

        if now.duration_since(entry.window_start) >= tier.window_duration() {
            entry.count = 0;
            entry.window_start = now;
        }

        if entry.count >= tier.max_requests() {
            return false;
        }

        entry.count += 1;
        true
    }

    pub fn cleanup(&self) {
        let now = Instant::now();

        // ip_limits/token_limits are shared across several tiers that are
        // all still 60s windows today, so the existing flat 120s (2x
        // headroom) cutoff stays correct for them.
        let shared_cutoff = now - Duration::from_secs(120);
        self.ip_limits
            .retain(|_, entry| entry.window_start > shared_cutoff);
        self.token_limits
            .retain(|_, entry| entry.window_start > shared_cutoff);

        // font_upload_limits/comment_limits/snippet_submit_limits are each
        // dedicated to exactly one tier, so their cutoff is that tier's own
        // window_duration() (with the same 2x headroom) instead of a
        // constant that silently assumes every tier is 60s. Without this,
        // a >120s window (like SnippetSubmit's 3600s) would have its entry
        // deleted at the very first cleanup tick after 120s -- the count
        // resets to 0 on the next request, and the "3/hour" limit is
        // effectively never enforced.
        let font_upload_cutoff = now - RateLimitTier::FontUpload.window_duration() * 2;
        self.font_upload_limits
            .retain(|_, entry| entry.window_start > font_upload_cutoff);

        let comment_cutoff = now - RateLimitTier::Comment.window_duration() * 2;
        self.comment_limits
            .retain(|_, entry| entry.window_start > comment_cutoff);

        let snippet_submit_cutoff = now - RateLimitTier::SnippetSubmit.window_duration() * 2;
        self.snippet_submit_limits
            .retain(|_, entry| entry.window_start > snippet_submit_cutoff);
    }
}

/// Internal enum to track who is making the request
enum RequestIdentity {
    Authenticated(AuthUser, String), // User and their token/key
    Public(String),                  // Public IP
    Anonymous(String),               // Anonymous IP
}

pub async fn rate_limit_middleware(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let ip = addr.ip().to_string();

    let identity = resolve_identity(&state, &headers, &ip).await;

    let (tier, key, is_token) = match &identity {
        RequestIdentity::Authenticated(user, token) => (user.rate_limit_tier, token.as_str(), true),
        RequestIdentity::Public(ip) => (RateLimitTier::FrontendPublic, ip.as_str(), false),
        RequestIdentity::Anonymous(ip) => (RateLimitTier::Anonymous, ip.as_str(), false),
    };

    let allowed = if is_token {
        state.rate_limiter.check_token(key, tier)
    } else {
        state.rate_limiter.check_ip(key, tier)
    };

    if !allowed {
        let msg = format!("Rate limit exceeded ({}/min).", tier.max_requests());
        return Err(AppError::TooManyRequests(msg));
    }

    let mut request = request;
    if let RequestIdentity::Authenticated(user, _) = identity {
        request.extensions_mut().insert(user);
    }

    Ok(next.run(request).await)
}

pub async fn font_upload_limit_middleware(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let ip = addr.ip().to_string();

    if !state.rate_limiter.check_font_upload(&ip) {
        let tier = RateLimitTier::FontUpload;
        let msg = format!(
            "IP upload rate limit exceeded ({}/min).",
            tier.max_requests()
        );
        return Err(AppError::TooManyRequests(msg));
    }
    if let Some(user) = request.extensions().get::<AuthUser>() {
        let user_key = format!("user:{}", user.id);
        if !state.rate_limiter.check_font_upload(&user_key) {
            return Err(AppError::TooManyRequests(
                "User upload rate limit exceeded (1/min).".into(),
            ));
        }
    }

    Ok(next.run(request).await)
}

pub async fn snippet_submit_limit_middleware(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let ip = addr.ip().to_string();

    if !state.rate_limiter.check_snippet_submit(&ip) {
        let tier = RateLimitTier::SnippetSubmit;
        let msg = format!(
            "IP snippet submission rate limit exceeded ({}/hour).",
            tier.max_requests()
        );
        return Err(AppError::TooManyRequests(msg));
    }
    if let Some(user) = request.extensions().get::<AuthUser>() {
        let user_key = format!("user:{}", user.id);
        if !state.rate_limiter.check_snippet_submit(&user_key) {
            return Err(AppError::TooManyRequests(
                "User snippet submission rate limit exceeded (3/hour).".into(),
            ));
        }
    }

    Ok(next.run(request).await)
}

pub async fn comment_limit_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    // This middleware assumes it's running after auth resolution
    let user = request
        .extensions()
        .get::<AuthUser>()
        .ok_or_else(|| AppError::Unauthorized("Auth required for comments".into()))?;

    if !state.rate_limiter.check_comment_limit(user.id) {
        return Err(AppError::TooManyRequests(
            "Comment rate limit exceeded (10/min).".into(),
        ));
    }

    Ok(next.run(request).await)
}

/// Helper to extract user info or identify anonymous requests
async fn resolve_identity(state: &AppState, headers: &HeaderMap, ip: &str) -> RequestIdentity {
    if let Some(token) = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
    {
        if let Ok((user, token_type)) =
            validate_token_cached(&state.db, &state.auth_cache, token).await
        {
            return RequestIdentity::Authenticated(
                AuthUser {
                    id: user.id,
                    rate_limit_tier: token_type.rate_limit_tier(),
                    //token_type,
                },
                token.to_string(),
            );
        }
    }

    if let Some(key) = headers.get("X-API-Key").and_then(|h| h.to_str().ok()) {
        if key == PUBLIC_API_KEY {
            return RequestIdentity::Public(ip.to_string());
        }

        if let Ok((user, token_type)) =
            validate_token_cached(&state.db, &state.auth_cache, key).await
        {
            return RequestIdentity::Authenticated(
                AuthUser {
                    id: user.id,
                    rate_limit_tier: token_type.rate_limit_tier(),
                    //token_type,
                },
                key.to_string(),
            );
        }
    }

    RequestIdentity::Anonymous(ip.to_string())
}
