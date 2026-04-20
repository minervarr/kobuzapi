//! Download and credential test setup helpers.

use std::path::PathBuf;

use {
    anyhow::{Error, Result, anyhow},
    tempfile::TempDir,
    tracing::info,
};

use qobuz_api_rust_refactor::api::service::QobuzApiService;

use crate::test_support::{
    create_authenticated_service, get_download_config,
    query::{find_album_id, find_track_id},
};

/// Setup for album download tests.
pub struct AlbumDownloadSetup {
    /// Authenticated API service.
    pub service: QobuzApiService,
    /// Album ID to download.
    pub album_id: String,
    /// Temporary directory for downloaded files.
    pub temp_dir: TempDir,
    /// Audio format ID.
    pub format_id: i32,
}

/// Setup for wrong credentials tests.
pub struct WrongCredentialsSetup {
    /// Authenticated API service (with invalid credentials).
    pub service: QobuzApiService,
    /// Track ID for testing.
    pub track_id: i32,
    /// Audio format ID.
    pub format_id: i32,
}

/// Sets up an album download test.
///
/// # Returns
///
/// The album download setup if successful.
pub fn setup_album_download() -> Result<AlbumDownloadSetup> {
    let config = get_download_config();
    let service = create_authenticated_service()?;
    let album_id = find_album_id(&service, config.album_query())?;
    let temp_dir = TempDir::new()?;

    info!(
        "Downloading album {album_id} to {}",
        temp_dir.path().display()
    );

    Ok(AlbumDownloadSetup {
        service,
        album_id,
        temp_dir,
        format_id: config.format_id,
    })
}

/// Downloads an album using the provided setup.
///
/// # Arguments
///
/// * `setup` - The album download setup
/// * `max_tracks` - Optional maximum number of tracks to download
///
/// # Returns
///
/// A vector of downloaded file paths.
pub fn download_album(
    setup: &mut AlbumDownloadSetup,
    max_tracks: Option<usize>,
) -> Result<Vec<PathBuf>> {
    setup
        .service
        .download_album(
            &setup.album_id,
            setup.format_id,
            setup.temp_dir.path(),
            None,
            max_tracks,
        )
        .map_err(Error::from)
}

/// Sets up a test with wrong credentials.
///
/// # Returns
///
/// The wrong credentials setup if successful.
pub fn setup_wrong_credentials() -> Result<WrongCredentialsSetup> {
    let config = get_download_config();

    let mut service =
        QobuzApiService::new().map_err(|e| anyhow!("Failed to create service: {e}"))?;

    service.login("invalid_test_user@example.com", "invalid_password")?;

    let track_id = find_track_id(&service, &config.track_query)?;

    Ok(WrongCredentialsSetup {
        service,
        track_id,
        format_id: config.format_id,
    })
}
