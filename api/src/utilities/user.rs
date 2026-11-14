use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use entity::users;
use password_hash::generate_salt;
use sea_orm::DatabaseConnection;
use crate::error::AppError;

pub async fn hash_password(password: String) -> Result<String, AppError> {
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::encode_b64(&generate_salt())
            .map_err(|e| AppError::Internal(format!("Failed to encode salt: {}", e)))?;
        let argon2 = Argon2::default();
        argon2
            .hash_password(password.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(|e| AppError::Internal(format!("Failed to hash password: {}", e)))
    })
    .await
    .map_err(|e| AppError::Internal(format!("Thread join error: {}", e)))?
}

pub async fn verify_password(password: String, hash: String) -> Result<bool, AppError> {
    tokio::task::spawn_blocking(move || {
        let parsed = PasswordHash::new(&hash)
            .map_err(|_| AppError::Internal("Security hash error".to_string()))?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    })
    .await
    .map_err(|e| AppError::Internal(format!("Thread join error: {}", e)))?
}

pub async fn create_user(
    db: &DatabaseConnection,
    username: &str,
    email: &str,
    password: &str,
) -> Result<users::Model, AppError> {
    use sea_orm::*;
    
    let username_norm = username.to_lowercase();
    let email_norm = email.to_lowercase();
    let password_hash = hash_password(password.to_string()).await?;
    
    let new_user = users::ActiveModel {
        username: Set(username_norm),
        email: Set(email_norm),
        password_hash: Set(password_hash),
        ..Default::default()
    };
    
    Ok(new_user.insert(db).await?)
}