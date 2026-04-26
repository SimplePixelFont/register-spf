use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub struct User {
    pub id: i64,
}

#[derive(Serialize)]
pub struct PublicUser {
    pub id: i64,
    pub username: String,
}

#[derive(Serialize)]
pub struct FontWithDetails {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub file_path: String,
    pub download_count: i64,
    pub favorite_count: i64,
    pub tags: Vec<String>,
    pub versions: Vec<FontVersionInfo>,
    pub comment_count: i64,
}

#[derive(Serialize)]
pub struct FontVersionInfo {
    pub id: i64,
    pub version_number: i64,
    pub file_path: String,
    pub changelog: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct CommentWithUser {
    pub id: i64,
    pub text: String,
    pub created_at: DateTime<Utc>,
    pub user: PublicUser,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub user: PublicUser,
    pub token: String,
    pub token_type: String,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
pub struct APIKeyResponse {
    pub token: String,
}

#[derive(Serialize)]
pub struct TokenInfo {
    pub id: i64,
    pub name: Option<String>,
    pub token_type: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
}

// REQUEST DTOs

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
}

#[derive(Deserialize)]
pub struct CreateCommentRequest {
    pub text: String,
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub query: Option<String>,
    pub tags: Option<String>,
    pub limit: Option<i64>,
}
