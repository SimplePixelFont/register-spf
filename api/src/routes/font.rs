use crate::{
    AppState,
    error::AppError,
    model::{FontVersionInfo, FontWithDetails, SearchQuery},
    utilities::{AuthUser, get_r2_client_and_bucket},
};
use ::entity::{comments, font_tags, font_versions, fonts, tags};
use axum::{
    Json,
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
};
use minima_r2_sdk::{Bytes, CompleteMultipartUploadRequest, CompletedPart, UploadPartRequest};
use rustrict::CensorStr;
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

    if name.is_inappropriate() {
        return Err(AppError::bad_request("Name contains inappropriate content"));
    }
    if let Some(desc) = &description {
        if desc.is_inappropriate() {
            return Err(AppError::bad_request("Description contains inappropriate content"));
        }
    }
    for tag in &tags_input {
        if tag.is_inappropriate() {
            return Err(AppError::bad_request("Tag contains inappropriate content"));
        }
    }
    if tags_input.len() > 10 {
        return Err(AppError::bad_request("Too many tags"));
    }
    if let Some(changelog) = &changelog {
        if changelog.is_inappropriate() {
            return Err(AppError::bad_request("Changelog contains inappropriate content"));
        }
    }

    let same_slug = fonts::Entity::find()
        .filter(fonts::Column::Slug.eq(slug.clone()))
        .one(&state.db)
        .await?;
    if same_slug.is_some() {
        return Err(AppError::bad_request("Slug is already taken"));
    }


    let (_, data) = file_data.ok_or_else(|| AppError::bad_request("File is required"))?;
    layout_from_data(&data)?;
    let file_path = format!("{}/{}-v{}.spf", slug, slug, version);

    let upload = client
        .create_multipart_upload(&r2_bucket, &file_path)
        .await
        .map_err(|e| AppError::Internal(format!("R2 Upload Init Failed: {}", e)))?;

    let part = client
        .upload_part(UploadPartRequest {
            bucket: &r2_bucket,
            key: &file_path,
            upload_id: &upload.upload_id,
            part_number: 1,
            body: Bytes::from(data),
        })
        .await
        .map_err(|e| AppError::Internal(format!("R2 Part Upload Failed: {}", e)))?;

    client
        .complete_multipart_upload(CompleteMultipartUploadRequest {
            bucket: &r2_bucket,
            key: &file_path,
            upload_id: &upload.upload_id,
            parts: vec![CompletedPart {
                part_number: 1,
                etag: part.etag,
            }],
        })
        .await
        .map_err(|e| AppError::Internal(format!("R2 Completion Failed: {}", e)))?;

    let font_model = fonts::ActiveModel {
        name: Set(name),
        slug: Set(slug),
        description: Set(description),
        version_number: Set(version),
        file_path: Set(file_path.clone()),
        uploaded_by: Set(Some(auth.id)),
        status: Set("approved".to_string()),
        ..Default::default()
    };

    let font = font_model.insert(&state.db).await?;

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
        query_builder = query_builder.filter(fonts::Column::Name.contains(&q));
    }

    if let Some(t) = query.tags {
        let tag_list: Vec<String> = t.split(',').map(|s| s.trim().to_string()).collect();
        query_builder = query_builder
            .join(JoinType::InnerJoin, fonts::Relation::FontTags.def())
            .join(JoinType::InnerJoin, font_tags::Relation::Tags.def())
            .filter(tags::Column::Name.is_in(tag_list));
    }

    if let Some(limit) = query.limit {
        let limit = Ord::max(limit, 50);
        query_builder = query_builder.limit(limit as u64);
    }

    let fonts_list = query_builder.all(&state.db).await?;

    let mut result = Vec::new();
    for font in fonts_list {
        let tags_list = font_tags::Entity::find()
            .filter(font_tags::Column::FontId.eq(font.id))
            .find_also_related(tags::Entity)
            .all(&state.db)
            .await?;

        let tag_names: Vec<String> = tags_list
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

        result.push(FontWithDetails {
            id: font.id,
            name: font.name,
            slug: font.slug,
            description: font.description,
            file_path: font.file_path,
            download_count: font.download_count,
            favorite_count: font.favorite_count,
            tags: tag_names,
            comment_count,
            versions: version_list,
        });
    }

    Ok(Json(result))
}
