// ───── Protected CRUD routes for API keys (feature: api-keys) ─────

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AppState;
use crate::models::ApiKey;

#[derive(Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
}

#[derive(Deserialize)]
pub struct UpdateApiKeyRequest {
    pub name: String,
}

type ApiResult = Result<(StatusCode, Json<serde_json::Value>), String>;

/// Detecta una violación del constraint UNIQUE de SQLite (race condition
/// sobre `name` o `api_key`). Los métodos de DB devuelven `anyhow::Error` que
/// envuelve el `sqlx::Error` original; se hace downcast al error de base de
/// datos y se comprueba el kind/código de violación de unicidad.
fn is_unique_violation(e: &anyhow::Error) -> bool {
    e.downcast_ref::<sqlx::Error>().is_some_and(|sqlx_err| {
        matches!(sqlx_err, sqlx::Error::Database(db)
                if db.kind() == sqlx::error::ErrorKind::UniqueViolation
                  || db.code().as_deref() == Some("2067"))
    })
}

pub async fn list(State(state): State<Arc<AppState>>) -> ApiResult {
    let keys = state.db.list_api_keys().await.map_err(|e| e.to_string())?;
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "api_keys": keys })),
    ))
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateApiKeyRequest>,
) -> ApiResult {
    let name = req.name.trim();
    if name.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "El nombre no puede estar vacío" })),
        ));
    }

    if !state
        .db
        .api_key_name_unique(name, None)
        .await
        .map_err(|e| e.to_string())?
    {
        return Ok((
            StatusCode::CONFLICT,
            Json(
                serde_json::json!({ "error": format!("Ya existe una API key con el nombre '{}'", name) }),
            ),
        ));
    }

    let now = chrono::Utc::now().to_rfc3339();
    let key = ApiKey {
        id: Uuid::new_v4().to_string(),
        name: name.to_string(),
        api_key: Uuid::new_v4().to_string(),
        created_at: now.clone(),
        updated_at: now,
    };

    let res = state
        .db
        .create_api_key(&key.id, &key.name, &key.api_key)
        .await;
    match res {
        Ok(()) => Ok((StatusCode::CREATED, Json(serde_json::json!(key)))),
        Err(e) if is_unique_violation(&e) => Ok((
            StatusCode::CONFLICT,
            Json(
                serde_json::json!({ "error": format!("Ya existe una API key con el nombre '{}'", name) }),
            ),
        )),
        Err(e) => Err(e.to_string()),
    }
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateApiKeyRequest>,
) -> ApiResult {
    let existing = state.db.get_api_key(&id).await.map_err(|e| e.to_string())?;
    let Some(existing) = existing else {
        return Ok((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "API key not found" })),
        ));
    };

    let name = req.name.trim();
    if name.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "El nombre no puede estar vacío" })),
        ));
    }

    if name != existing.name
        && !state
            .db
            .api_key_name_unique(name, Some(&id))
            .await
            .map_err(|e| e.to_string())?
    {
        return Ok((
            StatusCode::CONFLICT,
            Json(
                serde_json::json!({ "error": format!("Ya existe una API key con el nombre '{}'", name) }),
            ),
        ));
    }

    match state.db.update_api_key_name(&id, name).await {
        Ok(true) => {}
        Ok(false) => {
            return Ok((
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "API key not found" })),
            ));
        }
        // Race condition: otro request renombró a este mismo `name` entre el
        // pre-check y el UPDATE → el UNIQUE dispara y debe ser un 409.
        Err(e) if is_unique_violation(&e) => {
            return Ok((
                StatusCode::CONFLICT,
                Json(
                    serde_json::json!({ "error": format!("Ya existe una API key con el nombre '{}'", name) }),
                ),
            ));
        }
        Err(e) => return Err(e.to_string()),
    }

    let updated = state.db.get_api_key(&id).await.map_err(|e| e.to_string())?;

    Ok((StatusCode::OK, Json(serde_json::json!(updated))))
}

pub async fn delete(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> ApiResult {
    let deleted = state
        .db
        .delete_api_key(&id)
        .await
        .map_err(|e| e.to_string())?;
    if !deleted {
        return Ok((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "API key not found" })),
        ));
    }
    Ok((StatusCode::OK, Json(serde_json::json!({ "deleted": true }))))
}
