//! JWT token generation for Better Auth authentication.
//!
//! Generates JWT tokens compatible with moshi-server's `BetterAuthClaims` structure.

use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{EncodingKey, Header, encode};
use serde::{Deserialize, Serialize};

/// Session claims matching moshi-server's BetterAuthClaims.session
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionClaims {
    pub id: String,
    pub user_id: String,
    pub created_at: String,
    pub updated_at: String,
    pub expires_at: String,
    pub token: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

/// User claims matching moshi-server's BetterAuthClaims.user
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserClaims {
    pub id: String,
    pub name: String,
    pub email: String,
    pub email_verified: bool,
    pub image: Option<String>,
}

/// Complete JWT claims matching moshi-server's BetterAuthClaims
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BetterAuthClaims {
    pub session: SessionClaims,
    pub user: UserClaims,
    pub iat: i64,
    pub exp: i64,
}

/// Generate a JWT token for moshi-server authentication.
///
/// # Arguments
/// * `secret` - The BETTER_AUTH_SECRET used for signing
/// * `hours` - Token validity duration in hours (default: 1.0)
///
/// # Returns
/// A signed JWT token string
pub fn generate_token(secret: &str, hours: f64) -> Result<String, jsonwebtoken::errors::Error> {
    let now: DateTime<Utc> = Utc::now();
    let exp = now + Duration::seconds((hours * 3600.0) as i64);

    let claims = BetterAuthClaims {
        session: SessionClaims {
            id: "cli-session-id".to_string(),
            user_id: "cli-user-id".to_string(),
            created_at: now.to_rfc3339(),
            updated_at: now.to_rfc3339(),
            expires_at: exp.to_rfc3339(),
            token: "cli-session-token".to_string(),
            ip_address: Some("127.0.0.1".to_string()),
            user_agent: Some("kyutai-stt-cli/0.1.0".to_string()),
        },
        user: UserClaims {
            id: "cli-user-id".to_string(),
            name: "CLI User".to_string(),
            email: "cli@localhost".to_string(),
            email_verified: false,
            image: None,
        },
        iat: now.timestamp(),
        exp: exp.timestamp(),
    };

    let header = Header::default(); // HS256
    let key = EncodingKey::from_secret(secret.as_bytes());

    encode(&header, &claims, &key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{DecodingKey, Validation, decode};

    #[test]
    fn test_generate_token_valid_jwt() {
        let secret = "test-secret-key";
        let token = generate_token(secret, 1.0).expect("Failed to generate token");

        // Verify the token structure
        assert!(!token.is_empty());
        assert!(token.contains('.'));

        let parts: Vec<&str> = token.split('.').collect();
        assert_eq!(parts.len(), 3, "JWT should have 3 parts");
    }

    #[test]
    fn test_claims_structure() {
        let secret = "test-secret-key";
        let token = generate_token(secret, 1.0).expect("Failed to generate token");

        // Decode and verify claims
        let key = DecodingKey::from_secret(secret.as_bytes());
        let mut validation = Validation::default();
        validation.validate_exp = true;

        let decoded =
            decode::<BetterAuthClaims>(&token, &key, &validation).expect("Failed to decode token");

        assert_eq!(decoded.claims.session.id, "cli-session-id");
        assert_eq!(decoded.claims.user.id, "cli-user-id");
        assert_eq!(decoded.claims.user.name, "CLI User");
        assert!(decoded.claims.exp > decoded.claims.iat);
    }

    #[test]
    fn test_token_expiry() {
        let secret = "test-secret-key";
        let token = generate_token(secret, 24.0).expect("Failed to generate token");

        let key = DecodingKey::from_secret(secret.as_bytes());
        let mut validation = Validation::default();
        validation.validate_exp = false;

        let decoded =
            decode::<BetterAuthClaims>(&token, &key, &validation).expect("Failed to decode token");

        // 24 hours = 86400 seconds
        let expected_duration = 24 * 60 * 60;
        let actual_duration = decoded.claims.exp - decoded.claims.iat;

        assert!(
            (actual_duration - expected_duration).abs() < 2,
            "Expected ~24h duration, got {} seconds",
            actual_duration
        );
    }
}
