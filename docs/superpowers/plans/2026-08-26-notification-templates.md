# Notification Templates with minijinja Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add configurable notification templates using minijinja for DOWN, LATENCY, and UP events, with per-monitor latency thresholds.

**Architecture:** A new `template.rs` module renders Jinja templates with monitor/check variables. The scheduler detects latency threshold breaches and status transitions, renders the appropriate template, and passes the rendered message to each notifier. Each notifier receives the pre-rendered message instead of building its own.

**Tech Stack:** Rust, minijinja, Axum, SQLite, React 19, Ant Design 6

## Global Constraints

- All existing tests must pass
- `cargo clippy -- -D warnings` must pass
- `cargo fmt --check` must pass
- New columns added via ALTER TABLE for backward compatibility
- Default templates provided for DOWN, LATENCY, UP
- Template variables documented for users

---

### Task 1: Add minijinja dependency and create template module

**Files:**
- Modify: `backend/Cargo.toml`
- Create: `backend/src/template.rs`
- Modify: `backend/src/lib.rs`

**Interfaces:**
- Consumes: `Monitor`, `CheckResult` models
- Produces: `TemplateEngine` struct with `render()` method, `TemplateContext` builder

- [ ] **Step 1: Add minijinja to Cargo.toml**

Add after the `regex` dependency line:
```toml
minijinja = { version = "2.6", features = ["builtins", "json"] }
```

- [ ] **Step 2: Create template.rs module**

```rust
use minijinja::{Environment, Value};
use crate::models::{Monitor, CheckResult};

/// Build the template context for rendering notification messages.
pub struct TemplateContext;

impl TemplateContext {
    pub fn for_down(monitor: &Monitor, check: &CheckResult, previous_status: &str) -> Value {
        let mut ctx = serde_json::Map::new();
        Self::add_common(&mut ctx, monitor, check, previous_status);
        Value::from(ctx)
    }

    pub fn for_latency(monitor: &Monitor, check: &CheckResult, threshold_ms: i64) -> Value {
        let mut ctx = serde_json::Map::new();
        Self::add_common(&mut ctx, monitor, check, "up");
        ctx.insert("latency_threshold_ms".into(), Value::from(threshold_ms));
        ctx.insert("latency_exceeded_by_ms".into(), Value::from(check.response_time_ms - threshold_ms));
        Value::from(ctx)
    }

    pub fn for_up(monitor: &Monitor, check: &CheckResult, previous_status: &str) -> Value {
        let mut ctx = serde_json::Map::new();
        Self::add_common(&mut ctx, monitor, check, previous_status);
        Value::from(ctx)
    }

    fn add_common(ctx: &mut serde_json::Map<String, serde_json::Value>, monitor: &Monitor, check: &CheckResult, previous_status: &str) {
        ctx.insert("monitor_name".into(), serde_json::Value::String(monitor.name.clone()));
        ctx.insert("monitor_type".into(), serde_json::Value::String(monitor.monitor_type.clone()));
        ctx.insert("target".into(), serde_json::Value::String(monitor.target.clone()));
        ctx.insert("status".into(), serde_json::Value::String(check.status.clone()));
        ctx.insert("previous_status".into(), serde_json::Value::String(previous_status.to_string()));
        ctx.insert("response_time_ms".into(), serde_json::Value::Number(serde_json::Number::from(check.response_time_ms)));
        ctx.insert("error_message".into(), check.error_message.as_ref().map(|s| serde_json::Value::String(s.clone())).unwrap_or(serde_json::Value::Null));
        ctx.insert("checked_at".into(), serde_json::Value::String(check.checked_at.clone()));
        ctx.insert("status_code".into(), check.status_code.map(|c| serde_json::Value::Number(serde_json::Number::from(c as u64))).unwrap_or(serde_json::Value::Null));
        ctx.insert("tags".into(), serde_json::Value::Array(monitor.tags.iter().map(|t| serde_json::Value::String(t.clone())).collect()));
    }
}

/// Render a Jinja2 template string with the given context.
/// Returns the rendered string, or a fallback message if rendering fails.
pub fn render_template(template: &str, context: &Value) -> String {
    let mut env = Environment::new();
    env.add_template("message", template).ok();
    match env.get_template("message") {
        Ok(tmpl) => match tmpl.render(context) {
            Ok(result) => result,
            Err(e) => {
                tracing::warn!("Template rendering failed: {}", e);
                format!("[Template error: {}]", e)
            }
        },
        Err(e) => {
            tracing::warn!("Template parse failed: {}", e);
            format!("[Template error: {}]", e)
        }
    }
}

/// Default templates for each notification type.
pub mod defaults {
    pub const DOWN: &str = "🔴 {{ monitor_name }} — {{ target }}\nStatus: {{ status }}\nResponse: {{ response_time_ms }}ms\n{{ error_message }}";
    pub const LATENCY: &str = "🟡 {{ monitor_name }} — {{ target }}\nHigh latency: {{ response_time_ms }}ms (threshold: {{ latency_threshold_ms }}ms, exceeded by {{ latency_exceeded_by_ms }}ms)";
    pub const UP: &str = "🟢 {{ monitor_name }} — {{ target }}\nRecovered — Status: {{ status }}\nResponse: {{ response_time_ms }}ms";
}
```

