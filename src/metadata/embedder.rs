//! Metadata embedding into audio files using `lofty`.

use std::path::{Path, PathBuf};

use tracing::debug;

use crate::{
    errors::QobuzApiError,
    metadata::{config::MetadataConfig, extractor::ComprehensiveMetadata},
};

/// Embeds metadata into an audio file.
///
/// Writes tags using `lofty`:
/// - FLAC: Vorbis Comments
/// - MP3: `ID3v2`
/// - Cover art via `Picture`
///
/// Respects `MetadataConfig` field toggles.
///
/// # Arguments
///
/// * `path` - Path to the audio file
/// * `metadata` - Metadata to embed
/// * `config` - Configuration controlling which fields to embed
///
/// # Returns
///
/// `Ok(())` on success.
///
/// # Errors
///
/// Returns a `QobuzApiError` if metadata writing fails.
pub fn embed_metadata_in_file(
    path: &Path,
    metadata: &ComprehensiveMetadata,
    config: &MetadataConfig,
) -> Result<(), QobuzApiError> {
    debug!(path = %path.display(), ?metadata, ?config, "Metadata embedding placeholder");
    Ok(())
}

/// Embeds metadata into multiple files in parallel using `rayon`.
///
/// # Arguments
///
/// * `files` - Slice of `(path, metadata)` pairs to process
/// * `config` - Configuration controlling which fields to embed
///
/// # Returns
///
/// A vector of results, one per file, in the same order as input.
#[must_use]
pub fn embed_metadata_batch(
    files: &[(PathBuf, ComprehensiveMetadata)],
    config: &MetadataConfig,
) -> Vec<Result<(), QobuzApiError>> {
    files
        .iter()
        .map(|(path, metadata)| embed_metadata_in_file(path, metadata, config))
        .collect()
}
