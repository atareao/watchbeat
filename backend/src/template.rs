use crate::db::Database;
use crate::models::{CheckResult, Monitor};
use minijinja::{Environment, Value};
use serde_json::json;

/// Global default templates loaded from settings.
pub struct GlobalDefaults {
    pub down: Option<String>,
    pub latency: Option<String>,
    pub up: Option<String>,
    pub expiry: Option<String>,
}

impl GlobalDefaults {
    pub fn empty() -> Self {
        Self {
            down: None,
            latency: None,
            up: None,
            expiry: None,
        }
    }
}

/// Load global default templates from the database settings.
pub async fn load_global_defaults(db: &Database) -> GlobalDefaults {
    GlobalDefaults {
        down: db.get_setting("default_template_down").await.ok().flatten(),
        latency: db
            .get_setting("default_template_latency")
            .await
            .ok()
            .flatten(),
        up: db.get_setting("default_template_up").await.ok().flatten(),
        expiry: db
            .get_setting("default_template_expiry")
            .await
            .ok()
            .flatten(),
    }
}

/// Build the template context for rendering notification messages.
pub struct TemplateContext;

impl TemplateContext {
    pub fn for_down(monitor: &Monitor, check: &CheckResult, previous_status: &str) -> Value {
        let ctx = Self::build_ctx(monitor, check, previous_status);
        Value::from_serialize(&ctx)
    }

    pub fn for_latency(monitor: &Monitor, check: &CheckResult, threshold_ms: i64) -> Value {
        let mut ctx = Self::build_ctx(monitor, check, "up");
        ctx["latency_threshold_ms"] = json!(threshold_ms);
        ctx["latency_exceeded_by_ms"] = json!(check.response_time_ms - threshold_ms);
        Value::from_serialize(&ctx)
    }

    pub fn for_up(monitor: &Monitor, check: &CheckResult, previous_status: &str) -> Value {
        let ctx = Self::build_ctx(monitor, check, previous_status);
        Value::from_serialize(&ctx)
    }

    pub fn for_expiry(
        monitor: &Monitor,
        check: &CheckResult,
        days_left: i64,
        expiry_days: i64,
    ) -> Value {
        let mut ctx = Self::build_ctx(monitor, check, "up");
        ctx["days_left"] = json!(days_left);
        ctx["expiry_threshold_days"] = json!(expiry_days);
        Value::from_serialize(&ctx)
    }

    /// Context for the aggregated *storm* alert (Fase 5, D7). Unlike the other
    /// notifications it is not about a single monitor: it reports how many
    /// monitors went down inside the detection window and which ones.
    pub fn for_storm(
        count: usize,
        monitors: &[String],
        window_seconds: u64,
        threshold_percent: u8,
    ) -> Value {
        let ctx = json!({
            "count": count,
            "monitors": monitors,
            "monitors_list": monitors.join(", "),
            "window_seconds": window_seconds,
            "threshold_percent": threshold_percent,
        });
        Value::from_serialize(&ctx)
    }

    fn build_ctx(
        monitor: &Monitor,
        check: &CheckResult,
        previous_status: &str,
    ) -> serde_json::Value {
        json!({
            "monitor_name": monitor.name,
            "monitor_type": monitor.monitor_type,
            "target": monitor.target,
            "status": check.status,
            "previous_status": previous_status,
            "response_time_ms": check.response_time_ms,
            "error_message": check.error_message,
            "checked_at": check.checked_at,
            "status_code": check.status_code,
            "tags": monitor.tags,
        })
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
    pub const EXPIRY: &str = "🟡 {{ monitor_name }} — {{ target }}\nCertificate expires in {{ days_left }} days (threshold: {{ expiry_threshold_days }} days)";
    pub const STORM: &str = "🟠 Monitorización degradada\n{{ count }} monitores caídos en {{ window_seconds }}s (umbral {{ threshold_percent }}%)\nAfectados: {{ monitors_list }}";
    /// Aggregated alert for a storm of latency breaches (#72).
    pub const STORM_LATENCY: &str = "🟠 Latencia degradada\n{{ count }} monitores con latencia alta en {{ window_seconds }}s (umbral {{ threshold_percent }}%)\nAfectados: {{ monitors_list }}";
    /// Aggregated alert for a storm of certificate expirations (#72).
    pub const STORM_EXPIRY: &str = "🟠 Certificados próximos a expirar\n{{ count }} monitores con certificado a punto de expirar en {{ window_seconds }}s (umbral {{ threshold_percent }}%)\nAfectados: {{ monitors_list }}";
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three storm templates share `TemplateContext::for_storm`, so the same
    /// context must render count, window and monitor list in all of them (#72, M1).
    fn storm_ctx() -> Value {
        TemplateContext::for_storm(2, &["web".to_string(), "api".to_string()], 30, 50)
    }

    #[test]
    fn the_three_storm_templates_render_the_shared_context() {
        let cases: [(&str, &str); 3] = [
            (defaults::STORM, "2 monitores caídos en 30s (umbral 50%)"),
            (
                defaults::STORM_LATENCY,
                "2 monitores con latencia alta en 30s (umbral 50%)",
            ),
            (
                defaults::STORM_EXPIRY,
                "2 monitores con certificado a punto de expirar en 30s (umbral 50%)",
            ),
        ];

        for (template, expected) in cases {
            let rendered = render_template(template, &storm_ctx());
            assert!(
                rendered.contains(expected),
                "count/window/threshold missing in '{template}': {rendered}"
            );
            assert!(
                rendered.contains("web, api"),
                "monitors_list missing in '{template}': {rendered}"
            );
            assert!(
                rendered.contains("Afectados:"),
                "the affected monitors header is missing in '{template}': {rendered}"
            );
        }
    }
}
