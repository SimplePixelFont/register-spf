use crate::error::{AppError, AuthError};
use crate::model::{TokenInfo, User};
use crate::utilities::ratelimit::RateLimitTier;
use crate::utilities::{AuthCache, CachedAuthUser};
use ::entity::access_tokens;
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use chrono::{DateTime, Duration, Utc};
use password_hash::generate_salt;
use rand::distr::Alphanumeric;
use rand::{RngExt, rng};
use sea_orm::DatabaseConnection;
use sea_orm::*;
use std::time::Instant;

const MAX_SESSIONS_PER_USER: usize = 10;
const MAX_API_KEYS_PER_USER: usize = 3;

#[derive(Debug)]
pub enum TokenError {
    HashError,
    InvalidHash,
}

impl From<TokenError> for AuthError {
    fn from(_: TokenError) -> Self {
        AuthError::TokenHashError
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenType {
    Session,
    ApiKey,
}

impl TokenType {
    fn as_str(&self) -> &str {
        match self {
            TokenType::Session => "session",
            TokenType::ApiKey => "api_key",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "session" => Some(TokenType::Session),
            "api_key" => Some(TokenType::ApiKey),
            _ => None,
        }
    }

    pub fn from_prefix(s: &str) -> Option<Self> {
        match s {
            "ses_" => Some(TokenType::Session),
            "fgk_" => Some(TokenType::ApiKey),
            _ => None,
        }
    }

    pub fn prefix(&self) -> &str {
        match self {
            TokenType::Session => "ses_",
            TokenType::ApiKey => "fgk_",
        }
    }

    pub fn rate_limit_tier(&self) -> RateLimitTier {
        match self {
            TokenType::Session => RateLimitTier::Authenticated,
            TokenType::ApiKey => RateLimitTier::ApiKey,
        }
    }

    pub fn default_expiration(&self) -> Option<DateTime<Utc>> {
        match self {
            TokenType::Session => Some(Utc::now() + Duration::days(30)),
            TokenType::ApiKey => None,
        }
    }
}

pub fn generate_token(token_type: &TokenType) -> String {
    let prefix = token_type.prefix();

    let random: String = rng()
        .sample_iter(&Alphanumeric)
        .take(64)
        .map(char::from)
        .collect();

    format!("{}{}", prefix, random)
}

pub fn hash_token(token: &str) -> Result<String, TokenError> {
    let salt = SaltString::encode_b64(&generate_salt()).unwrap();
    let argon2 = Argon2::default();

    let hash = argon2
        .hash_password(token.as_bytes(), &salt)
        .map_err(|_| TokenError::HashError)?
        .to_string();

    Ok(hash)
}

pub fn verify_token(token: &str, hash: &str) -> Result<bool, TokenError> {
    let parsed_hash = PasswordHash::new(hash).map_err(|_| TokenError::InvalidHash)?;

    Ok(Argon2::default()
        .verify_password(token.as_bytes(), &parsed_hash)
        .is_ok())
}

pub async fn validate_token_cached(
    db: &DatabaseConnection,
    cache: &AuthCache,
    token: &str,
) -> Result<(User, TokenType), AppError> {
    let token_hash = hash_token(token)?;

    if let Some(cached) = cache.get(&token_hash) {
        let user = User { id: cached.id };
        return Ok((user, cached.token_type));
    }

    let (user, token_type) = validate_token_from_db(db, token).await?;

    cache.set(
        token_hash,
        CachedAuthUser {
            id: user.id,
            token_type: token_type.clone(),
            cached_at: Instant::now(),
        },
    );

    Ok((user, token_type))
}

pub async fn validate_token_from_db(
    db: &DatabaseConnection,
    token: &str,
) -> Result<(User, TokenType), AppError> {
    if token.len() < 4 {
        return Err(AppError::Auth(AuthError::InvalidToken));
    }

    let prefix = &token[..4];
    let token_type =
        TokenType::from_prefix(prefix).ok_or(AppError::Auth(AuthError::InvalidToken))?;

    let possible_tokens = access_tokens::Entity::find()
        .filter(access_tokens::Column::TokenType.eq(token_type.as_str()))
        .all(db)
        .await?;

    let mut matched_token = None;
    for token_record in possible_tokens {
        if verify_token(token, &token_record.token)? {
            matched_token = Some(token_record);
            break;
        }
    }

    let token_record = matched_token.ok_or(AppError::Auth(AuthError::InvalidToken))?;

    if let Some(expires_at) = token_record.expires_at {
        if Utc::now() > expires_at {
            return Err(AppError::Auth(AuthError::TokenExpired));
        }
    }

    let token_type = TokenType::from_str(&token_record.token_type)
        .ok_or(AppError::Auth(AuthError::InvalidTokenType))?;

    // Update last_used_at, maybe later, also update when the token is removed from the dashmap
    // let mut active_token: access_tokens::ActiveModel = token_record.into();
    // active_token.last_used_at = Set(Some(Utc::now()));

    // active_token.update(db).await.ok();

    Ok((
        User {
            id: token_record.user_id,
        },
        token_type,
    ))
}

pub async fn create_session(db: &DatabaseConnection, user_id: i64) -> Result<String, AppError> {
    let token = generate_token(&TokenType::Session);
    let token_hash = hash_token(&token)?;

    let existing_sessions = access_tokens::Entity::find()
        .filter(access_tokens::Column::UserId.eq(user_id))
        .filter(access_tokens::Column::TokenType.eq(TokenType::Session.as_str()))
        .order_by_asc(access_tokens::Column::CreatedAt)
        .all(db)
        .await?;

    if existing_sessions.len() >= MAX_SESSIONS_PER_USER {
        if let Some(oldest) = existing_sessions.first() {
            access_tokens::Entity::delete_by_id(oldest.id).exec(db).await?;
        }
    }

    let session = access_tokens::ActiveModel {
        user_id: Set(user_id),
        token: Set(token_hash),
        token_type: Set(TokenType::Session.as_str().to_string()),
        name: Set(None),
        expires_at: Set(TokenType::Session.default_expiration()),
        last_used_at: Set(Some(Utc::now())),
        created_at: Set(Utc::now()),
        ..Default::default()
    };

    session.insert(db).await?;

    Ok(token)
}

pub async fn create_api_key(
    db: &DatabaseConnection,
    user_id: i64,
    name: String,
) -> Result<String, AppError> {
    let token = generate_token(&TokenType::ApiKey);
    let token_hash = hash_token(&token)?;

    let api_key_count = access_tokens::Entity::find()
        .filter(access_tokens::Column::UserId.eq(user_id))
        .filter(access_tokens::Column::TokenType.eq(TokenType::ApiKey.as_str()))
        .count(db)
        .await?;

    if api_key_count >= MAX_API_KEYS_PER_USER as u64 {
        return Err(AppError::Forbidden(format!("Maximum limit of {} API keys reached. Please revoke an existing key first.", MAX_API_KEYS_PER_USER)));
    }

    let api_key = access_tokens::ActiveModel {
        user_id: Set(user_id),
        token: Set(token_hash),
        token_type: Set(TokenType::ApiKey.as_str().to_string()),
        name: Set(Some(name)),
        expires_at: Set(None),
        last_used_at: Set(None),
        created_at: Set(Utc::now()),
        ..Default::default()
    };

    api_key.insert(db).await?;

    Ok(token)
}

pub async fn revoke_token(
    db: &DatabaseConnection,
    token: &str,
    user_id: i64,
) -> Result<(), AppError> {
    let token_hash = hash_token(token)?;
    access_tokens::Entity::delete_many()
        .filter(access_tokens::Column::Token.eq(token_hash))
        .filter(access_tokens::Column::UserId.eq(user_id))
        .exec(db)
        .await?;

    Ok(())
}

pub async fn list_user_tokens(
    db: &DatabaseConnection,
    user_id: i64,
    token_type: Option<TokenType>,
) -> Result<Vec<TokenInfo>, AppError> {
    use sea_orm::*;

    let mut query = access_tokens::Entity::find().filter(access_tokens::Column::UserId.eq(user_id));

    if let Some(tt) = token_type {
        query = query.filter(access_tokens::Column::TokenType.eq(tt.as_str()));
    }

    let tokens = query.all(db).await?;

    Ok(tokens
        .into_iter()
        .map(|t| TokenInfo {
            id: t.id,
            name: t.name.clone(),
            token_type: t.token_type.clone(),
            created_at: t.created_at,
            last_used_at: t.last_used_at,
            expires_at: t.expires_at,
        })
        .collect())
}
