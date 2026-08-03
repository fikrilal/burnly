//! Cloud endpoint configuration for burnly-api and web login origin.

use std::env;

const ENV_API_BASE_URL: &str = "BURNLY_API_BASE_URL";
const ENV_WEB_ORIGIN: &str = "BURNLY_WEB_ORIGIN";
const ENV_REDIRECT_URI: &str = "BURNLY_DESKTOP_REDIRECT_URI";

const DEFAULT_API_BASE_URL: &str = "https://api.burnly.dev";
const DEFAULT_WEB_ORIGIN: &str = "https://burnly.dev";
const DEFAULT_REDIRECT_URI: &str = "http://127.0.0.1:39201/callback";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CloudConfig {
    api_base_url: String,
    web_origin: String,
    redirect_uri: String,
    app_version: String,
}

impl CloudConfig {
    pub(crate) fn new(
        api_base_url: impl Into<String>,
        web_origin: impl Into<String>,
        redirect_uri: impl Into<String>,
        app_version: impl Into<String>,
    ) -> Result<Self, CloudConfigError> {
        let api_base_url = normalize_origin(api_base_url.into(), "api_base_url")?;
        let web_origin = normalize_origin(web_origin.into(), "web_origin")?;
        let redirect_uri = normalize_required(redirect_uri.into(), "redirect_uri")?;
        let app_version = normalize_required(app_version.into(), "app_version")?;
        Ok(Self {
            api_base_url,
            web_origin,
            redirect_uri,
            app_version,
        })
    }

    /// Release defaults with optional env overrides for local multi-service smoke tests.
    pub(crate) fn from_env(app_version: impl Into<String>) -> Result<Self, CloudConfigError> {
        Self::new(
            env::var(ENV_API_BASE_URL).unwrap_or_else(|_| DEFAULT_API_BASE_URL.to_owned()),
            env::var(ENV_WEB_ORIGIN).unwrap_or_else(|_| DEFAULT_WEB_ORIGIN.to_owned()),
            env::var(ENV_REDIRECT_URI).unwrap_or_else(|_| DEFAULT_REDIRECT_URI.to_owned()),
            app_version,
        )
    }

    pub(crate) fn api_base_url(&self) -> &str {
        &self.api_base_url
    }

    pub(crate) fn web_origin(&self) -> &str {
        &self.web_origin
    }

    pub(crate) fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }

    pub(crate) fn app_version(&self) -> &str {
        &self.app_version
    }

    pub(crate) fn api_url(&self, path: &str) -> String {
        let path = if path.starts_with('/') {
            path.to_owned()
        } else {
            format!("/{path}")
        };
        format!("{}{path}", self.api_base_url)
    }
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub(crate) enum CloudConfigError {
    #[error("cloud config {field} must be non-empty")]
    Empty { field: &'static str },
}

fn normalize_required(value: String, field: &'static str) -> Result<String, CloudConfigError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CloudConfigError::Empty { field });
    }
    Ok(trimmed.to_owned())
}

fn normalize_origin(value: String, field: &'static str) -> Result<String, CloudConfigError> {
    let mut origin = normalize_required(value, field)?;
    while origin.ends_with('/') {
        origin.pop();
    }
    if origin.is_empty() {
        return Err(CloudConfigError::Empty { field });
    }
    Ok(origin)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_trailing_slashes_from_origins() {
        let config = CloudConfig::new(
            "http://127.0.0.1:4000/",
            "http://127.0.0.1:3000///",
            "http://127.0.0.1:39201/callback",
            "0.1.20",
        )
        .expect("config");
        assert_eq!(config.api_base_url(), "http://127.0.0.1:4000");
        assert_eq!(config.web_origin(), "http://127.0.0.1:3000");
        assert_eq!(
            config.api_url("/v1/auth/refresh"),
            "http://127.0.0.1:4000/v1/auth/refresh"
        );
    }

    #[test]
    fn rejects_empty_fields() {
        assert!(matches!(
            CloudConfig::new("", "http://x", "http://y", "1.0.0"),
            Err(CloudConfigError::Empty {
                field: "api_base_url"
            })
        ));
    }
}
