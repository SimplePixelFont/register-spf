use crate::error::AppError;
use minima_r2_sdk::{
    Bytes, CompleteMultipartUploadRequest, CompletedPart, Credentials, R2Client, UploadPartRequest,
};
use std::env;

pub async fn get_r2_client_and_bucket() -> Result<(R2Client, String), AppError> {
    let r2_account_id = env::var("R2_ACCOUNT_ID")
        .map_err(|_| AppError::Internal("R2_ACCOUNT_ID not set".into()))?;
    let r2_access_key = env::var("R2_ACCESS_KEY")
        .map_err(|_| AppError::Internal("R2_ACCESS_KEY not set".into()))?;
    let r2_secret_key = env::var("R2_SECRET_KEY")
        .map_err(|_| AppError::Internal("R2_SECRET_KEY not set".into()))?;
    let r2_bucket = env::var("R2_BUCKET_NAME").unwrap_or_else(|_| "fonts".to_string());

    let credentials = Credentials::new(r2_access_key, r2_secret_key);
    let client = R2Client::new(r2_account_id, credentials);
    Ok((client, r2_bucket))
}

pub async fn upload_file(
    client: &R2Client,
    r2_bucket: &str,
    file_path: &str,
    data: Vec<u8>,
) -> Result<(), AppError> {
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

    Ok(())
}
