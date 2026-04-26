use entity::*;
use sea_orm::DatabaseConnection;
use crate::error::AppError;

pub async fn is_unique_username(username: &str, db: &DatabaseConnection) -> Result<bool, AppError> {
    use sea_orm::*;

    let same_username = users::Entity::find()
        .filter(users::Column::Username.eq(username))
        .one(db)
        .await?;
    
    if same_username.is_some() {
        return Ok(false)
    }
    Ok(true)
}

pub async fn is_unique_email(email: &str, db: &DatabaseConnection) -> Result<bool, AppError> {
    use sea_orm::*;

    let same_email = users::Entity::find()
        .filter(users::Column::Email.eq(email))
        .one(db)
        .await?;
    
    if same_email.is_some() {
        return Ok(false)
    }
    Ok(true)   
}