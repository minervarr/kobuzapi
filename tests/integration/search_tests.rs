//! Integration tests for search against the live Qobuz API.
//!
//! These tests authenticate against the real API and perform actual searches
//! using well-known artist/album/track names, verifying that results contain
//! meaningful data matching the query.
//!
//! **Tests FAIL if credentials are missing or wrong.** There is no silent skip.
//!
//! Setup: copy `.env.example` to `.env` and fill in your credentials, then:
//!
//! `cargo test --test search-integration --features live-tests`
//!
//! In CI without credentials, run `cargo test` to run only unit tests and the mock integration.

mod common;

use std::{env::var, path::Path, sync::OnceLock};

use qobuz_api_rust_refactor::api::service::QobuzApiService;

use {
    anyhow::{Result, anyhow},
    dotenvy::from_path,
    tracing::{Level, warn},
    tracing_subscriber::{EnvFilter, fmt},
};

use common::ensure_env_credentials;

/// Static once-cell to ensure logging is initialized only once across all tests.
static LOG_GUARD: OnceLock<()> = OnceLock::new();

/// Static once-cell to cache test keywords from environment variables.
static TEST_KEYWORDS: OnceLock<TestKeywords> = OnceLock::new();

/// Search keywords for integration tests, configurable via `.env`.
struct TestKeywords {
    /// Album search query #1.
    album_query_1: String,
    /// Album search query #2.
    album_query_2: String,
    /// Artist search query #1.
    artist_query_1: String,
    /// Artist search query #2.
    artist_query_2: String,
    /// Track search query #1.
    track_query_1: String,
    /// Track search query #2.
    track_query_2: String,
    /// Playlist search query #1.
    playlist_query_1: String,
    /// Catalog search query.
    catalog_query: String,
    /// Pagination search query.
    pagination_query: String,
}

impl TestKeywords {
    /// Creates a `TestKeywords` instance from environment variables.
    ///
    /// # Returns
    ///
    /// A `TestKeywords` with values from `.env` or defaults.
    fn from_env() -> Self {
        // Load .env before reading variables
        let env_path = Path::new(".env");
        if env_path.exists()
            && let Err(e) = from_path(env_path)
        {
            warn!(error = %e, "Failed to load .env");
        }

        Self {
            album_query_1: var("TEST_SEARCH_ALBUM_QUERY_1")
                .unwrap_or_else(|_| "The Dark Side of the Moon Pink Floyd".to_string()),
            album_query_2: var("TEST_SEARCH_ALBUM_QUERY_2")
                .unwrap_or_else(|_| "Kind of Blue Miles Davis".to_string()),
            artist_query_1: var("TEST_SEARCH_ARTIST_QUERY_1")
                .unwrap_or_else(|_| "Miles Davis".to_string()),
            artist_query_2: var("TEST_SEARCH_ARTIST_QUERY_2")
                .unwrap_or_else(|_| "Radiohead".to_string()),
            track_query_1: var("TEST_SEARCH_TRACK_QUERY_1")
                .unwrap_or_else(|_| "So What Miles Davis".to_string()),
            track_query_2: var("TEST_SEARCH_TRACK_QUERY_2")
                .unwrap_or_else(|_| "Bohemian Rhapsody Queen".to_string()),
            playlist_query_1: var("TEST_SEARCH_PLAYLIST_QUERY_1")
                .unwrap_or_else(|_| "jazz".to_string()),
            catalog_query: var("TEST_SEARCH_CATALOG_QUERY")
                .unwrap_or_else(|_| "Miles Davis".to_string()),
            pagination_query: var("TEST_SEARCH_PAGINATION_QUERY")
                .unwrap_or_else(|_| "Rock".to_string()),
        }
    }
}

/// Initializes logging for tests using tracing.
fn init_logging() {
    let () = LOG_GUARD.get_or_init(|| {
        fmt()
            .with_env_filter(EnvFilter::from_default_env().add_directive(Level::INFO.into()))
            .init();
    });
}

/// Returns the test keywords, initializing from environment on first call.
///
/// # Returns
///
/// A static reference to `TestKeywords` with values from `.env` or defaults.
fn get_test_keywords() -> &'static TestKeywords {
    TEST_KEYWORDS.get_or_init(TestKeywords::from_env)
}

/// Creates an authenticated Qobuz API service.
///
/// # Returns
///
/// An authenticated `QobuzApiService` ready for API calls.
///
/// # Errors
///
/// Returns an error if credentials are missing or authentication fails.
fn create_authenticated_service() -> Result<QobuzApiService> {
    ensure_env_credentials()?;

    let mut service =
        QobuzApiService::new().map_err(|e| anyhow!("Failed to create service: {e}"))?;
    service
        .authenticate_with_env()
        .map_err(|e| anyhow!("Authentication failed: {e}"))?;
    Ok(service)
}

#[cfg(test)]
mod tests {
    use {
        anyhow::{Result, anyhow, ensure},
        tracing::info,
    };

    use qobuz_api_rust_refactor::models::{
        album::Album, artist::Artist, playlist::Playlist, track::Track,
    };

