// Copyright (c) Kyutai, all rights reserved.
// This source code is licensed under the license found in the
// LICENSE file in the root directory of this source tree.

use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

use crate::metrics::errors as error_metrics;

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
    ExpiredToken,
    MissingCredentials,
    JwtValidationFailed,
    /// Account is pending admin approval
    PendingApproval,
    /// Account has been rejected by admin
    AccountRejected,
}

impl std::fmt::Display for AuthErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExpiredToken => write!(f, "expired_token"),
            Self::MissingCredentials => write!(f, "missing_credentials"),
            Self::JwtValidationFailed => write!(f, "jwt_validation_failed"),
            Self::PendingApproval => write!(f, "pending_approval"),
            Self::AccountRejected => write!(f, "account_rejected"),
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
            hint: "Provide Authorization Bearer token, ?token query param, or session cookie",
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

    /// Account is pending admin approval
    pub fn pending_approval(email: Option<&str>) -> Self {
        let message = match email {
            Some(e) => format!("Account {} is pending admin approval", e),
            None => "Account is pending admin approval".to_string(),
        };
        Self {
            error: "forbidden",
            code: AuthErrorCode::PendingApproval,
            message,
            hint: "Please wait for an administrator to approve your account",
        }
    }

    /// Account has been rejected by admin
    pub fn account_rejected(email: Option<&str>) -> Self {
        let message = match email {
            Some(e) => format!("Account {} has been rejected", e),
            None => "Account has been rejected".to_string(),
        };
        Self {
            error: "forbidden",
            code: AuthErrorCode::AccountRejected,
            message,
            hint: "Contact the administrator for more information",
        }
    }

    /// Get the error code as a string for metrics labels
    pub fn error_type(&self) -> &'static str {
        match self.code {
            AuthErrorCode::ExpiredToken => "expired_token",
            AuthErrorCode::MissingCredentials => "missing_credentials",
            AuthErrorCode::JwtValidationFailed => "jwt_validation_failed",
            AuthErrorCode::PendingApproval => "pending_approval",
            AuthErrorCode::AccountRejected => "account_rejected",
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
    /// User role (e.g., "user", "admin")
    #[serde(default)]
    pub role: Option<String>,
    /// User approval status (e.g., "pending", "approved", "rejected")
    #[serde(default)]
    pub status: Option<String>,
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

/// Check if a user's approval status allows access.
/// Returns Ok(()) if status is "approved" or not set (backwards compatibility).
/// Returns Err with appropriate AuthError for "pending" or "rejected" status.
pub fn check_approval_status(claims: &BetterAuthClaims) -> Result<(), AuthError> {
    let email = claims.user.email.as_deref();
    
    match claims.user.status.as_deref() {
        // Approved or not set (backwards compatibility with existing JWTs)
        Some("approved") | None => {
            tracing::debug!(
                user_id = %claims.user.id,
                status = ?claims.user.status,
                "User approval status: OK"
            );
            Ok(())
        }
        // Pending approval
        Some("pending") => {
            tracing::warn!(
                user_id = %claims.user.id,
                email = ?email,
                "User account is pending approval"
            );
            Err(AuthError::pending_approval(email))
        }
        // Rejected
        Some("rejected") => {
            tracing::warn!(
                user_id = %claims.user.id,
                email = ?email,
                "User account has been rejected"
            );
            Err(AuthError::account_rejected(email))
        }
        // Unknown status - treat as rejected for security
        Some(unknown) => {
            tracing::warn!(
                user_id = %claims.user.id,
                email = ?email,
                status = %unknown,
                "User has unknown approval status, denying access"
            );
            Err(AuthError::account_rejected(email))
        }
    }
}

/// Authentication configuration
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// JWT secret for Better Auth validation (from BETTER_AUTH_SECRET env var)
    pub jwt_secret: Option<String>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self { jwt_secret: None }
    }
}

impl AuthConfig {
    /// Load authentication configuration from environment
    pub fn from_env() -> Self {
        // Load JWT secret for Better Auth
        let jwt_secret = std::env::var("BETTER_AUTH_SECRET").ok();
        Self { jwt_secret }
    }

    /// Log authentication configuration (call after tracing is initialized)
    pub fn log_config(&self) {
        if self.jwt_secret.is_some() {
            tracing::info!("Better Auth JWT validation enabled (BETTER_AUTH_SECRET is set)");
        } else {
            tracing::warn!("No authentication configured (BETTER_AUTH_SECRET not set)");
        }
    }
}

