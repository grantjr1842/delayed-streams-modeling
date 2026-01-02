use anyhow::{Result, Context};
use chrono::{DateTime, Duration, SecondsFormat, Utc};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Session claims matching moshi-server's BetterAuthClaims.session.
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

/// User claims matching moshi-server's BetterAuthClaims.user.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserClaims {
    pub id: String,
    pub name: String,
    pub email: String,
    pub email_verified: bool,
    pub image: Option<String>,
}

/// Complete JWT claims matching moshi-server's BetterAuthClaims.
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
/// * `secret` - The BETTER_AUTH_SECRET used for signing.
/// * `hours` - Token validity duration in hours.
/// * `user_agent` - Value to include in the session userAgent field.
pub fn generate_token(
    secret: &str,
    hours: f64,
    user_agent: &str,
) -> Result<String, jsonwebtoken::errors::Error> {
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
            user_agent: Some(user_agent.to_string()),
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

#[derive(Debug, Serialize)]
struct DevSessionData {
    id: String,
    #[serde(rename = "userId")]
    user_id: String,
    #[serde(rename = "createdAt")]
    created_at: String,
    #[serde(rename = "updatedAt")]
    updated_at: String,
    #[serde(rename = "expiresAt")]
    expires_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    token: Option<String>,
    #[serde(rename = "ipAddress", skip_serializing_if = "Option::is_none")]
    ip_address: Option<String>,
    #[serde(rename = "userAgent", skip_serializing_if = "Option::is_none")]
    user_agent: Option<String>,
}

#[derive(Debug, Serialize)]
struct DevUserData {
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,
    #[serde(rename = "emailVerified", skip_serializing_if = "Option::is_none")]
    email_verified: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
}

#[derive(Debug, Serialize)]
struct DevBetterAuthClaims {
    session: DevSessionData,
    user: DevUserData,
    #[serde(skip_serializing_if = "Option::is_none")]
    iat: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exp: Option<i64>,
}

/// Generate a dev JWT for moshi-server using the Better Auth claims format.
pub fn generate_dev_jwt(
    secret: &str,
    hours: i64,
) -> Result<String, jsonwebtoken::errors::Error> {
    let user_id = std::env::var("MOSHI_USER_ID").unwrap_or_else(|_| "local-dev-user".to_string());
    let session_id =
        std::env::var("MOSHI_SESSION_ID").unwrap_or_else(|_| "local-dev-session".to_string());

    let now = Utc::now();
    let created_at = now.to_rfc3339_opts(SecondsFormat::Millis, true);
    let expires_at = (now + Duration::hours(hours)).to_rfc3339_opts(SecondsFormat::Millis, true);

    let claims = DevBetterAuthClaims {
        session: DevSessionData {
            id: session_id,
            user_id: user_id.clone(),
            created_at: created_at.clone(),
            updated_at: created_at,
            expires_at,
            token: None,
            ip_address: None,
            user_agent: None,
        },
        user: DevUserData {
            id: user_id,
            name: None,
            email: None,
            email_verified: None,
            image: None,
            role: None,
            status: Some("approved".to_string()),
        },
        iat: Some(now.timestamp()),
        exp: Some((now + Duration::hours(hours)).timestamp()),
    };

    let mut header = Header::new(Algorithm::HS256);
    header.typ = Some("JWT".to_string());

    encode(&header, &claims, &EncodingKey::from_secret(secret.as_bytes()))
}

pub fn env_is_set_nonempty(key: &str) -> bool {
    match std::env::var(key) {
        Ok(value) => !value.trim().is_empty(),
        Err(_) => false,
    }
}

pub fn maybe_set_env_from_file(path: &Path, key: &str) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }

    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read env file: {}", path.display()))?;

    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        let name = name.trim();
        let name = name.strip_prefix("export ").unwrap_or(name).trim();
        if name == key {
            let value = value.trim().trim_matches('"').trim_matches('\'');
            if !value.is_empty() {
                unsafe {
                    std::env::set_var(key, value);
                }
                return Ok(true);
            }
        }
    }

    Ok(false)
}

pub fn load_better_auth_secret_from_env_files_if_needed(root: &Path) -> Result<()> {
    if env_is_set_nonempty("BETTER_AUTH_SECRET") {
        return Ok(());
    }

    let env_name = std::env::var("MOSHI_ENV")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| std::env::var("NODE_ENV").ok().filter(|value| !value.trim().is_empty()))
        .unwrap_or_else(|| "development".to_string());

    let candidates = [
        root.join(format!("env.{env_name}")),
        root.join(format!(".env.{env_name}")),
        root.join("env.development"),
        root.join(".env.development"),
        root.join("env.production"),
        root.join(".env.production"),
        root.join(".env"),
    ];

    for candidate in candidates {
        if maybe_set_env_from_file(&candidate, "BETTER_AUTH_SECRET")? {
            break;
        }
    }

    Ok(())
}

