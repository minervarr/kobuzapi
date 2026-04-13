//! Album search and browse operations.

use std::{
    fs::create_dir_all,
    path::{Path, PathBuf},
    sync::Arc,
};

use {
    tokio::{spawn, sync::Semaphore},
    tracing::{error, info},
};

use crate::{
    api::{
        content::{
            get_by_id, search,
            tracks::{get_track_file_url_raw, save_track_to_disk},
        },
        requests::{RequestAuth, download_stream},
        service::QobuzApiService,
    },
    errors::QobuzApiError::{self, DownloadError},
    metadata::config::MetadataConfig,
    models::{
        album::Album,
        search::{AlbumSearchResponse, ItemSearchResult},
    },
    sanitize::sanitize_filename,
};

/// Searches for albums matching the query.
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
/// A paginated `ItemSearchResult` containing matching albums.
///
/// # Errors
///
/// Returns a `QobuzApiError` if not authenticated or the API request fails.
pub async fn search_albums(
    service: &QobuzApiService,
    query: &str,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<ItemSearchResult<Box<Album>>, QobuzApiError> {
    let resp: AlbumSearchResponse = search(service, "/album/search", query, limit, offset).await?;
    Ok(resp.albums)
}

/// Retrieves album details by ID.
///
/// # Arguments
///
/// * `service` - Authenticated API service
/// * `album_id` - Album identifier
/// * `extra` - Optional extra fields to include (e.g., `"track_ids"`)
///
/// # Returns
///
/// The album details.
///
/// # Errors
///
/// Returns a `QobuzApiError` if not authenticated or the API request fails.
pub async fn get_album(
    service: &QobuzApiService,
    album_id: &str,
    extra: Option<&str>,
) -> Result<Album, QobuzApiError> {
    get_by_id(service, "/album/get", "album_id", album_id, extra).await
}

/// Downloads all tracks in an album concurrently.
///
/// Creates `{output_dir}/{artist}/{album_title}/` directory structure.
/// Uses `tokio::sync::Semaphore` for bounded concurrency.
///
/// # Arguments
///
/// * `service` - Authenticated API service
/// * `album_id` - Album identifier
/// * `format_id` - Quality format ID (5=MP3, 6=FLAC 16-bit, 7=FLAC 24-bit/96kHz, 27=FLAC
///   24-bit/192kHz)
/// * `output_dir` - Base output directory for downloaded files
/// * `config` - Optional metadata configuration for tagging
/// * `concurrency` - Maximum number of concurrent downloads
///
/// # Returns
///
/// A vector of paths to the downloaded track files.
///
/// # Errors
///
/// Returns a `QobuzApiError` if album retrieval, track download, or I/O fails.
pub async fn download_album(
    service: &QobuzApiService,
    album_id: &str,
    format_id: i32,
    output_dir: &Path,
    config: Option<&MetadataConfig>,
    concurrency: Option<usize>,
) -> Result<Vec<PathBuf>, QobuzApiError> {
    let album = get_album(service, album_id, Some("track_ids")).await?;

    if let Some(cfg) = config {
        info!(
            album_id,
            ?cfg,
            "Metadata config provided for album download"
        );
    }

    let artist_name = album
        .artist
        .as_ref()
        .and_then(|a| a.name.as_deref())
        .unwrap_or("Unknown Artist");

    let album_title = album.title.as_deref().unwrap_or("Unknown Album");

    let dir = output_dir
        .join(sanitize_filename(artist_name))
        .join(sanitize_filename(album_title));

    create_dir_all(&dir)?;

    let track_ids = album.track_ids.unwrap_or_default();
    let max_concurrent = concurrency.unwrap_or(4);
    let semaphore = Arc::new(Semaphore::new(max_concurrent));
    let mut handles = Vec::new();

    for track_id in track_ids {
        let permit = Arc::clone(&semaphore)
            .acquire_owned()
            .await
            .map_err(|e| DownloadError {
                message: format!("Semaphore error: {e}"),
            })?;

        let dir_clone = dir.clone();
        let service_client = QobuzApiService::http_client_ref()?;
        let app_id = service.app_id.clone();
        let app_secret = service.app_secret.clone();
        let token = service.require_auth_token()?.to_string();
        let base_url = service.base_url().to_string();

        let handle = spawn(async move {
            let permit = permit;

            let auth = RequestAuth {
                app_id: &app_id,
                app_secret: &app_secret,
                user_auth_token: &token,
            };

            let file_url =
                get_track_file_url_raw(&*service_client, &base_url, &auth, track_id, format_id)
                    .await?;

            let url = file_url.url.ok_or_else(|| DownloadError {
                message: format!("No download URL for track {track_id}"),
            })?;

            let response = download_stream(&*service_client, &url, &token, None).await?;

            let path = save_track_to_disk(response, track_id, &dir_clone, format_id).await?;

            drop(permit);
            Ok::<PathBuf, QobuzApiError>(path)
        });

        handles.push(handle);
    }

    let mut results = Vec::new();
    for handle in handles {
        match handle.await {
            Ok(Ok(path)) => results.push(path),
            Ok(Err(e)) => {
                error!(error = %e, "Track download failed");
                return Err(e);
            }
            Err(e) => {
                return Err(DownloadError {
                    message: format!("Task join error: {e}"),
                });
            }
        }
    }

    info!(album_id, count = results.len(), "Album download complete");
    Ok(results)
}

#[cfg(test)]
mod tests {
    use {
        anyhow::{Result, anyhow, ensure},
        tokio::runtime::Runtime,
    };

    use crate::{
        api::{
            content::albums::{get_album, search_albums},
            test_support::{MockServer, make_service},
        },
        assert_empty_search_test,
    };

    #[test]
    fn search_albums_deserializes_results() -> Result<()> {
        let body = r#"{"albums":{"items":[{"id":"123","title":"Test Album"}],"total":1}}"#;
        let server = MockServer::start(200, body)?;
        let service = make_service(&server.base_url())?;
        let rt = Runtime::new()?;
        let result = rt.block_on(search_albums(&service, "Test", Some(5), None))?;
        let items = result.items.ok_or_else(|| anyhow!("no items"))?;
        ensure!(items.len() == 1);
        ensure!(items[0].title.as_deref() == Some("Test Album"));
        Ok(())
    }

    #[test]
    fn search_albums_empty_results() -> Result<()> {
        assert_empty_search_test!(
            search_albums,
            "Nonexistent",
            r#"{"albums":{"items":[],"total":0}}"#
        );
        Ok(())
    }

    #[test]
    fn search_albums_error_response() -> Result<()> {
        let body = r#"{"status":"error","code":400,"message":"Bad request"}"#;
        let server = MockServer::start(400, body)?;
        let service = make_service(&server.base_url())?;
        let rt = Runtime::new()?;
        let result = rt.block_on(search_albums(&service, "Test", None, None));
        ensure!(result.is_err());
        Ok(())
    }

    #[test]
    fn get_album_by_id() -> Result<()> {
        let body = r#"{"id":"sr6843","title":"Kind of Blue","tracks_count":5}"#;
        let server = MockServer::start(200, body)?;
        let service = make_service(&server.base_url())?;
        let rt = Runtime::new()?;
        let album = rt.block_on(get_album(&service, "sr6843", None))?;
        ensure!(album.title.as_deref() == Some("Kind of Blue"));
        ensure!(album.tracks_count == Some(5));
        Ok(())
    }

    #[test]
    fn get_album_with_extra_param() -> Result<()> {
        let body = r#"{"id":"sr6843","title":"Kind of Blue","track_ids":[1,2,3]}"#;
        let server = MockServer::start(200, body)?;
        let service = make_service(&server.base_url())?;
        let rt = Runtime::new()?;
        let album = rt.block_on(get_album(&service, "sr6843", Some("track_ids")))?;
        let ids = album.track_ids.ok_or_else(|| anyhow!("no track_ids"))?;
        ensure!(ids == vec![1, 2, 3]);
        Ok(())
    }

    #[test]
    fn get_album_not_found() -> Result<()> {
        let body = r#"{"status":"error","code":404,"message":"Album not found"}"#;
        let server = MockServer::start(404, body)?;
        let service = make_service(&server.base_url())?;
        let rt = Runtime::new()?;
        let result = rt.block_on(get_album(&service, "nonexistent", None));
        ensure!(result.is_err());
        Ok(())
    }
}
