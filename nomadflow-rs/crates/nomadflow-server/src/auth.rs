use std::sync::Arc;

use axum::{
    extract::State,
    http::{header, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::Engine;
use subtle::ConstantTimeEq;

use crate::state::AppState;

/// Auth middleware: verifies Bearer token or Basic Auth if a secret is configured.
pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let secret = &state.settings.auth.secret;

    // Skip auth if no secret configured
    if secret.is_empty() {
        return next.run(request).await;
    }

    // Check Authorization header
    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok());

    let authenticated = match auth_header {
        Some(h) if h.starts_with("Bearer ") => {
            h.as_bytes()[7..].ct_eq(secret.as_bytes()).into()
        }
        Some(h) if h.starts_with("Basic ") => {
            // Decode Basic Auth and check password matches secret
            base64::engine::general_purpose::STANDARD
                .decode(&h[6..])
                .ok()
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .and_then(|decoded| {
                    decoded
                        .split_once(':')
                        .map(|(_, pw)| pw.as_bytes().ct_eq(secret.as_bytes()).into())
                })
                .unwrap_or(false)
        }
        _ => false,
    };

    if authenticated {
        next.run(request).await
    } else {
        // Include WWW-Authenticate so WebView sends Basic Auth credentials
        (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Basic realm=\"NomadFlow\"")],
        )
            .into_response()
    }
}
