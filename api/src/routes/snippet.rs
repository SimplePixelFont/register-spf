use crate::{
    AppState,
    error::AppError,
    model::{CreateSnippetRequest, PublicUser, SnippetInfo, SnippetSearchQuery},
    utilities::{AuthUser, validate_snippet},
};
use ::entity::{snippets, users};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};

fn to_snippet_info(model: snippets::Model, user: Option<users::Model>) -> SnippetInfo {
    SnippetInfo {
        id: model.id,
        name: model.name,
        description: model.description,
        source: model.source,
        created_at: model.created_at,
        user: user.map(|u| PublicUser {
            id: u.id,
            username: u.username,
        }),
    }
}

pub async fn create_snippet(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<CreateSnippetRequest>,
) -> Result<Json<SnippetInfo>, AppError> {
    use sea_orm::*;

    validate_snippet(
        &payload.name,
        payload.description.as_deref(),
        &payload.source,
    )?;

    let snippet_model = snippets::ActiveModel {
        user_id: Set(Some(auth.id)),
        name: Set(payload.name),
        description: Set(payload.description),
        source: Set(payload.source),
        status: Set("approved".to_string()),
        ..Default::default()
    };

    let snippet = snippet_model.insert(&state.db).await?;

    let user = users::Entity::find_by_id(auth.id).one(&state.db).await?;

    Ok(Json(to_snippet_info(snippet, user)))
}

pub async fn search_snippets(
    State(state): State<AppState>,
    Query(query): Query<SnippetSearchQuery>,
) -> Result<Json<Vec<SnippetInfo>>, AppError> {
    use sea_orm::*;

    let mut query_builder =
        snippets::Entity::find().filter(snippets::Column::Status.eq("approved"));

    if let Some(q) = query.query {
        query_builder = query_builder.filter(snippets::Column::Name.contains(&q));
    }

    let limit = query.limit.unwrap_or(50).clamp(1, 50);
    query_builder = query_builder.limit(limit as u64);

    let snippet_list = query_builder.all(&state.db).await?;

    let mut result = Vec::new();
    for snippet in snippet_list {
        let user = match snippet.user_id {
            Some(uid) => users::Entity::find_by_id(uid).one(&state.db).await?,
            None => None,
        };
        result.push(to_snippet_info(snippet, user));
    }

    Ok(Json(result))
}

pub async fn get_snippet(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<SnippetInfo>, AppError> {
    use sea_orm::*;

    let snippet = snippets::Entity::find_by_id(id)
        .filter(snippets::Column::Status.eq("approved"))
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Snippet not found".to_string()))?;

    let user = match snippet.user_id {
        Some(uid) => users::Entity::find_by_id(uid).one(&state.db).await?,
        None => None,
    };

    Ok(Json(to_snippet_info(snippet, user)))
}

pub async fn delete_snippet(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    use sea_orm::*;

    let snippet = snippets::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Snippet not found".to_string()))?;

    if snippet.user_id != Some(auth.id) {
        return Err(AppError::Forbidden(
            "You do not have permission to delete this snippet".into(),
        ));
    }

    snippets::Entity::delete_by_id(snippet.id)
        .exec(&state.db)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}
