use entity::users;
use sea_orm::DatabaseConnection;
use crate::utilities::hash_token;
use crate::error::AppError;

pub async fn create_user(
    db: &DatabaseConnection,
    username: &str,
    email: &str,
    password: &str,
) -> Result<users::Model, AppError> {
    use sea_orm::*;
    
    let password_hash = hash_token(password)?;
    
    let new_user = users::ActiveModel {
        username: Set(username.to_string()),
        email: Set(email.to_string()),
        password_hash: Set(password_hash),
        ..Default::default()
    };
    
    Ok(new_user.insert(db).await?)
}