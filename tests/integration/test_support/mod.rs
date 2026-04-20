//! Shared test helpers for integration tests.

#![allow(dead_code)]

pub mod query;
pub mod setup;

use std::{env::var, path::Path, sync::OnceLock};

use {
    anyhow::{Result, anyhow, bail},
    dotenvy::from_path,
    tracing::{Level, warn},
    tracing_subscriber::{EnvFilter, fmt},
};

use qobuz_api_rust_refactor::{api::service::QobuzApiService, models::file_url::quality::MP3_320};

/// Test support imports macro for integration tests.
#[macro_export]
macro_rules! test_support_imports {
    () => {
        use {
            anyhow::{Result, anyhow, ensure},
            tracing::info,
        };

        use qobuz_api_rust_refactor::models::{
            album::Album, artist::Artist, playlist::Playlist, track::Track,
        };
    };
}

/// Threshold in seconds for trial duration tests.
pub const TRIAL_DURATION_THRESHOLD_SECS: f64 = 30.0;

/// Global download configuration, initialized once from environment variables.
static CONFIG: OnceLock<DownloadConfig> = OnceLock::new();

/// Global logging guard, ensures tracing subscriber is initialized exactly once.
static LOG_GUARD: OnceLock<()> = OnceLock::new();

/// Browse IDs for browse/content-detail tests, initialized once from environment variables.
static BROWSE_IDS: OnceLock<BrowseIds> = OnceLock::new();

/// Test keywords for search tests, initialized once from environment variables.
static TEST_KEYWORDS: OnceLock<TestKeywords> = OnceLock::new();

/// Test data: IDs and queries configurable via `.env`.
pub struct BrowseIds {
    /// Album query string.
    pub album: String,
    /// Artist query string.
    pub artist: String,
    /// Track query string.
    pub track: String,
    /// Playlist query string.
    pub playlist: String,
    /// Release list artist query string.
    pub release_list_artist: String,
}

impl BrowseIds {
    /// Creates IDs from environment variables.
    ///
    /// # Returns
    ///
    /// A new `BrowseIds` instance populated from environment.
    fn from_env() -> Self {
        load_env_file();

        Self {
            album: var("TEST_BROWSE_ALBUM_QUERY")
                .unwrap_or_else(|_| "Kind of Blue Miles Davis".to_string()),
            artist: var("TEST_BROWSE_ARTIST_QUERY").unwrap_or_else(|_| "Miles Davis".to_string()),
            track: var("TEST_BROWSE_TRACK_QUERY")
                .unwrap_or_else(|_| "So What Miles Davis".to_string()),
            playlist: var("TEST_BROWSE_PLAYLIST_QUERY")
                .unwrap_or_else(|_| "jazz classics".to_string()),
            release_list_artist: var("TEST_BROWSE_RELEASE_LIST_QUERY")
                .unwrap_or_else(|_| "John Coltrane".to_string()),
        }
    }
}

/// Configuration for download tests.
pub struct DownloadConfig {
    /// Query string for searching tracks.
    pub track_query: String,
    /// Query string for searching albums.
    pub album_query: String,
    /// Audio format ID (e.g., 3 for 320kbps MP3).
    pub format_id: i32,
}

impl DownloadConfig {
    /// Creates configuration from environment variables.
    ///
    /// # Returns
    ///
    /// A new `DownloadConfig` instance populated from environment.
    fn from_env() -> Self {
        load_env_file();

        Self {
            track_query: var("TEST_DOWNLOAD_TRACK_QUERY")
                .unwrap_or_else(|_| "So What Miles Davis".to_string()),
            album_query: var("TEST_DOWNLOAD_ALBUM_QUERY")
                .unwrap_or_else(|_| "Kind of Blue Miles Davis".to_string()),
            format_id: var("TEST_DOWNLOAD_FORMAT_ID")
                .ok()
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(MP3_320),
        }
    }

    /// Returns the album query string.
    ///
    /// # Returns
    ///
    /// A string slice containing the album query.
    pub fn album_query(&self) -> &str {
        &self.album_query
    }
}

/// Search keywords for integration tests, configurable via `.env`.
pub struct TestKeywords {
    /// Album search query #1.
    pub album_query_1: String,
    /// Album search query #2.
    pub album_query_2: String,
    /// Artist search query #1.
    pub artist_query_1: String,
    /// Artist search query #2.
    pub artist_query_2: String,
    /// Track search query #1.
    pub track_query_1: String,
    /// Track search query #2.
    pub track_query_2: String,
    /// Playlist search query #1.
    pub playlist_query_1: String,
    /// Catalog search query.
    pub catalog_query: String,
    /// Pagination search query.
    pub pagination_query: String,
}

