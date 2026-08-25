use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock};

use crate::config::Config;

// ───── OIDC Metadata ─────

#[derive(Debug, Clone, Deserialize)]
pub struct OidcMetadata {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub userinfo_endpoint: String,
    pub jwks_uri: String,
    pub scopes_supported: Option<Vec<String>>,
}

// ───── JWT Claims (OIDC token claims) ─────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub preferred_username: Option<String>,
}

// ───── JWT Validator (exactamente como populates) ─────

pub struct JwtValidator {
    jwks: Arc<RwLock<Vec<DecodingKey>>>,
    issuer: String,
    client_id: String,
}

impl JwtValidator {
    pub fn new(issuer: &str, client_id: &str) -> Self {
        Self {
            jwks: Arc::new(RwLock::new(Vec::new())),
            issuer: issuer.to_string(),
            client_id: client_id.to_string(),
        }
    }

    pub async fn fetch_jwks(&self, issuer: &str) -> Result<(), String> {
        let jwks_url = format!("{}/.well-known/jwks.json", issuer.trim_end_matches('/'));
        let resp: serde_json::Value = reqwest::get(&jwks_url)
            .await
            .map_err(|e| format!("failed to fetch JWKS: {e}"))?
            .json()
            .await
            .map_err(|e| format!("failed to parse JWKS: {e}"))?;

        let keys = resp["keys"]
            .as_array()
            .ok_or_else(|| "JWKS response missing 'keys' array".to_string())?;

        let mut decoding_keys = Vec::new();
        for key in keys {
            if let (Some(n), Some(e)) = (
                key["n"].as_str().and_then(|s| {
                    base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, s)
                        .ok()
                }),
                key["e"].as_str().and_then(|s| {
                    base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, s)
                        .ok()
                }),
            ) {
                let dk = DecodingKey::from_rsa_raw_components(&n, &e);
                decoding_keys.push(dk);
            }
        }

        tracing::info!(
            count = decoding_keys.len(),
            "JWKS fetched from {}",
            jwks_url
        );
        *self.jwks.write().await = decoding_keys;
        Ok(())
    }

    pub async fn validate_token(&self, token: &str) -> Result<Claims, String> {
        let keys = {
            let jwks = self.jwks.read().await;
            if jwks.is_empty() {
                tracing::warn!("JWKS cache empty, re-fetching...");
                drop(jwks);
                self.fetch_jwks(&self.issuer).await?;
                return Box::pin(self.validate_token(token)).await;
            }
            jwks.clone()
        };

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[&self.client_id]);

        for (i, key) in keys.iter().enumerate() {
            match decode::<Claims>(token, key, &validation) {
                Ok(data) => {
                    tracing::debug!(key_index = i, sub = %data.claims.sub, "token validated successfully");
                    return Ok(data.claims);
                }
                Err(e) => {
                    tracing::debug!(key_index = i, error = %e, "JWK decode attempt failed");
                }
            }
        }
        Err("no matching JWK found for token".to_string())
    }
}

// ───── AppState ─────

/// SSE event broadcast channel capacity
pub const SSE_CHANNEL_CAPACITY: usize = 256;

/// OIDC CSRF states: map of state_value -> (state_value, timestamp)
pub type OidcStates = Arc<Mutex<HashMap<String, (String, std::time::Instant)>>>;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: crate::db::Database,
    pub oidc_metadata: Option<OidcMetadata>,
    pub jwt_validator: Arc<JwtValidator>,
    pub oidc_states: OidcStates,
    pub scheduler_status: Arc<Mutex<SchedulerStatus>>,
    pub event_tx: tokio::sync::broadcast::Sender<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SchedulerStatus {
    pub last_run_at: Option<String>,
    pub next_run_at: Option<String>,
    pub last_monitors_checked: u64,
}

// ───── Auth Middleware (exactamente como populates) ─────

pub async fn require_auth(mut req: Request, next: Next) -> Result<Response, StatusCode> {
    let path = req.uri().path();

    // Public endpoints
    if path.starts_with("/auth/") || path == "/health" || path == "/" {
        tracing::trace!(path = %path, "public endpoint — skipping auth");
        return Ok(next.run(req).await);
    }
    // Non-API routes (frontend assets) pass through
    if !path.starts_with("/api/") {
        return Ok(next.run(req).await);
    }

    // Extract Bearer token from Authorization header
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            tracing::warn!(path = %path, "no Authorization header");
            StatusCode::UNAUTHORIZED
        })?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Get state from extension (inserted by wrapper in main.rs)
    let state = req
        .extensions()
        .get::<Arc<AppState>>()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    // Validate token against JWKS
    let claims = state
        .jwt_validator
        .validate_token(token)
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, path = %path, "token validation failed");
            StatusCode::UNAUTHORIZED
        })?;

    // Insert claims for downstream handlers (like me endpoint)
    req.extensions_mut().insert(claims);

    Ok(next.run(req).await)
}

// ───── Auth Error ─────

#[derive(Debug)]
pub enum AuthError {
    Unauthorized(String),
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        match self {
            Self::Unauthorized(msg) => (
                StatusCode::UNAUTHORIZED,
                serde_json::json!({"error": msg}).to_string(),
            )
                .into_response(),
        }
    }
}

// ───── OIDC Discovery ─────

pub async fn discover_oidc(config: &Config) -> anyhow::Result<OidcMetadata> {
    let issuer = config.oidc_issuer_url.trim_end_matches('/');
    let well_known = format!("{}/.well-known/openid-configuration", issuer);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let resp = client.get(&well_known).send().await?;
    let metadata: OidcMetadata = resp.json().await?;
    Ok(metadata)
}
