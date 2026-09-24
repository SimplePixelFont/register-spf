use crate::error::AppError;
use rustrict::CensorStr;
use unicode_segmentation::UnicodeSegmentation;

const SMALL_STRING_MAX_GRAPHEME_LENGTH: usize = 80;
const SMALL_STRING_MAX_BYTE_LENGTH: usize = 640;

const MEDIUM_STRING_MAX_GRAPHEME_LENGTH: usize = 200;
const MEDIUM_STRING_MAX_BYTE_LENGTH: usize = 1600;

const LARGE_STRING_MAX_GRAPHEME_LENGTH: usize = 500;
const LARGE_STRING_MAX_BYTE_LENGTH: usize = 4000;

pub fn is_url_component_safe(s: &str) -> bool {
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~')
}

pub fn validate_comment(text: &str) -> Result<(), AppError> {
    if text.graphemes(true).count() > MEDIUM_STRING_MAX_GRAPHEME_LENGTH {
        return Err(AppError::bad_request(format!(
            "Comment too long (max {} characters)",
            MEDIUM_STRING_MAX_GRAPHEME_LENGTH
        )));
    }
    if text.len() > MEDIUM_STRING_MAX_BYTE_LENGTH {
        return Err(AppError::bad_request(format!(
            "Comment exceeded byte limit (max {} bytes)",
            MEDIUM_STRING_MAX_BYTE_LENGTH
        )));
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
    if name.graphemes(true).count() > SMALL_STRING_MAX_GRAPHEME_LENGTH {
        return Err(AppError::bad_request(format!(
            "Name too long (max {} characters)",
            SMALL_STRING_MAX_GRAPHEME_LENGTH
        )));
    }
    if name.len() > SMALL_STRING_MAX_BYTE_LENGTH {
        return Err(AppError::bad_request(format!(
            "Name exceeded byte limit (max {} bytes)",
            SMALL_STRING_MAX_BYTE_LENGTH
        )));
    }

    if slug.is_empty() {
        return Err(AppError::bad_request("Slug is required"));
    }
    if slug.len() > SMALL_STRING_MAX_GRAPHEME_LENGTH {
        return Err(AppError::bad_request(format!(
            "Slug too long (max {} characters)",
            SMALL_STRING_MAX_GRAPHEME_LENGTH
        )));
    }
    if slug.is_inappropriate() {
        return Err(AppError::bad_request("Slug contains inappropriate content"));
    }
    if !is_url_component_safe(slug) {
        return Err(AppError::bad_request("Slug contains invalid characters"));
    }

    if let Some(desc) = &description {
        if desc.is_inappropriate() {
            return Err(AppError::bad_request(
                "Description contains inappropriate content",
            ));
        }
        if desc.graphemes(true).count() > LARGE_STRING_MAX_GRAPHEME_LENGTH {
            return Err(AppError::bad_request(format!(
                "Description too long (max {} characters)",
                LARGE_STRING_MAX_GRAPHEME_LENGTH
            )));
        }
        if desc.len() > LARGE_STRING_MAX_BYTE_LENGTH {
            return Err(AppError::bad_request(format!(
                "Description exceeded byte limit (max {} bytes)",
                LARGE_STRING_MAX_BYTE_LENGTH
            )));
        }
    }

    for tag in tags_input {
        if tag.len() > SMALL_STRING_MAX_GRAPHEME_LENGTH {
            return Err(AppError::bad_request(format!(
                "Tag too long (max {} characters)",
                SMALL_STRING_MAX_GRAPHEME_LENGTH
            )));
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
        if changelog.graphemes(true).count() > LARGE_STRING_MAX_GRAPHEME_LENGTH {
            return Err(AppError::bad_request(format!(
                "Changelog too long (max {} characters)",
                LARGE_STRING_MAX_GRAPHEME_LENGTH
            )));
        }
        if changelog.len() > LARGE_STRING_MAX_BYTE_LENGTH {
            return Err(AppError::bad_request(format!(
                "Changelog exceeded byte limit (max {} bytes)",
                LARGE_STRING_MAX_BYTE_LENGTH
            )));
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
    if name.graphemes(true).count() > SMALL_STRING_MAX_GRAPHEME_LENGTH {
        return Err(AppError::bad_request(format!(
            "Name too long (max {} characters)",
            SMALL_STRING_MAX_GRAPHEME_LENGTH
        )));
    }
    if name.len() > SMALL_STRING_MAX_BYTE_LENGTH {
        return Err(AppError::bad_request(format!(
            "Name exceeded byte limit (max {} bytes)",
            SMALL_STRING_MAX_BYTE_LENGTH
        )));
    }
    if name.is_inappropriate() {
        return Err(AppError::bad_request("Name contains inappropriate content"));
    }
    if let Some(desc) = description {
        if desc.graphemes(true).count() > LARGE_STRING_MAX_GRAPHEME_LENGTH {
            return Err(AppError::bad_request(format!(
                "Description too long (max {} characters)",
                LARGE_STRING_MAX_GRAPHEME_LENGTH
            )));
        }
        if desc.is_inappropriate() {
            return Err(AppError::bad_request(
                "Description contains inappropriate content",
            ));
        }
        if desc.len() > LARGE_STRING_MAX_BYTE_LENGTH {
            return Err(AppError::bad_request(format!(
                "Description exceeded byte limit (max {} bytes)",
                LARGE_STRING_MAX_BYTE_LENGTH
            )));
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
