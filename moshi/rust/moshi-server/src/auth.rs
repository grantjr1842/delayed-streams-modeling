// Copyright (c) Kyutai, all rights reserved.
// This source code is licensed under the license found in the
// LICENSE file in the root directory of this source tree.

use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::OnceLock;

use crate::metrics::errors as error_metrics;

/// Header for legacy API key authentication
pub const ID_HEADER: &str = "kyutai-api-key";

/// Header for Bearer token authentication (Better Auth JWT)
pub const AUTHORIZATION_HEADER: &str = "authorization";

/// Cookie name for Better Auth session (when using cookie cache with JWT strategy)
pub const SESSION_COOKIE: &str = "better-auth.session_token";

/// Global JWT secret loaded from environment
static JWT_SECRET: OnceLock<Option<String>> = OnceLock::new();

// ============================================================================
// AuthError - Structured authentication error type
// ============================================================================

/// Authentication error variants with structured JSON responses
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthErrorCode {
    InvalidKey,
    ExpiredToken,
    MissingCredentials,
    JwtValidationFailed,
}

impl std::fmt::Display for AuthErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidKey => write!(f, "invalid_key"),
            Self::ExpiredToken => write!(f, "expired_token"),
            Self::MissingCredentials => write!(f, "missing_credentials"),
            Self::JwtValidationFailed => write!(f, "jwt_validation_failed"),
        }
    }
}

/// Structured authentication error with JSON response body
#[derive(Debug, Clone, Serialize)]
pub struct AuthError {
    pub error: &'static str,
    pub code: AuthErrorCode,
    pub message: String,
    pub hint: &'static str,
}

impl AuthError {
    /// Invalid or unrecognized API key
    pub fn invalid_key(masked_key: Option<String>) -> Self {
        let message = match masked_key {
            Some(k) => format!("Invalid API key: {k}"),
            None => "Invalid API key".to_string(),
        };
        Self {
            error: "unauthorized",
            code: AuthErrorCode::InvalidKey,
            message,
            hint: "Provide a valid key via kyutai-api-key header or auth_id query param",
        }
    }

    /// JWT session has expired
    pub fn expired_token() -> Self {
        Self {
            error: "unauthorized",
            code: AuthErrorCode::ExpiredToken,
            message: "Session has expired".to_string(),
            hint: "Re-authenticate to obtain a new session token",
        }
    }

    /// No authentication credentials provided
    pub fn missing_credentials() -> Self {
        Self {
            error: "unauthorized",
            code: AuthErrorCode::MissingCredentials,
            message: "No authentication credentials provided".to_string(),
            hint: "Provide kyutai-api-key header, Authorization Bearer token, or session cookie",
        }
    }

    /// JWT validation failed (signature, format, etc.)
    pub fn jwt_validation_failed(reason: &str) -> Self {
        Self {
            error: "unauthorized",
            code: AuthErrorCode::JwtValidationFailed,
            message: format!("JWT validation failed: {reason}"),
            hint: "Ensure the token is properly signed and not corrupted",
        }
    }

    /// Get the error code as a string for metrics labels
    pub fn error_type(&self) -> &'static str {
        match self.code {
            AuthErrorCode::InvalidKey => "invalid_key",
            AuthErrorCode::ExpiredToken => "expired_token",
            AuthErrorCode::MissingCredentials => "missing_credentials",
            AuthErrorCode::JwtValidationFailed => "jwt_validation_failed",
        }
    }
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        // Increment Prometheus counter with error type label
        error_metrics::record_auth_error(self.error_type());

        (StatusCode::UNAUTHORIZED, Json(self)).into_response()
    }
}

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

impl Default for AuthConfig {
    fn default() -> Self {
        Self { authorized_ids: HashSet::new(), jwt_secret: None }
    }
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

        Self {
            authorized_ids,
            jwt_secret,
        }
    }

    /// Log authentication configuration (call after tracing is initialized)
    pub fn log_config(&self) {
        let masked: Vec<String> = self.authorized_ids.iter().map(|k| mask_key(k)).collect();
        if masked.is_empty() {
            tracing::warn!("No API keys configured (MOSHI_API_KEY not set)");
        } else {
            tracing::info!(keys = ?masked, count = masked.len(), "Authorized API keys loaded");
        }
        if self.jwt_secret.is_some() {
            tracing::info!("Better Auth JWT validation enabled (BETTER_AUTH_SECRET is set)");
        }
    }
}

