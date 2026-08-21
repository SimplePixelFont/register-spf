use crate::error::AppError;
use rustrict::CensorStr;

pub fn validate_comment(text: &str) -> Result<(), AppError> {
    if text.len() > 200 {
        return Err(AppError::bad_request("Comment too long (max 200 characters)"));
    }
    if text.is_inappropriate() {
        return Err(AppError::bad_request("Comment contains inappropriate content"));
    }
    Ok(())
}

pub fn validate_snippet(
    name: &str,
    description: Option<&str>,
    source: &str,
) -> Result<(), AppError> {
    if name.is_empty() {
        return Err(AppError::bad_request("Name is required"));
    }
    if name.len() > 80 {
        return Err(AppError::bad_request("Name too long (max 80 characters)"));
    }
    if name.is_inappropriate() {
        return Err(AppError::bad_request("Name contains inappropriate content"));
    }
    if let Some(desc) = description {
        if desc.len() > 500 {
            return Err(AppError::bad_request(
                "Description too long (max 500 characters)",
            ));
        }
        if desc.is_inappropriate() {
            return Err(AppError::bad_request(
                "Description contains inappropriate content",
            ));
        }
    }
    if source.is_empty() {
        return Err(AppError::bad_request("Source is required"));
    }
    if source.len() > 16_384 {
        return Err(AppError::bad_request("Source too large (max 16 KB)"));
    }
    Ok(())
}
