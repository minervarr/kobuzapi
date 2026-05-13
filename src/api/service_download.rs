//! Download delegates for the central API service.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
};

use crate::{
    api::{
        content::{
            album_download::download_album,
            artist_download::download_artist,
            playlist_download::download_playlist,
            tracks::{download_track, get_track_file_url},
        },
        service::QobuzApiService,
    },
    errors::QobuzApiError,
    metadata::config::MetadataConfig,
    models::file_url::FileUrl,
};

impl QobuzApiService {
    delegate_with_retry!(pub fn get_track_file_url(track_id: i32, format_id: i32) -> FileUrl = get_track_file_url);

    /// Downloads a single track with cancellation support.
    ///
    /// # Arguments
    ///
    /// * `track_id` - Track identifier
    /// * `format_id` - Quality format ID
    /// * `output_dir` - Directory to save the downloaded file
    /// * `config` - Optional metadata configuration for tagging
    /// * `cancel` - Optional cancellation flag checked during the download
    ///
    /// # Returns
    ///
    /// The path to the downloaded file.
    ///
    /// # Errors
    ///
    /// Returns a `QobuzApiError` if URL retrieval, download, I/O fails, or `Canceled` if cancelled.
    pub fn download_track(
        &mut self,
        track_id: i32,
        format_id: i32,
        output_dir: &Path,
        config: Option<&MetadataConfig>,
    ) -> Result<PathBuf, QobuzApiError> {
        self.download_track_cancellable(track_id, format_id, output_dir, config, None)
    }

    delegate_with_retry_cancellable!(
        pub fn download_track_cancellable(track_id: i32, format_id: i32, output_dir: &Path, config: Option<&MetadataConfig>) -> PathBuf = download_track,
        cancel: Option<&AtomicBool>
    );

    /// Downloads an album.
    ///
    /// # Arguments
    ///
    /// * `album_id` - Album identifier
    /// * `format_id` - Quality format ID
    /// * `output_dir` - Base output directory for downloaded files
    /// * `config` - Optional metadata configuration for tagging
    /// * `concurrency` - Maximum number of concurrent track downloads
    ///
    /// # Returns
    ///
    /// A vector of paths to the downloaded track files.
    ///
    /// # Errors
    ///
    /// Returns a `QobuzApiError` if album retrieval, track download, or I/O fails.
    pub fn download_album(
        &mut self,
        album_id: &str,
        format_id: i32,
        output_dir: &Path,
        config: Option<&MetadataConfig>,
        concurrency: Option<usize>,
    ) -> Result<Vec<PathBuf>, QobuzApiError> {
        self.download_album_cancellable(album_id, format_id, output_dir, config, concurrency, None)
    }

    delegate_with_retry_cancellable!(
        pub fn download_album_cancellable(album_id: &str, format_id: i32, output_dir: &Path, config: Option<&MetadataConfig>, concurrency: Option<usize>) -> Vec<PathBuf> = download_album,
        cancel: Option<Arc<AtomicBool>>
    );

    /// Downloads all albums by an artist.
    ///
    /// # Arguments
    ///
    /// * `artist_id` - Artist identifier
    /// * `format_id` - Quality format ID
    /// * `output_dir` - Base output directory for downloaded files
    /// * `config` - Optional metadata configuration for tagging
    /// * `concurrency` - Maximum number of concurrent track downloads per album
    ///
    /// # Returns
    ///
    /// A vector of paths to the downloaded track files.
    ///
    /// # Errors
    ///
    /// Returns a `QobuzApiError` if artist release list retrieval or any album download fails.
    pub fn download_artist(
        &mut self,
        artist_id: i32,
        format_id: i32,
        output_dir: &Path,
        config: Option<&MetadataConfig>,
        concurrency: Option<usize>,
    ) -> Result<Vec<PathBuf>, QobuzApiError> {
        self.download_artist_cancellable(
            artist_id,
            format_id,
            output_dir,
            config,
            concurrency,
            None,
        )
    }

    delegate_with_retry_cancellable!(
        pub fn download_artist_cancellable(artist_id: i32, format_id: i32, output_dir: &Path, config: Option<&MetadataConfig>, concurrency: Option<usize>) -> Vec<PathBuf> = download_artist,
        cancel: Option<Arc<AtomicBool>>
    );

    /// Downloads all tracks in a playlist.
    ///
    /// # Arguments
    ///
    /// * `playlist_id` - Playlist identifier
    /// * `format_id` - Quality format ID
    /// * `output_dir` - Base output directory for downloaded files
    /// * `config` - Optional metadata configuration for tagging
    /// * `concurrency` - Maximum number of concurrent track downloads (unused for playlists)
    ///
    /// # Returns
    ///
    /// A vector of paths to the downloaded track files.
    ///
    /// # Errors
    ///
    /// Returns a `QobuzApiError` if playlist retrieval, directory creation, or any track download
    /// fails.
    pub fn download_playlist(
        &mut self,
        playlist_id: &str,
        format_id: i32,
        output_dir: &Path,
        config: Option<&MetadataConfig>,
        concurrency: Option<usize>,
    ) -> Result<Vec<PathBuf>, QobuzApiError> {
        self.download_playlist_cancellable(
            playlist_id,
            format_id,
            output_dir,
            config,
            concurrency,
            None,
        )
    }

    delegate_with_retry_cancellable!(
        pub fn download_playlist_cancellable(playlist_id: &str, format_id: i32, output_dir: &Path, config: Option<&MetadataConfig>, concurrency: Option<usize>) -> Vec<PathBuf> = download_playlist,
        cancel: Option<Arc<AtomicBool>>
    );
}
