use axum::{
    extract::Path,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "../../../nomadflowcode/dist"]
struct WebAssets;

/// Serve index.html for the root path.
async fn index() -> impl IntoResponse {
    serve_file("index.html")
}

/// Serve static files by path.
/// Also acts as SPA fallback: unknown routes serve index.html.
async fn static_file(Path(path): Path<String>) -> impl IntoResponse {
    serve_file(&path)
}

fn serve_file(path: &str) -> Response {
    match WebAssets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, mime.as_ref().to_string())],
                content.data.into_owned(),
            )
                .into_response()
        }
        None => {
            // SPA fallback: serve index.html for unknown routes
            if let Some(index) = WebAssets::get("index.html") {
                return (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "text/html".to_string())],
                    index.data.into_owned(),
                )
                    .into_response();
            }
            StatusCode::NOT_FOUND.into_response()
        }
    }
}

/// Build the dashboard router (static files + SPA fallback).
pub fn router() -> Router {
    Router::new()
        .route("/", get(index))
        .route("/{*path}", get(static_file))
}
