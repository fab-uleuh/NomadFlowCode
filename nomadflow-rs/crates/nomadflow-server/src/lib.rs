pub mod auth;
pub mod routes;
pub mod state;

use std::sync::Arc;

use axum::{middleware, Router};
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

use nomadflow_core::config::Settings;
use nomadflow_core::services::tmux::TmuxService;
use nomadflow_core::services::ttyd::TtydService;

use crate::auth::auth_middleware;
use crate::state::AppState;

/// Initialize tracing/logging for the server.
/// Call this before `serve()` when running in server-only mode.
/// Do NOT call when running alongside the TUI (logs would corrupt the terminal).
pub fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "nomadflow_server=info,tower_http=info".into()),
        )
        .init();
}

/// Build the axum router with all routes.
pub fn build_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::permissive();

    // Health endpoint has no auth
    let public = Router::new().merge(routes::health::router());

    // API endpoints require auth
    let api = Router::new()
        .merge(routes::repos::router())
        .merge(routes::features::router())
        .route_layer(middleware::from_fn_with_state(state.clone(), auth_middleware));

    // WebSocket proxy to ttyd (auth via query param, handled in handler)
    let ws = Router::new().merge(routes::terminal::router());

    public
        .merge(api)
        .merge(ws)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Run the HTTP server (with tmux session setup and ttyd startup).
pub async fn serve(settings: Settings) -> color_eyre::Result<()> {
    // 1. Ensure tmux session exists (ttyd needs it)
    let tmux = TmuxService::new(&settings.tmux.session);
    if let Err(e) = tmux.ensure_session().await {
        tracing::warn!("Failed to ensure tmux session: {e}");
    } else {
        info!(session = %settings.tmux.session, "Tmux session ready");
    }

    // 2. Start ttyd subprocess
    let mut ttyd = TtydService::new(&settings);
    match ttyd.start().await {
        Ok(()) => info!(port = settings.ttyd.port, "ttyd started"),
        Err(e) => tracing::warn!("Failed to start ttyd: {e} (terminal proxy will not work)"),
    }

    // 3. Build state and router
    let state = Arc::new(AppState::new(settings.clone()));
    let addr = format!("{}:{}", settings.api.host, settings.api.port);
    let router = build_router(state);

    info!(%addr, "NomadFlow server listening");

    let listener = TcpListener::bind(&addr).await?;
    let result = axum::serve(listener, router).await;

    // Cleanup: stop ttyd on shutdown
    ttyd.stop().await;

    result?;
    Ok(())
}
