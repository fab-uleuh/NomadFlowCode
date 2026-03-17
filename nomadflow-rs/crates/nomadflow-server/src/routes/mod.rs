pub mod dashboard;
pub mod features;
pub mod file_tree;
pub mod git_diff;
pub mod health;
pub mod panes;
pub mod repos;
pub mod sessions;

use axum::{http::StatusCode, Json};
use serde_json::{json, Value};

pub(crate) fn map_error(e: nomadflow_core::error::NomadError) -> (StatusCode, Json<Value>) {
    match &e {
        nomadflow_core::error::NomadError::NotFound(_) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "detail": e.to_string() })),
        ),
        nomadflow_core::error::NomadError::InvalidInput(_) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "detail": e.to_string() })),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "detail": e.to_string() })),
        ),
    }
}

pub(crate) fn map_join_error(e: tokio::task::JoinError) -> (StatusCode, Json<Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "detail": e.to_string() })),
    )
}
