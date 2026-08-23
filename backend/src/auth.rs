use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

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

// ───── JWKS ─────

#[derive(Debug, Deserialize)]
pub struct Jwks {
    pub keys: Vec<Jwk>,
}

#[derive(Debug, Deserialize)]
pub struct Jwk {
    pub kid: Option<String>,
    pub kty: String,
    pub alg: Option<String>,
    pub n: Option<String>,
    pub e: Option<String>,
}

// ───── JWT Claims ─────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub preferred_username: Option<String>,
    pub exp: usize,
    pub iat: usize,
    pub iss: String,
}

// ───── JWT Validator ─────

pub struct JwtValidator {
    issuer: String,
    client_id: String,
    decoding_keys: Arc<Mutex<Vec<DecodingKey>>>,
    algorithms: Vec<Algorithm>,
}

impl JwtValidator {
    pub fn new(issuer: &str, client_id: &str) -> Self {
        Self {
            issuer: issuer.to_string(),
            client_id: client_id.to_string(),
            decoding_keys: Arc::new(Mutex::new(Vec::new())),
            algorithms: vec![Algorithm::RS256],
        }
    }

    pub async fn fetch_jwks(&self, jwks_uri: &str) -> anyhow::Result<()> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()?;

        let resp = client.get(jwks_uri).send().await?;
        let jwks: Jwks = resp.json().await?;

        let mut keys = Vec::new();
        for jwk in &jwks.keys {
            if let (Some(n), Some(e)) = (&jwk.n, &jwk.e) {
                let key = DecodingKey::from_rsa_components(n, e)?;
                keys.push(key);
            }
        }

        let mut store = self.decoding_keys.lock().await;
        *store = keys;
        tracing::info!("✅ Loaded {} JWKS keys", store.len());
        Ok(())
    }

    pub async fn validate_token(&self, token: &str) -> anyhow::Result<Claims> {
        let keys = self.decoding_keys.lock().await;

        for key in keys.iter() {
            let mut validation = Validation::new(Algorithm::RS256);
            validation.set_issuer(&[&self.issuer]);
            validation.set_audience(&[&self.client_id]);
            let empty: Vec<String> = Vec::new();
            validation.set_required_spec_claims(&empty);

            if let Ok(token_data) = decode::<Claims>(token, key, &validation) {
                return Ok(token_data.claims);
            }
        }

        anyhow::bail!("Token validation failed: no matching JWK key found")
    }
}

// ───── AppState ─────

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: crate::db::Database,
    pub oidc_metadata: Option<OidcMetadata>,
    pub jwt_validator: Arc<JwtValidator>,
    pub oidc_states: Arc<Mutex<HashMap<String, OidcState>>>,
    pub scheduler_status: Arc<Mutex<SchedulerStatus>>,
}

#[derive(Debug, Clone)]
pub struct OidcState {
    pub code_verifier: String,
    pub state: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Default)]
pub struct SchedulerStatus {
    pub last_run_at: Option<String>,
    pub next_run_at: Option<String>,
    pub last_monitors_checked: u64,
}

// ───── Auth Middleware ─────

pub async fn require_auth(
    mut req: Request,
    next: Next,
) -> Result<Response, AuthError> {
    let state = req
        .extensions()
        .get::<Arc<AppState>>()
        .cloned()
        .ok_or_else(|| AuthError::Unauthorized("No app state in request".into()))?;

    let path = req.uri().path().to_string();

    // Public paths
    if path == "/"
        || path == "/health"
        || path.starts_with("/auth/")
        || path.starts_with("/assets/")
        || path.ends_with(".html")
        || path.ends_with(".js")
        || path.ends_with(".css")
        || path.ends_with(".png")
        || path.ends_with(".ico")
        || path.ends_with(".svg")
        || path.ends_with(".woff2")
        || path.ends_with(".woff")
        || path.ends_with(".ttf")
    {
        return Ok(next.run(req).await);
    }

    // Extract JWT from Authorization header
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    let token = match auth_header {
        Some(t) => t,
        None => {
            // Try cookie as fallback
            let cookie_header = req.headers().get("Cookie").and_then(|v| v.to_str().ok());
            let token_from_cookie = cookie_header
                .and_then(|c| {
                    c.split(';')
                        .find(|part| part.trim().starts_with("token="))
                        .map(|part| part.trim().trim_start_matches("token="))
                });
            match token_from_cookie {
                Some(t) => t,
                None => return Err(AuthError::Unauthorized("Missing authentication token".into())),
            }
        }
    };

    match state.jwt_validator.validate_token(token).await {
        Ok(claims) => {
            req.extensions_mut().insert(claims);
            Ok(next.run(req).await)
        }
        Err(e) => Err(AuthError::Unauthorized(format!("Invalid token: {}", e))),
    }
}

// ───── Auth Error ─────

#[derive(Debug)]
pub enum AuthError {
    Unauthorized(String),
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        match self {
            Self::Unauthorized(msg) => {
                (StatusCode::UNAUTHORIZED, serde_json::json!({"error": msg}).to_string())
                    .into_response()
            }
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