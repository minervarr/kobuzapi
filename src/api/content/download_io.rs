//! Download file I/O helpers: partial file detection, stream-to-disk writing, cover art fetch.

use std::{
    fs::{create_dir_all, metadata},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering::Relaxed},
};

use {
    reqwest::Response,
    tokio::{
        fs::{File, OpenOptions},
        io::AsyncWriteExt,
    },
    tokio_stream::StreamExt,
    tracing::{debug, warn},
};

use crate::{
    api::{
        content::tracks::get_track_file_url, requests::download_stream, service::QobuzApiService,
    },
    errors::QobuzApiError::{self, Canceled, DownloadError, HttpError},
    metadata::extractor::ComprehensiveMetadata,
    models::album::Album,
};

/// Maximum number of retry attempts on download network errors.
pub const MAX_DOWNLOAD_RETRIES: u32 = 3;

/// Base delay in milliseconds for download retry exponential backoff.
pub const DOWNLOAD_RETRY_BASE_DELAY_MS: u64 = 2000;

/// Detects whether a partial file exists on disk with non-zero size.
///
/// # Arguments
///
/// * `path` - File path to check
///
/// # Returns
///
/// `Some(size)` if the file exists and has content, `None` otherwise.
#[must_use]
pub fn detect_partial_file(path: &Path) -> Option<u64> {
    let size = match metadata(path) {
        Ok(m) => m.len(),
        Err(_) => return None,
    };
    (size > 0).then_some(size)
}

/// Writes a streaming HTTP response to a file, optionally appending.
///
/// # Arguments
///
/// * `response` - HTTP response containing the audio stream
/// * `path` - Destination file path
/// * `append` - If `true`, append to existing file; otherwise create/overwrite
/// * `cancel` - Optional cancellation flag checked between chunks
///
/// # Returns
///
/// `Ok(())` on success.
///
/// # Errors
///
/// Returns a `QobuzApiError` on file I/O or stream read failures, or `Canceled` when the
/// cancellation flag is set.
pub async fn write_response_to_file(
    response: Response,
    path: &Path,
    append: bool,
    cancel: Option<&AtomicBool>,
) -> Result<(), QobuzApiError> {
    let mut file = if append {
        OpenOptions::new().append(true).open(path).await?
    } else {
        File::create(path).await?
    };
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        if cancel.is_some_and(|c| c.load(Relaxed)) {
            return Err(Canceled);
        }
        file.write_all(&chunk?).await?;
    }
    file.flush().await?;
    Ok(())
}

/// Saves a streaming response to disk with formatted filename.
///
/// # Arguments
///
/// * `response` - HTTP response containing the audio stream
/// * `track_id` - Track identifier (used in filename)
/// * `output_dir` - Directory to save the file
/// * `format_id` - Quality format ID (determines file extension)
/// * `append` - If `true`, append to existing file (resume); otherwise create new
/// * `cancel` - Optional cancellation flag checked during streaming
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
    append: bool,
    cancel: Option<&AtomicBool>,
) -> Result<PathBuf, QobuzApiError> {
    create_dir_all(output_dir)?;

    let ext = Album::extension_for_format(format_id);
    let path = output_dir.join(format!("{track_id:02}.{ext}"));
    write_response_to_file(response, &path, append, cancel).await?;
    Ok(path)
}

/// Fetches cover art binary data for a track if a cover art URL is available.
///
/// # Arguments
///
/// * `service` - Authenticated API service
/// * `meta` - Comprehensive metadata containing the cover art URL
/// * `token` - User authentication token
///
/// # Returns
///
/// Cover art image data as bytes, or `None` if unavailable.
pub async fn fetch_track_cover(
    service: &QobuzApiService,
    meta: &ComprehensiveMetadata,
    token: &str,
) -> Option<Vec<u8>> {
    let url = meta.cover_art_url.as_deref()?;
    let resp = match service.http_client().get_with_auth(url, token, None).await {
        Ok(r) => r,
        Err(e) => {
            debug!(error = %e, "Cover art HTTP request failed");
            return None;
        }
    };
    match resp.bytes().await {
        Err(e) => {
            debug!(error = %e, "Failed to read cover art bytes");
            None
        }
        Ok(b) => Some(b.to_vec()),
    }
}

/// Performs a single download attempt, optionally resuming from a partial file.
///
/// # Arguments
///
/// * `service` - Authenticated API service
/// * `track_id` - Track identifier
/// * `format_id` - Quality format ID
/// * `path` - Destination file path
/// * `cancel` - Optional cancellation flag checked during streaming
///
/// # Returns
///
/// `Ok(true)` if the download resumed from a partial file, `Ok(false)` if fresh.
///
/// # Errors
///
/// Returns a `QobuzApiError` if the track file URL cannot be retrieved, the download stream
/// fails, or writing to disk fails.
pub async fn attempt_download(
    service: &QobuzApiService,
    track_id: i32,
    format_id: i32,
    path: &Path,
    cancel: Option<&AtomicBool>,
) -> Result<bool, QobuzApiError> {
    let offset = detect_partial_file(path);
    let range = offset.map(|s| format!("bytes={s}-"));

    let file_url = get_track_file_url(service, track_id, format_id).await?;

    let url = file_url.url.ok_or_else(|| DownloadError {
        message: format!("No download URL for track {track_id}"),
    })?;

    let response = download_stream(service.http_client(), &url, range.as_deref()).await?;

    let resumed = offset.is_some() && response.status().as_u16() == 206;
    if offset.is_some() && !resumed {
        warn!(
            track_id,
            path = %path.display(),
            "Server did not support Range request, re-downloading full file"
        );
    }

    write_response_to_file(response, path, resumed, cancel).await?;

    Ok(resumed)
}

/// Checks whether an error is a transient network failure worth retrying.
///
/// # Arguments
///
/// * `err` - The error to inspect
///
/// # Returns
///
/// `true` if the error is a connect timeout, body, or decode failure.
#[must_use]
pub fn is_retryable_network_error(err: &QobuzApiError) -> bool {
    let HttpError(reqwest_err) = err else {
        return false;
    };
    reqwest_err.is_connect()
        || reqwest_err.is_timeout()
        || reqwest_err.is_body()
        || reqwest_err.is_decode()
        || reqwest_err.is_request()
}
