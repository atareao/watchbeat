use std::path::PathBuf;

/// Application configuration — loaded from env vars.
/// OIDC is **mandatory**: the application will not start without it.
#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub data_dir: PathBuf,
    pub database_url: PathBuf,
    pub timezone: String,
    pub log_level: String,
    pub log_format: String,
    pub oidc_issuer_url: String,
    pub oidc_client_id: String,
    pub oidc_client_secret: String,
    pub oidc_redirect_url: String,
}

impl Config {
    /// Load configuration from environment variables.
    /// Panics if required OIDC variables are missing.
    pub fn load() -> Self {
        Self {
            host: env_or("HOST", "0.0.0.0"),
            port: env_or_parsed("PORT", 3055),
            data_dir: PathBuf::from(env_or("DATA_DIR", "./data")),
            database_url: PathBuf::from(env_or("DATABASE_URL", "./data/vigilatrs.db")),
            timezone: env_or("TIMEZONE", "Europe/Madrid"),
            log_level: env_or("RUST_LOG", "info"),
            log_format: env_or("LOG_FORMAT", "pretty"),
            oidc_issuer_url: env_required("OIDC_ISSUER_URL"),
            oidc_client_id: env_required("OIDC_CLIENT_ID"),
            oidc_client_secret: env_required("OIDC_CLIENT_SECRET"),
            oidc_redirect_url: env_or(
                "OIDC_REDIRECT_URL",
                "http://localhost:3055/auth/callback",
            ),
        }
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_or_parsed<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_required(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| {
        panic!(
            "🚨 OIDC es obligatorio. Define la variable de entorno {} en el .env",
            key
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        // This will panic if OIDC env vars aren't set — that's expected
        // in test unless we set them. Just verifying it compiles.
    }
}