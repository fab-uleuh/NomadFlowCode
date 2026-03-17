pub mod auth;
pub mod display;
pub mod frame_handler;
pub mod routes;
pub mod socket;
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
        .merge(routes::sessions::router())
        .merge(routes::git_diff::router())
        .merge(routes::file_tree::router())
        .merge(routes::panes::rest_router())
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // WebSocket handler (auth via query param, handled in handler)
    let ws = Router::new().merge(routes::panes::ws_router());

    let router = public.merge(api).merge(ws);

    router
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

/// Build the connect URL from a host override or local IP detection.
/// - IP address → `http://{ip}:{port}`
/// - Domain name → `https://{domain}` (sans port, on suppose reverse proxy + TLS)
/// - None → détecte l'IP locale → `http://{ip}:{port}`
fn build_connect_url(host_override: &Option<String>, port: u16) -> String {
    match host_override {
        Some(h) => {
            if h.parse::<std::net::IpAddr>().is_ok() {
                format!("http://{}:{}", h, port)
            } else {
                format!("https://{}", h)
            }
        }
        None => {
            let local_ip = local_ip_address::local_ip()
                .map(|ip| ip.to_string())
                .unwrap_or_else(|_| "127.0.0.1".to_string());
            format!("http://{}:{}", local_ip, port)
        }
    }
}

/// Run the HTTP server (with PTY pane manager and Unix socket listener for CLI attach).
/// The server shuts down gracefully when `shutdown` is cancelled.
/// When `public` is true, a bore tunnel is started and the server is exposed via the relay.
/// When `quiet` is true, connection info (QR code) is not printed (used when running alongside TUI).
pub async fn serve(
    mut settings: Settings,
    shutdown: CancellationToken,
    public: bool,
    quiet: bool,
    host_override: Option<String>,
) -> color_eyre::Result<()> {
    // 0. Auth secret handling
    if public && settings.auth.secret.is_empty() {
        return Err(color_eyre::eyre::eyre!(
            "Refusing to start in --public mode without an explicit auth secret.\n\
             Set one with: --auth-secret <SECRET> or NOMADFLOW_AUTH_SECRET=<SECRET>"
        ));
    }

    if settings.auth.secret.is_empty() {
        use rand::Rng;
        let secret: String = rand::rng()
            .sample_iter(rand::distr::Alphanumeric)
            .take(32)
            .map(|b| b as char)
            .collect();
        settings.auth.secret = secret.clone();
        // Print to stdout (not tracing) so it's easy to copy
        eprintln!("Auth secret (auto-generated): {secret}");
    }

    // 1. Build state and router
    let state = Arc::new(AppState::new(settings.clone()));

    // Install/update hook scripts before accepting connections (AC #1)
    if let Err(e) = state.agent_state.ensure_hook_scripts().await {
        tracing::warn!("Failed to install hook scripts: {e}");
    }

    // Purge stale agent state files (older than 24 hours)
    state
        .agent_state
        .purge_stale_state_files(std::time::Duration::from_secs(24 * 3600))
        .await;

    // Auto-inject hooks into all tracked repos so Claude Code state tracking
    // works without requiring a manual create-session first.
    match state.git.list_repos().await {
        Ok(repos) => {
            for repo in &repos {
                let repo_path = std::path::Path::new(&repo.path);
                // Resolve symlinks (repos in ~/.nomadflowcode/repos/ are symlinks)
                let real_path = repo_path.canonicalize().unwrap_or(repo_path.to_path_buf());
                if let Err(e) = state.agent_state.inject_hooks(&real_path).await {
                    tracing::warn!(repo = %repo.name, "Failed to inject hooks: {e}");
                }
            }
            if !repos.is_empty() {
                info!(count = repos.len(), "Hooks injected into tracked repos");
            }
        }
        Err(e) => tracing::warn!("Failed to list repos for hook injection: {e}"),
    }

    // Start agent state watcher (AC #5)
    spawn_agent_state_watcher(state.clone(), shutdown.clone());

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
                build_connect_url(&host_override, settings.api.port)
            }
        }
    } else {
        build_connect_url(&host_override, settings.api.port)
    };

    // 5. Display connection info with QR code (only in foreground serve mode)
    if !quiet {
        display::print_connection_info(&connect_url, &settings.auth.secret, public);
    }

    // 6. Start Unix socket listener alongside TCP (for `nomadflow attach`)
    let socket_path = settings.socket_path();
    let socket_state = state.clone();
    let socket_shutdown = shutdown.clone();
    tokio::spawn(async move {
        if let Err(e) =
            socket::serve_unix_socket(&socket_path, socket_state, socket_shutdown).await
        {
            tracing::error!("Unix socket listener failed: {e}");
        }
    });

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown.cancelled_owned())
        .await?;

    info!("Server stopped");

    Ok(())
}

