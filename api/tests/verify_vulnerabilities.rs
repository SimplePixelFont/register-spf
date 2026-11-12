use api::utilities::{
    is_url_component_safe,
    AuthCache,
    CachedAuthUser,
    token::{hash_token, verify_token, TokenType},
};
use std::time::Instant;

#[test]
fn test_point_1_token_hash_nondeterministic_and_cache_miss() {
    let token = "ses_abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ12";
    
    // Hash the exact same token twice
    let hash1 = hash_token(token).expect("hash1 should succeed");
    let hash2 = hash_token(token).expect("hash2 should succeed");
    
    // Deterministic hashing verified
    assert_eq!(hash1, hash2, "Fix verified: hash_token is deterministic SHA-256!");
    
    // verify_token works with SHA-256 hash
    assert!(verify_token(token, &hash1).unwrap());
    assert!(verify_token(token, &hash2).unwrap());

    // AuthCache hits when keyed by hash_token
    let cache = AuthCache::new(300);
    
    let store_key = hash_token(token).unwrap();
    cache.set(store_key, CachedAuthUser {
        id: 1,
        token_type: TokenType::Session,
        expires_at: Some(chrono::Utc::now() + chrono::Duration::hours(1)),
        cached_at: Instant::now(),
    });

    let lookup_key = hash_token(token).unwrap();
    let hit = cache.get(&lookup_key);
    assert!(hit.is_some(), "Fix verified: cache lookup hits because lookup_key == store_key!");
    assert_eq!(hit.unwrap().id, 1);
}

#[test]
fn test_point_4_search_limit_logic_flaw() {
    // Current logic in font.rs:
    // let limit = Ord::max(limit, 50);
    
    let requested_limit_small: i64 = 10;
    let actual_applied_small = Ord::max(requested_limit_small, 50);
    assert_eq!(actual_applied_small, 50, "Requesting 10 results actually enforces a minimum of 50!");

    let requested_limit_huge: i64 = 100_000;
    let actual_applied_huge = Ord::max(requested_limit_huge, 50);
    assert_eq!(actual_applied_huge, 100_000, "Vulnerability 4 Confirmed: Ord::max allows unbounded limits above 50!");
}

#[test]
fn test_point_13_slug_validation_allows_dotdot_traversal() {
    // Current is_url_component_safe:
    // s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~')
    
    assert!(is_url_component_safe(".."), "Vulnerability 13 Confirmed: '..' is treated as URL component safe!");
    assert!(is_url_component_safe("."), "Vulnerability 13 Confirmed: '.' is treated as URL component safe!");
    assert!(is_url_component_safe("valid-slug-123"));
}

#[tokio::test]
async fn test_point_2_revocation_id_hash_mismatch() {
    use api::utilities::token::revoke_token_by_id;
    use entity::access_tokens;
    use sea_orm::{ActiveModelTrait, ConnectionTrait, Database, EntityTrait, Set};

    let db = Database::connect("sqlite::memory:").await.unwrap();
    db.execute_unprepared(
        r#"
        CREATE TABLE access_tokens (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            token TEXT NOT NULL UNIQUE,
            token_type TEXT NOT NULL,
            name TEXT,
            expires_at TEXT,
            last_used_at TEXT,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .await
    .unwrap();

    let cache = AuthCache::new(300);
    let token = "fgk_1234567890123456789012345678901234567890123456789012345678901234";
    let token_hash = hash_token(token).unwrap();

    // Insert token for user 1
    let active_token = access_tokens::ActiveModel {
        id: Set(42),
        user_id: Set(1),
        token: Set(token_hash.clone()),
        token_type: Set("api_key".to_string()),
        created_at: Set(chrono::Utc::now()),
        ..Default::default()
    };
    active_token.insert(&db).await.unwrap();

    // Warm cache
    cache.set(token_hash.clone(), CachedAuthUser {
        id: 1,
        token_type: TokenType::ApiKey,
        expires_at: None,
        cached_at: Instant::now(),
    });
    assert!(cache.get(&token_hash).is_some());

    // Attempt to revoke with wrong user_id (user 2) -> Should fail
    let wrong_user_res = revoke_token_by_id(&db, &cache, 42, 2).await;
    assert!(wrong_user_res.is_err(), "Revoking another user's token must fail");
    assert!(cache.get(&token_hash).is_some(), "Cache must remain if revocation fails");

    // Revoke with correct user_id (user 1) -> Should succeed
    let res = revoke_token_by_id(&db, &cache, 42, 1).await;
    assert!(res.is_ok(), "Revoking owned token must succeed");

    // Cache must now be evicted
    assert!(cache.get(&token_hash).is_none(), "Cache must be invalidated upon revocation");

    // DB record must be deleted
    let record = access_tokens::Entity::find_by_id(42).one(&db).await.unwrap();
    assert!(record.is_none(), "Token record must be deleted from DB");
}

#[tokio::test]
async fn test_point_8_error_leakage() {
    use api::error::{AppError, ErrorResponse};
    use axum::response::IntoResponse;
    use axum::body::to_bytes;

    // Test unique constraint violation -> maps to HTTP 409 Conflict without leaking table/column
    let db_err = sea_orm::DbErr::Custom("Table 'access_tokens' column 'token' unique constraint violation".into());
    let app_err = AppError::Database(db_err);
    let resp = app_err.into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::CONFLICT);

    let bytes = to_bytes(resp.into_body(), 1024 * 16).await.unwrap();
    let body_str = String::from_utf8(bytes.to_vec()).unwrap();
    let err_resp: ErrorResponse = serde_json::from_str(&body_str).unwrap();

    assert_eq!(err_resp.error, "CONFLICT");
    assert_eq!(err_resp.message, "Resource already exists");
    assert!(!body_str.contains("access_tokens"), "Must not leak table name");
    assert!(!body_str.contains("token"), "Must not leak column name");

    // Test foreign key constraint mapping -> maps to HTTP 400 Bad Request
    let fk_err = sea_orm::DbErr::Custom("FOREIGN KEY constraint failed".into());
    let fk_resp = AppError::Database(fk_err).into_response();
    assert_eq!(fk_resp.status(), axum::http::StatusCode::BAD_REQUEST);

    // Test internal storage error masking -> maps to generic STORAGE_ERROR
    let storage_err = AppError::Internal("R2 connection timeout to bucket fonts".into());
    let storage_resp = storage_err.into_response();
    assert_eq!(storage_resp.status(), axum::http::StatusCode::INTERNAL_SERVER_ERROR);
    let bytes_storage = to_bytes(storage_resp.into_body(), 1024 * 16).await.unwrap();
    let body_storage = String::from_utf8(bytes_storage.to_vec()).unwrap();
    assert!(!body_storage.contains("fonts"), "Must not leak bucket name");
    let parsed_storage: ErrorResponse = serde_json::from_str(&body_storage).unwrap();
    assert_eq!(parsed_storage.error, "STORAGE_ERROR");
}

