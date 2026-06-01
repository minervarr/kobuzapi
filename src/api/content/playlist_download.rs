//! Playlist download operations.

use std::{
    fs::create_dir_all,
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
};

use {
    tokio::{spawn, sync::Semaphore},
    tracing::{error, info, warn},
};

use crate::{
    api::{
        content::{check_cancel, fetch_with_cancel, playlists::get_playlist, tracks::download_track},
        service::QobuzApiService,
    },
    errors::QobuzApiError::{self, DownloadError},
    metadata::config::MetadataConfig,
    sanitize::sanitize_filename,
};

/// Downloads all tracks in a playlist concurrently.
///
/// Fetches the playlist with tracks included and downloads tracks with bounded concurrency.
/// One track failure does not abort the remaining tracks.
///
/// # Arguments
///
/// * `service` - Authenticated API service
/// * `playlist_id` - Playlist identifier
/// * `format_id` - Quality format ID
/// * `output_dir` - Base output directory (playlist name subdirectory is created)
/// * `config` - Optional metadata configuration for tagging
/// * `concurrency` - Maximum number of concurrent track downloads
/// * `cancel` - Optional cancellation flag
///
/// # Returns
///
/// A vector of paths to all successfully downloaded track files.
///
/// # Errors
///
/// Returns a `QobuzApiError` if playlist retrieval, directory creation, or all tracks fail.
pub async fn download_playlist(
    service: &QobuzApiService,
    playlist_id: &str,
    format_id: i32,
    output_dir: &Path,
    config: Option<&MetadataConfig>,
    concurrency: Option<usize>,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<Vec<PathBuf>, QobuzApiError> {
    let playlist = fetch_with_cancel(cancel.as_deref(), || {
        get_playlist(service, playlist_id, Some("tracks"))
    })
    .await?;

    let title = playlist
        .name
        .unwrap_or_else(|| "Unknown Playlist".to_string());
    let dir = output_dir.join(sanitize_filename(&title));
    create_dir_all(&dir)?;

    let tracks = playlist.tracks.and_then(|t| t.items).unwrap_or_default();
    if tracks.is_empty() {
        return Err(DownloadError {
            message: "No tracks in playlist".to_string(),
        });
    }

    let max_concurrent = concurrency.unwrap_or(4);
    let semaphore = Arc::new(Semaphore::new(max_concurrent));
    let mut handles = Vec::new();

    for track_item in &tracks {
        check_cancel(cancel.as_deref())?;

        let Some(track_id) = track_item.id else {
            continue;
        };

        let permit = Arc::clone(&semaphore)
            .acquire_owned()
            .await
            .map_err(|e| DownloadError {
                message: format!("Semaphore error: {e}"),
            })?;

        let service_client = service.clone_http_client();
        let app_id = service.app_id.clone();
        let app_secret = service.app_secret.clone();
        let token = service.require_auth_token()?.to_string();
        let base_url = service.base_url().to_string();
        let dir_clone = dir.clone();
        let config = config.cloned();
        let cancel_clone = cancel.clone();

        let handle = spawn(async move {
            let _permit = permit;
            let svc = QobuzApiService::new_for_task(app_id, app_secret, token, service_client, base_url);
            let result = download_track(
                &svc,
                track_id,
                format_id,
                &dir_clone,
                config.as_ref(),
                cancel_clone.as_deref(),
            )
            .await;
            (track_id, result)
        });

        handles.push(handle);
    }

    let mut all_paths = Vec::new();
    let mut failed = 0usize;

    for handle in handles {
        check_cancel(cancel.as_deref())?;
        match handle.await {
            Ok((_, Ok(path))) => all_paths.push(path),
            Ok((track_id, Err(e))) => {
                error!(track_id, error = %e, "Track download failed, continuing with remaining tracks");
                failed += 1;
            }
            Err(e) => {
                error!(error = %e, "Track task join error, continuing with remaining tracks");
                failed += 1;
            }
        }
    }

    if all_paths.is_empty() && failed > 0 {
        return Err(DownloadError {
            message: format!("All {failed} track(s) failed to download"),
        });
    }

    if failed > 0 {
        warn!(failed, succeeded = all_paths.len(), "Some tracks failed to download");
    }

    info!(playlist_id, count = all_paths.len(), "Playlist download complete");

    Ok(all_paths)
}
