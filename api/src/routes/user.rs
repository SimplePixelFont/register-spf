use crate::{
    AppState,
    error::AppError,
    model::{
        APIKeyResponse, AuthResponse, CreateApiKeyRequest, LoginRequest, PublicUser,
        RegisterRequest, TokenInfo,
    },
    utilities::{
        AuthUser, create_api_key, create_session, create_user, is_unique_email, is_unique_username,
        is_url_component_safe, list_user_tokens, revoke_token_by_id, verify_password,
    },
};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use chrono::{Duration, Utc};
use entity::users;
use rustrict::CensorStr;
use sea_orm::DatabaseConnection;
use serde_json::json;

pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    let username = req.username.trim().to_lowercase();
    let email = req.email.trim().to_lowercase();
    let password = &req.password;
    let db = &state.db;

    if username.len() < 3 || username.len() > 30 {
        return Err(AppError::bad_request(
            "Username must be between 3 and 30 characters",
        ));
    }
    if !is_url_component_safe(&username) {
        return Err(AppError::bad_request(
            "Username contains invalid characters",
        ));
    }
    if username.is_inappropriate() {
        return Err(AppError::bad_request(
            "Username contains inappropriate content",
        ));
    }
    if password.len() < 8 || password.len() > 50 {
        return Err(AppError::bad_request(
            "Password must be between 8 and 50 characters",
        ));
    }
    if !email.contains('@') {
        return Err(AppError::bad_request("Email address is invalid"));
    }

    if !is_unique_username(&username, db)
        .await
        .map_err(AppError::from)?
    {
        return Err(AppError::bad_request("Username is already taken"));
    }
    if !is_unique_email(&email, db).await.map_err(AppError::from)? {
        return Err(AppError::bad_request("Email address is already taken"));
    }

    if !mailchecker::is_valid(&email) {
        return Err(AppError::with_context(
            "Email provider not allowed",
            json!({ "reason": "disposable_provider" }),
        ));
    }

    let user = create_user(&state.db, &username, &email, password).await?;
    let token = create_session(&state.db, user.id).await?;

    Ok(Json(AuthResponse {
        user: PublicUser {
            id: user.id,
            username: user.username,
        },
        token,
        token_type: "session".to_string(),
        expires_at: Some(Utc::now() + Duration::days(30)),
    }))
}

pub async fn authenticate_user(
    db: &DatabaseConnection,
    email: &str,
    password: &str,
) -> Result<users::Model, AppError> {
    use sea_orm::*;

    const DUMMY_ARGON2_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$c29tZXNhbHQAAAAAAAAAAA$iEOsWw2FkP+H2L7t2hI8g6tqZ1x0e4s8f9q2b5v1w0A";

    let email_normalized = email.trim().to_lowercase();

    let user_opt = users::Entity::find()
        .filter(users::Column::Email.eq(&email_normalized))
        .one(db)
        .await?;

    match user_opt {
        Some(user) => {
            let valid = verify_password(password.to_string(), user.password_hash.clone()).await?;
            if !valid {
                return Err(AppError::Unauthorized("Invalid credentials".to_string()));
            }
            Ok(user)
        }
        None => {
            verify_password(password.to_string(), DUMMY_ARGON2_HASH.to_string())
                .await
                .ok();
            Err(AppError::Unauthorized("Invalid credentials".to_string()))
        }
    }
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    let user = authenticate_user(&state.db, &req.email, &req.password).await?;
    let token = create_session(&state.db, user.id).await?;

    Ok(Json(AuthResponse {
        user: PublicUser {
            id: user.id,
            username: user.username,
        },
        token,
        token_type: "session".to_string(),
        expires_at: Some(Utc::now() + Duration::days(30)),
    }))
}

pub async fn create_user_api_key(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateApiKeyRequest>,
) -> Result<Json<APIKeyResponse>, AppError> {
    let api_key = create_api_key(&state.db, auth.id, req.name).await?;

    Ok(Json(APIKeyResponse { token: api_key }))
}

pub async fn get_my_tokens(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<TokenInfo>>, AppError> {
    let tokens = list_user_tokens(&state.db, auth.id, None).await?;

    Ok(Json(tokens))
}

pub async fn revoke_my_token(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(token_id): Path<i64>,
) -> Result<StatusCode, AppError> {
    revoke_token_by_id(&state.db, &state.auth_cache, token_id, auth.id).await?;

    Ok(StatusCode::NO_CONTENT)
}
