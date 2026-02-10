use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{
    body::Body,
    extract::{
        connect_info::ConnectInfo,
        ws::{WebSocket, WebSocketUpgrade},
        FromRequest, Query, State,
    },
    http::{header, uri::Uri, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use dashmap::DashMap;
use hyper_util::{client::legacy::Client as HyperClient, rt::TokioExecutor};
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tower_http::trace::TraceLayer;
use tracing::{error, info, warn};

/// Mapping entry: subdomain → bore port + last usage time
struct TunnelEntry {
    bore_port: u16,
    last_used: Instant,
    client_ip: IpAddr,
}

struct RelayState {
    /// subdomain → TunnelEntry
    tunnels: DashMap<String, TunnelEntry>,
    /// IP → list of registration timestamps (for rate limiting)
    rate_limits: DashMap<IpAddr, Vec<Instant>>,
    /// Shared secret for relay registration
    relay_secret: String,
    /// Host where bore tunnel ports are accessible (default: 127.0.0.1, in Docker: host.docker.internal)
    bore_host: String,
    /// HTTP client for proxying to bore tunnels
    http_client: HyperClient<hyper_util::client::legacy::connect::HttpConnector, Body>,
}

// ── Registration API ──────────────────────────────────────────────────

#[derive(Deserialize)]
struct RegisterRequest {
    port: u16,
    secret: String,
    subdomain: Option<String>,
}

#[derive(Serialize)]
struct RegisterResponse {
    subdomain: String,
}

/// Maximum number of active tunnels per IP address.
const MAX_TUNNELS_PER_IP: usize = 3;
/// Maximum number of tunnel registrations per IP per hour.
const MAX_REGISTRATIONS_PER_HOUR: usize = 10;

/// Extract client IP from X-Forwarded-For (set by Caddy) with fallback to ConnectInfo.
fn extract_client_ip(headers: &axum::http::HeaderMap, connect_info: &ConnectInfo<SocketAddr>) -> IpAddr {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .and_then(|s| s.trim().parse::<IpAddr>().ok())
        .unwrap_or_else(|| connect_info.0.ip())
}

async fn register(
    State(state): State<Arc<RelayState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: axum::http::HeaderMap,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, StatusCode> {
    let client_ip = extract_client_ip(&headers, &ConnectInfo(addr));

    // Reject privileged ports
    if req.port < 1024 {
        warn!(port = req.port, %client_ip, "Registration rejected: port below 1024");
        return Err(StatusCode::BAD_REQUEST);
    }

    // Verify secret (constant-time comparison)
    if !state.relay_secret.is_empty() {
        let matches: bool = req
            .secret
            .as_bytes()
            .ct_eq(state.relay_secret.as_bytes())
            .into();
        if !matches {
            warn!(%client_ip, "Registration rejected: invalid secret");
            return Err(StatusCode::UNAUTHORIZED);
        }
    }

    // Rate limit: count active tunnels for this IP
    let active_count = state
        .tunnels
        .iter()
        .filter(|entry| entry.value().client_ip == client_ip)
        .count();
    if active_count >= MAX_TUNNELS_PER_IP {
        warn!(%client_ip, active_count, "Registration rejected: too many active tunnels");
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    // Rate limit: count recent registrations for this IP (last hour)
    let one_hour_ago = Instant::now() - Duration::from_secs(3600);
    let recent_count = {
        let mut entry = state.rate_limits.entry(client_ip).or_default();
        entry.retain(|ts| *ts > one_hour_ago);
        entry.len()
    };
    if recent_count >= MAX_REGISTRATIONS_PER_HOUR {
        warn!(%client_ip, recent_count, "Registration rejected: too many registrations per hour");
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    // Record this registration
    state
        .rate_limits
        .entry(client_ip)
        .or_default()
        .push(Instant::now());

    // Resolve subdomain: use preferred if provided, otherwise generate random
    let subdomain = if let Some(preferred) = req.subdomain {
        // Validate format: alphanumeric + hyphens, 3-32 chars, no leading/trailing hyphens
        if preferred.len() < 3
            || preferred.len() > 32
            || !preferred
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
            || preferred.starts_with('-')
            || preferred.ends_with('-')
        {
            warn!(%client_ip, subdomain = %preferred, "Registration rejected: invalid subdomain format");
            return Err(StatusCode::BAD_REQUEST);
        }

        // Check if already taken
        if let Some(existing) = state.tunnels.get(&preferred) {
            if existing.client_ip != client_ip {
                warn!(%client_ip, subdomain = %preferred, "Registration rejected: subdomain taken by another IP");
                return Err(StatusCode::CONFLICT);
            }
            // Same IP → re-register (update bore_port)
            drop(existing);
        }

        preferred
    } else {
        generate_subdomain()
    };

    state.tunnels.insert(
        subdomain.clone(),
        TunnelEntry {
            bore_port: req.port,
            last_used: Instant::now(),
            client_ip,
        },
    );

    info!(subdomain = %subdomain, port = req.port, %client_ip, "Tunnel registered");

    Ok(Json(RegisterResponse { subdomain }))
}

fn generate_subdomain() -> String {
    use rand::Rng;
    let mut rng = rand::rng();
    let chars: Vec<char> = (0..6)
        .map(|_| {
            let idx = rng.random_range(0..36u32);
            if idx < 10 {
                (b'0' + idx as u8) as char
            } else {
                (b'a' + (idx - 10) as u8) as char
            }
        })
        .collect();
    chars.into_iter().collect()
}

// ── Check endpoint (for Caddy on_demand TLS) ─────────────────────────

#[derive(Deserialize)]
struct CheckQuery {
    domain: String,
}

async fn check(
    State(state): State<Arc<RelayState>>,
    Query(query): Query<CheckQuery>,
) -> StatusCode {
    // domain looks like "abc123.tunnel.nomadflowcode.dev"
    let subdomain = query
        .domain
        .split('.')
        .next()
        .unwrap_or_default()
        .to_string();

    if state.tunnels.contains_key(&subdomain) {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

// ── Health check ──────────────────────────────────────────────────────

async fn health() -> &'static str {
    "ok"
}

// ── Reverse proxy (subdomain → bore port) ─────────────────────────────

/// Resolve subdomain from Host header → bore port. Updates `last_used` on each access.
fn resolve_tunnel(state: &RelayState, req: &Request<Body>) -> Result<(String, u16), StatusCode> {
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let subdomain = host.split('.').next().unwrap_or_default().to_string();

    let mut entry = state.tunnels.get_mut(&subdomain).ok_or_else(|| {
        warn!(subdomain = %subdomain, "Unknown tunnel subdomain");
        StatusCode::NOT_FOUND
    })?;

    entry.last_used = Instant::now();
    let bore_port = entry.bore_port;
    drop(entry);
    Ok((subdomain, bore_port))
}

/// Check if the request is a WebSocket upgrade.
fn is_ws_upgrade(req: &Request<Body>) -> bool {
    req.headers()
        .get(header::UPGRADE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false)
}

async fn proxy_handler(
    State(state): State<Arc<RelayState>>,
    req: Request<Body>,
) -> Result<Response, StatusCode> {
    let (subdomain, bore_port) = resolve_tunnel(&state, &req)?;

    // WebSocket upgrade: proxy bidirectionally
    if is_ws_upgrade(&req) {
        let bore_host = state.bore_host.clone();
        let path_and_query = req
            .uri()
            .path_and_query()
            .map(|pq| pq.as_str().to_string())
            .unwrap_or_else(|| req.uri().path().to_string());

        // Forward any subprotocols from the client
        let protocols: Vec<String> = req
            .headers()
            .get_all(header::SEC_WEBSOCKET_PROTOCOL)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .flat_map(|v| v.split(',').map(|s| s.trim().to_string()))
            .collect();

        let protocols_for_closure = protocols.clone();
        let ws = WebSocketUpgrade::from_request(req, &state)
            .await
            .map_err(|_| StatusCode::BAD_REQUEST)?;

        let mut upgrade = ws;
        for proto in protocols.iter().cloned() {
            upgrade = upgrade.protocols([proto]);
        }

        return Ok(upgrade
            .on_upgrade(move |client_ws| {
                handle_ws_proxy(
                    client_ws,
                    bore_host,
                    bore_port,
                    path_and_query,
                    protocols_for_closure,
                )
            })
            .into_response());
    }

    // Regular HTTP proxy
    let path = req.uri().path();
    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or(path);

    let bore_host = &state.bore_host;
    let target_uri: Uri = format!("http://{bore_host}:{bore_port}{path_and_query}")
        .parse()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let (mut parts, body) = req.into_parts();
    parts.uri = target_uri;

    // Strip hop-by-hop headers
    for name in &[
        header::HOST,
        header::CONNECTION,
        header::PROXY_AUTHENTICATE,
        header::PROXY_AUTHORIZATION,
        header::TE,
        header::TRAILER,
        header::TRANSFER_ENCODING,
    ] {
        parts.headers.remove(name);
    }
    parts.headers.remove("Keep-Alive");

    let proxy_req = Request::from_parts(parts, body);

    let resp = state.http_client.request(proxy_req).await.map_err(|e| {
        error!(subdomain = %subdomain, port = bore_port, "Proxy error: {e}");
        StatusCode::BAD_GATEWAY
    })?;

    Ok(resp.into_response())
}

async fn handle_ws_proxy(
    client_ws: WebSocket,
    bore_host: String,
    bore_port: u16,
    path_and_query: String,
    protocols: Vec<String>,
) {
    let ws_url = format!("ws://{bore_host}:{bore_port}{path_and_query}");

    // Use IntoClientRequest to auto-generate sec-websocket-key and other WS headers
    let mut request = match ws_url.into_client_request() {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to build WS request: {e}");
            return;
        }
    };

    if !protocols.is_empty() {
        let proto_header = protocols.join(", ");
        match proto_header.parse() {
            Ok(value) => {
                request
                    .headers_mut()
                    .insert("Sec-WebSocket-Protocol", value);
            }
            Err(e) => {
                warn!("Failed to parse WebSocket protocol header: {e}");
            }
        }
    }

    let upstream_ws = match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio_tungstenite::connect_async(request),
    )
    .await
    {
        Ok(Ok((ws, _))) => ws,
        Ok(Err(e)) => {
            error!(bore_port, "Failed to connect upstream WS: {e}");
            return;
        }
        Err(_) => {
            error!(bore_port, "Upstream WS connection timed out after 5s");
            return;
        }
    };

    nomadflow_ws::bridge(client_ws, upstream_ws).await;
}

// ── Cleanup task ──────────────────────────────────────────────────────

async fn cleanup_stale_tunnels(state: Arc<RelayState>) {
    let ttl = Duration::from_secs(24 * 60 * 60); // 24 hours
    let rate_limit_window = Duration::from_secs(3600); // 1 hour
    loop {
        tokio::time::sleep(Duration::from_secs(300)).await;

        // Cleanup stale tunnels based on last usage (not creation time)
        let before = state.tunnels.len();
        state.tunnels.retain(|_, entry| entry.last_used.elapsed() < ttl);
        let removed = before - state.tunnels.len();
        if removed > 0 {
            info!(removed, "Cleaned up stale tunnel entries");
        }

        // Cleanup stale rate limit entries
        let now = Instant::now();
        state.rate_limits.retain(|_, timestamps| {
            timestamps.retain(|ts| now.duration_since(*ts) < rate_limit_window);
            !timestamps.is_empty()
        });
    }
}

// ── Main ──────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "nomadflow_relay=info,tower_http=info".into()),
        )
        .init();

    let relay_secret = std::env::var("RELAY_SECRET").unwrap_or_default();
    let bore_host =
        std::env::var("BORE_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = std::env::var("RELAY_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);

    info!(%bore_host, "Bore tunnel host");

    let state = Arc::new(RelayState {
        tunnels: DashMap::new(),
        rate_limits: DashMap::new(),
        relay_secret,
        bore_host,
        http_client: HyperClient::builder(TokioExecutor::new()).build_http(),
    });

    // Spawn cleanup task
    tokio::spawn(cleanup_stale_tunnels(state.clone()));

    // Internal API routes (matched by path prefix)
    let api = Router::new()
        .route("/_api/register", post(register))
        .route("/_api/check", get(check))
        .route("/_api/health", get(health));

    // Catch-all proxy for subdomain traffic
    let proxy = Router::new().fallback(proxy_handler);

    let app = api
        .merge(proxy)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!(%addr, "NomadFlow Relay listening");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}
