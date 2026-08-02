//! kumokara-auth — Minimal token-based authentication for Phase 0.
//!
//! Phase 0: single statically-configured token.
//! Phase 3: extended with API Keys + GitHub OAuth.

pub mod middleware;

use rand::Rng;
use std::sync::Arc;

/// Manages authentication tokens.
#[derive(Clone)]
pub struct AuthManager {
    /// The valid server token (Phase 0: single token)
    server_token: Arc<String>,
}

impl AuthManager {
    /// Create a new AuthManager with a randomly generated token.
    pub fn new() -> Self {
        let token = generate_token();
        Self {
            server_token: Arc::new(token),
        }
    }

    /// Create an AuthManager with a specific token.
    pub fn with_token(token: String) -> Self {
        Self {
            server_token: Arc::new(token),
        }
    }

    /// Validate a token against the stored server token.
    pub fn validate_token(&self, token: &str) -> bool {
        // Constant-time comparison to prevent timing attacks
        let stored = self.server_token.as_bytes();
        let provided = token.as_bytes();
        if stored.len() != provided.len() {
            return false;
        }
        stored
            .iter()
            .zip(provided.iter())
            .fold(0, |acc, (a, b)| acc | (a ^ b))
            == 0
    }

    /// Get the current server token (for display on first run).
    pub fn server_token(&self) -> &str {
        &self.server_token
    }
}

impl Default for AuthManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate a cryptographically random token string.
fn generate_token() -> String {
    let mut rng = rand::rng();
    let bytes: [u8; 32] = rng.random();
    hex::encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_generation_and_validation() {
        let auth = AuthManager::new();
        let token = auth.server_token().to_string();

        assert!(auth.validate_token(&token));
        assert!(!auth.validate_token("wrong-token"));
        assert!(!auth.validate_token(""));
    }

    #[test]
    fn test_custom_token() {
        let auth = AuthManager::with_token("my-secret-token".to_string());
        assert!(auth.validate_token("my-secret-token"));
        assert!(!auth.validate_token("other-token"));
    }
}
