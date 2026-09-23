use crate::error::AppError;
use rustrict::CensorStr;
use unicode_segmentation::UnicodeSegmentation;

fn validate_slug(s: &str) -> bool {
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn validate_comment(text: &str) -> Result<(), AppError> {
    if text.graphemes(true).count() > 200 {
        return Err(AppError::bad_request(
            "Comment too long (max 200 characters)",
        ));
    }
    if text.is_inappropriate() {
        return Err(AppError::bad_request(
            "Comment contains inappropriate content",
        ));
    }
    Ok(())
}

pub fn validate_font(
    name: &str,
    slug: &str,
    description: &Option<String>,
    tags_input: &Vec<String>,
    changelog: &Option<String>,
) -> Result<(), AppError> {
    if name.is_inappropriate() {
        return Err(AppError::bad_request("Name contains inappropriate content"));
    }
    if name.graphemes(true).count() > 100 {
        return Err(AppError::bad_request("Name too long (max 100 characters)"));
    }

    if slug.len() > 100 {
        return Err(AppError::bad_request("Slug too long (max 100 characters)"));
    }
    if slug.is_inappropriate() {
        return Err(AppError::bad_request("Slug contains inappropriate content"));
    }
    if !validate_slug(slug) {
        return Err(AppError::bad_request("Slug contains invalid characters"));
    }

    if let Some(desc) = &description {
        if desc.is_inappropriate() {
            return Err(AppError::bad_request(
                "Description contains inappropriate content",
            ));
        }
        if desc.graphemes(true).count() > 1000 {
            return Err(AppError::bad_request(
                "Description too long (max 1000 characters)",
            ));
        }
    }

    for tag in tags_input {
        if tag.len() > 80 {
            return Err(AppError::bad_request("Tag too long (max 50 characters)"));
        }
        if tag.is_inappropriate() {
            return Err(AppError::bad_request("Tag contains inappropriate content"));
        }
    }
    if tags_input.len() > 10 {
        return Err(AppError::bad_request("Too many tags"));
    }
    if let Some(changelog) = &changelog {
        if changelog.is_inappropriate() {
            return Err(AppError::bad_request(
                "Changelog contains inappropriate content",
            ));
        }
        if changelog.graphemes(true).count() > 1000 {
            return Err(AppError::bad_request(
                "Changelog too long (max 1000 characters)",
            ));
        }
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
    if name.graphemes(true).count() > 100 {
        return Err(AppError::bad_request("Name too long (max 100 characters)"));
    }
    if name.is_inappropriate() {
        return Err(AppError::bad_request("Name contains inappropriate content"));
    }
    if let Some(desc) = description {
        if desc.graphemes(true).count() > 500 {
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
