// Copyright (c) Kyutai, all rights reserved.
// This source code is licensed under the license found in the
// LICENSE file in the root directory of this source tree.

use axum::http::{HeaderMap, StatusCode};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::OnceLock;

/// Header for legacy API key authentication
pub const ID_HEADER: &str = "kyutai-api-key";

/// Header for Bearer token authentication (Better Auth JWT)
pub const AUTHORIZATION_HEADER: &str = "authorization";

/// Cookie name for Better Auth session (when using cookie cache with JWT strategy)
pub const SESSION_COOKIE: &str = "better-auth.session_token";

/// Global JWT secret loaded from environment
static JWT_SECRET: OnceLock<Option<String>> = OnceLock::new();

/// Session data within the Better Auth JWT
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
    /// Session ID
    pub id: String,
    /// User ID
    #[serde(rename = "userId")]
    pub user_id: String,
    /// Session creation time (ISO 8601 string)
    #[serde(rename = "createdAt")]
    pub created_at: String,
    /// Session update time (ISO 8601 string)
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    /// Session expiration time (ISO 8601 string)
    #[serde(rename = "expiresAt")]
    pub expires_at: String,
    /// Session token
    #[serde(default)]
    pub token: Option<String>,
    /// IP address (optional)
    #[serde(rename = "ipAddress", default)]
    pub ip_address: Option<String>,
    /// User agent (optional)
    #[serde(rename = "userAgent", default)]
    pub user_agent: Option<String>,
}

/// User data within the Better Auth JWT
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserData {
    /// User ID
    pub id: String,
    /// User name
    #[serde(default)]
    pub name: Option<String>,
    /// User email
    #[serde(default)]
    pub email: Option<String>,
    /// Email verified flag
    #[serde(rename = "emailVerified", default)]
    pub email_verified: Option<bool>,
    /// User image URL
    #[serde(default)]
    pub image: Option<String>,
}

/// Better Auth session claims structure
/// This matches the JWT payload from Better Auth's cookie cache with JWT strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BetterAuthClaims {
    /// Session data
    pub session: SessionData,
    /// User data
    pub user: UserData,
    /// JWT standard claims
    #[serde(default)]
    pub iat: Option<i64>,
    #[serde(default)]
    pub exp: Option<i64>,
}

/// Authentication configuration
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// Legacy API keys (from MOSHI_API_KEY env var)
    pub authorized_ids: HashSet<String>,
    /// JWT secret for Better Auth validation (from BETTER_AUTH_SECRET env var)
    pub jwt_secret: Option<String>,
}

impl AuthConfig {
    /// Load authentication configuration from environment
    pub fn from_env(mut authorized_ids: HashSet<String>) -> Self {
        // Load additional API keys from environment
        if let Ok(env_keys) = std::env::var("MOSHI_API_KEY") {
            for key in env_keys.split(',') {
                let key = key.trim();
                if !key.is_empty() {
                    authorized_ids.insert(key.to_string());
                }
            }
        }

        // Load JWT secret for Better Auth
        let jwt_secret = std::env::var("BETTER_AUTH_SECRET").ok();
        if jwt_secret.is_some() {
            tracing::info!("Better Auth JWT validation enabled");
        }

        Self {
            authorized_ids,
            jwt_secret,
        }
    }
}

/// Get the JWT secret from environment (cached)
fn get_jwt_secret() -> Option<&'static str> {
    JWT_SECRET
        .get_or_init(|| std::env::var("BETTER_AUTH_SECRET").ok())
        .as_deref()
}

/// Extract Bearer token from Authorization header
fn extract_bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION_HEADER)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
}

/// Extract session token from cookie
fn extract_session_cookie(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|cookie| {
                let cookie = cookie.trim();
                cookie
                    .strip_prefix(SESSION_COOKIE)
                    .and_then(|rest| rest.strip_prefix('='))
            })
        })
}