    use super::{create_authenticated_service, get_test_keywords, init_logging};

    #[test]
    fn init() {
        init_logging();
    }

    fn log_albums(label: &str, items: &[Box<Album>]) {
        info!("{} ({} results)", label, items.len());
        for (i, a) in items.iter().take(5).enumerate() {
            let artist = a
                .artist
                .as_ref()
                .and_then(|a| a.name.as_deref())
                .unwrap_or("?");
            info!(
                "  {}. {} — {}",
                i + 1,
                a.title.as_deref().unwrap_or("?"),
                artist
            );
        }
    }

    fn log_artists(label: &str, items: &[Box<Artist>]) {
        info!("{} ({} results)", label, items.len());
        for (i, a) in items.iter().take(5).enumerate() {
            let albums = a.albums_count.map_or("?".to_string(), |c| c.to_string());
            info!(
                "  {}. {} ({albums} albums)",
                i + 1,
                a.name.as_deref().unwrap_or("?")
            );
        }
    }

    fn log_tracks(label: &str, items: &[Box<Track>]) {
        info!("{} ({} results)", label, items.len());
        for (i, t) in items.iter().take(5).enumerate() {
            let album = t
                .album
                .as_ref()
                .and_then(|a| a.title.as_deref())
                .unwrap_or("?");
            let artist = t
                .performer
                .as_ref()
                .and_then(|p| p.name.as_deref())
                .unwrap_or("?");
            info!(
                "  {}. {} — {} [{album}]",
                i + 1,
                t.title.as_deref().unwrap_or("?"),
                artist
            );
        }
    }

    fn log_playlists(label: &str, items: &[Box<Playlist>]) {
        info!("{} ({} results)", label, items.len());
        for (i, p) in items.iter().take(5).enumerate() {
            let count = p.tracks_count.map_or("?".to_string(), |c| c.to_string());
            info!(
                "  {}. {} ({count} tracks)",
                i + 1,
                p.name.as_deref().unwrap_or("?")
            );
        }
    }

    fn validate_album_results(items: &[Box<Album>], query: &str) -> Result<()> {
        ensure!(
            !items.is_empty(),
            "search_albums should return at least one album for '{query}'"
        );

        ensure!(
            items.iter().any(|a| a.title.is_some()),
            "At least one album should have a title, got: {:?}",
            items
                .iter()
                .filter_map(|a| a.title.as_deref())
                .collect::<Vec<_>>()
        );

        Ok(())
    }

    fn validate_artist_results(items: &[Box<Artist>], query: &str) -> Result<()> {
        ensure!(
            !items.is_empty(),
            "search_artists should return artists for '{query}'"
        );

        ensure!(
            items.iter().any(|a| a.name.is_some()),
            "At least one artist should have a name, got: {:?}",
            items
                .iter()
                .filter_map(|a| a.name.as_deref())
                .collect::<Vec<_>>()
        );

        Ok(())
    }

    fn validate_track_results(items: &[Box<Track>], query: &str) -> Result<()> {
        ensure!(
            !items.is_empty(),
            "search_tracks should return tracks for '{query}'"
        );

        ensure!(
            items.iter().any(|t| t.title.is_some()),
            "At least one track should have a title, got: {:?}",
            items
                .iter()
                .filter_map(|t| t.title.as_deref())
                .collect::<Vec<_>>()
        );

        Ok(())
    }

    #[test]
    fn live_search_albums_query_1() -> Result<()> {
        let kw = get_test_keywords();
        let service = create_authenticated_service()?;

        let result = service.search_albums(&kw.album_query_1, Some(5), None)?;
        let items = result
            .items
            .ok_or_else(|| anyhow!("search_albums returned no items for '{}'", kw.album_query_1))?;

        validate_album_results(&items, &kw.album_query_1)?;
        log_albums(&format!("Albums for '{}'", kw.album_query_1), &items);
        Ok(())
    }

    #[test]
    fn live_search_albums_query_2() -> Result<()> {
        let kw = get_test_keywords();
        let service = create_authenticated_service()?;

        let result = service.search_albums(&kw.album_query_2, Some(5), None)?;
        let items = result
            .items
            .ok_or_else(|| anyhow!("search_albums returned no items for '{}'", kw.album_query_2))?;

        validate_album_results(&items, &kw.album_query_2)?;

        let first = items
            .first()
            .ok_or_else(|| anyhow!("empty album results for '{}'", kw.album_query_2))?;

        ensure!(first.title.is_some(), "Album should have a title");
        ensure!(first.artist.is_some(), "Album should have artist metadata");

        ensure!(
            first
                .artist
                .as_ref()
                .and_then(|a| a.name.as_deref())
                .is_some(),
            "Album artist should have a name, got: {:?}",
            first.artist.as_ref().and_then(|a| a.name.as_deref())
        );

        log_albums(&format!("Albums for '{}'", kw.album_query_2), &items);
        Ok(())
    }

