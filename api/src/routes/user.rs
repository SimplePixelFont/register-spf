use crate::{
    AppState,
    error::AppError,
    model::{
        APIKeyResponse, AuthResponse, CreateApiKeyRequest, LoginRequest, PublicUser,
        RegisterRequest, TokenInfo,
    },
    utilities::{
        AuthUser, create_api_key, create_session, create_user, is_unique_email, is_unique_username,
        list_user_tokens, revoke_token,
    },
};
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use check_if_email_exists::{CheckEmailInputBuilder, Reachable, check_email};
use chrono::{Duration, Utc};
use entity::users;
use rustrict::CensorStr;
use sea_orm::DatabaseConnection;
use serde_json::json;

pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    let username = &req.username;
    let password = &req.password;
    let email = &req.email;
    let db = &state.db;

    if username.len() < 3 || username.len() > 30 {
        return Err(AppError::bad_request(
            "Username must be between 3 and 30 characters",
        ));
    }
    if username.is_inappropriate() {
        return Err(AppError::bad_request("Username contains inappropriate content"));
    }
    if password.len() < 8 || password.len() > 50 {
        return Err(AppError::bad_request(
            "Password must be between 8 and 50 characters",
        ));
    }
    if !email.contains('@') {
        return Err(AppError::bad_request("Email address is invalid"));
    }

    if !is_unique_username(username, db)
        .await
        .map_err(AppError::from)?
    {
        return Err(AppError::bad_request("Username is already taken"));
    }
    if !is_unique_email(email, db).await.map_err(AppError::from)? {
        return Err(AppError::bad_request("Email address is already taken"));
    }

    if !mailchecker::is_valid(&req.email) {
        return Err(AppError::with_context(
            "Email provider not allowed",
            json!({ "reason": "disposable_provider" }),
        ));
    }

    let input = CheckEmailInputBuilder::default()
        .to_email(req.email.clone())
        .build()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let result = check_email(&input).await;
    if !(result.is_reachable == Reachable::Safe) {
        return Err(AppError::bad_request("Email address is not reachable"));
    }

    let user = create_user(&state.db, &req.username, &req.email, &req.password).await?;
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

    let user = users::Entity::find()
        .filter(users::Column::Email.eq(email))
        .one(db)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid credentials".to_string()))?;

    let parsed_hash = PasswordHash::new(&user.password_hash)
        .map_err(|_| AppError::Internal("Security hash error".to_string()))?;

    if !(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
    {
        return Err(AppError::Unauthorized("Invalid credentials".to_string()));
    }

    Ok(user)
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
    Path(token_id): Path<i32>,
) -> Result<StatusCode, AppError> {
    revoke_token(&state.db, &token_id.to_string(), auth.id).await?;

    Ok(StatusCode::NO_CONTENT)
}