/// Get the JWT secret from environment (cached)
fn get_jwt_secret() -> Option<&'static str> {
    JWT_SECRET.get_or_init(|| std::env::var("BETTER_AUTH_SECRET").ok()).as_deref()
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
    headers.get("cookie").and_then(|v| v.to_str().ok()).and_then(|cookies| {
        cookies.split(';').find_map(|cookie| {
            let cookie = cookie.trim();
            cookie.strip_prefix(SESSION_COOKIE).and_then(|rest| rest.strip_prefix('='))
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

/// Check authentication using Better Auth JWT:
/// 1. Bearer token (Authorization header with JWT)
/// 2. JWT token via query parameter (?token=...)
/// 3. Session cookie (better-auth.session_token)
///
/// Returns Ok(()) if any method succeeds, Err(AuthError) with structured JSON otherwise.
pub fn check(
    headers: &HeaderMap,
    query_token: Option<&str>,
) -> Result<(), AuthError> {
    // Method 1: Bearer token (JWT)
    if let Some(token) = extract_bearer_token(headers) {
        match validate_jwt(token) {
            Ok(claims) => {
                // Validate approval status
                check_approval_status(&claims)?;
                return Ok(());
            }
            Err(e) => {
                // Demote expired token to debug - it's expected behavior, not a security issue
                if matches!(e.code, AuthErrorCode::ExpiredToken) {
                    tracing::debug!(error_type = %e.code, "Authentication failed: JWT expired");
                } else {
                    tracing::warn!(error_type = %e.code, "Authentication failed: JWT validation error");
                }
                return Err(e);
            }
        }
    }

    // Method 2: JWT token via query parameter (?token=...)
    if let Some(token) = query_token {
        match validate_jwt(token) {
            Ok(claims) => {
                // Validate approval status
                check_approval_status(&claims)?;
                tracing::debug!("Authenticated via query token parameter");
                return Ok(());
            }
            Err(e) => {
                // Demote expired token to debug - it's expected behavior, not a security issue
                if matches!(e.code, AuthErrorCode::ExpiredToken) {
                    tracing::debug!(error_type = %e.code, "Authentication failed: query token expired");
                } else {
                    tracing::warn!(error_type = %e.code, "Authentication failed: query token validation error");
                }
                return Err(e);
            }
        }
    }

    // Method 3: Session cookie
    if let Some(token) = extract_session_cookie(headers) {
        match validate_jwt(token) {
            Ok(claims) => {
                // Validate approval status
                check_approval_status(&claims)?;
                return Ok(());
            }
            Err(e) => {
                // Demote expired token to debug - it's expected behavior, not a security issue
                if matches!(e.code, AuthErrorCode::ExpiredToken) {
                    tracing::debug!(error_type = %e.code, "Authentication failed: session cookie expired");
                } else {
                    tracing::warn!(error_type = %e.code, "Authentication failed: session cookie validation error");
                }
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
    query_token: Option<&str>,
) -> Result<BetterAuthClaims, AuthError> {
    // Method 1: Bearer token (JWT)
    if let Some(token) = extract_bearer_token(headers) {
        match validate_jwt(token) {
            Ok(claims) => {
                // Validate approval status before returning claims
                check_approval_status(&claims)?;
                return Ok(claims);
            }
            Err(e) => {
                // Demote expired token to debug - it's expected behavior, not a security issue
                if matches!(e.code, AuthErrorCode::ExpiredToken) {
                    tracing::debug!(error_type = %e.code, "Authentication failed: JWT expired");
                } else {
                    tracing::warn!(error_type = %e.code, "Authentication failed: JWT validation error");
                }
                return Err(e);
            }
        }
    }

    // Method 2: JWT token via query parameter (?token=...)
    if let Some(token) = query_token {
        match validate_jwt(token) {
            Ok(claims) => {
                // Validate approval status before returning claims
                check_approval_status(&claims)?;
                tracing::debug!("Authenticated via query token parameter");
                return Ok(claims);
            }
            Err(e) => {
                // Demote expired token to debug - it's expected behavior, not a security issue
                if matches!(e.code, AuthErrorCode::ExpiredToken) {
                    tracing::debug!(error_type = %e.code, "Authentication failed: query token expired");
                } else {
                    tracing::warn!(error_type = %e.code, "Authentication failed: query token validation error");
                }
                return Err(e);
            }
        }
    }

    // Method 3: Session cookie
    if let Some(token) = extract_session_cookie(headers) {
        match validate_jwt(token) {
            Ok(claims) => {
                // Validate approval status before returning claims
                check_approval_status(&claims)?;
                return Ok(claims);
            }
            Err(e) => {
                // Demote expired token to debug - it's expected behavior, not a security issue
                if matches!(e.code, AuthErrorCode::ExpiredToken) {
                    tracing::debug!(error_type = %e.code, "Authentication failed: session cookie expired");
                } else {
                    tracing::warn!(error_type = %e.code, "Authentication failed: session cookie validation error");
                }
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
            "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.test".parse().unwrap(),
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
            "other=value; better-auth.session_token=abc123; another=test".parse().unwrap(),
        );
        assert_eq!(extract_session_cookie(&headers), Some("abc123"));
    }

    #[test]
    fn test_missing_credentials_error() {
        let headers = HeaderMap::new();

        let err = check(&headers, None).unwrap_err();
        assert!(matches!(err.code, AuthErrorCode::MissingCredentials));
    }

    // Helper to create test claims with a specific status
    fn make_test_claims(status: Option<&str>) -> BetterAuthClaims {
        BetterAuthClaims {
            session: SessionData {
                id: "session-123".to_string(),
                user_id: "user-456".to_string(),
                created_at: "2025-01-01T00:00:00Z".to_string(),
                updated_at: "2025-01-01T00:00:00Z".to_string(),
                expires_at: "2099-01-01T00:00:00Z".to_string(),
                token: None,
                ip_address: None,
                user_agent: None,
            },
            user: UserData {
                id: "user-456".to_string(),
                name: Some("Test User".to_string()),
                email: Some("test@example.com".to_string()),
                email_verified: Some(true),
                image: None,
                role: Some("user".to_string()),
                status: status.map(String::from),
            },
            iat: Some(1704067200),
            exp: Some(4102444800),
        }
    }

    #[test]
    fn test_check_approval_status_approved() {
        let claims = make_test_claims(Some("approved"));
        assert!(check_approval_status(&claims).is_ok());
    }

    #[test]
    fn test_check_approval_status_none_backwards_compat() {
        // When status is None (old JWTs without status field), should be allowed
        let claims = make_test_claims(None);
        assert!(check_approval_status(&claims).is_ok());
    }

    #[test]
    fn test_check_approval_status_pending() {
        let claims = make_test_claims(Some("pending"));
        let err = check_approval_status(&claims).unwrap_err();
        assert!(matches!(err.code, AuthErrorCode::PendingApproval));
        assert_eq!(err.error, "forbidden");
        assert!(err.message.contains("pending"));
    }

    #[test]
    fn test_check_approval_status_rejected() {
        let claims = make_test_claims(Some("rejected"));
        let err = check_approval_status(&claims).unwrap_err();
        assert!(matches!(err.code, AuthErrorCode::AccountRejected));
        assert_eq!(err.error, "forbidden");
        assert!(err.message.contains("rejected"));
    }

    #[test]
    fn test_check_approval_status_unknown_treated_as_rejected() {
        let claims = make_test_claims(Some("unknown_status"));
        let err = check_approval_status(&claims).unwrap_err();
        // Unknown statuses are treated as rejected for security
        assert!(matches!(err.code, AuthErrorCode::AccountRejected));
    }

    #[test]
    fn test_pending_approval_error_with_email() {
        let err = AuthError::pending_approval(Some("user@example.com"));
        assert!(matches!(err.code, AuthErrorCode::PendingApproval));
        assert!(err.message.contains("user@example.com"));
        assert_eq!(err.error, "forbidden");
    }

    #[test]
    fn test_account_rejected_error_with_email() {
        let err = AuthError::account_rejected(Some("user@example.com"));
        assert!(matches!(err.code, AuthErrorCode::AccountRejected));
        assert!(err.message.contains("user@example.com"));
        assert_eq!(err.error, "forbidden");
    }

    #[test]
    fn test_error_type_new_variants() {
        let pending = AuthError::pending_approval(None);
        assert_eq!(pending.error_type(), "pending_approval");

        let rejected = AuthError::account_rejected(None);
        assert_eq!(rejected.error_type(), "account_rejected");
    }
}
