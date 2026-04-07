//! Catalog search: searches all content types simultaneously.

use crate::{
    api::{
        content::{
            albums::search_albums, artists::search_artists, playlists::search_playlists,
            tracks::search_tracks,
        },
        service::QobuzApiService,
    },
    errors::QobuzApiError,
    models::search::SearchResult,
};

/// Searches all content types (albums, artists, tracks, playlists).
///
/// Returns grouped results for each content type.
///
/// # Arguments
///
/// * `service` - Authenticated API service
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
/// Returns a `QobuzApiError` if any of the parallel search requests fails.
pub async fn search_catalog(
    service: &QobuzApiService,
    query: &str,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<SearchResult, QobuzApiError> {
    let (albums, artists, tracks, playlists) = tokio::try_join!(
        search_albums(service, query, limit, offset),
        search_artists(service, query, limit, offset),
        search_tracks(service, query, limit, offset),
        search_playlists(service, query, limit, offset),
    )?;

    Ok(SearchResult {
        albums: Some(albums),
        artists: Some(artists),
        tracks: Some(tracks),
        playlists: Some(playlists),
    })
}
