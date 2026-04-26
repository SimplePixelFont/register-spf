use crate::error::AppError;
use minima_r2_sdk::{Credentials, R2Client};
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
