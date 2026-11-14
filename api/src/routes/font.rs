use crate::{
    AppState,
    error::AppError,
    model::{FontVersionInfo, FontWithDetails, SearchQuery},
    utilities::{
        AuthUser, escape_like_wildcards, get_r2_client_and_bucket, trim_and_validate_non_empty,
        upload_file, validate_font,
    },
};
use ::entity::{comments, font_tags, font_versions, fonts, tags};
use axum::{
    Json,
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
};
use spf::core::layout_from_data;

pub async fn get_font_with_details(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<FontWithDetails>, AppError> {
    use sea_orm::*;

    let font = fonts::Entity::find()
        .filter(fonts::Column::Slug.eq(&slug))
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Font not found".to_string()))?;

    let tags: Vec<(font_tags::Model, Option<tags::Model>)> = font_tags::Entity::find()
        .filter(font_tags::Column::FontId.eq(font.id))
        .find_also_related(tags::Entity)
        .all(&state.db)
        .await?;

    let tag_names: Vec<String> = tags
        .into_iter()
        .filter_map(|(_, tag)| tag.map(|t| t.name))
        .collect();

    let versions = font_versions::Entity::find()
        .filter(font_versions::Column::FontId.eq(font.id))
        .order_by_desc(font_versions::Column::CreatedAt)
        .all(&state.db)
        .await?;

    let version_list: Vec<FontVersionInfo> = versions
        .into_iter()
        .map(|v| FontVersionInfo {
            id: v.id,
            version_number: v.version_number,
            file_path: v.file_path,
            changelog: v.changelog,
            created_at: v.created_at,
        })
        .collect();

    let comment_count = comments::Entity::find()
        .filter(comments::Column::FontId.eq(font.id))
        .count(&state.db)
        .await? as i64;

    Ok(Json(FontWithDetails {
        id: font.id,
        name: font.name,
        slug: font.slug,
        description: font.description,
        file_path: font.file_path,
        download_count: font.download_count,
        favorite_count: font.favorite_count,
        tags: tag_names,
        versions: version_list,
        comment_count,
    }))
}

pub async fn create_font(
    State(state): State<AppState>,
    auth: AuthUser,
    mut multipart: Multipart,
) -> Result<Json<FontWithDetails>, AppError> {
    use sea_orm::*;

    let (client, r2_bucket) = get_r2_client_and_bucket().await?;
    let mut name = String::new();
    let mut slug = String::new();
    let mut version = 0;
    let mut changelog = None;
    let mut description = None;
    let mut tags_input = Vec::new();
    let mut file_data: Option<(String, axum::body::Bytes)> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::bad_request(e.to_string()))?
    {
        let field_name = field.name().unwrap_or_default().to_string();
        match field_name.as_str() {
            "name" => name = field.text().await.unwrap_or_default(),
            "slug" => slug = field.text().await.unwrap_or_default(),
            "version" => {
                version = field
                    .text()
                    .await
                    .unwrap_or_default()
                    .parse()
                    .unwrap_or_default()
            }
            "changelog" => changelog = Some(field.text().await.unwrap_or_default()),
            "description" => description = Some(field.text().await.unwrap_or_default()),
            "tags" => {
                let t = field.text().await.unwrap_or_default();
                tags_input = t.split(',').map(|s| s.trim().to_string()).collect();
            }
            "file" => {
                let filename = field.file_name().unwrap_or("unknown").to_string();
                let data = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))?;
                file_data = Some((filename, data));
            }
            _ => {}
        }
    }

    let name = trim_and_validate_non_empty(&name, "Name")?;
    let slug = slug.trim().to_lowercase();
    let description = description.map(|d| d.trim().to_string()).filter(|d| !d.is_empty());
    let changelog = changelog.map(|c| c.trim().to_string()).filter(|c| !c.is_empty());

    validate_font(&name, &slug, &description, &tags_input, &changelog)?;

    let same_slug = fonts::Entity::find()
        .filter(fonts::Column::Slug.eq(slug.clone()))
        .one(&state.db)
        .await?;
    if same_slug.is_some() {
        return Err(AppError::bad_request("Slug is already taken"));
    }

    let (_, data) = file_data.ok_or_else(|| AppError::bad_request("File is required"))?;
    layout_from_data(&data)?;
    let file_path = format!("fonts/{}-v{}.spf", slug, version);

    // 1. Insert DB row first as draft to claim slug uniqueness in SQLite
    let font_model = fonts::ActiveModel {
        name: Set(name),
        slug: Set(slug),
        description: Set(description),
        version_number: Set(version),
        file_path: Set(file_path.clone()),
        uploaded_by: Set(Some(auth.id)),
        status: Set("draft".to_string()),
        ..Default::default()
    };

    let font = font_model.insert(&state.db).await?;

    // 2. Upload file to R2; if upload fails, delete draft row and return error
    if let Err(upload_err) = upload_file(&client, &r2_bucket, &file_path, data.into()).await {
        fonts::Entity::delete_by_id(font.id).exec(&state.db).await.ok();
        return Err(upload_err);
    }

    // 3. Mark font approved
    let mut font_update: fonts::ActiveModel = font.into();
    font_update.status = Set("approved".to_string());
    let font = font_update.update(&state.db).await?;

    for tag_name in &tags_input {
        let tag = tags::Entity::find()
            .filter(tags::Column::Name.eq(tag_name))
            .one(&state.db)
            .await?;

        let tag = match tag {
            Some(t) => t,
            None => {
                let new_tag = tags::ActiveModel {
                    name: Set(tag_name.clone()),
                    slug: Set(tag_name.to_lowercase().replace(" ", "-")),
                    ..Default::default()
                };
                new_tag.insert(&state.db).await?
            }
        };

        let font_tag = font_tags::ActiveModel {
            font_id: Set(font.id),
            tag_id: Set(tag.id),
            ..Default::default()
        };
        font_tag.insert(&state.db).await?;
    }

    let version_model = font_versions::ActiveModel {
        font_id: Set(font.id),
        version_number: Set(version),
        file_path: Set(file_path),
        changelog: Set(changelog),
        ..Default::default()
    };

    let version = version_model.insert(&state.db).await?;

    Ok(Json(FontWithDetails {
        id: font.id,
        name: font.name,
        slug: font.slug,
        description: font.description,
        file_path: font.file_path,
        download_count: font.download_count,
        favorite_count: font.favorite_count,
        tags: tags_input,
        versions: vec![FontVersionInfo {
            id: version.id,
            version_number: version.version_number,
            file_path: version.file_path,
            changelog: version.changelog,
            created_at: version.created_at,
        }],
        comment_count: 0,
    }))
}

