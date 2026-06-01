//! Artist download operations.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
};

use {
    tokio::{spawn, sync::Semaphore},
    tracing::{error, info, warn},
};

use crate::{
    api::{
        content::{
            album_download::download_album, artists::get_release_list, check_cancel,
            fetch_with_cancel,
        },
        service::QobuzApiService,
    },
    errors::QobuzApiError::{self, DownloadError},
    metadata::config::MetadataConfig,
};

/// Number of albums to download concurrently for artist downloads.
const DEFAULT_ALBUM_CONCURRENCY: usize = 2;

/// Downloads all albums by an artist concurrently.
///
/// Fetches the artist's release list and downloads albums with bounded concurrency.
/// Each album in turn downloads its tracks concurrently. One album failure does not
/// abort the remaining albums.
///
/// # Arguments
///
/// * `service` - Authenticated API service
/// * `artist_id` - Artist identifier
/// * `format_id` - Quality format ID
/// * `output_dir` - Base output directory (subdirectories created per album)
/// * `config` - Optional metadata configuration for tagging
/// * `concurrency` - Maximum number of concurrent track downloads per album
/// * `cancel` - Optional cancellation flag
///
/// # Returns
///
/// A vector of paths to all successfully downloaded track files.
///
/// # Errors
///
/// Returns a `QobuzApiError` if the release list cannot be fetched or all albums fail.
pub async fn download_artist(
    service: &QobuzApiService,
    artist_id: i32,
    format_id: i32,
    output_dir: &Path,
    config: Option<&MetadataConfig>,
    concurrency: Option<usize>,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<Vec<PathBuf>, QobuzApiError> {
    let releases = fetch_with_cancel(cancel.as_deref(), || {
        get_release_list(service, artist_id, Some(50), None)
    })
    .await?;

    let album_items = releases.items.unwrap_or_default();
    if album_items.is_empty() {
        return Err(DownloadError {
            message: "No releases found for artist".to_string(),
        });
    }

    let semaphore = Arc::new(Semaphore::new(DEFAULT_ALBUM_CONCURRENCY));
    let mut handles = Vec::new();

    for album_info in &album_items {
        check_cancel(cancel.as_deref())?;

        let Some(album_id) = album_info.id.clone() else {
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
        let output_dir = output_dir.to_path_buf();
        let config = config.cloned();
        let cancel_clone = cancel.clone();

        let handle = spawn(async move {
            let _permit = permit;
            let svc = QobuzApiService::new_for_task(app_id, app_secret, token, service_client, base_url);
            let result = download_album(
                &svc,
                &album_id,
                format_id,
                &output_dir,
                config.as_ref(),
                concurrency,
                cancel_clone,
            )
            .await;
            (album_id, result)
        });

        handles.push(handle);
    }

    let mut all_paths = Vec::new();
    let mut failed = 0usize;

    for handle in handles {
        check_cancel(cancel.as_deref())?;
        match handle.await {
            Ok((_album_id, Ok(paths))) => all_paths.extend(paths),
            Ok((album_id, Err(e))) => {
                error!(album_id, error = %e, "Album download failed, continuing with remaining albums");
                failed += 1;
            }
            Err(e) => {
                error!(error = %e, "Album task join error, continuing with remaining albums");
                failed += 1;
            }
        }
    }

    if all_paths.is_empty() && failed > 0 {
        return Err(DownloadError {
            message: format!("All {failed} album(s) failed to download"),
        });
    }

    if failed > 0 {
        warn!(failed, succeeded_tracks = all_paths.len(), "Some albums failed to download");
    }

    info!(artist_id, count = all_paths.len(), "Artist download complete");

    Ok(all_paths)
}