    #[test]
    fn live_search_artists_query_1() -> Result<()> {
        let kw = get_test_keywords();
        let service = create_authenticated_service()?;

        let result = service.search_artists(&kw.artist_query_1, Some(5), None)?;
        let items = result.items.ok_or_else(|| {
            anyhow!(
                "search_artists returned no items for '{}'",
                kw.artist_query_1
            )
        })?;

        validate_artist_results(&items, &kw.artist_query_1)?;
        log_artists(&format!("Artists for '{}'", kw.artist_query_1), &items);
        Ok(())
    }

    #[test]
    fn live_search_artists_query_2() -> Result<()> {
        let kw = get_test_keywords();
        let service = create_authenticated_service()?;

        let result = service.search_artists(&kw.artist_query_2, Some(5), None)?;
        let items = result.items.ok_or_else(|| {
            anyhow!(
                "search_artists returned no items for '{}'",
                kw.artist_query_2
            )
        })?;

        validate_artist_results(&items, &kw.artist_query_2)?;
        log_artists(&format!("Artists for '{}'", kw.artist_query_2), &items);
        Ok(())
    }

    #[test]
    fn live_search_tracks_query_1() -> Result<()> {
        let kw = get_test_keywords();
        let service = create_authenticated_service()?;

        let result = service.search_tracks(&kw.track_query_1, Some(5), None)?;
        let items = result
            .items
            .ok_or_else(|| anyhow!("search_tracks returned no items for '{}'", kw.track_query_1))?;

        validate_track_results(&items, &kw.track_query_1)?;

        let first = items
            .first()
            .ok_or_else(|| anyhow!("empty track results for '{}'", kw.track_query_1))?;

        ensure!(first.title.is_some(), "Track should have a title");
        ensure!(first.album.is_some(), "Track should have album metadata");

        log_tracks(&format!("Tracks for '{}'", kw.track_query_1), &items);
        Ok(())
    }

    #[test]
    fn live_search_tracks_query_2() -> Result<()> {
        let kw = get_test_keywords();
        let service = create_authenticated_service()?;

        let result = service.search_tracks(&kw.track_query_2, Some(5), None)?;
        let items = result
            .items
            .ok_or_else(|| anyhow!("search_tracks returned no items for '{}'", kw.track_query_2))?;

        validate_track_results(&items, &kw.track_query_2)?;
        log_tracks(&format!("Tracks for '{}'", kw.track_query_2), &items);
        Ok(())
    }

    #[test]
    fn live_search_playlists_query_1() -> Result<()> {
        let kw = get_test_keywords();
        let service = create_authenticated_service()?;

        let result = service.search_playlists(&kw.playlist_query_1, Some(5), None)?;

        let items = result.items.as_ref().ok_or_else(|| {
            anyhow!(
                "search_playlists returned no items for '{}'",
                kw.playlist_query_1
            )
        })?;

        ensure!(
            !items.is_empty(),
            "search_playlists should return playlists for '{}'",
            kw.playlist_query_1
        );

        ensure!(
            items.iter().any(|p| p.name.is_some()),
            "At least one playlist should have a name, got: {:?}",
            items
                .iter()
                .filter_map(|p| p.name.as_deref())
                .collect::<Vec<_>>()
        );

        log_playlists(&format!("Playlists for '{}'", kw.playlist_query_1), items);
        Ok(())
    }

    #[test]
    fn live_search_catalog() -> Result<()> {
        let kw = get_test_keywords();
        let service = create_authenticated_service()?;

        let result = service.search_catalog(&kw.catalog_query, Some(3), None)?;

        let albums = result.albums.and_then(|a| a.items).unwrap_or_default();
        let artists = result.artists.and_then(|a| a.items).unwrap_or_default();
        let tracks = result.tracks.and_then(|t| t.items).unwrap_or_default();

        ensure!(
            !albums.is_empty(),
            "catalog search for '{}' should return albums",
            kw.catalog_query
        );
        ensure!(
            !artists.is_empty(),
            "catalog search for '{}' should return artists",
            kw.catalog_query
        );
        ensure!(
            !tracks.is_empty(),
            "catalog search for '{}' should return tracks",
            kw.catalog_query
        );

        ensure!(
            artists.iter().any(|a| a.name.is_some()),
            "At least one artist should have a name, got: {:?}",
            artists
                .iter()
                .filter_map(|a| a.name.as_deref())
                .collect::<Vec<_>>()
        );

        info!("\n  Catalog search for '{}':", kw.catalog_query);
        log_albums("Albums", &albums);
        log_artists("Artists", &artists);
        log_tracks("Tracks", &tracks);
        Ok(())
    }

    #[test]
    fn live_search_albums_pagination() -> Result<()> {
        let kw = get_test_keywords();
        let service = create_authenticated_service()?;

        let result = service.search_albums(&kw.pagination_query, Some(2), None)?;

        let items = result.items.as_ref().ok_or_else(|| {
            anyhow!(
                "search_albums returned no items for '{}'",
                kw.pagination_query
            )
        })?;

        ensure!(
            items.len() <= 2,
            "search_albums with limit=2 should return at most 2 items, got {}",
            items.len()
        );

        log_albums(
            &format!("Albums for '{}' (limit=2)", kw.pagination_query),
            items,
        );
        Ok(())
    }
}
