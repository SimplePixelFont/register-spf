use ::entity::{font_versions, fonts};
use axum::{
    extract::{Multipart, Path, State},
    Json,
};
use minima_r2_sdk::{Bytes, CompleteMultipartUploadRequest, CompletedPart, UploadPartRequest};
use rustrict::CensorStr;
use spf::core::layout_from_data;

use crate::{
    AppState,
    error::AppError,
    model::FontVersionInfo,
    utilities::{get_r2_client_and_bucket, AuthUser},
};

pub async fn create_version(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(font_id): Path<i64>,
    mut multipart: Multipart,
) -> Result<Json<FontVersionInfo>, AppError> {
    use sea_orm::*;

    let font = fonts::Entity::find_by_id(font_id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Font not found".to_string()))?;

    if font.uploaded_by != Some(auth.id) {
        return Err(AppError::Forbidden("Not your font".into()));
    }

    let mut version_number = 0;
    let mut changelog = None;
    let mut file_data: Option<(String, axum::body::Bytes)> = None;

    // Parse multipart fields
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::bad_request(e.to_string()))?
    {
        let name = field.name().unwrap_or_default().to_string();
        match name.as_str() {
            "version" => {
                version_number = field
                    .text()
                    .await
                    .unwrap_or_default()
                    .parse()
                    .unwrap_or_default()
            }
            "changelog" => changelog = Some(field.text().await.unwrap_or_default()),
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

    if let Some(changelog) = &changelog {
        if changelog.is_inappropriate() {
            return Err(AppError::bad_request("Changelog contains inappropriate content"));
        }
    }
    let same_version = font_versions::Entity::find()
        .filter(font_versions::Column::FontId.eq(font.id))
        .filter(font_versions::Column::VersionNumber.eq(version_number))
        .one(&state.db)
        .await?;
    if same_version.is_some() {
        return Err(AppError::bad_request("Version already exists"));
    }


    let (_, data) = file_data.ok_or_else(|| AppError::bad_request("File is required"))?;
    layout_from_data(&data)?;
    let file_path = format!("{}/{}-v{}.spf", font.slug, font.slug, version_number);

    let (client, r2_bucket) = get_r2_client_and_bucket().await?;
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

    let version = font_versions::ActiveModel {
        font_id: Set(font.id),
        version_number: Set(version_number),
        file_path: Set(file_path.clone()),
        changelog: Set(changelog),
        ..Default::default()
    };

    let version = version.insert(&state.db).await?;

    let font_model = fonts::ActiveModel {
        id: Set(font.id),
        version_number: Set(version_number),
        file_path: Set(file_path),
        ..Default::default()
    };
    font_model
        .update(&state.db)
        .await?;

    Ok(Json(FontVersionInfo {
        id: version.id,
        version_number: version.version_number,
        file_path: version.file_path,
        changelog: version.changelog,
        created_at: version.created_at,
    }))
}
