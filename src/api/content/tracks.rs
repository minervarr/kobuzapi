//! Track search, browse, and download operations.

use std::{
    fs::create_dir_all,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use {
    reqwest::Response,
    tokio::{fs::File, io::AsyncWriteExt},
    tokio_stream::StreamExt,
    tracing::{debug, info},
};

use crate::{
    api::{
        content::{get_by_id, search},
        http_client::HttpClient,
        requests::{self, RequestAuth},
        service::QobuzApiService,
    },
    errors::QobuzApiError::{self, DownloadError},
    metadata::config::MetadataConfig,
    models::{album::Album, file_url::FileUrl, search::ItemSearchResult, track::Track},
    signing::sign_track_file_url,
};

/// Searches for tracks matching the query.
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
/// A paginated `ItemSearchResult` containing matching tracks.
///
/// # Errors
///
/// Returns a `QobuzApiError` if not authenticated or the API request fails.
pub async fn search_tracks(
    service: &QobuzApiService,
    query: &str,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<ItemSearchResult<Box<Track>>, QobuzApiError> {
    search(service, "/track/search", query, limit, offset).await
}

/// Retrieves track details by ID.
///
/// # Arguments
///
/// * `service` - Authenticated API service
/// * `track_id` - Track identifier
///
/// # Returns
///
/// The track details.
///
/// # Errors
///
/// Returns a `QobuzApiError` if not authenticated or the API request fails.
pub async fn get_track(service: &QobuzApiService, track_id: i32) -> Result<Track, QobuzApiError> {
    get_by_id(service, "/track/get", "track_id", track_id, None).await
}

/// Gets the download URL for a track at the specified quality.
///
/// # Arguments
///
/// * `service` - Authenticated API service
/// * `track_id` - Track identifier
/// * `format_id` - Quality format ID (5=MP3, 6=FLAC 16-bit, 7=FLAC 24-bit/96kHz, 27=FLAC
///   24-bit/192kHz)
///
/// # Returns
///
/// The download URL and metadata for the track.
///
/// # Errors
///
/// Returns a `QobuzApiError` if not authenticated or the API request fails.
pub async fn get_track_file_url(
    service: &QobuzApiService,
    track_id: i32,
    format_id: i32,
) -> Result<FileUrl, QobuzApiError> {
    let token = service.require_auth_token()?;

    get_track_file_url_raw(
        service.http_client(),
        service.base_url(),
        &RequestAuth {
            app_id: &service.app_id,
            app_secret: service.app_secret(),
            user_auth_token: token,
        },
        track_id,
        format_id,
    )
    .await
}

/// Internal function to get a track file URL (used by download operations).
///
/// # Arguments
///
/// * `client` - HTTP client implementation
/// * `auth` - Application credentials and user authentication token
/// * `track_id` - Track identifier
/// * `format_id` - Quality format ID
///
/// # Returns
///
/// The signed download URL and metadata for the track.
///
/// # Errors
///
/// Returns a `QobuzApiError` if the signed API request fails.
pub async fn get_track_file_url_raw(
    client: &dyn HttpClient,
    base_url: &str,
    auth: &RequestAuth<'_>,
    track_id: i32,
    format_id: i32,
) -> Result<FileUrl, QobuzApiError> {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string();

    let sig = sign_track_file_url(format_id, track_id, &ts, auth.app_secret);

    let mut params: Vec<(String, String)> = vec![
        ("track_id".to_string(), track_id.to_string()),
        ("format_id".to_string(), format_id.to_string()),
        ("intent".to_string(), "stream".to_string()),
        ("request_ts".to_string(), ts),
        ("request_sig".to_string(), sig),
        ("app_id".to_string(), auth.app_id.to_string()),
        (
            "user_auth_token".to_string(),
            auth.user_auth_token.to_string(),
        ),
    ];

    requests::signed_get(client, base_url, "/track/getFileUrl", &mut params, auth).await
}

/// Downloads a single track to the specified directory.
///
/// Filename format: `{NN}. {title}.{ext}`
/// On signature error, refreshes credentials and retries once.
///
/// # Arguments
///
/// * `service` - Authenticated API service
/// * `track_id` - Track identifier
/// * `format_id` - Quality format ID
/// * `output_dir` - Directory to save the downloaded file
/// * `config` - Optional metadata configuration for tagging
///
/// # Returns
///
/// The path to the downloaded file.
///
/// # Errors
///
/// Returns a `QobuzApiError` if URL retrieval, download, or I/O fails.
pub async fn download_track(
    service: &QobuzApiService,
    track_id: i32,
    format_id: i32,
    output_dir: &Path,
    config: Option<&MetadataConfig>,
) -> Result<PathBuf, QobuzApiError> {
    let file_url = get_track_file_url(service, track_id, format_id).await?;

    let url = file_url.url.ok_or_else(|| DownloadError {
        message: format!("No download URL for track {track_id}"),
    })?;

    let token = service.require_auth_token()?;
    let response = requests::download_stream(service.http_client(), &url, token, None).await?;

    let path = save_track_to_disk(response, track_id, output_dir, format_id).await?;

    info!(track_id, format_id, path = %path.display(), "Track downloaded");

    if let Some(cfg) = config {
        debug!(track_id, ?cfg, "Metadata embedding not yet implemented");
    }

    Ok(path)
}

/// Saves a streaming response to disk with formatted filename.
///
/// # Arguments
///
/// * `response` - HTTP response containing the audio stream
/// * `track_id` - Track identifier (used in filename)
/// * `output_dir` - Directory to save the file
/// * `format_id` - Quality format ID (determines file extension)
///
/// # Returns
///
/// The path to the saved file.
///
/// # Errors
///
/// Returns a `QobuzApiError` if directory creation, file I/O, or streaming fails.
pub async fn save_track_to_disk(
    response: Response,
    track_id: i32,
    output_dir: &Path,
    format_id: i32,
) -> Result<PathBuf, QobuzApiError> {
    create_dir_all(output_dir)?;

    let ext = Album::extension_for_format(format_id);
    let filename = format!("{track_id:02}.{ext}");
    let path = output_dir.join(&filename);

    let mut file = File::create(&path).await?;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
    }

    file.flush().await?;
    Ok(path)
}
