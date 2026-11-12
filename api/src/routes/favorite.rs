use axum::{
    extract::{Path, State},
    http::StatusCode,
};
use ::entity::{fonts, user_favorites};

use crate::{AppState, utilities::AuthUser, error::AppError};

pub async fn add_favorite(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(font_id): Path<i64>,
) -> Result<StatusCode, AppError> {
    use sea_orm::*;

    let _font = fonts::Entity::find_by_id(font_id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Font not found".to_string()))?;

    let existing = user_favorites::Entity::find()
        .filter(user_favorites::Column::UserId.eq(auth.id))
        .filter(user_favorites::Column::FontId.eq(font_id))
        .one(&state.db)
        .await?;

    if existing.is_some() {
        return Ok(StatusCode::OK);
    }

    let favorite = user_favorites::ActiveModel {
        user_id: Set(auth.id),
        font_id: Set(font_id),
        ..Default::default()
    };

    favorite.insert(&state.db).await?;

    state
        .db
        .execute_unprepared(&format!(
            "UPDATE fonts SET favorite_count = favorite_count + 1 WHERE id = {}",
            font_id
        ))
        .await?;

    Ok(StatusCode::OK)
}

pub async fn remove_favorite(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(font_id): Path<i64>,
) -> Result<StatusCode, AppError> {
    use sea_orm::*;

    let delete_result = user_favorites::Entity::delete_many()
        .filter(user_favorites::Column::UserId.eq(auth.id))
        .filter(user_favorites::Column::FontId.eq(font_id))
        .exec(&state.db)
        .await?;

    if delete_result.rows_affected > 0 {
        state
            .db
            .execute_unprepared(&format!(
                "UPDATE fonts SET favorite_count = MAX(0, favorite_count - 1) WHERE id = {}",
                font_id
            ))
            .await?;
    }

    Ok(StatusCode::NO_CONTENT)
}