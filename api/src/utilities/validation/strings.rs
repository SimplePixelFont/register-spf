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
