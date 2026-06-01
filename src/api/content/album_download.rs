//! Album download operations with concurrent track fetching and metadata embedding.

use std::{
    fs::{create_dir_all, metadata},
    path::{Path, PathBuf},
    result::Result,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

use {
    tokio::{fs::remove_file, spawn, sync::Semaphore, time::sleep},
    tracing::{debug, error, info, warn},
};

use crate::{
    api::{
        content::{
            albums::get_album,
            check_cancel,
            download_io::{
                DOWNLOAD_RETRY_BASE_DELAY_MS, MAX_DOWNLOAD_RETRIES, is_retryable_network_error,
                save_track_to_disk,
            },
            fetch_with_cancel,
            tracks::{get_track, get_track_file_url_raw},
        },
        requests::{RequestAuth, download_stream},
        service::QobuzApiService,
    },
    errors::QobuzApiError::{self, DownloadError},
    metadata::{
        config::{MetadataConfig, MetadataField::CoverArt},
        embedder::embed_metadata_batch,
        extractor::{best_cover_url, extract_comprehensive_metadata},
    },
    models::album::Album,
    sanitize::sanitize_filename,
};

fn quality_tag(format_id: i32, album: &Album) -> String {
    use crate::models::file_url::quality::*;

    if format_id == MP3_320 {
        return "(320kbps)".to_string();
    }

    let (ceil_depth, ceil_rate): (i32, f64) = match format_id {
        FLAC_24_192 => (24, 192.0),
        FLAC_24_96 => (24, 96.0),
        _ => (16, 44.1),
    };

    let depth = album
        .maximum_bit_depth
        .map_or(ceil_depth, |d| d.min(ceil_depth));
    let rate = album
        .maximum_sampling_rate
        .map_or(ceil_rate, |r| r.min(ceil_rate));

    let rate_str = if rate.fract() == 0.0 {
        format!("{}", rate as i32)
    } else {
        format!("{rate}")
    };

    format!("({depth}-{rate_str})")
}

fn prepare_album_directory(
    album: &Album,
    format_id: i32,
    output_dir: &Path,
) -> Result<(Vec<i32>, PathBuf), QobuzApiError> {
    let artist_name = album
        .artist
        .as_ref()
        .and_then(|a| a.name.as_deref())
        .unwrap_or("Unknown Artist");

    let album_title = album.title.as_deref().unwrap_or("Unknown Album");
    let album_display = match album.version.as_deref().filter(|v| !v.is_empty()) {
        Some(version) => format!("{album_title} ({version})"),
        None => album_title.to_string(),
    };
    let tag = quality_tag(format_id, album);
    let folder_name = format!("{} {tag}", sanitize_filename(&album_display));

    let dir = output_dir
        .join(sanitize_filename(artist_name))
        .join(&folder_name);

    create_dir_all(&dir)?;

    let track_ids = album.track_ids.clone().unwrap_or_default();

    Ok((track_ids, dir))
}

/// Downloads all tracks in an album concurrently, returning their file paths.
///
/// Spawns a tokio task per track, bounded by a semaphore for concurrency control.
///
/// # Arguments
///
/// * `service` - Authenticated API service for signing requests
/// * `track_ids` - Slice of track IDs to download
/// * `format_id` - Quality format ID (5=MP3, 6=FLAC 16-bit, 7=FLAC 24-bit/96kHz, 27=FLAC
///   24-bit/192kHz)
/// * `dir` - Output directory for downloaded files
/// * `concurrency` - Maximum number of concurrent downloads
/// * `cancel` - Optional cancellation flag
///
/// # Returns
///
/// A vector of paths to the downloaded track files.
async fn download_album_tracks(
    service: &QobuzApiService,
    track_ids: &[i32],
    format_id: i32,
    dir: &Path,
    concurrency: Option<usize>,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<Vec<(i32, PathBuf)>, QobuzApiError> {
    let max_concurrent = concurrency.unwrap_or(4);
    let semaphore = Arc::new(Semaphore::new(max_concurrent));
    let mut handles = Vec::new();

    for &track_id in track_ids {
        check_cancel(cancel.as_deref())?;

        let permit = Arc::clone(&semaphore)
            .acquire_owned()
            .await
            .map_err(|e| DownloadError {
                message: format!("Semaphore error: {e}"),
            })?;

        let dir_clone = dir.to_path_buf();
        let service_client = service.clone_http_client();
        let app_id = service.app_id.clone();
        let app_secret = service.app_secret.clone();
        let token = service.require_auth_token()?.to_string();
        let base_url = service.base_url().to_string();
        let cancel_clone = cancel.clone();

        let handle = spawn(async move {
            let permit = permit;

            let auth = RequestAuth {
                app_id: &app_id,
                app_secret: &app_secret,
                user_auth_token: &token,
            };

            let mut last_err: Option<QobuzApiError> = None;

            for attempt in 0..=MAX_DOWNLOAD_RETRIES {
                check_cancel(cancel_clone.as_deref())?;

                let file_url = match get_track_file_url_raw(
                    &*service_client,
                    &base_url,
                    &auth,
                    track_id,
                    format_id,
                )
                .await
                {
                    Ok(fu) => fu,
                    Err(e) if is_retryable_network_error(&e)
                        && attempt < MAX_DOWNLOAD_RETRIES =>
                    {
                        let delay = DOWNLOAD_RETRY_BASE_DELAY_MS * 2u64.pow(attempt);
                        warn!(
                            track_id,
                            attempt,
                            delay_ms = delay,
                            error = %e,
                            "Track URL fetch failed, retrying"
                        );
                        sleep(Duration::from_millis(delay)).await;
                        last_err = Some(e);
                        continue;
                    }
                    Err(e) => {
                        drop(permit);
                        return Err(e);
                    }
                };

                let url = match file_url.url {
                    Some(u) => u,
                    None => {
                        drop(permit);
                        return Err(DownloadError {
                            message: format!("No download URL for track {track_id}"),
                        });
                    }
                };

                let (offset, range) = detect_resume_offset(&dir_clone, track_id, format_id);

                let response =
                    download_stream(&*service_client, &url, range.as_deref()).await;

                // HTTP 416 means the Range offset equals or exceeds the file size —
                // the file on disk is already complete. Return it as-is.
                if let Err(ref e) = response {
                    if offset.is_some() && format!("{e}").contains("416") {
                        let ext = crate::models::album::Album::extension_for_format(format_id);
                        let existing = dir_clone.join(format!("{track_id:02}.{ext}"));
                        if existing.exists() {
                            info!(track_id, format_id, path = %existing.display(), "Track already complete, skipping");
                            drop(permit);
                            return Ok((track_id, existing));
                        }
                    }
                }

                let response = match response {
                    Ok(r) => r,
                    Err(e) if is_retryable_network_error(&e)
                        && attempt < MAX_DOWNLOAD_RETRIES =>
                    {
                        let delay = DOWNLOAD_RETRY_BASE_DELAY_MS * 2u64.pow(attempt);
                        warn!(
                            track_id,
                            attempt,
                            delay_ms = delay,
                            error = %e,
                            "Track download failed, retrying with resume"
                        );
                        sleep(Duration::from_millis(delay)).await;
                        last_err = Some(e);
                        continue;
                    }
                    Err(e) => {
                        drop(permit);
                        return Err(e);
                    }
                };

                let resumed = offset.is_some() && response.status().as_u16() == 206;
                warn_if_not_resumed(offset.is_some(), resumed, track_id);

                if offset.is_some() && !resumed {
                    let ext = crate::models::album::Album::extension_for_format(format_id);
                    let partial_path = dir_clone.join(format!("{track_id:02}.{ext}"));
                    if partial_path.exists() {
                        if let Err(e) = remove_file(&partial_path).await {
                            warn!(track_id, error = %e, "Failed to remove partial file before re-download");
                        }
                    }
                }

                match save_track_to_disk(
                    response,
                    track_id,
                    &dir_clone,
                    format_id,
                    resumed,
                    cancel_clone.as_deref(),
                )
                .await
                {
                    Ok(path) => {
                        info!(
                            track_id,
                            format_id,
                            path = %path.display(),
                            resumed,
                            "Track downloaded"
                        );
                        drop(permit);
                        return Ok((track_id, path));
                    }
                    Err(e) if is_retryable_network_error(&e)
                        && attempt < MAX_DOWNLOAD_RETRIES =>
                    {
                        let delay = DOWNLOAD_RETRY_BASE_DELAY_MS * 2u64.pow(attempt);
                        warn!(
                            track_id,
                            attempt,
                            delay_ms = delay,
                            error = %e,
                            "Track streaming failed, retrying with resume"
                        );
                        sleep(Duration::from_millis(delay)).await;
                        last_err = Some(e);
                        continue;
                    }
                    Err(e) => {
                        drop(permit);
                        return Err(e);
                    }
                }
            }

            drop(permit);
            Err(last_err.unwrap_or_else(|| DownloadError {
                message: format!(
                    "Track {track_id} failed after {MAX_DOWNLOAD_RETRIES} retries"
                ),
            }))
        });

        handles.push(handle);
    }

    let mut results = Vec::new();
    let mut failed = 0usize;
    for handle in handles {
        check_cancel(cancel.as_deref())?;

        match handle.await {
            Ok(Ok(pair)) => results.push(pair),
            Ok(Err(e)) => {
                error!(error = %e, "Track download failed, continuing with remaining tracks");
                failed += 1;
            }
            Err(e) => {
                error!(error = %e, "Track task join error, continuing with remaining tracks");
                failed += 1;
            }
        }
    }

    if results.is_empty() && failed > 0 {
        return Err(DownloadError {
            message: format!("All {failed} track(s) failed to download"),
        });
    }

    if failed > 0 {
        warn!(failed, succeeded = results.len(), "Some tracks failed to download");
    }

    Ok(results)
}

/// Splits a vec of `(track_id, path)` pairs into parallel slices for metadata embedding.
fn unzip_track_results(pairs: Vec<(i32, PathBuf)>) -> (Vec<i32>, Vec<PathBuf>) {
    pairs.into_iter().unzip()
}

/// Embeds metadata into downloaded album tracks.
///
/// Downloads cover art, fetches track metadata, and embeds it into each file.
///
/// # Arguments
///
/// * `service` - Authenticated API service for fetching track metadata
/// * `album` - Album used as fallback metadata source
/// * `track_ids` - Slice of track IDs matching the results order
/// * `results` - Downloaded file paths, one per track ID
/// * `config` - Metadata configuration controlling which fields to embed
///
/// # Returns
///
/// `Ok(())` on success.
async fn embed_album_metadata(
    service: &QobuzApiService,
    album: &Album,
    track_ids: &[i32],
    results: &[PathBuf],
    config: &MetadataConfig,
) -> Result<(), QobuzApiError> {
    let cover_data = download_cover_data(service, album, config).await?;

    let mut file_metas = Vec::new();
    for (&track_id, path) in track_ids.iter().zip(results.iter()) {
        let track = get_track(service, track_id).await?;
        let mut meta = extract_comprehensive_metadata(&track, Some(album), None);
        meta.cover_art_data.clone_from(&cover_data);
        file_metas.push((path.clone(), meta));
    }

    for result in embed_metadata_batch(&file_metas, config) {
        result?;
    }

    Ok(())
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
    cancel: Option<Arc<AtomicBool>>,
) -> Result<Vec<PathBuf>, QobuzApiError> {
    let album = fetch_with_cancel(cancel.as_deref(), || {
        get_album(service, album_id, Some("track_ids"))
    })
    .await?;

    let (track_ids, dir) = prepare_album_directory(&album, format_id, output_dir)?;

    let pairs =
        download_album_tracks(service, &track_ids, format_id, &dir, concurrency, cancel).await?;

    info!(album_id, count = pairs.len(), "Album download complete");

    let (downloaded_ids, paths) = unzip_track_results(pairs);

    if let Some(cfg) = config {
        embed_album_metadata(service, &album, &downloaded_ids, &paths, cfg).await?;
    }

    Ok(paths)
}

/// Detects a partial file on disk and returns the byte offset and Range header value for resumable
/// downloads.
///
/// Checks if a partial file exists for the given track and format, and if so, returns the
/// byte offset (file size) and a formatted Range header string for the HTTP request.
///
/// # Arguments
///
/// * `dir` - Directory containing the track file
/// * `track_id` - Track identifier
/// * `format_id` - Quality format ID (e.g., 5=MP3, 6=FLAC 16-bit, 7=FLAC 24-bit/96kHz)
///
/// # Returns
///
/// A tuple of `(offset, range)` where:
/// - `offset`: The byte position to resume from (file size), or `None` if no partial file exists
/// - `range`: The `Range` HTTP header value (e.g., `"bytes=1234567-"`), or `None`
fn detect_resume_offset(
    dir: &Path,
    track_id: i32,
    format_id: i32,
) -> (Option<u64>, Option<String>) {
    let ext = Album::extension_for_format(format_id);
    let existing_path = dir.join(format!("{track_id:02}.{ext}"));
    let offset = existing_path
        .exists()
        .then(|| match metadata(&existing_path) {
            Ok(m) if m.len() > 0 => Some(m.len()),
            _ => None,
        })
        .flatten();
    let range = offset.map(|s| format!("bytes={s}-"));
    (offset, range)
}

/// Warns if a Range request was made but the server responded with 200 instead of 206.
///
/// This indicates the server does not support resumable downloads, so the full file
/// will be re-downloaded from byte 0.
///
/// # Arguments
///
/// * `had_offset` - Whether we attempted to resume (found a partial file)
/// * `resumed` - Whether the server responded with 206 Partial Content
/// * `track_id` - Track identifier for logging
fn warn_if_not_resumed(had_offset: bool, resumed: bool, track_id: i32) {
    if had_offset && !resumed {
        warn!(
            track_id,
            "Server did not support Range request, re-downloading full file"
        );
    }
}

/// Downloads cover art data for an album if cover art metadata is enabled.
///
/// # Arguments
///
/// * `service` - Authenticated API service
/// * `album` - Album containing cover art URL
/// * `config` - Metadata configuration controlling field embedding
///
/// # Returns
///
/// Cover art image bytes, or `None` if cover art is disabled or unavailable.
async fn download_cover_data(
    service: &QobuzApiService,
    album: &Album,
    config: &MetadataConfig,
) -> Result<Option<Vec<u8>>, QobuzApiError> {
    if !config.is_enabled(CoverArt) {
        return Ok(None);
    }
    let Some(url) = album.image.as_ref().and_then(best_cover_url) else {
        return Ok(None);
    };
    let token = service.require_auth_token()?;
    let resp = match service.http_client().get_with_auth(&url, token, None).await {
        Ok(r) => r,
        Err(e) => {
            debug!(error = %e, "Cover art download failed");
            return Ok(None);
        }
    };
    match resp.bytes().await {
        Err(e) => {
            debug!(error = %e, "Failed to read cover art bytes");
            Ok(None)
        }
        Ok(b) => Ok(Some(b.to_vec())),
    }
}
