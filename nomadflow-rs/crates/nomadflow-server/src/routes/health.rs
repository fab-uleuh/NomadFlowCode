use std::sync::Arc;

use axum::{extract::State, routing::get, Json, Router};

use nomadflow_core::models::HealthResponse;

use crate::state::AppState;

async fn health(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        tmux_session: state.settings.tmux.session.clone(),
        api_port: state.settings.api.port,
    })
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/health", get(health))
}
