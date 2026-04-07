//! Artist search and browse operations.

use crate::{
    api::{
        content::{get_by_id, paginated, search},
        service::QobuzApiService,
    },
    errors::QobuzApiError,
    models::{album::Album, artist::Artist, search::ItemSearchResult},
};
/// Searches for artists matching the query.
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
/// A paginated `ItemSearchResult` containing matching artists.
///
/// # Errors
///
/// Returns a `QobuzApiError` if not authenticated or the API request fails.
pub async fn search_artists(
    service: &QobuzApiService,
    query: &str,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<ItemSearchResult<Box<Artist>>, QobuzApiError> {
    search(service, "/artist/search", query, limit, offset).await
}

/// Retrieves artist details by ID.
///
/// # Arguments
///
/// * `service` - Authenticated API service
/// * `artist_id` - Artist identifier
/// * `extra` - Optional extra fields to include
///
/// # Returns
///
/// The artist details.
///
/// # Errors
///
/// Returns a `QobuzApiError` if not authenticated or the API request fails.
pub async fn get_artist(
    service: &QobuzApiService,
    artist_id: i32,
    extra: Option<&str>,
) -> Result<Artist, QobuzApiError> {
    get_by_id(service, "/artist/get", "artist_id", artist_id, extra).await
}

/// Retrieves an artist's release list (discography).
///
/// # Arguments
///
/// * `service` - Authenticated API service
/// * `artist_id` - Artist identifier
/// * `limit` - Maximum number of releases to return
/// * `offset` - Pagination offset
///
/// # Returns
///
/// A paginated `ItemSearchResult` containing the artist's albums.
///
/// # Errors
///
/// Returns a `QobuzApiError` if not authenticated or the API request fails.
pub async fn get_release_list(
    service: &QobuzApiService,
    artist_id: i32,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<ItemSearchResult<Box<Album>>, QobuzApiError> {
    paginated(
        service,
        "/artist/getReleasesList",
        "artist_id",
        artist_id,
        limit,
        offset,
    )
    .await
}
