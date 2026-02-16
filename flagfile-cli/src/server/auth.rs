use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use super::config::NamespaceConfig;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TokenPermission {
    Read,
    Write,
}

/// Check if a token has the required permission for a namespace.
/// Write tokens implicitly grant read access.
/// Returns true if no tokens are configured (backward compat with no-auth mode).
pub fn check_token(
    ns_config: &NamespaceConfig,
    token: Option<&str>,
    required: TokenPermission,
) -> bool {
    // If no tokens configured, allow all (backward compat)
    if ns_config.read_tokens.is_empty() && ns_config.write_tokens.is_empty() {
        return true;
    }

    let Some(token) = token else { return false };

    match required {
        TokenPermission::Write => ns_config.write_tokens.iter().any(|t| t == token),
        TokenPermission::Read => {
            ns_config.read_tokens.iter().any(|t| t == token)
                || ns_config.write_tokens.iter().any(|t| t == token)
        }
    }
}

/// Extract bearer token from Authorization header value.
pub fn extract_bearer_token(header_value: &str) -> Option<&str> {
    header_value.strip_prefix("Bearer ")
}

/// Return 401 Unauthorized response.
pub fn unauthorized() -> Response {
    (StatusCode::UNAUTHORIZED, "Unauthorized").into_response()
}

/// Return 403 Forbidden response.
pub fn forbidden() -> Response {
    (StatusCode::FORBIDDEN, "Forbidden").into_response()
}
