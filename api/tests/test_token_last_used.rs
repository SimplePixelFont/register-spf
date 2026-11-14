use api::utilities::auth::{AuthCache, CachedAuthUser};
use api::utilities::token::{hash_token, validate_token_cached, TokenType};
use entity::access_tokens;
use sea_orm::{ActiveModelTrait, ConnectionTrait, Database, EntityTrait, Set};
use std::sync::Arc;
use std::sync::atomic::AtomicI64;
use std::time::Instant;

async fn setup_test_db() -> sea_orm::DatabaseConnection {
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
    db
}

#[tokio::test]
async fn test_initial_use_updates_last_used_at_in_background() {
    let db = setup_test_db().await;
    let cache = AuthCache::new(300);

    let raw_token = "ses_abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ12";
    let token_hash = hash_token(raw_token);

    // Insert token record into database with last_used_at = None
    let new_token = access_tokens::ActiveModel {
        id: Set(1),
        user_id: Set(100),
        token: Set(token_hash.clone()),
        token_type: Set("session".to_string()),
        name: Set(None),
        expires_at: Set(None),
        last_used_at: Set(None),
        created_at: Set(chrono::Utc::now()),
    };
    new_token.insert(&db).await.unwrap();

    // Verify initially None
    let record_before = access_tokens::Entity::find_by_id(1).one(&db).await.unwrap().unwrap();
    assert!(record_before.last_used_at.is_none());

    // Validate token (cache miss -> loads from DB -> spawns background update)
    let (user, token_type) = validate_token_cached(&db, &cache, raw_token).await.unwrap();
    assert_eq!(user.id, 100);
    assert_eq!(token_type, TokenType::Session);

    // Yield to allow background tokio task to run
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Verify last_used_at was updated in the database
    let record_after = access_tokens::Entity::find_by_id(1).one(&db).await.unwrap().unwrap();
    assert!(
        record_after.last_used_at.is_some(),
        "Initial token validation must record last_used_at in the database"
    );
    let initial_timestamp = record_after.last_used_at.unwrap();

    // Immediate second validation (cache hit -> throttled)
    let (user2, token_type2) = validate_token_cached(&db, &cache, raw_token).await.unwrap();
    assert_eq!(user2.id, 100);
    assert_eq!(token_type2, TokenType::Session);

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Verify last_used_at was NOT modified again
    let record_after_second = access_tokens::Entity::find_by_id(1).one(&db).await.unwrap().unwrap();
    assert_eq!(
        record_after_second.last_used_at.unwrap(),
        initial_timestamp,
        "Subsequent validation within throttle window must NOT perform database write"
    );
}

#[tokio::test]
async fn test_cache_miss_with_recent_db_timestamp_does_not_reupdate() {
    let db = setup_test_db().await;
    let cache = AuthCache::new(300);

    let raw_token = "fgk_1111222233334444555566667777888899990000aaaabbbbccccddddeeeeffff";
    let token_hash = hash_token(raw_token);

    // Token was already used 10 seconds ago in DB
    let recent_time = chrono::Utc::now() - chrono::Duration::seconds(10);
    let new_token = access_tokens::ActiveModel {
        id: Set(2),
        user_id: Set(200),
        token: Set(token_hash.clone()),
        token_type: Set("api_key".to_string()),
        name: Set(Some("CLI Key".to_string())),
        expires_at: Set(None),
        last_used_at: Set(Some(recent_time)),
        created_at: Set(chrono::Utc::now()),
    };
    new_token.insert(&db).await.unwrap();

    // Cache miss occurs (e.g., server restart)
    let (user, token_type) = validate_token_cached(&db, &cache, raw_token).await.unwrap();
    assert_eq!(user.id, 200);
    assert_eq!(token_type, TokenType::ApiKey);

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Database timestamp should not have been updated because 10s < 300s
    let record = access_tokens::Entity::find_by_id(2).one(&db).await.unwrap().unwrap();
    assert_eq!(
        record.last_used_at.unwrap().timestamp(),
        recent_time.timestamp(),
        "Cache miss on token used recently must respect throttle and skip DB write"
    );
}

#[tokio::test]
async fn test_concurrent_requests_only_trigger_single_throttle_update() {
    let _db = setup_test_db().await;
    let cache = AuthCache::new(300);

    let raw_token = "ses_9999888877776666555544443333222211110000zzzyyyxxxwwwvvvuuutttsss";
    let token_hash = hash_token(raw_token);

    // Warm cache with a stale timestamp (simulating throttle window elapsed)
    let stale_ts = (chrono::Utc::now() - chrono::Duration::seconds(600)).timestamp();
    cache.set(
        token_hash.clone(),
        CachedAuthUser {
            token_id: 42,
            id: 300,
            token_type: TokenType::Session,
            expires_at: None,
            cached_at: Instant::now(),
            last_synced_timestamp: Arc::new(AtomicI64::new(stale_ts)),
        },
    );

    // Fire 50 concurrent attempts to record token use with a 300s throttle window
    let cache = Arc::new(cache);
    let mut handles = Vec::new();
    for _ in 0..50 {
        let cache_ref = cache.clone();
        let hash = token_hash.clone();
        handles.push(tokio::spawn(async move {
            cache_ref.try_record_token_use(&hash, 300)
        }));
    }

    let mut update_count = 0;
    for handle in handles {
        if handle.await.unwrap().is_some() {
            update_count += 1;
        }
    }
    assert_eq!(
        update_count, 1,
        "Exactly one concurrent request must win the compare_exchange throttle update"
    );
}
