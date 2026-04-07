//! Metadata extraction from API models.

use crate::models::{album::Album, artist::Artist, track::Track};

/// Comprehensive metadata extracted from API models.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComprehensiveMetadata {
    /// Track title.
    pub title: Option<String>,
    /// Artist name.
    pub artist: Option<String>,
    /// Album title.
    pub album: Option<String>,
    /// Album artist.
    pub album_artist: Option<String>,
    /// Genre.
    pub genre: Option<String>,
    /// Release date.
    pub date: Option<String>,
    /// Composer.
    pub composer: Option<String>,
    /// Conductor.
    pub conductor: Option<String>,
    /// Performer.
    pub performer: Option<String>,
    /// Track number.
    pub track_number: Option<i32>,
    /// Disc number.
    pub disc_number: Option<i32>,
    /// ISRC code.
    pub isrc: Option<String>,
    /// Copyright notice.
    pub copyright: Option<String>,
    /// Record label.
    pub label: Option<String>,
    /// Cover art URL.
    pub cover_art_url: Option<String>,
}

/// Extracts comprehensive metadata from a track, its album, and artist.
///
/// Uses `rayon::par_iter` for batch extraction when processing album tracks.
///
/// # Arguments
///
/// * `track` - The track to extract metadata from
/// * `album` - Optional album containing the track
/// * `artist` - Optional artist for the track
///
/// # Returns
///
/// A `ComprehensiveMetadata` struct with all extracted fields.
#[must_use]
pub fn extract_comprehensive_metadata(
    track: &Track,
    album: Option<&Album>,
    artist: Option<&Artist>,
) -> ComprehensiveMetadata {
    ComprehensiveMetadata {
        title: track.title.clone(),
        artist: track
            .performer
            .as_ref()
            .and_then(|a| a.name.clone())
            .or_else(|| {
                let a = artist?;
                a.name.clone()
            }),
        album: album.and_then(|a| a.title.clone()),
        album_artist: album
            .and_then(|a| a.artist.as_ref())
            .and_then(|a| a.name.clone()),
        genre: album
            .and_then(|a| a.genre.as_ref())
            .and_then(|g| g.name.clone()),
        date: album.and_then(|a| a.release_date_original.clone()),
        composer: track.composer.as_ref().and_then(|a| a.name.clone()),
        conductor: None,
        performer: track.performers.clone(),
        track_number: track.track_number,
        disc_number: track.media_number,
        isrc: track.isrc.clone(),
        copyright: album
            .and_then(|a| a.copyright.clone())
            .or(track.copyright.clone()),
        label: album
            .and_then(|a| a.label.as_ref())
            .and_then(|l| l.name.clone()),
        cover_art_url: album
            .and_then(|a| a.image.as_ref())
            .and_then(|i| i.large.clone()),
    }
}