/// Validate a Better Auth JWT token
fn validate_jwt(token: &str) -> Result<BetterAuthClaims, StatusCode> {
    let secret = get_jwt_secret().ok_or_else(|| {
        tracing::warn!("JWT validation attempted but BETTER_AUTH_SECRET not configured");
        StatusCode::UNAUTHORIZED
    })?;

    let key = DecodingKey::from_secret(secret.as_bytes());

    // Better Auth uses HS256 by default for JWT cookie cache
    let mut validation = Validation::new(Algorithm::HS256);
    // Better Auth may not include standard aud/iss claims
    validation.validate_aud = false;
    validation.required_spec_claims.clear();

    match decode::<BetterAuthClaims>(token, &key, &validation) {
        Ok(token_data) => {
            let claims = token_data.claims;

            // Check if session has expired using the expiresAt field (ISO 8601 string)
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);

            // Parse ISO 8601 date string to Unix timestamp
            // Format: "2025-12-09T01:45:53.707Z"
            let expires_at = chrono::DateTime::parse_from_rfc3339(&claims.session.expires_at)
                .map(|dt| dt.timestamp() as u64)
                .unwrap_or(0);

            if expires_at < now {
                tracing::debug!(
                    user_id = %claims.session.user_id,
                    expires_at = %claims.session.expires_at,
                    now = now,
                    "Session expired"
                );
                return Err(StatusCode::UNAUTHORIZED);
            }

            tracing::debug!(
                user_id = %claims.session.user_id,
                session_id = %claims.session.id,
                email = ?claims.user.email,
                "JWT validated successfully"
            );
            Ok(claims)
        }
        Err(e) => {
            tracing::debug!(error = %e, "JWT validation failed");
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

/// Check authentication using multiple methods:
/// 1. Legacy API key (kyutai-api-key header or query param)
/// 2. Bearer token (Authorization header with JWT)
/// 3. Session cookie (better-auth.session_token)
///
/// Returns Ok(()) if any method succeeds, Err(StatusCode::UNAUTHORIZED) otherwise.
pub fn check(
    headers: &HeaderMap,
    query_auth_id: Option<&str>,
    authorized_ids: &HashSet<String>,
) -> Result<(), StatusCode> {
    // Method 1: Legacy API key authentication
    let api_key = headers
        .get(ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .or(query_auth_id);

    if let Some(key) = api_key {
        if authorized_ids.contains(key) {
            tracing::debug!("Authenticated via API key");
            return Ok(());
        }
    }

    // Method 2: Bearer token (JWT)
    if let Some(token) = extract_bearer_token(headers) {
        if validate_jwt(token).is_ok() {
            return Ok(());
        }
    }

    // Method 3: Session cookie
    if let Some(token) = extract_session_cookie(headers) {
        if validate_jwt(token).is_ok() {
            return Ok(());
        }
    }

    // No valid authentication found
    Err(StatusCode::UNAUTHORIZED)
}

/// Extended check that returns user information if authenticated via JWT
pub fn check_with_user(
    headers: &HeaderMap,
    query_auth_id: Option<&str>,
    authorized_ids: &HashSet<String>,
) -> Result<Option<BetterAuthClaims>, StatusCode> {
    // Method 1: Legacy API key authentication
    let api_key = headers
        .get(ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .or(query_auth_id);

    if let Some(key) = api_key {
        if authorized_ids.contains(key) {
            tracing::debug!("Authenticated via API key");
            return Ok(None); // No user info for API key auth
        }
    }

    // Method 2: Bearer token (JWT)
    if let Some(token) = extract_bearer_token(headers) {
        if let Ok(claims) = validate_jwt(token) {
            return Ok(Some(claims));
        }
    }

    // Method 3: Session cookie
    if let Some(token) = extract_session_cookie(headers) {
        if let Ok(claims) = validate_jwt(token) {
            return Ok(Some(claims));
        }
    }

    // No valid authentication found
    Err(StatusCode::UNAUTHORIZED)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_bearer_token() {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION_HEADER,
            "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.test"
                .parse()
                .unwrap(),
        );
        assert_eq!(
            extract_bearer_token(&headers),
            Some("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.test")
        );
    }

    #[test]
    fn test_extract_session_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "cookie",
            "other=value; better-auth.session_token=abc123; another=test"
                .parse()
                .unwrap(),
        );
        assert_eq!(extract_session_cookie(&headers), Some("abc123"));
    }

    #[test]
    fn test_legacy_api_key_auth() {
        let mut headers = HeaderMap::new();
        headers.insert(ID_HEADER, "test-key".parse().unwrap());

        let mut authorized = HashSet::new();
        authorized.insert("test-key".to_string());

        assert!(check(&headers, None, &authorized).is_ok());
    }

    #[test]
    fn test_query_param_auth() {
        let headers = HeaderMap::new();
        let mut authorized = HashSet::new();
        authorized.insert("query-key".to_string());

        assert!(check(&headers, Some("query-key"), &authorized).is_ok());
        assert!(check(&headers, Some("wrong-key"), &authorized).is_err());
    }
}
