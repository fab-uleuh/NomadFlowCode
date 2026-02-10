use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use nomadflow_core::config::TunnelConfig;

type Result<T> = color_eyre::Result<T>;

pub struct TunnelInfo {
    pub public_url: String,
}

#[derive(Serialize)]
struct RegisterRequest {
    port: u16,
    secret: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    subdomain: Option<String>,
}

#[derive(Deserialize)]
struct RegisterResponse {
    subdomain: String,
}

/// Start a bore tunnel and register with the relay server.
///
/// 1. Connect bore client → obtain remote port
/// 2. POST to relay registration API → receive subdomain
/// 3. Build public URL
/// 4. Spawn bore.listen() in background
pub async fn start_tunnel(
    local_port: u16,
    config: &TunnelConfig,
    shutdown: CancellationToken,
    http_client: &reqwest::Client,
) -> Result<TunnelInfo> {
    info!(
        relay_host = %config.relay_host,
        relay_port = config.relay_port,
        "Connecting to tunnel relay…"
    );

    // 1. Connect bore client to the relay's bore server
    let secret = if config.relay_secret.is_empty() {
        None
    } else {
        Some(config.relay_secret.as_str())
    };

    // bore Client::new(local_host, local_port, to, remote_port, secret)
    // - `to` = server hostname (bore uses CONTROL_PORT=7835 by default)
    // - `remote_port` = 0 means "assign a random port"
    let client = tokio::time::timeout(
        Duration::from_secs(15),
        bore_cli::client::Client::new("localhost", local_port, &config.relay_host, 0, secret),
    )
    .await
    .map_err(|_| color_eyre::eyre::eyre!("Bore connection timed out after 15s"))?
    .map_err(|e| color_eyre::eyre::eyre!("Bore connection failed: {e}"))?;

    let remote_port = client.remote_port();
    info!(remote_port, "Bore tunnel established");

    // 2. Register with the relay API to get a subdomain
    let relay_url = format!("https://{}/_api/register", config.relay_host);

    let resp = http_client
        .post(&relay_url)
        .timeout(Duration::from_secs(10))
        .json(&RegisterRequest {
            port: remote_port,
            secret: config.relay_secret.clone(),
            subdomain: if config.subdomain.is_empty() {
                None
            } else {
                Some(config.subdomain.clone())
            },
        })
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(color_eyre::eyre::eyre!(
            "Relay registration failed ({status}): {body}"
        ));
    }

    let register: RegisterResponse = resp.json().await?;
    let subdomain = register.subdomain;
    // relay_host is "relay.nomadflowcode.dev", tunnel domain is "*.tunnel.nomadflowcode.dev"
    // Extract the base domain by removing the "relay." prefix
    let base_domain = config
        .relay_host
        .strip_prefix("relay.")
        .unwrap_or(&config.relay_host);
    let public_url = format!("https://{subdomain}.tunnel.{base_domain}");

    info!(%public_url, "Tunnel registered");

    // 3. Spawn bore client listener in background
    tokio::spawn(async move {
        tokio::select! {
            result = client.listen() => {
                if let Err(e) = result {
                    error!("Bore tunnel closed: {e}");
                }
            }
            _ = shutdown.cancelled() => {
                info!("Shutting down bore tunnel");
            }
        }
    });

    Ok(TunnelInfo { public_url })
}