/// Run a lightweight HTTP server that serves the web dashboard (static files).
/// This is a separate server from the API — no auth.
pub async fn serve_web(settings: &Settings, shutdown: CancellationToken) -> color_eyre::Result<()> {
    let port = settings.web.port;
    let cors = CorsLayer::permissive();

    let router = routes::dashboard::router()
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    let addr = format!("0.0.0.0:{port}");
    let listener = TcpListener::bind(&addr).await?;
    info!(%addr, "Web dashboard listening");

    display::open_browser(&format!("http://localhost:{port}"));

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown.cancelled_owned())
        .await?;

    info!("Web dashboard stopped");
    Ok(())
}

/// Periodically poll for agent state changes across all active panes.
fn spawn_agent_state_watcher(state: Arc<AppState>, shutdown: CancellationToken) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(2000));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let panes = {
                        let manager = state.pane_manager.lock().await;
                        manager.list_panes()
                    };

                    for pane in panes {
                        // Match pane to state file via CWD
                        match state.agent_state.get_state_by_cwd(&pane.cwd).await {
                            Ok(Some(current_state)) => {
                                // Map nomadflow_core AgentStateKind to nomadflow_pty AgentStateKind
                                let kind = match current_state.state {
                                    nomadflow_core::models::AgentStateKind::WaitingForInput => nomadflow_pty::types::AgentStateKind::WaitingForInput,
                                    nomadflow_core::models::AgentStateKind::WaitingForPermission => nomadflow_pty::types::AgentStateKind::WaitingForPermission,
                                    nomadflow_core::models::AgentStateKind::Generating => nomadflow_pty::types::AgentStateKind::Generating,
                                    nomadflow_core::models::AgentStateKind::Idle => nomadflow_pty::types::AgentStateKind::Idle,
                                    nomadflow_core::models::AgentStateKind::Done => nomadflow_pty::types::AgentStateKind::Done,
                                    nomadflow_core::models::AgentStateKind::Error => nomadflow_pty::types::AgentStateKind::Error,
                                    nomadflow_core::models::AgentStateKind::Unknown => nomadflow_pty::types::AgentStateKind::Unknown,
                                };

                                if kind != pane.agent_state {
                                    // Update PaneManager
                                    let mut manager = state.pane_manager.lock().await;
                                    let _ = manager.update_pane_state(pane.id, kind);
                                    // Notify subscribers
                                    let _ = state.agent_state_broadcast.send((pane.id.0, kind));
                                }
                            }
                            Ok(None) => {} // No state file for this CWD
                            Err(e) => {
                                tracing::debug!(cwd = %pane.cwd, "Failed to read agent state: {e}");
                            }
                        }
                    }
                }
                _ = shutdown.cancelled() => break,
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_connect_url_with_ipv4() {
        let host = Some("192.168.1.42".to_string());
        assert_eq!(build_connect_url(&host, 8080), "http://192.168.1.42:8080");
    }

    #[test]
    fn test_build_connect_url_with_ipv6() {
        let host = Some("::1".to_string());
        assert_eq!(build_connect_url(&host, 3000), "http://::1:3000");
    }

    #[test]
    fn test_build_connect_url_with_domain() {
        let host = Some("myserver.example.com".to_string());
        assert_eq!(
            build_connect_url(&host, 8080),
            "https://myserver.example.com"
        );
    }

    #[test]
    fn test_build_connect_url_with_subdomain() {
        let host = Some("dev.internal.company.io".to_string());
        assert_eq!(
            build_connect_url(&host, 9090),
            "https://dev.internal.company.io"
        );
    }

    #[test]
    fn test_build_connect_url_none_falls_back_to_local_ip() {
        let url = build_connect_url(&None, 8080);
        assert!(url.starts_with("http://"));
        assert!(url.ends_with(":8080"));
    }

    #[test]
    fn test_build_connect_url_domain_ignores_port() {
        let host = Some("example.com".to_string());
        let url = build_connect_url(&host, 9999);
        assert!(!url.contains("9999"));
        assert_eq!(url, "https://example.com");
    }

    #[test]
    fn test_build_connect_url_localhost_ip() {
        let host = Some("127.0.0.1".to_string());
        assert_eq!(build_connect_url(&host, 4000), "http://127.0.0.1:4000");
    }
}
