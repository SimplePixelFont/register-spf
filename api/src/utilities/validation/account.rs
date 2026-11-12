use entity::*;
use sea_orm::DatabaseConnection;
use crate::error::AppError;

pub async fn is_unique_username(username: &str, db: &DatabaseConnection) -> Result<bool, AppError> {
    use sea_orm::*;

    let username_norm = username.trim().to_lowercase();
    let same_username = users::Entity::find()
        .filter(users::Column::Username.eq(&username_norm))
        .one(db)
        .await?;
    
    Ok(same_username.is_none())
}

pub async fn is_unique_email(email: &str, db: &DatabaseConnection) -> Result<bool, AppError> {
    use sea_orm::*;

    let email_norm = email.trim().to_lowercase();
    let same_email = users::Entity::find()
        .filter(users::Column::Email.eq(&email_norm))
        .one(db)
        .await?;
    
    Ok(same_email.is_none())   
}