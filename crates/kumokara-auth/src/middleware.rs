//! Axum middleware and extractors for token authentication.

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::json;

use crate::AuthManager;

/// Error returned when authentication fails.
#[derive(Debug)]
pub enum AuthError {
    MissingToken,
    InvalidToken,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> axum::response::Response {
        let (status, code, message) = match self {
            AuthError::MissingToken => (
                StatusCode::UNAUTHORIZED,
                "AUTH_INVALID",
                "Missing authentication token",
            ),
            AuthError::InvalidToken => (
                StatusCode::UNAUTHORIZED,
                "AUTH_INVALID",
                "Invalid authentication token",
            ),
        };

        let body = json!({
            "error": {
                "code": code,
                "message": message,
            }
        });

        (status, Json(body)).into_response()
    }
}

/// Extractor that validates the Authorization Bearer token.
///
/// Usage in axum handlers:
/// ```ignore
/// async fn handler(AuthenticatedUser: auth) {
///     // handler only reached if token is valid
/// }
/// ```
#[derive(Clone)]
pub struct AuthenticatedUser;

impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // AuthManager is injected via axum Extension
        let auth_manager = parts
            .extensions
            .get::<AuthManager>()
            .ok_or(AuthError::InvalidToken)?;

        // Extract token from Authorization header
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or(AuthError::MissingToken)?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or(AuthError::InvalidToken)?;

        if auth_manager.validate_token(token) {
            Ok(AuthenticatedUser)
        } else {
            Err(AuthError::InvalidToken)
        }
    }
}

/// Convenience function to create the axum Extension layer for AuthManager.
pub fn auth_layer(auth_manager: AuthManager) -> axum::Extension<AuthManager> {
    axum::Extension(auth_manager)
}
