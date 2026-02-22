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
        Some(h) if h.starts_with("Bearer ") => h.as_bytes()[7..].ct_eq(secret.as_bytes()).into(),
        Some(h) if h.starts_with("Basic ") => check_basic_auth(&h[6..], secret),
        _ => false,
    };

    if authenticated {
        next.run(request).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Basic realm=\"nomadflow\"")],
        )
            .into_response()
    }
}

/// Decode Basic Auth `base64(user:password)` and compare password to secret.
fn check_basic_auth(encoded: &str, secret: &str) -> bool {
    let decoded = match base64::engine::general_purpose::STANDARD.decode(encoded.trim()) {
        Ok(d) => d,
        Err(_) => return false,
    };
    let decoded_str = match std::str::from_utf8(&decoded) {
        Ok(s) => s,
        Err(_) => return false,
    };
    // Format: "user:password" — we only care about the password part
    match decoded_str.split_once(':') {
        Some((_, password)) => password.as_bytes().ct_eq(secret.as_bytes()).into(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_auth_valid() {
        // "nomadflow:mysecret" -> base64
        let encoded = base64::engine::general_purpose::STANDARD
            .encode("nomadflow:mysecret");
        assert!(check_basic_auth(&encoded, "mysecret"));
    }

    #[test]
    fn test_basic_auth_wrong_password() {
        let encoded = base64::engine::general_purpose::STANDARD
            .encode("nomadflow:wrongpassword");
        assert!(!check_basic_auth(&encoded, "mysecret"));
    }

    #[test]
    fn test_basic_auth_invalid_base64() {
        assert!(!check_basic_auth("not-valid-base64!!!", "mysecret"));
    }

    #[test]
    fn test_basic_auth_no_colon() {
        let encoded = base64::engine::general_purpose::STANDARD
            .encode("nocolonhere");
        assert!(!check_basic_auth(&encoded, "mysecret"));
    }

    #[test]
    fn test_basic_auth_any_username() {
        // Should work with any username, only password matters
        let encoded = base64::engine::general_purpose::STANDARD
            .encode("anyone:mysecret");
        assert!(check_basic_auth(&encoded, "mysecret"));
    }
}
