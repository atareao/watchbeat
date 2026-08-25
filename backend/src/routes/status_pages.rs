use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AppState;
use crate::models::StatusPage;

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

    let overall = if page_monitors
        .iter()
        .any(|m| m.last_status.as_deref() == Some("down"))
    {
        "down"
    } else if page_monitors
        .iter()
        .all(|m| m.last_status.as_deref() == Some("up"))
    {
        "up"
    } else {
        "unknown"
    };

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
  <div class="footer">Generado por Vigilatrs</div>
</div>
</body>
</html>"#,
        title = page.title,
        description = page.description.clone().unwrap_or_default(),
    )
}
