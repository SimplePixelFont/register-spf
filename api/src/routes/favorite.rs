use axum::{
    extract::{Path, State},
    http::StatusCode,
};
use ::entity::{fonts, user_favorites};

use crate::{AppState, utilities::AuthUser, error::AppError};

pub async fn add_favorite(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(font_id): Path<i32>,
) -> Result<StatusCode, AppError> {
    use sea_orm::*;
 
    let favorite = user_favorites::ActiveModel {
        user_id: Set(auth.id),
        font_id: Set(font_id as i64),
        ..Default::default()
    };
 
    favorite.insert(&state.db).await?;
 
    let font = fonts::Entity::find_by_id(font_id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Font not found".to_string()))?;
 
    let mut font: fonts::ActiveModel = font.into();
    font.favorite_count = Set(font.favorite_count.unwrap() + 1);
    font.update(&state.db).await?;
 
    Ok(StatusCode::OK)
}