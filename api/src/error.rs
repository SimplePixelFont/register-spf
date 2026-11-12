use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Value>,
}

#[derive(Debug)]
pub enum AuthError {
    InvalidToken,
    TokenExpired,
    // UserNotFound,
    InvalidTokenType,
    TokenHashError,
}

#[derive(Debug)]
pub enum AppError {
    Auth(AuthError),
    Database(sea_orm::DbErr),
    SPF(spf::core::DeserializeError),
    BadRequest(String, Option<Value>),
    Unauthorized(String),
    Forbidden(String),
    NotFound(String),
    Internal(String),
    TooManyRequests(String),
}

impl AppError {
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::BadRequest(msg.into(), None)
    }

    pub fn with_context(msg: impl Into<String>, context: Value) -> Self {
        Self::BadRequest(msg.into(), Some(context))
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message, context) = match self {
            AppError::BadRequest(msg, ctx) => (StatusCode::BAD_REQUEST, "BAD_REQUEST", msg, ctx),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED", msg, None),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, "FORBIDDEN", msg, None),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "NOT_FOUND", msg, None),
            AppError::Internal(msg) => {
                tracing::error!(%msg, "Internal error");
                let msg_lower = msg.to_lowercase();
                if msg.contains("R2") || msg_lower.contains("storage") || msg_lower.contains("bucket") {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "STORAGE_ERROR",
                        "An unexpected storage error occurred".to_string(),
                        None,
                    )
                } else {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "INTERNAL_ERROR",
                        "An internal server error occurred".to_string(),
                        None,
                    )
                }
            }
            AppError::TooManyRequests(msg) => (StatusCode::TOO_MANY_REQUESTS, "RATE_LIMIT_EXCEEDED", msg, None),
            AppError::Database(err) => {
                tracing::error!(%err, "Database error");
                let err_msg = err.to_string();
                let err_lower = err_msg.to_lowercase();
                if err_msg.contains("UNIQUE constraint failed") || err_lower.contains("unique constraint") {
                    (
                        StatusCode::CONFLICT,
                        "CONFLICT",
                        "Resource already exists".to_string(),
                        None,
                    )
                } else if err_msg.contains("FOREIGN KEY constraint failed") || err_lower.contains("foreign key") {
                    (
                        StatusCode::BAD_REQUEST,
                        "BAD_REQUEST",
                        "Referenced entity does not exist".to_string(),
                        None,
                    )
                } else {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "DATABASE_ERROR",
                        "A database error occurred".to_string(),
                        None,
                    )
                }
            }
            AppError::SPF(err) => {
                tracing::error!(?err, "SPF parse error");
                (
                    StatusCode::BAD_REQUEST,
                    "BAD_REQUEST",
                    "Invalid SimplePixelFont format".to_string(),
                    None,
                )
            }
            AppError::Auth(err) => {
                let msg = match err {
                    AuthError::InvalidToken => "Invalid authentication token",
                    AuthError::TokenExpired => "Authentication token has expired",
                    // AuthError::UserNotFound => "User associated with token not found",
                    AuthError::InvalidTokenType => "Invalid token type provided",
                    AuthError::TokenHashError => "Security processing error",
                };
                (StatusCode::UNAUTHORIZED, "AUTH_ERROR", msg.to_string(), None)
            }
        };

        let body = Json(ErrorResponse {
            error: code.to_string(),
            message,
            context,
        });

        (status, body).into_response()
    }
}

impl From<sea_orm::DbErr> for AppError {
    fn from(err: sea_orm::DbErr) -> Self {
        AppError::Database(err)
    }
}

impl From<spf::core::DeserializeError> for AppError {
    fn from(err: spf::core::DeserializeError) -> Self {
        AppError::SPF(err)
    }
}

impl From<AuthError> for AppError {
    fn from(err: AuthError) -> Self {
        AppError::Auth(err)
    }
}

impl From<crate::utilities::token::TokenError> for AppError {
    fn from(err: crate::utilities::token::TokenError) -> Self {
        AppError::Auth(AuthError::from(err))
    }
}