pub mod auth;
pub mod display;
pub mod routes;
pub mod state;
pub mod tunnel;

use std::sync::Arc;

use axum::{middleware, Router};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
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
        .merge(routes::terminal::http_proxy_router())
        .route_layer(middleware::from_fn_with_state(state.clone(), auth_middleware));

    // WebSocket proxy to ttyd (auth via query param, handled in handler)
    let ws = Router::new().merge(routes::terminal::ws_router());

    public
        .merge(api)
        .merge(ws)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Spawn a task that listens for Ctrl+C and SIGTERM, then cancels the token.
pub fn spawn_signal_handler(shutdown: CancellationToken) {
    tokio::spawn(async move {
        let ctrl_c = tokio::signal::ctrl_c();
        #[cfg(unix)]
        {
            let mut sigterm =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("failed to register SIGTERM handler");
            tokio::select! {
                _ = ctrl_c => info!("Received Ctrl+C, shutting down…"),
                _ = sigterm.recv() => info!("Received SIGTERM, shutting down…"),
            }
        }
        #[cfg(not(unix))]
        {
            ctrl_c.await.ok();
            info!("Received Ctrl+C, shutting down…");
        }
        shutdown.cancel();
    });
}

/// Run the HTTP server (with tmux session setup and ttyd startup).
/// The server shuts down gracefully when `shutdown` is cancelled.
/// When `public` is true, a bore tunnel is started and the server is exposed via the relay.
pub async fn serve(
    mut settings: Settings,
    shutdown: CancellationToken,
    public: bool,
) -> color_eyre::Result<()> {
    // 0. Auto-generate a secret if --public and none configured
    if public && settings.auth.secret.is_empty() {
        use rand::Rng;
        let secret: String = rand::rng()
            .sample_iter(rand::distr::Alphanumeric)
            .take(32)
            .map(|b| b as char)
            .collect();
        tracing::warn!("No auth secret configured — generated a temporary one for this session");
        settings.auth.secret = secret;
    }

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
    let router = build_router(state.clone());

    let listener = TcpListener::bind(&addr).await?;
    info!(%addr, "NomadFlow server listening");

    // 4. Start tunnel if --public
    let connect_url = if public {
        match tunnel::start_tunnel(
            settings.api.port,
            &settings.tunnel,
            shutdown.clone(),
            &state.http_client,
        )
        .await
        {
            Ok(info) => info.public_url,
            Err(e) => {
                tracing::warn!("Tunnel failed: {e}");
                let local_ip = local_ip_address::local_ip()
                    .map(|ip| ip.to_string())
                    .unwrap_or_else(|_| "127.0.0.1".to_string());
                format!("http://{local_ip}:{}", settings.api.port)
            }
        }
    } else {
        let local_ip = local_ip_address::local_ip()
            .map(|ip| ip.to_string())
            .unwrap_or_else(|_| "127.0.0.1".to_string());
        format!("http://{local_ip}:{}", settings.api.port)
    };

    // 5. Display connection info with QR code
    display::print_connection_info(&connect_url, &settings.auth.secret, public);

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown.cancelled_owned())
        .await?;

    // Cleanup: stop ttyd after graceful shutdown
    info!("Stopping ttyd…");
    ttyd.stop().await;
    info!("Server stopped");

    Ok(())
}