impl TestKeywords {
    /// Creates a `TestKeywords` instance from environment variables.
    ///
    /// # Returns
    ///
    /// A `TestKeywords` with values from `.env` or defaults.
    fn from_env() -> Self {
        load_env_file();

        Self {
            album_query_1: var("TEST_SEARCH_ALBUM_QUERY_1")
                .unwrap_or_else(|_| "The Dark Side of the Moon Pink Floyd".to_string()),
            album_query_2: var("TEST_SEARCH_ALBUM_QUERY_2")
                .unwrap_or_else(|_| "Kind of Blue Miles Davis".to_string()),
            artist_query_1: var("TEST_SEARCH_ARTIST_QUERY_1")
                .unwrap_or_else(|_| "Pink Floyd".to_string()),
            artist_query_2: var("TEST_SEARCH_ARTIST_QUERY_2")
                .unwrap_or_else(|_| "Miles Davis".to_string()),
            track_query_1: var("TEST_SEARCH_TRACK_QUERY_1")
                .unwrap_or_else(|_| "Comfortably Numb Pink Floyd".to_string()),
            track_query_2: var("TEST_SEARCH_TRACK_QUERY_2")
                .unwrap_or_else(|_| "So What Miles Davis".to_string()),
            playlist_query_1: var("TEST_SEARCH_PLAYLIST_QUERY_1")
                .unwrap_or_else(|_| "jazz classics".to_string()),
            catalog_query: var("TEST_SEARCH_CATALOG_QUERY")
                .unwrap_or_else(|_| "0190244849000".to_string()),
            pagination_query: var("TEST_SEARCH_PAGINATION_QUERY")
                .unwrap_or_else(|_| "Pink Floyd".to_string()),
        }
    }
}

/// Returns the browse IDs from environment variables.
///
/// # Returns
///
/// A static reference to the browse IDs.
pub fn get_browse_ids() -> &'static BrowseIds {
    BROWSE_IDS.get_or_init(BrowseIds::from_env)
}

/// Returns the test keywords from environment variables.
///
/// # Returns
///
/// A static reference to `TestKeywords` with values from `.env` or defaults.
pub fn get_test_keywords() -> &'static TestKeywords {
    TEST_KEYWORDS.get_or_init(TestKeywords::from_env)
}

/// Returns the download configuration from environment variables.
///
/// # Returns
///
/// A static reference to the download configuration.
pub fn get_download_config() -> &'static DownloadConfig {
    CONFIG.get_or_init(DownloadConfig::from_env)
}

/// Loads environment variables from `.env` file if present.
pub fn load_env_file() {
    let env_path = Path::new(".env");
    if env_path.exists()
        && let Err(e) = from_path(env_path)
    {
        warn!(error = %e, "Failed to load .env");
    }
}

/// Initializes logging for tests.
pub fn init_logging() {
    let () = LOG_GUARD.get_or_init(|| {
        fmt()
            .with_env_filter(EnvFilter::from_default_env().add_directive(Level::INFO.into()))
            .init();
    });
}

/// Validates that user credentials exist in the environment.
///
/// Loads the `.env` file and checks for either email/password or user ID/token credentials.
///
/// # Returns
///
/// `Ok(())` if valid credentials are found.
pub fn ensure_env_credentials() -> Result<()> {
    let env_path = Path::new(".env");

    if !env_path.exists() {
        bail!(
            "No .env file found. Copy .env.example to .env and fill in your Qobuz \
             credentials.\nSet either QOBUZ_EMAIL + QOBUZ_PASSWORD or QOBUZ_USER_ID + \
             QOBUZ_USER_AUTH_TOKEN."
        );
    }

    from_path(env_path).map_err(|e| anyhow!("Failed to parse .env: {e}"))?;

    let email = var("QOBUZ_EMAIL").or_else(|_| var("QOBUZ_USERNAME")).ok();
    let password = var("QOBUZ_PASSWORD").ok();
    let user_id = var("QOBUZ_USER_ID").ok();
    let user_auth_token = var("QOBUZ_USER_AUTH_TOKEN").ok();

    let has_email_auth = email.is_some() && password.is_some();
    let has_token_auth = user_id.is_some() && user_auth_token.is_some();

    if !has_email_auth && !has_token_auth {
        bail!(
            "No user credentials in .env. Provide one of:\n- QOBUZ_EMAIL (or QOBUZ_USERNAME) + \
             QOBUZ_PASSWORD\n- QOBUZ_USER_ID + QOBUZ_USER_AUTH_TOKEN"
        );
    }

    if email.is_some() && password.is_none() {
        bail!("QOBUZ_EMAIL is set but QOBUZ_PASSWORD is missing in .env");
    }

    Ok(())
}

/// Creates an authenticated Qobuz API service.
///
/// # Returns
///
/// An authenticated service if credentials are valid.
pub fn create_authenticated_service() -> Result<QobuzApiService> {
    ensure_env_credentials()?;

    let mut service =
        QobuzApiService::new().map_err(|e| anyhow!("Failed to create service: {e}"))?;
    service
        .authenticate_with_env()
        .map_err(|e| anyhow!("Authentication failed: {e}"))?;
    Ok(service)
}
