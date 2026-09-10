mod error;
mod handlers;
mod metadata;

use axum::extract::DefaultBodyLimit;
use axum::response::Html;
use axum::routing::{delete, get, post};
use axum::Router;
use handlers::AppState;
use std::env;
use std::fs;
use std::path::PathBuf;
use tower_http::cors::{Any, CorsLayer};

const INDEX_HTML: &str = include_str!("../static/index.html");

#[tokio::main]
async fn main() {
    let video_dir = env::var("VIDEO_DIR")
        .unwrap_or_else(|_| "./videos".to_string());
    let port = env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string());

    let video_path = PathBuf::from(&video_dir);
    fs::create_dir_all(&video_path).expect("Failed to create video directory");

    let state = AppState {
        video_dir: video_path,
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let body_limit = DefaultBodyLimit::max(4 * 1024 * 1024 * 1024); // 4 GB

    let app = Router::new()
        .route("/", get(|| async { Html(INDEX_HTML) }))
        .route("/api/videos", get(handlers::list_videos))
        .route("/api/upload", post(handlers::upload_video))
        .route("/api/videos/{id}/stream", get(handlers::stream_video))
        .route("/api/videos/{id}", delete(handlers::delete_video))
        .layer(cors)
        .layer(body_limit)
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    println!("Video Manager running at http://localhost:{port}");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
