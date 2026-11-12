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

pub fn validate_slug(slug: &str) -> Result<(), AppError> {
    if slug.is_empty() {
        return Err(AppError::bad_request("Slug cannot be empty"));
    }
    if slug.len() > SMALL_STRING_MAX_GRAPHEME_LENGTH {
        return Err(AppError::bad_request(format!(
            "Slug too long (max {} characters)",
            SMALL_STRING_MAX_GRAPHEME_LENGTH
        )));
    }
    if slug.starts_with('-') || slug.ends_with('-') || slug.contains("--") {
        return Err(AppError::bad_request(
            "Slug must contain only lowercase alphanumeric characters and single hyphens",
        ));
    }
    let valid_chars = slug
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !valid_chars {
        return Err(AppError::bad_request(
            "Slug must contain only lowercase alphanumeric characters and single hyphens",
        ));
    }
    if slug.is_inappropriate() {
        return Err(AppError::bad_request("Slug contains inappropriate content"));
    }
    Ok(())
}

pub fn trim_and_validate_non_empty(val: &str, field_name: &str) -> Result<String, AppError> {
    let trimmed = val.trim();
    if trimmed.is_empty() {
        return Err(AppError::bad_request(format!(
            "{field_name} cannot be empty or whitespace"
        )));
    }
    Ok(trimmed.to_string())
}

pub fn escape_like_wildcards(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '%' => escaped.push_str(r"\%"),
            '_' => escaped.push_str(r"\_"),
            '\\' => escaped.push_str(r"\\"),
            other => escaped.push(other),
        }
    }
    escaped
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

    validate_slug(slug)?;

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
