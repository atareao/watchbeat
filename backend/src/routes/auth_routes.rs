use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth::{AppState, Claims};

#[derive(Deserialize)]
pub struct LoginQuery {
    pub redirect: Option<String>,
}

#[derive(Deserialize)]
pub struct AuthCallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Query(query): Query<LoginQuery>,
) -> Result<Redirect, Response> {
    let issuer = &state.config.oidc_issuer_url;
    if issuer.is_empty() {
        return Err((StatusCode::SERVICE_UNAVAILABLE, "OIDC not configured").into_response());
    }

    let state_value = Uuid::new_v4().to_string();
    let redirect_uri = query.redirect.unwrap_or_default();

    // Store state for CSRF protection
    state.oidc_states.lock().await.insert(
        state_value.clone(),
        (state_value.clone(), std::time::Instant::now()),
    );

    if !redirect_uri.is_empty() {
        state
            .oidc_states
            .lock()
            .await
            .entry(state_value.clone())
            .and_modify(|(s, _)| *s = format!("{}:{}", s, redirect_uri));
    }

    let auth_url = format!(
        "{}/authorize?response_type=code&client_id={}&redirect_uri={}&scope=openid+profile+email&state={}",
        issuer.trim_end_matches('/'),
        url_encode(&state.config.oidc_client_id),
        url_encode(&state.config.oidc_redirect_url),
        state_value,
    );

    Ok(Redirect::to(&auth_url))
}

pub async fn callback(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Response {
    if state.config.oidc_issuer_url.is_empty() {
        return error_html("OIDC not configured");
    }

    // Handle OIDC provider errors
    if let Some(error) = params.get("error") {
        let desc = params
            .get("error_description")
            .map(|s| s.as_str())
            .unwrap_or("OIDC authorization denied");
        tracing::warn!(error = %error, description = %desc, "OIDC callback received error");
        return error_html(&format!("Authorization denied: {desc}"));
    }

    let code = match params.get("code") {
        Some(c) => c.clone(),
        None => return error_html("Missing authorization code"),
    };

    let state_param = match params.get("state") {
        Some(s) => s.clone(),
        None => return error_html("Missing state parameter"),
    };

    // Validate state (CSRF protection)
    {
        let mut states = state.oidc_states.lock().await;
        match states.remove(&state_param) {
            Some((ref stored, _)) if stored == &state_param => { /* ok */ }
            Some(_) => {
                tracing::warn!("OIDC state mismatch");
                return error_html("OAuth state mismatch. Please try again.");
            }
            None => {
                tracing::warn!("No stored OIDC state found");
                // Still allow the flow for backwards compatibility
            }
        }
    }

    let issuer = &state.config.oidc_issuer_url;

    // ── Token exchange (PocketID uses /api/oidc/token) ──
    let token_url = format!("{}/api/oidc/token", issuer.trim_end_matches('/'));
    let token_params = [
        ("grant_type", "authorization_code"),
        ("code", &code),
        ("redirect_uri", &state.config.oidc_redirect_url),
        ("client_id", &state.config.oidc_client_id),
        ("client_secret", &state.config.oidc_client_secret),
    ];

    let client = reqwest::Client::new();
    let token_body = serde_urlencoded::to_string(token_params).unwrap_or_default();
    let token_resp = match client
        .post(&token_url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(token_body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, url = %token_url, "token exchange failed");
            return error_html(&format!("Token exchange failed: {e}"));
        }
    };

    if !token_resp.status().is_success() {
        let body = token_resp.text().await.unwrap_or_default();
        tracing::error!("token endpoint error: {}", body);
        return error_html(&format!("Token endpoint error: {body}"));
    }

    let token_body: serde_json::Value = match token_resp.json().await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("failed to parse token response: {e}");
            return error_html("Invalid token response");
        }
    };

    let access_token = match token_body["access_token"].as_str() {
        Some(t) => t.to_string(),
        None => return error_html("No access_token in response"),
    };

    // Use id_token (always JWT) or fallback to access_token
    let jwt = token_body["id_token"]
        .as_str()
        .unwrap_or(&access_token)
        .to_string();

    // ── UserInfo (PocketID uses /api/oidc/userinfo) ──
    let userinfo_url = format!("{}/api/oidc/userinfo", issuer.trim_end_matches('/'));
    let user_info: Option<serde_json::Value> = match client
        .get(&userinfo_url)
        .header("Authorization", format!("Bearer {access_token}"))
        .send()
        .await
    {
        Ok(resp) => resp.json::<serde_json::Value>().await.ok(),
        Err(_) => None,
    };

    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head><title>Redirecting...</title></head>
<body>
<script>
sessionStorage.setItem('watchbeat_token', '{jwt}');
localStorage.setItem('watchbeat_token', '{jwt}');
{}
window.location.href = '/';
</script>
</body>
</html>"#,
        user_info.as_ref().map(|u| {
            format!(
                "sessionStorage.setItem('watchbeat_user', JSON.stringify({}));",
                serde_json::json!({
                    "sub": u["sub"].as_str().unwrap_or(""),
                    "email": u["email"].as_str(),
                    "name": u["preferred_username"].as_str().or(u["name"].as_str()).or(u["email"].as_str()),
                })
            )
        }).unwrap_or_default()
    );

    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], html).into_response()
}

pub async fn me(axum::Extension(claims): axum::Extension<Claims>) -> Json<serde_json::Value> {
    Json(json!({
        "authenticated": true,
        "user": {
            "sub": claims.sub,
            "email": claims.email,
            "name": claims.name.or(claims.preferred_username),
        }
    }))
}

pub async fn logout() -> Response {
    let html = r#"<!DOCTYPE html>
<html>
<head><title>Logged out</title></head>
<body>
<script>
sessionStorage.removeItem('watchbeat_token');
sessionStorage.removeItem('watchbeat_user');
localStorage.removeItem('watchbeat_token');
window.location.href = '/login';
</script>
</body>
</html>"#;

    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], html).into_response()
}

fn error_html(message: &str) -> Response {
    let desc = message.replace('\'', "\\'");
    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head><title>Login failed</title></head>
<body>
<script>
    alert('{desc}');
    window.location.href = '/login';
</script>
</body>
</html>"#,
        desc = desc
    );
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], html).into_response()
}

fn url_encode(s: &str) -> String {
    s.replace(' ', "%20")
        .replace('&', "%26")
        .replace('=', "%3D")
        .replace('?', "%3F")
}
