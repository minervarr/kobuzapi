//! Central API service holding authentication state and providing all operations.

use std::path::Path;

use {tokio::runtime::Runtime, tracing::info};

use crate::{
    api::{
        auth::{authenticate_with_env, login, login_with_token, refresh_app_credentials},
        content::{
            albums::search_albums, artists::search_artists, catalog::search_catalog,
            playlists::search_playlists, tracks::search_tracks,
        },
        http_client::{HttpClient, ReqwestClient},
    },
    credentials::{extract_from_web_player, load_app_credentials, save_app_credentials},
    errors::QobuzApiError::{self, AuthenticationError, InitializationError},
    models::{
        album::Album,
        artist::Artist,
        playlist::Playlist,
        search::{ItemSearchResult, SearchResult},
        track::Track,
    },
};

/// Base URL for all Qobuz API v0.2 endpoints.
const BASE_URL: &str = "https://www.qobuz.com/api.json/0.2";

/// Central service for all Qobuz API operations.
///
/// Holds authentication state and a shared HTTP client with connection pooling.
pub struct QobuzApiService {
    /// API base URL for requests.
    base_url: String,
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
    /// Builds a service with the given credentials and a new HTTP client.
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
    fn build_service(app_id: String, app_secret: String) -> Result<Self, QobuzApiError> {
        if app_id.is_empty() || app_secret.is_empty() {
            return Err(InitializationError {
                message: "app_id and app_secret must be non-empty".to_string(),
            });
        }

        let client = ReqwestClient::new()?;

        Ok(Self {
            base_url: BASE_URL.to_string(),
            app_id,
            app_secret,
            user_auth_token: None,
            client: Box::new(client),
            credentials_refreshed: false,
        })
    }

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

        Self::build_service(app_id, app_secret)
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
        Self::build_service(app_id.to_string(), app_secret.to_string())
    }

    /// Returns the API base URL.
    ///
    /// # Returns
    ///
    /// A string slice of the base URL.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
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

    /// Authenticates using environment variables.
    ///
    /// Delegates to [`crate::api::auth::authenticate_with_env`].
    ///
    /// # Returns
    ///
    /// `Ok(())` on successful authentication.
    ///
    /// # Errors
    ///
    /// Returns a `QobuzApiError` if no valid credentials are found or login fails.
    pub fn authenticate_with_env(&mut self) -> Result<(), QobuzApiError> {
        authenticate_with_env(self)
    }

    /// Authenticates with email and MD5-hashed password.
    ///
    /// Delegates to [`crate::api::auth::login`].
    ///
    /// # Arguments
    ///
    /// * `email` - User email address
    /// * `password` - User password
    ///
    /// # Returns
    ///
    /// `Ok(())` on successful authentication.
    ///
    /// # Errors
    ///
    /// Returns a `QobuzApiError` if the login request fails.
    pub fn login(&mut self, email: &str, password: &str) -> Result<(), QobuzApiError> {
        login(self, email, password)
    }

    /// Authenticates with user ID and auth token.
    ///
    /// Delegates to [`crate::api::auth::login_with_token`].
    ///
    /// # Arguments
    ///
    /// * `user_id` - Qobuz user ID
    /// * `auth_token` - User authentication token
    ///
    /// # Returns
    ///
    /// `Ok(())` on successful authentication.
    ///
    /// # Errors
    ///
    /// Returns a `QobuzApiError` if the token validation request fails.
    pub fn login_with_token(
        &mut self,
        user_id: &str,
        auth_token: &str,
    ) -> Result<(), QobuzApiError> {
        login_with_token(self, user_id, auth_token)
    }

    /// Re-extracts app credentials from the Qobuz web player.
    ///
    /// Delegates to [`crate::api::auth::refresh_app_credentials`].
    ///
    /// # Returns
    ///
    /// `Ok(())` on successful credential refresh.
    ///
    /// # Errors
    ///
    /// Returns a `QobuzApiError` if credentials have already been refreshed or extraction fails.
    pub fn refresh_app_credentials(&mut self) -> Result<(), QobuzApiError> {
        refresh_app_credentials(self)
    }

    /// Searches all content types (albums, artists, tracks, playlists).
    ///
    /// # Arguments
    ///
    /// * `query` - Search query string
    /// * `limit` - Maximum number of results per content type
    /// * `offset` - Pagination offset
    ///
    /// # Returns
    ///
    /// A `SearchResult` with grouped results for each content type.
    ///
    /// # Errors
    ///
    /// Returns a `QobuzApiError` if not authenticated or any search request fails.
    pub fn search_catalog(
        &self,
        query: &str,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<SearchResult, QobuzApiError> {
        let rt = Runtime::new()?;
        rt.block_on(search_catalog(self, query, limit, offset))
    }

    /// Searches for albums matching the query.
    ///
    /// # Arguments
    ///
    /// * `query` - Search query string
    /// * `limit` - Maximum number of results
    /// * `offset` - Pagination offset
    ///
    /// # Returns
    ///
    /// A paginated `ItemSearchResult` containing matching albums.
    ///
    /// # Errors
    ///
    /// Returns a `QobuzApiError` if not authenticated or the API request fails.
    pub fn search_albums(
        &self,
        query: &str,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<ItemSearchResult<Box<Album>>, QobuzApiError> {
        let rt = Runtime::new()?;
        rt.block_on(search_albums(self, query, limit, offset))
    }

    /// Searches for artists matching the query.
    ///
    /// # Arguments
    ///
    /// * `query` - Search query string
    /// * `limit` - Maximum number of results
    /// * `offset` - Pagination offset
    ///
    /// # Returns
    ///
    /// A paginated `ItemSearchResult` containing matching artists.
    ///
    /// # Errors
    ///
    /// Returns a `QobuzApiError` if not authenticated or the API request fails.
    pub fn search_artists(
        &self,
        query: &str,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<ItemSearchResult<Box<Artist>>, QobuzApiError> {
        let rt = Runtime::new()?;
        rt.block_on(search_artists(self, query, limit, offset))
    }

    /// Searches for tracks matching the query.
    ///
    /// # Arguments
    ///
    /// * `query` - Search query string
    /// * `limit` - Maximum number of results
    /// * `offset` - Pagination offset
    ///
    /// # Returns
    ///
    /// A paginated `ItemSearchResult` containing matching tracks.
    ///
    /// # Errors
    ///
    /// Returns a `QobuzApiError` if not authenticated or the API request fails.
    pub fn search_tracks(
        &self,
        query: &str,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<ItemSearchResult<Box<Track>>, QobuzApiError> {
        let rt = Runtime::new()?;
        rt.block_on(search_tracks(self, query, limit, offset))
    }

    /// Searches for playlists matching the query.
    ///
    /// # Arguments
    ///
    /// * `query` - Search query string
    /// * `limit` - Maximum number of results
    /// * `offset` - Pagination offset
    ///
    /// # Returns
    ///
    /// A paginated `ItemSearchResult` containing matching playlists.
    ///
    /// # Errors
    ///
    /// Returns a `QobuzApiError` if not authenticated or the API request fails.
    pub fn search_playlists(
        &self,
        query: &str,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<ItemSearchResult<Box<Playlist>>, QobuzApiError> {
        let rt = Runtime::new()?;
        rt.block_on(search_playlists(self, query, limit, offset))
    }
}

#[cfg(test)]
impl QobuzApiService {
    #[must_use]
    pub fn new_test(client: Box<dyn HttpClient>, base_url: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
            app_id: "test-app-id".to_string(),
            app_secret: "test-app-secret".to_string(),
            user_auth_token: None,
            client,
            credentials_refreshed: false,
        }
    }
}