pub async fn delete_font(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(slug): Path<String>,
) -> Result<StatusCode, AppError> {
    use sea_orm::*;

    let font = fonts::Entity::find()
        .filter(fonts::Column::Slug.eq(&slug))
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Font not found".to_string()))?;

    if font.uploaded_by != Some(auth.id) {
        return Err(AppError::Forbidden(
            "You do not have permission to delete this font".into(),
        ));
    }

    let versions = font_versions::Entity::find()
        .filter(font_versions::Column::FontId.eq(font.id))
        .all(&state.db)
        .await?;

    let file_paths: Vec<String> = versions.into_iter().map(|v| v.file_path).collect();
    let (client, r2_bucket) = get_r2_client_and_bucket().await?;

    fonts::Entity::delete_by_id(font.id).exec(&state.db).await?;

    for path in file_paths {
        // Might be handled later on
        let _ = client.delete_object(&r2_bucket, &path).await;
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn search_fonts(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<FontWithDetails>>, AppError> {
    use sea_orm::*;

    let mut query_builder = fonts::Entity::find()
        .filter(fonts::Column::Status.eq("approved"))
        .distinct();

    if let Some(q) = query.query {
        let trimmed_q = q.trim();
        if !trimmed_q.is_empty() {
            let escaped_q = escape_like_wildcards(trimmed_q);
            query_builder = query_builder.filter(fonts::Column::Name.contains(&escaped_q));
        }
    }

    if let Some(t) = query.tags {
        let tag_list: Vec<String> = t
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if !tag_list.is_empty() {
            query_builder = query_builder
                .join(JoinType::InnerJoin, fonts::Relation::FontTags.def())
                .join(JoinType::InnerJoin, font_tags::Relation::Tags.def())
                .filter(tags::Column::Name.is_in(tag_list));
        }
    }

    let limit = query.limit.unwrap_or(50).clamp(1, 50) as u64;
    let page = std::cmp::max(query.page.unwrap_or(1), 1);
    let offset = (page - 1) * limit;
    query_builder = query_builder.limit(limit).offset(offset);

    let fonts_list = query_builder.all(&state.db).await?;
    if fonts_list.is_empty() {
        return Ok(Json(Vec::new()));
    }

    let font_ids: Vec<i64> = fonts_list.iter().map(|f| f.id).collect();

    // Batch 1: Tags
    let all_tags: Vec<(font_tags::Model, Option<tags::Model>)> = font_tags::Entity::find()
        .filter(font_tags::Column::FontId.is_in(font_ids.clone()))
        .find_also_related(tags::Entity)
        .all(&state.db)
        .await?;
    let mut tags_by_font: std::collections::HashMap<i64, Vec<String>> =
        std::collections::HashMap::new();
    for (ft, tag) in all_tags {
        if let Some(t) = tag {
            tags_by_font.entry(ft.font_id).or_default().push(t.name);
        }
    }

    // Batch 2: Versions
    let all_versions = font_versions::Entity::find()
        .filter(font_versions::Column::FontId.is_in(font_ids.clone()))
        .order_by_desc(font_versions::Column::CreatedAt)
        .all(&state.db)
        .await?;
    let mut versions_by_font: std::collections::HashMap<i64, Vec<FontVersionInfo>> =
        std::collections::HashMap::new();
    for v in all_versions {
        versions_by_font
            .entry(v.font_id)
            .or_default()
            .push(FontVersionInfo {
                id: v.id,
                version_number: v.version_number,
                file_path: v.file_path,
                changelog: v.changelog,
                created_at: v.created_at,
            });
    }

    // Batch 3: Comments count
    let all_comments = comments::Entity::find()
        .filter(comments::Column::FontId.is_in(font_ids))
        .all(&state.db)
        .await?;
    let mut comment_counts: std::collections::HashMap<i64, i64> =
        std::collections::HashMap::new();
    for c in all_comments {
        *comment_counts.entry(c.font_id).or_insert(0) += 1;
    }

    let result = fonts_list
        .into_iter()
        .map(|font| {
            let tags = tags_by_font.remove(&font.id).unwrap_or_default();
            let versions = versions_by_font.remove(&font.id).unwrap_or_default();
            let comment_count = comment_counts.get(&font.id).copied().unwrap_or(0);
            FontWithDetails {
                id: font.id,
                name: font.name,
                slug: font.slug,
                description: font.description,
                file_path: font.file_path,
                download_count: font.download_count,
                favorite_count: font.favorite_count,
                tags,
                comment_count,
                versions,
            }
        })
        .collect();

    Ok(Json(result))
}
