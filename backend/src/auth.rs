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
use crate::scheduler::SchedulerManager;

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
    /// Whether a key miss may trigger a JWKS re-fetch. Disabled by
    /// [`JwtValidator::with_keys`] so tests can run fully offline.
    refresh_on_miss: bool,
}

impl JwtValidator {
    pub fn new(issuer: &str, client_id: &str) -> Self {
        Self {
            jwks: Arc::new(RwLock::new(Vec::new())),
            issuer: issuer.to_string(),
            client_id: client_id.to_string(),
            refresh_on_miss: true,
        }
    }

    /// Builds a validator with an already-populated JWKS cache and no network
    /// refresh: a key miss returns an error instead of fetching `jwks_uri`.
    #[cfg(test)]
    pub(crate) fn with_keys(issuer: &str, client_id: &str, keys: Vec<DecodingKey>) -> Self {
        Self {
            jwks: Arc::new(RwLock::new(keys)),
            issuer: issuer.to_string(),
            client_id: client_id.to_string(),
            refresh_on_miss: false,
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
                drop(jwks);
                if !self.refresh_on_miss {
                    return Err("JWKS cache is empty and network refresh is disabled".to_string());
                }
                tracing::warn!("JWKS cache empty, re-fetching...");
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

        // All cached keys failed — re-fetch JWKS in case keys were rotated.
        // Skipped when refresh is disabled (offline/test mode).
        if !self.refresh_on_miss {
            return Err("no matching JWK found for token".to_string());
        }

        tracing::warn!("All cached JWKs failed, re-fetching JWKS (key rotation?)...");
        self.fetch_jwks(&self.issuer).await?;

        let fresh_keys = self.jwks.read().await.clone();
        for (i, key) in fresh_keys.iter().enumerate() {
            match decode::<Claims>(token, key, &validation) {
                Ok(data) => {
                    tracing::info!(key_index = i, sub = %data.claims.sub, "token validated after JWKS refresh");
                    return Ok(data.claims);
                }
                Err(e) => {
                    tracing::debug!(key_index = i, error = %e, "fresh JWK decode attempt failed");
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
    pub scheduler_mgr: SchedulerManager,
    pub event_tx: tokio::sync::broadcast::Sender<String>,
    /// Health of our own egress, published by the `netcheck` canary.
    pub netcheck: crate::netcheck::NetworkHealthHandle,
}

#[derive(Debug, Clone, Default)]
pub struct SchedulerStatus {
    pub last_check_at: Option<String>,
    pub active_tasks: u64,
}

// ───── Auth Middleware (exactamente como populates) ─────

pub async fn require_auth(mut req: Request, next: Next) -> Result<Response, StatusCode> {
    let path = req.uri().path();

    // Public endpoints
    if path.starts_with("/auth/")
        || path == "/health"
        || path == "/"
        || path.starts_with("/api/heartbeat/")
        || path.starts_with("/api/events")
    {
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

#[cfg(test)]
mod tests {
    use std::sync::LazyLock;

    use jsonwebtoken::{encode, EncodingKey, Header};
    use rand::rngs::StdRng;
    use rand::SeedableRng;
    use rsa::pkcs1::EncodeRsaPrivateKey;
    use rsa::traits::PublicKeyParts;
    use rsa::PrivateKey;

    use super::*;

    const TEST_ISSUER: &str = "https://issuer.test";
    const TEST_CLIENT_ID: &str = "watchbeat-test-client";
    const KID_A: &str = "test-key-a";
    const KID_B: &str = "test-key-b";

    /// Deterministic RSA-2048 key material for offline JWT tests.
    ///
    /// The keys are generated once per process from a fixed seed, so the suite
    /// never touches the network and never depends on checked-in secrets.
    struct TestKeyMaterial {
        pkcs1_der: Vec<u8>,
        modulus: Vec<u8>,
        exponent: Vec<u8>,
    }

    impl TestKeyMaterial {
        fn generate(seed: u64) -> Self {
            let mut rng = StdRng::seed_from_u64(seed);
            let private = PrivateKey::new(&mut rng, 2048).expect("generate test RSA key");
            let pkcs1_der = private
                .to_pkcs1_der()
                .expect("encode test key as PKCS#1 DER")
                .as_bytes()
                .to_vec();
            Self {
                pkcs1_der,
                modulus: private.n().to_be_bytes_trimmed_vartime().to_vec(),
                exponent: private.e().to_be_bytes_trimmed_vartime().to_vec(),
            }
        }

        /// Signing key, in the PKCS#1 DER form `jsonwebtoken` expects.
        fn encoding_key(&self) -> EncodingKey {
            EncodingKey::from_rsa_der(&self.pkcs1_der)
        }

        /// Verification key, built the same way `fetch_jwks` builds them.
        fn decoding_key(&self) -> DecodingKey {
            DecodingKey::from_rsa_raw_components(&self.modulus, &self.exponent)
        }

        /// The RSA *public* key material a forger can read straight from a JWKS
        /// document: the `n` and `e` components. A JWKS-based validator never sees
        /// a PEM file, so these are exactly the bytes used in the HMAC
        /// alg-confusion forgery of [`hs256_token_is_rejected`].
        fn public_jwks_bytes(&self) -> Vec<u8> {
            [self.modulus.as_slice(), self.exponent.as_slice()].concat()
        }
    }

    static KEY_A: LazyLock<TestKeyMaterial> = LazyLock::new(|| TestKeyMaterial::generate(0xA));
    static KEY_B: LazyLock<TestKeyMaterial> = LazyLock::new(|| TestKeyMaterial::generate(0xB));

    #[derive(Serialize)]
    struct TestClaims {
        sub: String,
        iss: String,
        aud: String,
        exp: u64,
    }

    fn test_claims(iss: &str, aud: &str, exp: u64) -> TestClaims {
        TestClaims {
            sub: "user-1".to_string(),
            iss: iss.to_string(),
            aud: aud.to_string(),
            exp,
        }
    }

    fn in_one_hour() -> u64 {
        jsonwebtoken::get_current_timestamp() + 3600
    }

    fn in_the_past() -> u64 {
        jsonwebtoken::get_current_timestamp() - 3600
    }

    fn sign_rs256(kid: &str, key: &EncodingKey, claims: &TestClaims) -> String {
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(kid.to_string());
        encode(&header, claims, key).expect("encode test RS256 token")
    }

    /// Validator with `KEY_A` precached and the network refresh disabled.
    fn validator_with_key_a() -> JwtValidator {
        JwtValidator::with_keys(TEST_ISSUER, TEST_CLIENT_ID, vec![KEY_A.decoding_key()])
    }

    #[tokio::test]
    async fn valid_token_with_known_kid_is_accepted() {
        let token = sign_rs256(
            KID_A,
            &KEY_A.encoding_key(),
            &test_claims(TEST_ISSUER, TEST_CLIENT_ID, in_one_hour()),
        );

        let claims = validator_with_key_a()
            .validate_token(&token)
            .await
            .expect("valid token must be accepted");

        assert_eq!(claims.sub, "user-1");
    }

    #[tokio::test]
    async fn token_with_wrong_audience_is_rejected() {
        let token = sign_rs256(
            KID_A,
            &KEY_A.encoding_key(),
            &test_claims(TEST_ISSUER, "some-other-client", in_one_hour()),
        );

        let result = validator_with_key_a().validate_token(&token).await;

        assert!(
            result.is_err(),
            "token with wrong audience must be rejected"
        );
    }

    #[tokio::test]
    async fn token_with_wrong_issuer_is_rejected() {
        let token = sign_rs256(
            KID_A,
            &KEY_A.encoding_key(),
            &test_claims("https://evil.test", TEST_CLIENT_ID, in_one_hour()),
        );

        let result = validator_with_key_a().validate_token(&token).await;

        assert!(result.is_err(), "token with wrong issuer must be rejected");
    }

    #[tokio::test]
    async fn expired_token_is_rejected() {
        let token = sign_rs256(
            KID_A,
            &KEY_A.encoding_key(),
            &test_claims(TEST_ISSUER, TEST_CLIENT_ID, in_the_past()),
        );

        let result = validator_with_key_a().validate_token(&token).await;

        assert!(result.is_err(), "expired token must be rejected");
    }

    #[tokio::test]
    async fn hs256_token_is_rejected() {
        // Classic alg-confusion forgery: the token is HMAC-signed with the RSA
        // *public* key material published in the JWKS, and its header claims
        // `alg = HS256`. Every other check (signature secret, iss, aud, exp)
        // passes, so the rejection can only be attributed to the validator
        // pinning RS256 instead of honouring the header algorithm.
        let header = Header::new(Algorithm::HS256);
        let secret = EncodingKey::from_secret(&KEY_A.public_jwks_bytes()).expect("hmac key");
        let token = encode(
            &header,
            &test_claims(TEST_ISSUER, TEST_CLIENT_ID, in_one_hour()),
            &secret,
        )
        .expect("encode test HS256 token");

        let result = validator_with_key_a().validate_token(&token).await;

        assert!(
            result.is_err(),
            "HS256 token must be rejected by an RS256 validator"
        );
    }

    #[tokio::test]
    async fn token_with_unknown_kid_is_rejected_without_network() {
        // Signed by KEY_B, whose public key is NOT in the validator's JWKS.
        let token = sign_rs256(
            KID_B,
            &KEY_B.encoding_key(),
            &test_claims(TEST_ISSUER, TEST_CLIENT_ID, in_one_hour()),
        );

        let err = validator_with_key_a()
            .validate_token(&token)
            .await
            .expect_err("token signed by an unknown key must be rejected");

        // Reaching "no matching JWK" proves no JWKS refresh (network) was attempted:
        // the configured issuer does not even resolve.
        assert!(
            err.contains("no matching JWK"),
            "expected offline key-miss error, got: {err}"
        );
    }
}
