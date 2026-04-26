use crate::{
    routes::login,
    utilities::tasks::run_cleanup_tasks,
    utilities::{
        AuthCache, RateLimiter, comment_limit_middleware, font_upload_limit_middleware,
        rate_limit_middleware,
    },
};
use axum::{
    Router,
    extract::DefaultBodyLimit,
    middleware,
    routing::{delete, get, post},
};
use routes::{
    comment::{create_comment, delete_comment, get_comments},
    favorite::add_favorite,
    font::{create_font, delete_font, get_font_with_details, search_fonts},
    user::{create_user_api_key, get_my_tokens, register, revoke_my_token},
    version::create_version,
};
use sea_orm::{Database, DatabaseConnection};
use std::{env, net::SocketAddr, sync::Arc};
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;

const PUBLIC_API_KEY: &str = "sk_public_abc123...";
mod error;
mod model;
mod routes;
mod utilities;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub trusted_domains: Vec<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            trusted_domains: vec![/*"localhost:3000".to_string(), "127.0.0.1:3000".to_string()*/],
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub rate_limiter: Arc<RateLimiter>,
    pub auth_cache: Arc<AuthCache>,
    pub config: Arc<AppConfig>,
}

fn auth_routes() -> Router<AppState> {
    Router::new()
        .route("/login", post(login))
        .route("/register", post(register))
        .route("/tokens", get(get_my_tokens))
        .route("/tokens", post(create_user_api_key))
        .route("/tokens/{id}", delete(revoke_my_token))
}

fn font_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/{id}/versions", post(create_version))
        .route("/", post(create_font))
        .layer(DefaultBodyLimit::max(32_000_000))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            font_upload_limit_middleware,
        ))
        .route("/search", get(search_fonts))
        .route("/{slug}", get(get_font_with_details))
        .route("/{slug}", delete(delete_font))
        .route("/{id}/favorite", post(add_favorite))
}

fn comment_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/{id}", post(create_comment))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            comment_limit_middleware,
        ))
        .route("/{id}", get(get_comments))
        .route("/{id}", delete(delete_comment))
}

fn api_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .nest("/auth", auth_routes())
        .nest("/fonts", font_routes(state.clone()))
        .nest("/comments", comment_routes(state.clone()))
}

#[tokio::main]
async fn start() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv()?;

    let config = Arc::new(AppConfig::default());
    let db = Database::connect(env::var("DATABASE_URL")?).await?;
    let rate_limiter = Arc::new(RateLimiter::new(config.trusted_domains.clone()));

    let state = AppState {
        db,
        rate_limiter,
        config,
        auth_cache: Arc::new(AuthCache::new(300)),
    };

    tokio::spawn(run_cleanup_tasks(state.clone()));

    let router = api_routes(state.clone())
        .layer(ServiceBuilder::new().layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit_middleware,
        )))
        .with_state(state);

    let app = Router::new()
        .nest("/api", router)
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}

pub fn main() {
    let result = start();

    if let Some(err) = result.err() {
        println!("Error: {err}");
    }
}
