use std::time::Duration;
use chrono::Utc;
use entity::access_tokens;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use crate::AppState;

pub async fn run_cleanup_tasks(state: AppState) {
    let mut interval = tokio::time::interval(Duration::from_secs(300));

    loop {
        interval.tick().await;
        state.auth_cache.cleanup();

        let _ = access_tokens::Entity::delete_many()
            .filter(access_tokens::Column::ExpiresAt.lt(Utc::now()))
            .exec(&state.db)
            .await;

        state.rate_limiter.cleanup();
    }
}
