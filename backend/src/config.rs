use std::path::PathBuf;

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
            oidc_redirect_url: env_or("OIDC_REDIRECT_URL", "http://localhost:3055/auth/callback"),
        }
    }
}

pub fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

pub fn env_or_parsed<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

pub fn env_required(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| {
        panic!("OIDC env var {} is required", key)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_or_default() {
        assert_eq!(env_or("UNSET_VAR", "def"), "def");
    }

    #[test]
    fn test_env_or_value() {
        std::env::set_var("TST_ENV", "custom");
        assert_eq!(env_or("TST_ENV", "def"), "custom");
        std::env::remove_var("TST_ENV");
    }

    #[test]
    fn test_env_or_parsed_default() {
        assert_eq!(env_or_parsed::<u16>("UNSET_PORT_X", 3055), 3055);
    }

    #[test]
    fn test_env_or_parsed_value() {
        std::env::set_var("TEST_PORT", "8080");
        assert_eq!(env_or_parsed::<u16>("TEST_PORT", 3055), 8080);
        std::env::remove_var("TEST_PORT");
    }

    #[test]
    fn test_env_or_parsed_invalid() {
        std::env::set_var("TEST_BAD", "abc");
        assert_eq!(env_or_parsed::<u16>("TEST_BAD", 3055), 3055);
        std::env::remove_var("TEST_BAD");
    }

    #[test]
    fn test_env_required_value() {
        std::env::set_var("OIDC_REQ_TEST", "v");
        assert_eq!(env_required("OIDC_REQ_TEST"), "v");
        std::env::remove_var("OIDC_REQ_TEST");
    }

    #[test]
    #[should_panic]
    fn test_env_required_panics() {
        env_required("OIDC_UNSET_XYZ");
    }
}