pub fn read_env_value(path: &Path, key: &str) -> Result<Option<String>> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read env file: {}", path.display()))?;
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        let name = name.trim();
        let name = name.strip_prefix("export ").unwrap_or(name).trim();
        if name == key {
            let value = value.trim().trim_matches('"').trim_matches('\'');
            return Ok(Some(value.to_string()));
        }
    }
    Ok(None)
}

pub fn load_secret_from_env_files(base_dir: &Path, env_name: Option<&str>) -> Result<Option<String>> {
    let env_name = env_name.unwrap_or("development");
    let candidates = [format!(".env.{env_name}"), ".env".to_string()];

    for file_name in candidates {
        let path = base_dir.join(file_name);
        if !path.exists() {
            continue;
        }
        if let Some(secret) = read_env_value(&path, "BETTER_AUTH_SECRET")? {
            return Ok(Some(secret));
        }
    }

    Ok(None)
}

pub fn resolve_secret(explicit: Option<&str>, base_dir: &Path, env_name: Option<&str>) -> Result<String> {
    if let Some(secret) = explicit {
        return Ok(secret.to_string());
    }

    if let Some(secret) = load_secret_from_env_files(base_dir, env_name)? {
        return Ok(secret);
    }

    anyhow::bail!(
        "--secret/BETTER_AUTH_SECRET or .env(.<env>) with BETTER_AUTH_SECRET is required"
    )
}

pub struct AuthResolver<'a> {
    pub token: Option<&'a str>,
    pub secret: Option<&'a str>,
    pub env_name: Option<&'a str>,
    pub user_agent: &'a str,
}

impl<'a> AuthResolver<'a> {
    pub fn new(user_agent: &'a str) -> Self {
        Self {
            token: None,
            secret: None,
            env_name: None,
            user_agent,
        }
    }

    pub fn with_token(mut self, token: Option<&'a str>) -> Self {
        self.token = token;
        self
    }

    pub fn with_secret(mut self, secret: Option<&'a str>) -> Self {
        self.secret = secret;
        self
    }

    pub fn with_env(mut self, env_name: Option<&'a str>) -> Self {
        self.env_name = env_name;
        self
    }

    pub fn resolve(&self, auto_token: bool) -> Result<Option<String>> {
        if let Some(token) = self.token {
            return Ok(Some(token.to_string()));
        }

        if let Ok(token) = std::env::var("MOSHI_JWT_TOKEN")
            && !token.trim().is_empty()
        {
            return Ok(Some(token));
        }

        if auto_token {
            let base_dir = std::env::current_dir()?;
            load_better_auth_secret_from_env_files_if_needed(&base_dir)?;
            let secret = resolve_secret(self.secret, &base_dir, self.env_name)?;
            let token = generate_token(&secret, 1.0, self.user_agent)
                .map_err(|e| anyhow::anyhow!("Failed to generate token: {}", e))?;
            return Ok(Some(token));
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{DecodingKey, Validation, decode};

    #[test]
    fn test_generate_token_valid_jwt() {
        let secret = "test-secret-key";
        let token = generate_token(secret, 1.0, "kyutai-test/0.1.0")
            .expect("Failed to generate token");

        assert!(!token.is_empty());
        assert!(token.contains('.'));

        let parts: Vec<&str> = token.split('.').collect();
        assert_eq!(parts.len(), 3, "JWT should have 3 parts");
    }

    #[test]
    fn test_claims_structure() {
        let secret = "test-secret-key";
        let token = generate_token(secret, 1.0, "kyutai-test/0.1.0")
            .expect("Failed to generate token");

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
        let token = generate_token(secret, 24.0, "kyutai-test/0.1.0")
            .expect("Failed to generate token");

        let key = DecodingKey::from_secret(secret.as_bytes());
        let mut validation = Validation::default();
        validation.validate_exp = false;

        let decoded =
            decode::<BetterAuthClaims>(&token, &key, &validation).expect("Failed to decode token");

        let expected_duration = 24 * 60 * 60;
        let actual_duration = decoded.claims.exp - decoded.claims.iat;

        assert!(
            (actual_duration - expected_duration).abs() < 2,
            "Expected ~24h duration, got {} seconds",
            actual_duration
        );
    }
}
