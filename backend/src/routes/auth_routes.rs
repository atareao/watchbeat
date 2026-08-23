use std::sync::Arc;

use axum::extract::{Query, State};
use axum::response::Redirect;
use axum::Json;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha1::{Sha1, Digest};
use uuid::Uuid;

use crate::auth::{AppState, Claims, OidcState};

#[derive(Deserialize)]
pub struct LoginQuery {
    pub redirect: Option<String>,
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Query(query): Query<LoginQuery>,
) -> Result<Redirect, String> {
    let metadata = state
        .oidc_metadata
        .as_ref()
        .ok_or("OIDC not configured".to_string())?;

    // Generate PKCE challenge
    let code_verifier = Uuid::new_v4().to_string() + &Uuid::new_v4().to_string();
    let code_challenge = {
        let mut hasher = sha1::Sha1::new();
        hasher.update(code_verifier.as_bytes());
        let result = hasher.finalize();
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(result)
    };

    let state_value = Uuid::new_v4().to_string();
    let redirect_uri = query.redirect.unwrap_or_default();

    let oidc_state = OidcState {
        code_verifier,
        state: state_value.clone(),
        created_at: chrono::Utc::now(),
    };

    state.oidc_states.lock().await.insert(state_value.clone(), oidc_state);

    let auth_url = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&scope=openid%20email%20profile&state={}&code_challenge={}&code_challenge_method=S256",
        metadata.authorization_endpoint,
        state.config.oidc_client_id,
        urlencoding(&state.config.oidc_redirect_url),
        state_value,
        code_challenge,
    );

    let mut redirect = Redirect::to(&auth_url);

    if !redirect_uri.is_empty() {
        // We store redirect in memory keyed by state — handled in callback
        state
            .oidc_states
            .lock()
            .await
            .entry(state_value)
            .and_modify(|s| s.state = format!("{}:{}", s.state, redirect_uri));
    }

    Ok(redirect)
}

fn urlencoding(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    pub code: String,
    pub state: String,
}

#[derive(Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub id_token: Option<String>,
    pub token_type: String,
    pub expires_in: Option<i64>,
}

pub async fn callback(
    State(state): State<Arc<AppState>>,
    Query(query): Query<CallbackQuery>,
) -> Result<axum::response::Response, String> {
    let metadata = state
        .oidc_metadata
        .as_ref()
        .ok_or("OIDC not configured")?;

    // Verify state
    let mut states = state.oidc_states.lock().await;
    let (code_verifier, redirect_uri) = match states.remove(&query.state) {
        Some(s) => {
            let parts: Vec<&str> = s.state.splitn(2, ':').collect();
            let orig_state = parts[0].to_string();
            let stored_redirect = parts.get(1).map(|s| s.to_string()).unwrap_or_default();
            (s.code_verifier, stored_redirect)
        }
        None => return Err("Invalid state parameter".to_string()),
    };
    drop(states);

    // Exchange code for token
    let client = reqwest::Client::new();
    let params = [
        ("grant_type", "authorization_code"),
        ("code", &query.code),
        ("redirect_uri", &state.config.oidc_redirect_url),
        ("client_id", &state.config.oidc_client_id),
        ("client_secret", &state.config.oidc_client_secret),
        ("code_verifier", &code_verifier),
    ];

    let resp = client
        .post(&metadata.token_endpoint)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Token request failed: {}", e))?;

    let token_resp: TokenResponse = resp
        .json()
        .await
        .map_err(|e| format!("Token parse failed: {}", e))?;

    let id_token = token_resp.id_token.as_deref().unwrap_or(&token_resp.access_token);

    // Set cookie and redirect
    let cookie = format!(
        "token={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=86400",
        id_token
    );

    let target = if redirect_uri.is_empty() {
        "/"
    } else {
        &redirect_uri
    };

    let response = axum::response::Response::builder()
        .header("Set-Cookie", cookie)
        .header("Location", target)
        .status(302)
        .body(axum::body::Body::empty())
        .unwrap();

    Ok(response)
}

pub async fn me(
    State(_state): State<Arc<AppState>>,
    claims: axum::extract::Extension<Claims>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "sub": claims.sub,
        "email": claims.email,
        "name": claims.name,
    }))
}