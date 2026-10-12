use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AppState;
use crate::models::{MonitorSummary, StatusPage};

#[derive(Deserialize)]
pub struct StatusPageRequest {
    pub slug: String,
    pub title: String,
    pub description: Option<String>,
    pub monitors: Vec<String>,
    pub public: bool,
}

pub async fn list(State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, String> {
    let pages = state
        .db
        .list_status_pages()
        .await
        .map_err(|e| e.to_string())?;
    Ok(Json(serde_json::json!({ "status_pages": pages })))
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Json(req): Json<StatusPageRequest>,
) -> Result<Json<serde_json::Value>, String> {
    // Check title uniqueness
    if !state
        .db
        .check_name_unique("status_pages", "title", &req.title, None)
        .await
        .map_err(|e| e.to_string())?
    {
        return Err(format!(
            "Ya existe una status page con el título '{}'",
            req.title
        ));
    }

    let now = chrono::Utc::now().to_rfc3339();
    let page = StatusPage {
        id: Uuid::new_v4().to_string(),
        slug: req.slug,
        title: req.title,
        description: req.description,
        monitors: req.monitors,
        public: req.public,
        created_at: now.clone(),
        updated_at: now,
    };

    state
        .db
        .upsert_status_page(&page.id, &page)
        .await
        .map_err(|e| e.to_string())?;

    Ok(Json(serde_json::json!(page)))
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<StatusPageRequest>,
) -> Result<Json<serde_json::Value>, String> {
    let existing = state
        .db
        .get_status_page(&id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Status page not found")?;

    // Check title uniqueness if title is being changed
    if req.title != existing.title
        && !state
            .db
            .check_name_unique("status_pages", "title", &req.title, Some(&id))
            .await
            .map_err(|e| e.to_string())?
    {
        return Err(format!(
            "Ya existe una status page con el título '{}'",
            req.title
        ));
    }

    let now = chrono::Utc::now().to_rfc3339();
    let page = StatusPage {
        id: existing.id,
        slug: req.slug,
        title: req.title,
        description: req.description,
        monitors: req.monitors,
        public: req.public,
        created_at: existing.created_at,
        updated_at: now,
    };

    state
        .db
        .upsert_status_page(&id, &page)
        .await
        .map_err(|e| e.to_string())?;

    Ok(Json(serde_json::json!(page)))
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, String> {
    let deleted = state
        .db
        .delete_status_page(&id)
        .await
        .map_err(|e| e.to_string())?;
    if !deleted {
        return Err("Status page not found".into());
    }
    Ok(Json(serde_json::json!({"deleted": true})))
}

// ───── Public status page (no auth) ─────

pub async fn public_page(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> Result<axum::response::Response, String> {
    let page = state
        .db
        .get_status_page_by_slug(&slug)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Status page not found")?;

    if !page.public {
        return Err("Status page not public".into());
    }

    // Build monitor summaries for the page
    let summaries = state
        .db
        .get_monitor_summaries()
        .await
        .map_err(|e| e.to_string())?;

    let page_monitors: Vec<_> = summaries
        .into_iter()
        .filter(|s| page.monitors.contains(&s.id))
        .collect();

    let overall = overall_status(&page_monitors);

    let html = render_status_page(&page, &page_monitors, overall);

    Ok(axum::response::Response::builder()
        .header("Content-Type", "text/html; charset=utf-8")
        .body(axum::body::Body::from(html))
        .unwrap())
}

fn render_status_page(
    page: &StatusPage,
    monitors: &[crate::models::MonitorSummary],
    overall: &str,
) -> String {
    let color = match overall {
        "up" => "#22c55e",
        "down" => "#ef4444",
        _ => "#f59e0b",
    };
    let label = match overall {
        "up" => "Todos los sistemas operativos",
        "down" => "Incidente detectado",
        _ => "Estado parcial",
    };

    let mut rows = String::new();
    for m in monitors {
        let (dot, status_text) = match m.last_status.as_deref() {
            Some("up") => ("#22c55e", "Operativo"),
            Some("down") => ("#ef4444", "Caído"),
            _ => ("#f59e0b", "Sin datos"),
        };
        let uptime = m
            .uptime_30d
            .map(|u| format!("{:.2}%", u))
            .unwrap_or_else(|| "—".into());
        rows.push_str(&format!(
            r#"<tr><td><span class="dot" style="background:{dot}"></span>{}</td>
               <td>{status_text}</td><td>{uptime}</td></tr>"#,
            m.name
        ));
    }

    format!(
        r#"<!DOCTYPE html>
<html lang="es">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; margin: 0; background: #f5f5f5; color: #111; }}
.container {{ max-width: 700px; margin: 40px auto; padding: 0 20px; }}
.banner {{ background: {color}; color: #fff; border-radius: 8px; padding: 20px 24px; margin-bottom: 24px; }}
.banner h1 {{ margin: 0 0 8px; font-size: 24px; }}
.banner p {{ margin: 0; opacity: 0.9; }}
table {{ width: 100%; border-collapse: collapse; background: #fff; border-radius: 8px; overflow: hidden; box-shadow: 0 1px 3px rgba(0,0,0,0.1); }}
th, td {{ padding: 12px 16px; text-align: left; border-bottom: 1px solid #eee; }}
th {{ background: #fafafa; font-size: 12px; text-transform: uppercase; color: #666; }}
.dot {{ display: inline-block; width: 10px; height: 10px; border-radius: 50%; margin-right: 8px; }}
.footer {{ margin-top: 16px; text-align: center; color: #999; font-size: 12px; }}
</style>
</head>
<body>
<div class="container">
  <div class="banner"><h1>{title}</h1><p>{description}</p></div>
  <div class="banner" style="background:{color}"><h1>{label}</h1></div>
  <table>
    <tr><th>Servicio</th><th>Estado</th><th>Uptime 30d</th></tr>
    {rows}
  </table>
  <div class="footer">Generado por WatchBeat</div>
</div>
</body>
</html>"#,
        title = page.title,
        description = page.description.clone().unwrap_or_default(),
    )
}

/// Query param extraído para autenticar con API key en el endpoint público JSON.
#[derive(Deserialize)]
pub struct ApiKeyQuery {
    pub api_key: Option<String>,
}

// ───── Public JSON endpoint (no auth) — `GET /status/{slug}/api.json` ─────
// Consumido por gethomepage (widget `customapi`) y otros agregadores.
// API key opcional: si el setting `homepage_api_key` está vacío → público;
// si está definido, la request debe presentarlo vía header `x-api-key` o
// query `api_key`. Una página inexistente o privada nunca se revela (404),
// aunque la key sea correcta.

pub async fn public_page_json(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    headers: HeaderMap,
    Query(query): Query<ApiKeyQuery>,
) -> Result<axum::response::Response, String> {
    let page = state
        .db
        .get_status_page_by_slug(&slug)
        .await
        .map_err(|e| e.to_string())?;

    // No revelar páginas inexistentes ni privadas → 404.
    if !page_access(page.as_ref()) {
        return Ok(axum::response::Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header("Content-Type", "text/plain")
            .body(axum::body::Body::from("Not found"))
            .unwrap());
    }
    // `page_access` == true implica que `page` es `Some` y pública (invariante
    // recién guardado): la rama `else` sería inalcanzable, así que se descarta.
    let page = page
        .as_ref()
        .expect("page_access() == true guarantees Some(&public StatusPage)");

    // API key opcional configurada en settings.
    let api_key = state
        .db
        .get_setting("homepage_api_key")
        .await
        .map_err(|e| e.to_string())?
        .unwrap_or_default();

    // Extraer key de header `x-api-key` o query `api_key` (header prioritario).
    let provided = extract_api_key(&headers, &query);

    if !api_key_authorized(&api_key, provided.as_deref()) {
        return Ok(axum::response::Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header("Content-Type", "text/plain")
            .body(axum::body::Body::from("Unauthorized"))
            .unwrap());
    }

    let summaries = state
        .db
        .get_monitor_summaries()
        .await
        .map_err(|e| e.to_string())?;

    // `build_public_page_json` filtra por `page.monitors` (única fuente de
    // filtrado). Para el estado global sí calculamos la lista ya filtrada.
    let page_monitors: Vec<MonitorSummary> = summaries
        .iter()
        .filter(|s| page.monitors.contains(&s.id))
        .cloned()
        .collect();

    let overall = overall_status(&page_monitors);
    let json = build_public_page_json(page, &summaries, overall);

    Ok(axum::response::Response::builder()
        .header("Content-Type", "application/json")
        .body(axum::body::Body::from(json.to_string()))
        .unwrap())
}

// ───── Public JSON endpoint helpers (pure, testable) ─────
// Contrato del endpoint público `GET /status/{slug}/api.json` (ver spec
// `homepage-api`). Estas funciones puras se combinan desde el handler
// `public_page_json` (por implementar en GREEN) y las testea la suite
// `#[cfg(test)]` — diseño acorde al patrón `sse_payload` de routes/mod.rs.

/// Calcula el estado global de los monitores de la página con la MISMA lógica
/// que la página HTML: `"down"` si alguno está caído, `"up"` si todos están up
/// y hay al menos uno, sino `"unknown"`.
///
fn overall_status(monitors: &[MonitorSummary]) -> &'static str {
    if monitors
        .iter()
        .any(|m| m.last_status.as_deref() == Some("down"))
    {
        "down"
    } else if monitors
        .iter()
        .all(|m| m.last_status.as_deref() == Some("up"))
        && !monitors.is_empty()
    {
        "up"
    } else {
        "unknown"
    }
}

/// Decide si una página puede servirse en el endpoint público JSON.
/// Devuelve `true` solo si la página existe Y es `public: true`. Una página
/// inexistente o privada nunca se revela (404), aunque se presente API key
/// correcta.
///
fn page_access(page: Option<&StatusPage>) -> bool {
    page.map(|p| p.public).unwrap_or(false)
}

/// Extrae la API key presentada por la request: header `x-api-key` con
/// prioridad sobre el query `api_key`. Función pura, testable.
///
fn extract_api_key(headers: &HeaderMap, query: &ApiKeyQuery) -> Option<String> {
    headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| query.api_key.clone())
}

/// Autoriza una request contra el API key opcional configurado
/// (`homepage_api_key`). Si el setting está vacío/ausente → público.
/// Si está definido, la request debe presentar la key que coincide con
/// `provided` (extraído del header `x-api-key` o del query `api_key`).
///
/// La comparación es constante-tiempo: se compara el SHA-256 de ambas
/// cadenas con `subtle::ConstantTimeEq`, de modo que el timing no depende de
/// la longitud ni de los bytes de la key.
///
fn api_key_authorized(configured: &str, provided: Option<&str>) -> bool {
    if configured.is_empty() {
        return true;
    }
    let Some(provided) = provided else {
        return false;
    };

    use sha2::{Digest, Sha256};
    use subtle::ConstantTimeEq;

    let configured_digest = Sha256::digest(configured.as_bytes());
    let provided_digest = Sha256::digest(provided.as_bytes());
    bool::from(configured_digest.ct_eq(&provided_digest))
}

/// Construye el payload JSON de la respuesta 200 del endpoint público.
/// Solo incluye los monitores presentes en `page.monitors`. Cada entrada lleva
/// `id`, `name`, `target`, `status`, `response_time_ms`, `uptime_7d`,
/// `uptime_30d`, `last_checked_at`. `status` y `updated_at` a nivel global.
///
fn build_public_page_json(
    page: &StatusPage,
    monitors: &[MonitorSummary],
    overall: &str,
) -> serde_json::Value {
    let monitor_entries: Vec<serde_json::Value> = monitors
        .iter()
        .filter(|s| page.monitors.contains(&s.id))
        .map(|s| {
            serde_json::json!({
                "id": s.id,
                "name": s.name,
                "target": s.target,
                "status": s.last_status,
                "response_time_ms": s.last_response_time_ms,
                "uptime_7d": s.uptime_7d,
                "uptime_30d": s.uptime_30d,
                "last_checked_at": s.last_checked_at,
            })
        })
        .collect();

    serde_json::json!({
        "status": overall,
        "title": page.title,
        "updated_at": page.updated_at,
        "monitors": monitor_entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construye un `MonitorSummary` mínimo para las pruebas.
    fn summary(id: &str, name: &str, target: &str, last_status: Option<&str>) -> MonitorSummary {
        MonitorSummary {
            id: id.into(),
            name: name.into(),
            monitor_type: "http".into(),
            target: target.into(),
            enabled: true,
            last_status: last_status.map(|s| s.to_string()),
            last_response_time_ms: Some(120),
            last_checked_at: Some("2026-10-12T10:00:00+00:00".into()),
            uptime_7d: Some(99.5),
            uptime_30d: Some(98.2),
            token: None,
            grace_seconds: None,
            last_seen_at: None,
        }
    }

    fn page(public: bool) -> StatusPage {
        StatusPage {
            id: "sp-1".into(),
            slug: "status".into(),
            title: "Mi Estado".into(),
            description: None,
            monitors: vec!["m-1".into(), "m-2".into()],
            public,
            created_at: "2026-01-01T00:00:00+00:00".into(),
            updated_at: "2026-10-12T10:05:00+00:00".into(),
        }
    }

    // ── Scenario 1: Consulta de una status page pública ──

    #[test]
    fn public_page_builds_full_json_shape() {
        let monitors = vec![
            summary("m-1", "Web", "https://a.example", Some("up")),
            summary("m-2", "API", "https://b.example", Some("up")),
            summary("m-3", "Otro", "https://c.example", Some("up")), // no listada
        ];
        let overall = overall_status(&monitors);
        let json = build_public_page_json(&page(true), &monitors, overall);

        assert_eq!(json["status"], "up", "estado global debe ser 'up'");
        assert_eq!(json["title"], "Mi Estado");
        assert!(json["updated_at"].is_string());
        assert_eq!(
            json["monitors"].as_array().map(|a| a.len()),
            Some(2),
            "solo deben incluirse los monitores de page.monitors"
        );
        // Cada monitor debe exponer el contrato completo
        for m in json["monitors"].as_array().unwrap() {
            assert!(m.get("id").is_some());
            assert!(m.get("name").is_some());
            assert!(m.get("target").is_some());
            assert!(m.get("status").is_some());
            assert!(m.get("response_time_ms").is_some());
            assert!(m.get("uptime_7d").is_some());
            assert!(m.get("uptime_30d").is_some());
            assert!(m.get("last_checked_at").is_some());
        }
        let ids: Vec<&str> = json["monitors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, vec!["m-1", "m-2"]);
    }

    // ── Scenario 2: Status page no existe ──

    #[test]
    fn nonexistent_slug_is_not_served() {
        // No hay página → `page_access` debe negar el acceso (404).
        assert!(!page_access(None), "un slug inexistente no debe servirse");
    }

    // ── Scenario 3: Status page privada ──

    #[test]
    fn private_page_is_not_served() {
        assert!(
            !page_access(Some(&page(false))),
            "una página privada no debe revelarse (404)"
        );
    }

    #[test]
    fn public_page_is_served() {
        assert!(
            page_access(Some(&page(true))),
            "una página pública debe servirse"
        );
    }

    // ── Scenario 4: Cálculo del estado global ──

    #[test]
    fn overall_status_down_when_any_down() {
        let monitors = vec![
            summary("m-1", "A", "t1", Some("up")),
            summary("m-2", "B", "t2", Some("down")),
        ];
        assert_eq!(overall_status(&monitors), "down");
    }

    #[test]
    fn overall_status_up_when_all_up() {
        let monitors = vec![
            summary("m-1", "A", "t1", Some("up")),
            summary("m-2", "B", "t2", Some("up")),
        ];
        assert_eq!(overall_status(&monitors), "up");
    }

    #[test]
    fn overall_status_unknown_when_no_known_status() {
        let monitors = vec![
            summary("m-1", "A", "t1", None),
            summary("m-2", "B", "t2", None),
        ];
        assert_eq!(overall_status(&monitors), "unknown");
    }

    // ── Scenario 5: API key no configurada → público ──

    #[test]
    fn no_api_key_configured_allows_access() {
        assert!(
            api_key_authorized("", None),
            "sin api key configurada el endpoint debe ser público"
        );
    }

    // ── Scenario 6: API key configurada y presente ──

    #[test]
    fn configured_key_present_authorizes() {
        assert!(api_key_authorized("secret", Some("secret")));
        // El header `x-api-key` y el query `api_key` se normalizan ambos
        // en `extract_api_key` antes de comparar: header con prioridad.
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", "secret".parse().unwrap());
        let query = ApiKeyQuery { api_key: None };
        let provided = extract_api_key(&headers, &query);
        assert_eq!(
            provided.as_deref(),
            Some("secret"),
            "header x-api-key se extrae"
        );
        assert!(
            api_key_authorized("secret", provided.as_deref()),
            "key vía header autoriza"
        );

        // Query `api_key` también se extrae y autoriza.
        let headers = HeaderMap::new();
        let query = ApiKeyQuery {
            api_key: Some("secret".into()),
        };
        let provided = extract_api_key(&headers, &query);
        assert_eq!(
            provided.as_deref(),
            Some("secret"),
            "query api_key se extrae"
        );
        assert!(
            api_key_authorized("secret", provided.as_deref()),
            "key vía query autoriza"
        );
    }

    // ── Scenario 6b: extracción pura de la API key ──

    #[test]
    fn extract_api_key_header_takes_priority_over_query() {
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", "header-key".parse().unwrap());
        let query = ApiKeyQuery {
            api_key: Some("query-key".into()),
        };
        assert_eq!(
            extract_api_key(&headers, &query).as_deref(),
            Some("header-key"),
            "header x-api-key tiene prioridad sobre el query"
        );
    }

    #[test]
    fn extract_api_key_falls_back_to_query() {
        let headers = HeaderMap::new();
        let query = ApiKeyQuery {
            api_key: Some("query-key".into()),
        };
        assert_eq!(
            extract_api_key(&headers, &query).as_deref(),
            Some("query-key"),
            "sin header se usa el query api_key"
        );
    }

    #[test]
    fn extract_api_key_none_when_absent() {
        let headers = HeaderMap::new();
        let query = ApiKeyQuery { api_key: None };
        assert_eq!(
            extract_api_key(&headers, &query),
            None,
            "sin header ni query no hay key"
        );
    }

    // ── Scenario 7: API key configurada y ausente/incorrecta ──

    #[test]
    fn configured_key_missing_rejects() {
        assert!(!api_key_authorized("secret", None));
    }

    #[test]
    fn configured_key_wrong_rejects() {
        assert!(!api_key_authorized("secret", Some("wrong")));
    }

    // ── Scenario 8: API key no exime de page pública ──

    #[test]
    fn correct_key_does_not_exempt_private_page() {
        // Aunque la key fuera correcta, una página privada debe seguir siendo 404.
        assert!(
            !page_access(Some(&page(false))),
            "la api key no exime de public:true"
        );
        // Y la combinación handler (acceso negado aunque key válida) no se sirve:
        let served =
            api_key_authorized("secret", Some("secret")) && page_access(Some(&page(false)));
        assert!(
            !served,
            "page privada + key correcta debe seguir devolviendo 404"
        );
    }
}
