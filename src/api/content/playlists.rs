//! Playlist search and browse operations.

use crate::{
    api::{
        content::{get_by_id, search},
        service::QobuzApiService,
    },
    errors::QobuzApiError,
    models::{playlist::Playlist, search::ItemSearchResult},
};

/// Searches for playlists matching the query.
///
/// # Arguments
///
/// * `service` - Authenticated API service
/// * `query` - Search query string
/// * `limit` - Maximum number of results to return
/// * `offset` - Pagination offset
///
/// # Returns
///
/// A paginated `ItemSearchResult` containing matching playlists.
///
/// # Errors
///
/// Returns a `QobuzApiError` if not authenticated or the API request fails.
pub async fn search_playlists(
    service: &QobuzApiService,
    query: &str,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<ItemSearchResult<Box<Playlist>>, QobuzApiError> {
    search(service, "/playlist/search", query, limit, offset).await
}

/// Retrieves playlist details by ID.
///
/// # Arguments
///
/// * `service` - Authenticated API service
/// * `playlist_id` - Playlist identifier
/// * `extra` - Optional extra fields to include
///
/// # Returns
///
/// The playlist details.
///
/// # Errors
///
/// Returns a `QobuzApiError` if not authenticated or the API request fails.
pub async fn get_playlist(
    service: &QobuzApiService,
    playlist_id: &str,
    extra: Option<&str>,
) -> Result<Playlist, QobuzApiError> {
    get_by_id(service, "/playlist/get", "playlist_id", playlist_id, extra).await
}
