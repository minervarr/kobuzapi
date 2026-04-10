//! Playlist search and browse operations.

use crate::{
    api::{
        content::{get_by_id, search},
        service::QobuzApiService,
    },
    errors::QobuzApiError,
    models::{
        playlist::Playlist,
        search::{ItemSearchResult, PlaylistSearchResponse},
    },
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
    let resp: PlaylistSearchResponse =
        search(service, "/playlist/search", query, limit, offset).await?;
    Ok(resp.playlists)
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

#[cfg(test)]
mod tests {
    use {
        anyhow::{Result, anyhow, ensure},
        tokio::runtime::Runtime,
    };

    use crate::{
        api::{
            content::playlists::{get_playlist, search_playlists},
            test_support::{MockServer, make_service},
        },
        assert_empty_search_test,
    };

    #[test]
    fn search_playlists_deserializes_results() -> Result<()> {
        let body = r#"{"playlists":{"items":[{"id":"pl1","name":"Jazz Mix"}],"total":1}}"#;
        let server = MockServer::start(200, body)?;
        let service = make_service(&server.base_url())?;
        let rt = Runtime::new()?;
        let result = rt.block_on(search_playlists(&service, "Jazz", Some(5), None))?;
        let items = result.items.ok_or_else(|| anyhow!("no items"))?;
        ensure!(items.len() == 1);
        ensure!(items[0].name.as_deref() == Some("Jazz Mix"));
        Ok(())
    }

    #[test]
    fn search_playlists_empty_results() -> Result<()> {
        assert_empty_search_test!(
            search_playlists,
            "Nothing",
            r#"{"playlists":{"items":[],"total":0}}"#
        );
        Ok(())
    }

    #[test]
    fn get_playlist_by_id() -> Result<()> {
        let body = r#"{"id":"pl42","name":"My Playlist","tracks_count":10}"#;
        let server = MockServer::start(200, body)?;
        let service = make_service(&server.base_url())?;
        let rt = Runtime::new()?;
        let playlist = rt.block_on(get_playlist(&service, "pl42", None))?;
        ensure!(playlist.name.as_deref() == Some("My Playlist"));
        Ok(())
    }

    #[test]
    fn search_playlists_error_response() -> Result<()> {
        let body = r#"{"status":"error","code":401,"message":"Unauthorized"}"#;
        let server = MockServer::start(401, body)?;
        let service = make_service(&server.base_url())?;
        let rt = Runtime::new()?;
        let result = rt.block_on(search_playlists(&service, "fail", None, None));
        ensure!(result.is_err());
        Ok(())
    }
}
