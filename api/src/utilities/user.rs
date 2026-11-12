use entity::users;
use sea_orm::DatabaseConnection;
use crate::error::AppError;
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHasher};
use password_hash::generate_salt;

pub fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::encode_b64(&generate_salt())
        .map_err(|e| AppError::Internal(format!("Failed to encode salt: {}", e)))?;
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| AppError::Internal(format!("Failed to hash password: {}", e)))?
        .to_string();
    Ok(hash)
}

pub async fn create_user(
    db: &DatabaseConnection,
    username: &str,
    email: &str,
    password: &str,
) -> Result<users::Model, AppError> {
    use sea_orm::*;
    
    let password_hash = hash_password(password)?;
    
    let new_user = users::ActiveModel {
        username: Set(username.to_string()),
        email: Set(email.to_string()),
        password_hash: Set(password_hash),
        ..Default::default()
    };
    
    Ok(new_user.insert(db).await?)
}