- [ ] **Step 3: Register template module in lib.rs**

Add after `pub mod notifier;`:
```rust
pub mod template;
```

- [ ] **Step 4: Verify compilation**

Run: `cd backend && cargo check 2>&1 | head -20`
Expected: Compilation succeeds (may warn about unused module — that's fine)

---

### Task 2: Add new fields to Monitor model

**Files:**
- Modify: `backend/src/models.rs`

**Interfaces:**
- Produces: Updated `Monitor` struct with `latency_threshold_ms`, `message_template_down`, `message_template_latency`, `message_template_up` fields
- Consumed by: db.rs, routes/monitors.rs, main.rs, template.rs

- [ ] **Step 1: Add fields to Monitor struct**

Add after `failed_checks`:
```rust
    pub latency_threshold_ms: Option<i64>,
    pub message_template_down: Option<String>,
    pub message_template_latency: Option<String>,
    pub message_template_up: Option<String>,
```

- [ ] **Step 2: Add fields to MonitorRow struct**

Add after `failed_checks`:
```rust
    pub latency_threshold_ms: Option<i64>,
    pub message_template_down: Option<String>,
    pub message_template_latency: Option<String>,
    pub message_template_up: Option<String>,
```

- [ ] **Step 3: Update MonitorRow → Monitor conversion**

In the `From<MonitorRow> for Monitor` impl, add after `failed_checks`:
```rust
            latency_threshold_ms: row.latency_threshold_ms,
            message_template_down: row.message_template_down,
            message_template_latency: row.message_template_latency,
            message_template_up: row.message_template_up,
```

- [ ] **Step 4: Update test helpers**

In `make_test_monitor` (in checker/mod.rs tests and notifier/telegram.rs tests), add the new fields:
```rust
            latency_threshold_ms: None,
            message_template_down: None,
            message_template_latency: None,
            message_template_up: None,
```

- [ ] **Step 5: Update models tests**

In `test_monitor_from_row`, add to the MonitorRow:
```rust
            latency_threshold_ms: None,
            message_template_down: None,
            message_template_latency: None,
            message_template_up: None,
```

Same for `test_monitor_from_row_disabled` and `test_monitor_from_row_invalid_config_json`.

- [ ] **Step 6: Verify compilation**

Run: `cd backend && cargo check 2>&1 | head -30`
Expected: Compilation errors about missing fields in all places that construct Monitor/MonitorRow

---

### Task 3: Update database schema

**Files:**
- Modify: `backend/src/db.rs`

- [ ] **Step 1: Add new columns to CREATE TABLE**

In the monitors CREATE TABLE, add after `failed_checks`:
```sql
                latency_threshold_ms INTEGER,
                message_template_down TEXT,
                message_template_latency TEXT,
                message_template_up TEXT,
```

- [ ] **Step 2: Add ALTER TABLE for existing databases**

After the existing ALTER TABLE blocks, add:
```rust
        let _ = sqlx::raw_sql(
            "ALTER TABLE monitors ADD COLUMN latency_threshold_ms INTEGER",
        )
        .execute(&pool)
        .await;
        let _ = sqlx::raw_sql(
            "ALTER TABLE monitors ADD COLUMN message_template_down TEXT",
        )
        .execute(&pool)
        .await;
        let _ = sqlx::raw_sql(
            "ALTER TABLE monitors ADD COLUMN message_template_latency TEXT",
        )
        .execute(&pool)
        .await;
        let _ = sqlx::raw_sql(
            "ALTER TABLE monitors ADD COLUMN message_template_up TEXT",
        )
        .execute(&pool)
        .await;
```

- [ ] **Step 3: Update list_monitors query**

Add the new columns to the SELECT:
```sql
             SELECT id, name, monitor_type, target, config_json, interval_seconds, \
              timeout_seconds, enabled, notifier_id, confirmations_required, failed_checks, \
              latency_threshold_ms, message_template_down, message_template_latency, message_template_up, \
              tags, \
              created_at, updated_at FROM monitors ORDER BY name
```

- [ ] **Step 4: Update get_monitor query**

Same change as list_monitors.

- [ ] **Step 5: Update create_monitor query**

Add the new columns to INSERT:
```sql
             INSERT INTO monitors (id, name, monitor_type, target, config_json, interval_seconds, \
              timeout_seconds, enabled, notifier_id, confirmations_required, failed_checks, \
              latency_threshold_ms, message_template_down, message_template_latency, message_template_up, \
              tags, created_at, updated_at) \
              VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
```

Add bindings after `failed_checks`:
```rust
        .bind(monitor.latency_threshold_ms).bind(&monitor.message_template_down)
        .bind(&monitor.message_template_latency).bind(&monitor.message_template_up)
```

- [ ] **Step 6: Update update_monitor query**

Add to SET clause:
```sql
              latency_threshold_ms=?, message_template_down=?, message_template_latency=?, message_template_up=?, \
```

Add bindings after `failed_checks`:
```rust
        .bind(monitor.latency_threshold_ms)
        .bind(&monitor.message_template_down)
        .bind(&monitor.message_template_latency)
        .bind(&monitor.message_template_up)
```

- [ ] **Step 7: Verify compilation**

Run: `cd backend && cargo check 2>&1 | head -30`
Expected: Still errors about missing fields in routes and test files

---

### Task 4: Update backend routes for new monitor fields

**Files:**
- Modify: `backend/src/routes/monitors.rs`

- [ ] **Step 1: Add new fields to CreateMonitorRequest**

Add after `confirmations_required`:
```rust
    pub latency_threshold_ms: Option<i64>,
    pub message_template_down: Option<String>,
    pub message_template_latency: Option<String>,
    pub message_template_up: Option<String>,
```

- [ ] **Step 2: Add new fields to UpdateMonitorRequest**

Add after `confirmations_required`:
```rust
    pub latency_threshold_ms: Option<i64>,
    pub message_template_down: Option<String>,
    pub message_template_latency: Option<String>,
    pub message_template_up: Option<String>,
```

- [ ] **Step 3: Update create handler**

In the Monitor construction, add after `failed_checks`:
```rust
        latency_threshold_ms: req.latency_threshold_ms,
        message_template_down: req.message_template_down,
        message_template_latency: req.message_template_latency,
        message_template_up: req.message_template_up,
```

- [ ] **Step 4: Update update handler**

In the Monitor construction, add after `failed_checks`:
```rust
        latency_threshold_ms: req.latency_threshold_ms.or(existing.latency_threshold_ms),
        message_template_down: req.message_template_down.or(existing.message_template_down),
        message_template_latency: req.message_template_latency.or(existing.message_template_latency),
        message_template_up: req.message_template_up.or(existing.message_template_up),
```

- [ ] **Step 5: Verify compilation**

Run: `cd backend && cargo check 2>&1 | head -30`
Expected: Only errors about missing fields in test helper functions

---

### Task 5: Update test helper functions with new fields

**Files:**
- Modify: `backend/src/checker/mod.rs` (test helper `make_monitor`)
- Modify: `backend/src/notifier/telegram.rs` (test helper `make_test_monitor`)
- Modify: `backend/src/models.rs` (test MonitorRow constructions)

- [ ] **Step 1: Update checker/mod.rs make_monitor**

Add after `failed_checks: 0,`:
```rust
            latency_threshold_ms: None,
            message_template_down: None,
            message_template_latency: None,
            message_template_up: None,
```

- [ ] **Step 2: Update notifier/telegram.rs make_test_monitor**

Same change.

- [ ] **Step 3: Update models.rs test MonitorRow constructions**

In `test_monitor_from_row`, add after `failed_checks: 1,`:
```rust
            latency_threshold_ms: None,
            message_template_down: None,
            message_template_latency: None,
            message_template_up: None,
```

Same for `test_monitor_from_row_disabled` and `test_monitor_from_row_invalid_config_json`.

- [ ] **Step 4: Verify compilation**

Run: `cd backend && cargo check 2>&1`
Expected: Compilation succeeds

---

### Task 6: Update scheduler to detect latency and render templates

**Files:**
- Modify: `backend/src/main.rs`

- [ ] **Step 1: Add template import**

Add after `use watchbeat::notifier;`:
```rust
use watchbeat::template::{self, TemplateContext};
```

- [ ] **Step 2: Update notification dispatch in run_monitor_check**

Replace the notification dispatch block (lines 473-663) with new logic that:
1. Detects latency threshold breaches
2. Renders the appropriate template
3. Passes the rendered message to each notifier

The new code:

```rust
    // ── Detect status changes and send notifications ──
    let is_up = check.status == "up" || check.status == "warning";
    
    // Determine notification type and render template
    let notification_type: Option<(&str, String)> = {
        // DOWN transition
        if was_up && !is_up {
            let template = monitor.message_template_down.as_deref().unwrap_or(template::defaults::DOWN);
            let ctx = TemplateContext::for_down(monitor, &check, "up");
            Some(("down", template::render_template(template, &ctx)))
        }
        // UP transition (recovery from DOWN or LATENCY)
        else if !was_up && is_up {
            let template = monitor.message_template_up.as_deref().unwrap_or(template::defaults::UP);
            let ctx = TemplateContext::for_up(monitor, &check, "down");
            Some(("up", template::render_template(template, &ctx)))
        }
        // Latency threshold breach (monitor is up but slow)
        else if is_up {
            if let Some(threshold) = monitor.latency_threshold_ms {
                if check.response_time_ms > threshold {
                    let template = monitor.message_template_latency.as_deref().unwrap_or(template::defaults::LATENCY);
                    let ctx = TemplateContext::for_latency(monitor, &check, threshold);
                    Some(("latency", template::render_template(template, &ctx)))
                } else {
                    None
                }
            } else {
                None
            }
        }
        else {
            None
        }
    };

    // Send notification if we have a rendered message
    if let Some((_notif_type, message)) = notification_type {
        let notifier_ids = db
            .get_monitor_notifier_ids(&monitor.id)
            .await
            .unwrap_or_default();
        for nid in &notifier_ids {
            if let Some(notifier) = notifiers.get(nid) {
                if !notifier.enabled {
                    continue;
                }
                match notifier.notifier_type.as_str() {
                    "telegram" => {
                        let bot_token = notifier
                            .config_json
                            .get("bot_token")
                            .and_then(|v| v.as_str());
                        let chat_id = notifier.config_json.get("chat_id").and_then(|v| v.as_str());
                        if let (Some(token), Some(chat)) = (bot_token, chat_id) {
                            if let Err(e) = notifier::telegram::send_telegram_notification(
                                token, chat, &message,
                            )
                            .await
                            {
                                tracing::warn!("Scheduler: telegram notification failed: {}", e);
                            }
                        }
                    }
                    "matrix" => {
                        let homeserver = notifier
                            .config_json
                            .get("homeserver_url")
                            .and_then(|v| v.as_str());
                        let access_token = notifier
                            .config_json
                            .get("access_token")
                            .and_then(|v| v.as_str());
                        let room_id = notifier.config_json.get("room_id").and_then(|v| v.as_str());
                        if let (Some(hs), Some(tok), Some(rid)) =
                            (homeserver, access_token, room_id)
                        {
                            if let Err(e) = notifier::matrix::send_matrix_notification(
                                hs, tok, rid, &message,
                            )
                            .await
                            {
                                tracing::warn!("Scheduler: matrix notification failed: {}", e);
                            }
                        }
                    }
                    "ntfy" => {
                        let topic = notifier.config_json.get("topic").and_then(|v| v.as_str());
                        let server_url = notifier
                            .config_json
                            .get("server_url")
                            .and_then(|v| v.as_str())
                            .unwrap_or("https://ntfy.sh");
                        let token = notifier.config_json.get("token").and_then(|v| v.as_str());
                        if let Some(t) = topic {
                            if let Err(e) = notifier::ntfy::send_ntfy_notification(
                                t, server_url, token, &message,
                            )
                            .await
                            {
                                tracing::warn!("Scheduler: ntfy notification failed: {}", e);
                            }
                        }
                    }
                    "webhook" => {
                        let url = notifier.config_json.get("url").and_then(|v| v.as_str());
                        let method = notifier
                            .config_json
                            .get("method")
                            .and_then(|v| v.as_str())
                            .unwrap_or("POST");
                        let headers_json = notifier
                            .config_json
                            .get("headers")
                            .map(|v| v.to_string())
                            .unwrap_or_default();
                        if let Some(u) = url {
                            if let Err(e) = notifier::webhook::send_webhook_notification(
                                u, method, &headers_json, &message,
                            )
                            .await
                            {
                                tracing::warn!("Scheduler: webhook notification failed: {}", e);
                            }
                        }
                    }
                    "slack" => {
                        let webhook_url = notifier
                            .config_json
                            .get("webhook_url")
                            .and_then(|v| v.as_str());
                        if let Some(u) = webhook_url {
                            if let Err(e) =
                                notifier::slack::send_slack_notification(u, &message)
                                    .await
                            {
                                tracing::warn!("Scheduler: slack notification failed: {}", e);
                            }
                        }
                    }
                    "discord" => {
                        let webhook_url = notifier
                            .config_json
                            .get("webhook_url")
                            .and_then(|v| v.as_str());
                        if let Some(u) = webhook_url {
                            if let Err(e) = notifier::discord::send_discord_notification(
                                u, &message,
                            )
                            .await
                            {
                                tracing::warn!("Scheduler: discord notification failed: {}", e);
                            }
                        }
                    }
                    "email" => {
                        let smtp_host = notifier
                            .config_json
                            .get("smtp_host")
                            .and_then(|v| v.as_str());
                        let smtp_port = notifier
                            .config_json
                            .get("smtp_port")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(587) as u16;
                        let username = notifier
                            .config_json
                            .get("username")
                            .and_then(|v| v.as_str());
                        let password = notifier
                            .config_json
                            .get("password")
                            .and_then(|v| v.as_str());
                        let from = notifier.config_json.get("from").and_then(|v| v.as_str());
                        let to = notifier.config_json.get("to").and_then(|v| v.as_str());
                        if let (Some(host), Some(user), Some(pass), Some(f), Some(t)) =
                            (smtp_host, username, password, from, to)
                        {
                            if let Err(e) = notifier::email::send_email_notification(
                                host, smtp_port, user, pass, f, t, &message,
                            )
                            .await
                            {
                                tracing::warn!("Scheduler: email notification failed: {}", e);
                            }
                        }
                    }
                    "gotify" => {
                        let server_url = notifier
                            .config_json
                            .get("server_url")
                            .and_then(|v| v.as_str())
                            .unwrap_or("http://localhost:8080");
                        let app_token = notifier
                            .config_json
                            .get("app_token")
                            .and_then(|v| v.as_str());
                        let priority = notifier
                            .config_json
                            .get("priority")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(5);
                        if let Some(t) = app_token {
                            if let Err(e) = notifier::gotify::send_gotify_notification(
                                server_url, t, priority, &message,
                            )
                            .await
                            {
                                tracing::warn!("Scheduler: gotify notification failed: {}", e);
                            }
                        }
                    }
                    _ => tracing::warn!(
                        "Scheduler: unknown notifier type '{}'",
                        notifier.notifier_type
                    ),
                }
            }
        }
    }
```

- [ ] **Step 2: Verify compilation**

Run: `cd backend && cargo check 2>&1`
Expected: Errors about changed function signatures in notifier modules

---

### Task 7: Update all notifier implementations to accept rendered message

**Files:**
- Modify: `backend/src/notifier/telegram.rs`
- Modify: `backend/src/notifier/matrix.rs`
- Modify: `backend/src/notifier/ntfy.rs`
- Modify: `backend/src/notifier/webhook.rs`
- Modify: `backend/src/notifier/slack.rs`
- Modify: `backend/src/notifier/discord.rs`
- Modify: `backend/src/notifier/email.rs`
- Modify: `backend/src/notifier/gotify.rs`
- Modify: `backend/src/notifier/mod.rs`

- [ ] **Step 1: Update telegram.rs**

Replace the function signature and body:
```rust
pub async fn send_telegram_notification(
    bot_token: &str,
    chat_id: &str,
    message: &str,
) -> anyhow::Result<()> {
    let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);
    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .json(&serde_json::json!({
            "chat_id": chat_id,
            "text": message,
            "parse_mode": "Markdown",
        }))
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Telegram API error: {} — {}", status, body);
    }
    Ok(())
}
```

Update tests to match new signature (remove monitor/check/was_up params, just pass a message string).

- [ ] **Step 2: Update matrix.rs**

```rust
pub async fn send_matrix_notification(
    homeserver_url: &str,
    access_token: &str,
    room_id: &str,
    message: &str,
) -> anyhow::Result<()> {
    let url = format!("{}/_matrix/client/v3/rooms/{}/send/m.room.message", homeserver_url.trim_end_matches('/'), room_id);
    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", access_token))
        .json(&serde_json::json!({
            "msgtype": "m.text",
            "body": message,
        }))
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Matrix API error: {} — {}", status, body);
    }
    Ok(())
}
```

- [ ] **Step 3: Update ntfy.rs**

```rust
pub async fn send_ntfy_notification(
    topic: &str,
    server_url: &str,
    token: Option<&str>,
    message: &str,
) -> anyhow::Result<()> {
    let url = format!("{}/{}", server_url.trim_end_matches('/'), topic);
    let mut req = reqwest::Client::new().post(&url).body(message.to_string());
    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {}", t));
    }
    let resp = req.send().await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Ntfy API error: {} — {}", status, body);
    }
    Ok(())
}
```

- [ ] **Step 4: Update webhook.rs**

```rust
pub async fn send_webhook_notification(
    url: &str,
    method: &str,
    headers_json: &str,
    message: &str,
) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let mut req = match method.to_uppercase().as_str() {
        "PUT" => client.put(url),
        "PATCH" => client.patch(url),
        _ => client.post(url),
    };
    if !headers_json.is_empty() {
        if let Ok(headers) = serde_json::from_str::<std::collections::HashMap<String, String>>(headers_json) {
            for (k, v) in headers {
                req = req.header(&k, &v);
            }
        }
    }
    let resp = req
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({"text": message}))
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Webhook error: {} — {}", status, body);
    }
    Ok(())
}
```

- [ ] **Step 5: Update slack.rs**

```rust
pub async fn send_slack_notification(
    webhook_url: &str,
    message: &str,
) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let resp = client
        .post(webhook_url)
        .json(&serde_json::json!({"text": message}))
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Slack webhook error: {} — {}", status, body);
    }
    Ok(())
}
```

- [ ] **Step 6: Update discord.rs**

```rust
pub async fn send_discord_notification(
    webhook_url: &str,
    message: &str,
) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let resp = client
        .post(webhook_url)
        .json(&serde_json::json!({"content": message}))
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Discord webhook error: {} — {}", status, body);
    }
    Ok(())
}
```

- [ ] **Step 7: Update email.rs**

```rust
pub async fn send_email_notification(
    smtp_host: &str,
    smtp_port: u16,
    username: &str,
    password: &str,
    from: &str,
    to: &str,
    message: &str,
) -> anyhow::Result<()> {
    use lettre::message::Mailbox;
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::{Message, SmtpTransport, Transport};
    
    let email = Message::builder()
        .from(from.parse::<Mailbox>().map_err(|e| anyhow::anyhow!("Invalid from address: {}", e))?)
        .to(to.parse::<Mailbox>().map_err(|e| anyhow::anyhow!("Invalid to address: {}", e))?)
        .subject("WatchBeat Alert")
        .body(message.to_string())
        .map_err(|e| anyhow::anyhow!("Failed to build email: {}", e))?;
    
    let creds = Credentials::new(username.to_string(), password.to_string());
    let transport = SmtpTransport::starttls_relay(smtp_host)
        .map_err(|e| anyhow::anyhow!("SMTP relay error: {}", e))?
        .port(smtp_port)
        .credentials(creds)
        .build();
    
    transport
        .send(&email)
        .map_err(|e| anyhow::anyhow!("Failed to send email: {}", e))?;
    
    Ok(())
}
```

- [ ] **Step 8: Update gotify.rs**

```rust
pub async fn send_gotify_notification(
    server_url: &str,
    app_token: &str,
    priority: i64,
    message: &str,
) -> anyhow::Result<()> {
    let url = format!("{}/message?token={}", server_url.trim_end_matches('/'), app_token);
    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .json(&serde_json::json!({
            "message": message,
            "priority": priority,
            "title": "WatchBeat Alert",
        }))
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Gotify API error: {} — {}", status, body);
    }
    Ok(())
}
```

- [ ] **Step 9: Update notifier/mod.rs NotifierTrait**

Change the trait to accept a `message: &str` parameter:
```rust
#[async_trait]
pub trait NotifierTrait: Send + Sync {
    async fn notify(
        &self,
        monitor: &Monitor,
        check: &CheckResult,
        was_up: bool,
        message: &str,
    ) -> anyhow::Result<()>;
}
```

Update each Notifier implementation to pass the message to the underlying function. For example, TelegramNotifier:
```rust
#[async_trait]
impl NotifierTrait for TelegramNotifier {
    async fn notify(
        &self,
        _monitor: &Monitor,
        _check: &CheckResult,
        _was_up: bool,
        message: &str,
    ) -> anyhow::Result<()> {
        let bot_token = self.config.get("bot_token").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing bot_token"))?;
        let chat_id = self.config.get("chat_id").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing chat_id"))?;
        crate::notifier::telegram::send_telegram_notification(bot_token, chat_id, message).await
    }
}
```

Same pattern for all other notifier implementations.

- [ ] **Step 10: Verify compilation**

Run: `cd backend && cargo check 2>&1`
Expected: Compilation succeeds

---

### Task 8: Update frontend Monitor type and form

**Files:**
- Modify: `frontend/src/api/http.ts`
- Modify: `frontend/src/pages/Monitors.tsx`

- [ ] **Step 1: Update Monitor interface in http.ts**

Add after `notifier_id`:
```typescript
  latency_threshold_ms: number | null;
  message_template_down: string | null;
  message_template_latency: string | null;
  message_template_up: string | null;
```

- [ ] **Step 2: Add template fields to Monitors.tsx form**

Add a new tab "Notificaciones" in the modal form with:
- Latency threshold (InputNumber)
- Three TextArea fields for DOWN, LATENCY, UP templates
- Default values shown as placeholders
- Help text showing available variables

Add to the Tabs in the modal:
```tsx
<Tabs.TabPane tab="Notificaciones" key="notifications">
  <Form.Item name="latency_threshold_ms" label="Umbral de latencia (ms)"
    extra="Si la respuesta supera este valor, se enviará una notificación de latencia. Dejar vacío para desactivar.">
    <InputNumber min={0} max={300000} style={{ width: '100%' }} placeholder="Ej: 5000" />
  </Form.Item>
  <Typography.Paragraph type="secondary" style={{ marginBottom: 8 }}>
    Variables disponibles en plantillas: {'{monitor_name}'}, {'{monitor_type}'}, {'{target}'}, {'{status}'}, {'{response_time_ms}'}, {'{error_message}'}, {'{status_code}'}, {'{tags}'}, {'{checked_at}'}
    <br />Para LATENCY además: {'{latency_threshold_ms}'}, {'{latency_exceeded_by_ms}'}
  </Typography.Paragraph>
  <Form.Item name={['message_template_down']} label="Plantilla DOWN"
    extra="Notificación cuando el monitor cae. Vacío = plantilla por defecto.">
    <Input.TextArea rows={3} placeholder={'🔴 {monitor_name} — {target}\nStatus: {status}\nResponse: {response_time_ms}ms\n{error_message}'} />
  </Form.Item>
  <Form.Item name={['message_template_latency']} label="Plantilla LATENCIA"
    extra="Notificación cuando la latencia supera el umbral. Vacío = plantilla por defecto.">
    <Input.TextArea rows={3} placeholder={'🟡 {monitor_name} — {target}\nHigh latency: {response_time_ms}ms (threshold: {latency_threshold_ms}ms)'} />
  </Form.Item>
  <Form.Item name={['message_template_up']} label="Plantilla RECUPERACIÓN"
    extra="Notificación cuando el monitor se recupera. Vacío = plantilla por defecto.">
    <Input.TextArea rows={3} placeholder={'🟢 {monitor_name} — {target}\nRecovered — Status: {status}\nResponse: {response_time_ms}ms'} />
  </Form.Item>
</Tabs.TabPane>
```

- [ ] **Step 3: Update form field initialization in handleCreate**

Add to the form reset:
```tsx
form.setFieldsValue({
  type: 'http', interval_seconds: 300, timeout_seconds: 30, enabled: true,
  confirmations_required: 0, config: {},
  latency_threshold_ms: null,
  message_template_down: null,
  message_template_latency: null,
  message_template_up: null,
});
```

- [ ] **Step 4: Update handleEdit to populate new fields**

Add after `config: m.config_json ?? {}`:
```tsx
latency_threshold_ms: (m as any).latency_threshold_ms ?? null,
message_template_down: (m as any).message_template_down ?? null,
message_template_latency: (m as any).message_template_latency ?? null,
message_template_up: (m as any).message_template_up ?? null,
```

- [ ] **Step 5: Update handleSave to include new fields**

Add to the payload:
```tsx
latency_threshold_ms: values.latency_threshold_ms || null,
message_template_down: values.message_template_down || null,
message_template_latency: values.message_template_latency || null,
message_template_up: values.message_template_up || null,
```

- [ ] **Step 6: Verify frontend builds**

Run: `cd frontend && npm run build 2>&1 | tail -20`
Expected: Build succeeds

---

### Task 9: Run tests and verify

**Files:**
- Run: `cd backend && cargo test`
- Run: `cd backend && cargo clippy -- -D warnings`
- Run: `cd backend && cargo fmt --check`

- [ ] **Step 1: Run all tests**

Run: `cd backend && cargo test 2>&1`
Expected: All tests pass (49+ tests)

- [ ] **Step 2: Run clippy**

Run: `cd backend && cargo clippy --all-targets --all-features -- -D warnings 2>&1`
Expected: No warnings

- [ ] **Step 3: Check formatting**

Run: `cd backend && cargo fmt --check 2>&1`
Expected: No formatting issues

- [ ] **Step 4: Build frontend**

Run: `cd frontend && npm run build 2>&1`
Expected: Build succeeds