/// Mask an API key for logging (keep first 6 and last 4 chars if long enough)
pub fn mask_key(key: &str) -> String {
    if key.len() <= 10 {
        return "*hidden*".to_string();
    }
    let prefix = &key[..6];
    let suffix = &key[key.len().saturating_sub(4)..];
    format!("{prefix}…{suffix}")
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
fn validate_jwt(token: &str) -> Result<BetterAuthClaims, AuthError> {
    let secret = get_jwt_secret().ok_or_else(|| {
        tracing::warn!("JWT validation attempted but BETTER_AUTH_SECRET not configured");
        AuthError::jwt_validation_failed("BETTER_AUTH_SECRET not configured")
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
                return Err(AuthError::expired_token());
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
            Err(AuthError::jwt_validation_failed(&e.to_string()))
        }
    }
}

/// Check authentication using multiple methods:
/// 1. Legacy API key (kyutai-api-key header or query param)
/// 2. Bearer token (Authorization header with JWT)
/// 3. Session cookie (better-auth.session_token)
///
/// Returns Ok(()) if any method succeeds, Err(AuthError) with structured JSON otherwise.
pub fn check(
    headers: &HeaderMap,
    query_auth_id: Option<&str>,
    authorized_ids: &HashSet<String>,
) -> Result<(), AuthError> {
    // Method 1: Legacy API key authentication
    let api_key = headers
        .get(ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .or(query_auth_id);

    if let Some(key) = api_key {
        if authorized_ids.contains(key) {
            tracing::debug!("Authenticated via API key");
            return Ok(());
        } else {
            // Invalid key provided - log at WARN with masked key
            let masked = mask_key(key);
            tracing::warn!(masked_key = %masked, "Authentication failed: invalid API key");
            return Err(AuthError::invalid_key(Some(masked)));
        }
    }

    // Method 2: Bearer token (JWT)
    if let Some(token) = extract_bearer_token(headers) {
        match validate_jwt(token) {
            Ok(_) => return Ok(()),
            Err(e) => {
                tracing::warn!(error_type = %e.code, "Authentication failed: JWT validation error");
                return Err(e);
            }
        }
    }

    // Method 3: Session cookie
    if let Some(token) = extract_session_cookie(headers) {
        match validate_jwt(token) {
            Ok(_) => return Ok(()),
            Err(e) => {
                tracing::warn!(error_type = %e.code, "Authentication failed: session cookie validation error");
                return Err(e);
            }
        }
    }

    // No credentials provided at all
    tracing::warn!("Authentication failed: no credentials provided");
    Err(AuthError::missing_credentials())
}

/// Extended check that returns user information if authenticated via JWT
pub fn check_with_user(
    headers: &HeaderMap,
    query_auth_id: Option<&str>,
    authorized_ids: &HashSet<String>,
) -> Result<Option<BetterAuthClaims>, AuthError> {
    // Method 1: Legacy API key authentication
    let api_key = headers
        .get(ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .or(query_auth_id);

    if let Some(key) = api_key {
        if authorized_ids.contains(key) {
            tracing::debug!("Authenticated via API key");
            return Ok(None); // No user info for API key auth
        } else {
            // Invalid key provided - log at WARN with masked key
            let masked = mask_key(key);
            tracing::warn!(masked_key = %masked, "Authentication failed: invalid API key");
            return Err(AuthError::invalid_key(Some(masked)));
        }
    }

    // Method 2: Bearer token (JWT)
    if let Some(token) = extract_bearer_token(headers) {
        match validate_jwt(token) {
            Ok(claims) => return Ok(Some(claims)),
            Err(e) => {
                tracing::warn!(error_type = %e.code, "Authentication failed: JWT validation error");
                return Err(e);
            }
        }
    }

    // Method 3: Session cookie
    if let Some(token) = extract_session_cookie(headers) {
        match validate_jwt(token) {
            Ok(claims) => return Ok(Some(claims)),
            Err(e) => {
                tracing::warn!(error_type = %e.code, "Authentication failed: session cookie validation error");
                return Err(e);
            }
        }
    }

    // No credentials provided at all
    tracing::warn!("Authentication failed: no credentials provided");
    Err(AuthError::missing_credentials())
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

    #[test]
    fn test_invalid_key_error() {
        let mut headers = HeaderMap::new();
        headers.insert(ID_HEADER, "wrong-key".parse().unwrap());

        let mut authorized = HashSet::new();
        authorized.insert("correct-key".to_string());

        let err = check(&headers, None, &authorized).unwrap_err();
        assert!(matches!(err.code, AuthErrorCode::InvalidKey));
    }

    #[test]
    fn test_missing_credentials_error() {
        let headers = HeaderMap::new();
        let authorized = HashSet::new();

        let err = check(&headers, None, &authorized).unwrap_err();
        assert!(matches!(err.code, AuthErrorCode::MissingCredentials));
    }

    #[test]
    fn test_mask_key() {
        assert_eq!(mask_key("short"), "*hidden*");
        assert_eq!(mask_key("abcdefghijklmnop"), "abcdef…mnop");
    }
}
