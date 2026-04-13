//! Download file I/O helpers: partial file detection, stream-to-disk writing, cover art fetch.

use std::{
    fs::{create_dir_all, metadata},
    path::{Path, PathBuf},
};

use {
    reqwest::Response,
    tokio::{
        fs::{File, OpenOptions},
        io::AsyncWriteExt,
    },
    tokio_stream::StreamExt,
};

use crate::{
    api::service::QobuzApiService, errors::QobuzApiError,
    metadata::extractor::ComprehensiveMetadata, models::album::Album,
};

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
    let size = metadata(path).ok().map(|m| m.len())?;
    (size > 0).then_some(size)
}

/// Writes a streaming HTTP response to a file, optionally appending.
///
/// # Arguments
///
/// * `response` - HTTP response containing the audio stream
/// * `path` - Destination file path
/// * `append` - If `true`, append to existing file; otherwise create/overwrite
///
/// # Returns
///
/// `Ok(())` on success.
///
/// # Errors
///
/// Returns a `QobuzApiError` on file I/O or stream read failures.
pub async fn write_response_to_file(
    response: Response,
    path: &Path,
    append: bool,
) -> Result<(), QobuzApiError> {
    let mut file = if append {
        OpenOptions::new().append(true).open(path).await?
    } else {
        File::create(path).await?
    };
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
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
) -> Result<PathBuf, QobuzApiError> {
    create_dir_all(output_dir)?;

    let ext = Album::extension_for_format(format_id);
    let path = output_dir.join(format!("{track_id:02}.{ext}"));
    write_response_to_file(response, &path, append).await?;
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
    meta.cover_art_url.as_ref()?;
    let url = meta.cover_art_url.as_deref()?;
    let resp = service
        .http_client()
        .get_with_auth(url, token, None)
        .await
        .ok()?;
    resp.bytes().await.ok().map(|b| b.to_vec())
}
