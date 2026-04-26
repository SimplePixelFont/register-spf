use crate::{
    AppState, error::AppError,
    model::{CommentWithUser, CreateCommentRequest, PublicUser},
    utilities::{AuthUser, validate_comment},
};
use ::entity::{comments, users};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

pub async fn create_comment(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(font_id): Path<i32>,
    Json(req): Json<CreateCommentRequest>,
) -> Result<Json<CommentWithUser>, AppError> {
    use sea_orm::*;
    validate_comment(&req.text)?;

    let comment = comments::ActiveModel {
        font_id: Set(font_id as i64),
        user_id: Set(auth.id),
        text: Set(req.text),
        ..Default::default()
    };

    let comment = comment
        .insert(&state.db)
        .await?;

    let user = users::Entity::find_by_id(auth.id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    Ok(Json(CommentWithUser {
        id: comment.id,
        text: comment.text,
        created_at: comment.created_at,
        user: PublicUser {
            id: user.id,
            username: user.username,
        },
    }))
}

pub async fn get_comments(
    State(state): State<AppState>,
    Path(font_id): Path<i32>,
) -> Result<Json<Vec<CommentWithUser>>, AppError> {
    use sea_orm::*;

    let comments = comments::Entity::find()
        .filter(comments::Column::FontId.eq(font_id))
        .find_also_related(users::Entity)
        .order_by_desc(comments::Column::CreatedAt)
        .all(&state.db)
        .await?;

    let results: Vec<CommentWithUser> = comments
        .into_iter()
        .filter_map(|(comment, user)| {
            user.map(|u| CommentWithUser {
                id: comment.id,
                text: comment.text,
                created_at: comment.created_at,
                user: PublicUser {
                    id: u.id,
                    username: u.username,
                },
            })
        })
        .collect();

    Ok(Json(results))
}

pub async fn delete_comment(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(comment_id): Path<i32>,
) -> Result<StatusCode, AppError> {
    use sea_orm::*;

    let comment = comments::Entity::find_by_id(comment_id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Comment not found".to_string()))?;

    if comment.user_id != auth.id {
        return Err(AppError::Forbidden("Not your comment".into()));
    }

    comments::Entity::delete_by_id(comment_id)
        .exec(&state.db)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}
