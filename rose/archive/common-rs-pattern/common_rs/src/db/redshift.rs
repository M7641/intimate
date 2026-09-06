use std::env;
use std::str::FromStr;

use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use thiserror::Error;

const DEFAULT_REDSHIFT_PORT: u16 = 5439;

const USERINFO_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'<')
    .add(b'>')
    .add(b'\\')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}')
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'=')
    .add(b'?')
    .add(b'@')
    .add(b'[')
    .add(b']');

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("missing environment variable: {0}")]
    MissingEnv(&'static str),

    #[error("environment variable {key} has invalid port `{value}`: {source}")]
    InvalidPort {
        key: &'static str,
        value: String,
        source: std::num::ParseIntError,
    },

    #[error(
        "invalid sslmode `{0}` (expected: disable, allow, prefer, require, verify-ca, verify-full)"
    )]
    InvalidSslMode(String),

    #[error("environment variable {0} contains non-UTF-8 bytes")]
    NonUtf8Env(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SslMode {
    Disable,
    Allow,
    Prefer,
    Require,
    VerifyCa,
    VerifyFull,
}

impl SslMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disable => "disable",
            Self::Allow => "allow",
            Self::Prefer => "prefer",
            Self::Require => "require",
            Self::VerifyCa => "verify-ca",
            Self::VerifyFull => "verify-full",
        }
    }
}

impl Default for SslMode {
    fn default() -> Self {
        Self::Require
    }
}

impl FromStr for SslMode {
    type Err = ConfigError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "disable" => Ok(Self::Disable),
            "allow" => Ok(Self::Allow),
            "prefer" => Ok(Self::Prefer),
            "require" => Ok(Self::Require),
            "verify-ca" | "verify_ca" => Ok(Self::VerifyCa),
            "verify-full" | "verify_full" => Ok(Self::VerifyFull),
            _ => Err(ConfigError::InvalidSslMode(s.to_string())),
        }
    }
}

#[derive(Clone)]
pub struct Password(String);

impl Password {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Password {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Password(***)")
    }
}

#[derive(Debug, Clone)]
pub struct RedshiftConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub password: Password,
    pub sslmode: SslMode,
}

impl RedshiftConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let port = match env_optional("REDSHIFT_PORT")? {
            Some(value) => value
                .parse::<u16>()
                .map_err(|source| ConfigError::InvalidPort {
                    key: "REDSHIFT_PORT",
                    value,
                    source,
                })?,
            None => DEFAULT_REDSHIFT_PORT,
        };

        let sslmode = match env_optional("REDSHIFT_SSLMODE")? {
            Some(value) => value.parse()?,
            None => SslMode::default(),
        };

        Ok(Self {
            host: env_required("REDSHIFT_HOST")?,
            port,
            database: env_required("TENANT")?,
            user: env_required("REDSHIFT_USERNAME")?,
            password: Password::new(env_required("REDSHIFT_PASSWORD")?),
            sslmode,
        })
    }

    pub fn to_url(&self) -> String {
        let user = utf8_percent_encode(&self.user, USERINFO_ENCODE_SET);
        let password = utf8_percent_encode(self.password.expose(), USERINFO_ENCODE_SET);
        format!(
            "postgres://{user}:{password}@{host}:{port}/{database}?sslmode={sslmode}",
            host = self.host,
            port = self.port,
            database = self.database,
            sslmode = self.sslmode.as_str(),
        )
    }

    pub fn to_redacted_url(&self) -> String {
        let user = utf8_percent_encode(&self.user, USERINFO_ENCODE_SET);
        format!(
            "postgres://{user}:***@{host}:{port}/{database}?sslmode={sslmode}",
            host = self.host,
            port = self.port,
            database = self.database,
            sslmode = self.sslmode.as_str(),
        )
    }
}

fn env_required(key: &'static str) -> Result<String, ConfigError> {
    match env::var(key) {
        Ok(v) => Ok(v),
        Err(env::VarError::NotPresent) => Err(ConfigError::MissingEnv(key)),
        Err(env::VarError::NotUnicode(_)) => Err(ConfigError::NonUtf8Env(key)),
    }
}

fn env_optional(key: &'static str) -> Result<Option<String>, ConfigError> {
    match env::var(key) {
        Ok(v) => Ok(Some(v)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => Err(ConfigError::NonUtf8Env(key)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(password: &str) -> RedshiftConfig {
        RedshiftConfig {
            host: "redshift.example.com".into(),
            port: 5439,
            database: "prod".into(),
            user: "svc".into(),
            password: Password::new(password),
            sslmode: SslMode::Require,
        }
    }

    #[test]
    fn url_basic_shape() {
        assert_eq!(
            sample("simple").to_url(),
            "postgres://svc:simple@redshift.example.com:5439/prod?sslmode=require",
        );
    }

    #[test]
    fn url_encodes_userinfo_special_chars() {
        let url = sample("p@ss:w/rd").to_url();
        assert!(
            url.contains("p%40ss%3Aw%2Frd"),
            "expected encoded password in: {url}"
        );
        assert!(!url.contains("p@ss:w/rd"), "raw password leaked: {url}");
    }

    #[test]
    fn url_encodes_user_special_chars() {
        let cfg = RedshiftConfig {
            user: "user@with:colon".into(),
            ..sample("p")
        };
        assert!(cfg.to_url().contains("user%40with%3Acolon"));
    }

    #[test]
    fn redacted_url_hides_password() {
        let url = sample("topsecret").to_redacted_url();
        assert!(!url.contains("topsecret"));
        assert!(url.contains(":***@"));
    }

    #[test]
    fn debug_does_not_leak_password() {
        let s = format!("{:?}", sample("topsecret"));
        assert!(!s.contains("topsecret"), "Debug leaked: {s}");
        assert!(
            s.contains("Password(***)"),
            "expected redaction marker, got: {s}"
        );
    }

    #[test]
    fn sslmode_default_is_require() {
        assert_eq!(SslMode::default(), SslMode::Require);
    }

    #[test]
    fn sslmode_parse_is_case_insensitive() {
        assert_eq!("REQUIRE".parse::<SslMode>().unwrap(), SslMode::Require);
        assert_eq!(
            "Verify-Full".parse::<SslMode>().unwrap(),
            SslMode::VerifyFull
        );
        assert_eq!(
            "verify_full".parse::<SslMode>().unwrap(),
            SslMode::VerifyFull
        );
    }

    #[test]
    fn sslmode_parse_rejects_unknown() {
        let err = "strict".parse::<SslMode>().unwrap_err();
        assert!(matches!(err, ConfigError::InvalidSslMode(s) if s == "strict"));
    }
}
