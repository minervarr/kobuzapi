//! Central API service holding authentication state and providing all operations.

use std::path::Path;

use {tokio::runtime::Runtime, tracing::info};

use crate::{
    api::http_client::{HttpClient, ReqwestClient},
    credentials::{extract_from_web_player, load_app_credentials, save_app_credentials},
    errors::QobuzApiError::{self, AuthenticationError, InitializationError},
};

/// Central service for all Qobuz API operations.
///
/// Holds authentication state and a shared HTTP client with connection pooling.
pub struct QobuzApiService {
    /// Qobuz application ID.
    pub app_id: String,
    /// Qobuz application secret.
    pub app_secret: String,
    /// User authentication token (set after login).
    user_auth_token: Option<String>,
    /// HTTP client with connection pooling.
    client: Box<dyn HttpClient>,
    /// Whether credentials have been refreshed this session.
    credentials_refreshed: bool,
}

impl QobuzApiService {
    /// Creates a new service instance.
    ///
    /// Extracts app credentials from `.env` or the Qobuz web player JS bundle.
    /// Returns a service with valid `app_id` and `app_secret`.
    /// `user_auth_token` is `None` (unauthenticated state).
    ///
    /// # Returns
    ///
    /// A `QobuzApiService` with valid credentials.
    ///
    /// # Errors
    ///
    /// Returns a `QobuzApiError` if credential extraction or HTTP client creation fails.
    pub fn new() -> Result<Self, QobuzApiError> {
        let env_path = Path::new(".env");

        let (app_id, app_secret) = if let Some(creds) = load_app_credentials(env_path)? {
            creds
        } else {
            info!("No .env credentials found, extracting from web player");

            let rt = Runtime::new()?;
            let (id, secret) = rt.block_on(extract_from_web_player())?;

            save_app_credentials(env_path, &id, &secret)?;

            (id, secret)
        };

        if app_id.is_empty() || app_secret.is_empty() {
            return Err(InitializationError {
                message: "app_id and app_secret must be non-empty".to_string(),
            });
        }

        let client = ReqwestClient::new()?;

        Ok(Self {
            app_id,
            app_secret,
            user_auth_token: None,
            client: Box::new(client),
            credentials_refreshed: false,
        })
    }

    /// Creates a service with explicitly provided app credentials.
    ///
    /// # Arguments
    ///
    /// * `app_id` - Qobuz application ID
    /// * `app_secret` - Qobuz application secret
    ///
    /// # Returns
    ///
    /// A `QobuzApiService` with the provided credentials.
    ///
    /// # Errors
    ///
    /// Returns a `QobuzApiError` if `app_id` or `app_secret` is empty, or HTTP client creation
    /// fails.
    pub fn with_credentials(app_id: &str, app_secret: &str) -> Result<Self, QobuzApiError> {
        if app_id.is_empty() || app_secret.is_empty() {
            return Err(InitializationError {
                message: "app_id and app_secret must be non-empty".to_string(),
            });
        }

        let client = ReqwestClient::new()?;

        Ok(Self {
            app_id: app_id.to_string(),
            app_secret: app_secret.to_string(),
            user_auth_token: None,
            client: Box::new(client),
            credentials_refreshed: false,
        })
    }

    /// Returns the user auth token or an error if not authenticated.
    ///
    /// # Returns
    ///
    /// A string slice of the auth token.
    ///
    /// # Errors
    ///
    /// Returns a `QobuzApiError::AuthenticationError` if not yet authenticated.
    pub fn require_auth_token(&self) -> Result<&str, QobuzApiError> {
        self.user_auth_token
            .as_deref()
            .ok_or_else(|| AuthenticationError {
                message: "Not authenticated. Call authenticate_with_env() or login() first."
                    .to_string(),
            })
    }

    /// Sets the user auth token.
    ///
    /// # Arguments
    ///
    /// * `token` - The authentication token to set
    pub fn set_auth_token(&mut self, token: String) {
        self.user_auth_token = Some(token);
    }

    /// Returns a reference to the HTTP client.
    ///
    /// # Returns
    ///
    /// A `&dyn HttpClient` reference.
    #[must_use]
    pub fn http_client(&self) -> &dyn HttpClient {
        &*self.client
    }

    /// Returns whether credentials have been refreshed this session.
    ///
    /// # Returns
    ///
    /// `true` if credentials have been refreshed.
    #[must_use]
    pub fn is_credentials_refreshed(&self) -> bool {
        self.credentials_refreshed
    }

    /// Marks credentials as refreshed.
    pub fn mark_credentials_refreshed(&mut self) {
        self.credentials_refreshed = true;
    }

    /// Returns the app secret.
    ///
    /// # Returns
    ///
    /// A string slice of the app secret.
    #[must_use]
    pub fn app_secret(&self) -> &str {
        &self.app_secret
    }

    /// Returns a cloned `Box<dyn HttpClient>` for use in spawned tasks.
    ///
    /// # Returns
    ///
    /// A boxed HTTP client suitable for use in async tasks.
    ///
    /// # Errors
    ///
    /// Returns a `QobuzApiError` if HTTP client creation fails.
    pub fn http_client_ref() -> Result<Box<dyn HttpClient>, QobuzApiError> {
        let client = ReqwestClient::new()?;
        Ok(client.into_boxed())
    }
}
