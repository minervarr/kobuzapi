//! Metadata extraction from API models.

use std::collections::HashSet;

use crate::models::{
    album::{Album, Image},
    artist::Artist,
    track::Track,
};

/// Comprehensive metadata extracted from API models.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComprehensiveMetadata {
    /// Track title.
    pub title: Option<String>,
    /// Artist name (deduplicated across roles).
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
    /// Conductor (extracted from performers string for classical music).
    pub conductor: Option<String>,
    /// Performer details (formatted string from API).
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
    /// Best available cover art URL.
    pub cover_art_url: Option<String>,
    /// Downloaded cover art binary data.
    pub cover_art_data: Option<Vec<u8>>,
}

/// Selects the best available cover art URL from an [`Image`](crate::models::album::Image).
///
/// Resolution priority (highest to lowest): mega > extralarge > large > medium > thumbnail > small.
#[must_use]
pub fn best_cover_url(image: &Image) -> Option<String> {
    image
        .mega
        .clone()
        .or_else(|| image.extra_large.clone())
        .or_else(|| image.large.clone())
        .or_else(|| image.medium.clone())
        .or_else(|| image.thumbnail.clone())
        .or_else(|| image.small.clone())
}

/// Extracts conductor name from a performers string like `"Name (conductor), Other (role)"`.
fn extract_conductor(performers: &str) -> Option<String> {
    for part in performers.split(',') {
        let trimmed = part.trim();
        let lower = trimmed.to_lowercase();
        if !lower.ends_with("(conductor)") {
            continue;
        }
        let name = trimmed[..trimmed.len() - "(conductor)".len()].trim();
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }
    None
}

/// Returns whether a track appears to be classical music based on the `work` field.
fn is_classical(track: &Track) -> bool {
    track.work.is_some()
}

/// Deduplicates artist name references, preserving insertion order.
fn dedupe_names(names: &[&str]) -> String {
    let mut seen = HashSet::new();
    names
        .iter()
        .filter(|n| seen.insert(n.to_lowercase()))
        .copied()
        .collect::<Vec<_>>()
        .join(", ")
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
pub fn extract_comprehensive_metadata(
    track: &Track,
    album: Option<&Album>,
    artist: Option<&Artist>,
) -> ComprehensiveMetadata {
    let conductor = track.performers.as_ref().and_then(|p| extract_conductor(p));

    let performer_name = track
        .performer
        .as_ref()
        .and_then(|a| a.name.clone())
        .or_else(|| {
            let a = artist?;
            a.name.clone()
        });

    let composer_name = track.composer.as_ref().and_then(|a| a.name.clone());

    let artist = if performer_name.as_deref() == composer_name.as_deref() {
        performer_name
    } else {
        match (&performer_name, &composer_name) {
            (Some(p), Some(c)) => Some(dedupe_names(&[p, c])),
            (Some(p), None) | (None, Some(p)) => Some(p.clone()),
            (None, None) => None,
        }
    };

    let album_artist = if is_classical(track)
        && let Some(cond) = &conductor
    {
        Some(cond.clone())
    } else {
        album
            .and_then(|a| a.artist.as_ref())
            .and_then(|a| a.name.clone())
    };

    ComprehensiveMetadata {
        title: track.title.clone(),
        artist,
        album: album.and_then(|a| a.title.clone()),
        album_artist,
        genre: album
            .and_then(|a| a.genre.as_ref())
            .and_then(|g| g.name.clone()),
        date: album.and_then(|a| a.release_date_original.clone()),
        composer: composer_name,
        conductor,
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
            .and_then(best_cover_url),
        cover_art_data: None,
    }
}

#[cfg(test)]
mod tests {
    use {
        anyhow::{Result, ensure},
        serde_json::from_str,
    };

    use crate::{
        metadata::extractor::{
            best_cover_url, dedupe_names, extract_comprehensive_metadata, extract_conductor,
            is_classical,
        },
        models::{
            album::{Album, Image},
            track::Track,
        },
    };

    fn make_track() -> Result<Track> {
        let json =
            r#"{"id":1,"title":"So What","track_number":1,"media_number":1,"isrc":"USPJ11001001"}"#;
        Ok(from_str(json)?)
    }

    fn make_album() -> Result<Album> {
        let json = r#"{"id":"123","title":"Kind of Blue","release_date_original":"1959-08-17","copyright":"(C) 1959","genre":{"id":1,"name":"Jazz"},"label":{"id":1,"name":"Columbia"},"artist":{"id":1,"name":"Miles Davis"},"image":{"small":"s","thumbnail":"t","medium":"m","large":"l","extralarge":"xl","mega":"mega"}}"#;
        Ok(from_str(json)?)
    }

    #[test]
    fn extract_all_fields() -> Result<()> {
        let track = make_track()?;
        let album = make_album()?;
        let meta = extract_comprehensive_metadata(&track, Some(&album), None);
        ensure!(meta.title.as_deref() == Some("So What"));
        ensure!(meta.album.as_deref() == Some("Kind of Blue"));
        ensure!(meta.album_artist.as_deref() == Some("Miles Davis"));
        ensure!(meta.genre.as_deref() == Some("Jazz"));
        ensure!(meta.date.as_deref() == Some("1959-08-17"));
        ensure!(meta.copyright.as_deref() == Some("(C) 1959"));
        ensure!(meta.label.as_deref() == Some("Columbia"));
        ensure!(meta.track_number == Some(1));
        ensure!(meta.disc_number == Some(1));
        ensure!(meta.isrc.as_deref() == Some("USPJ11001001"));
        ensure!(meta.cover_art_url.as_deref() == Some("mega"));
        Ok(())
    }

    #[test]
    fn extract_with_no_album() -> Result<()> {
        let track = make_track()?;
        let meta = extract_comprehensive_metadata(&track, None, None);
        ensure!(meta.album.is_none());
        ensure!(meta.album_artist.is_none());
        ensure!(meta.genre.is_none());
        ensure!(meta.cover_art_url.is_none());
        ensure!(meta.title.as_deref() == Some("So What"));
        Ok(())
    }

    #[test]
    fn cover_art_priority() -> Result<()> {
        let image = Image {
            small: Some("s".into()),
            thumbnail: None,
            medium: None,
            large: None,
            extra_large: None,
            mega: None,
            back: None,
        };
        ensure!(best_cover_url(&image) == Some("s".to_string()));

        let image = Image {
            small: Some("s".into()),
            thumbnail: None,
            medium: None,
            large: Some("l".into()),
            extra_large: None,
            mega: None,
            back: None,
        };
        ensure!(best_cover_url(&image) == Some("l".to_string()));

        let image = Image {
            small: None,
            thumbnail: None,
            medium: None,
            large: None,
            extra_large: None,
            mega: None,
            back: None,
        };
        ensure!(best_cover_url(&image).is_none());
        Ok(())
    }

    #[test]
    fn conductor_extraction() -> Result<()> {
        ensure!(
            extract_conductor("Herbert von Karajan (conductor), Anne-Sophie Mutter (violin)")
                == Some("Herbert von Karajan".to_string())
        );
        ensure!(extract_conductor("John Coltrane (saxophone)").is_none());
        ensure!(extract_conductor("").is_none());
        Ok(())
    }

    #[test]
    fn classical_music_conductor_as_album_artist() -> Result<()> {
        let json = r#"{"id":2,"title":"Symphony No. 5","track_number":1,"work":"Symphony No. 5","performers":"Herbert von Karajan (conductor), Berlin Philharmonic (orchestra)"}"#;
        let track: Track = from_str(json)?;
        let album = make_album()?;
        let meta = extract_comprehensive_metadata(&track, Some(&album), None);
        ensure!(is_classical(&track));
        ensure!(meta.conductor.as_deref() == Some("Herbert von Karajan"));
        ensure!(meta.album_artist.as_deref() == Some("Herbert von Karajan"));
        Ok(())
    }

    #[test]
    fn artist_dedup_same_performer_composer() -> Result<()> {
        let json = r#"{"id":3,"title":"Test","track_number":1,"performer":{"id":1,"name":"Bach"},"composer":{"id":1,"name":"Bach"}}"#;
        let track: Track = from_str(json)?;
        let meta = extract_comprehensive_metadata(&track, None, None);
        ensure!(meta.artist.as_deref() == Some("Bach"));
        ensure!(meta.composer.as_deref() == Some("Bach"));
        Ok(())
    }

    #[test]
    fn dedupe_names_removes_duplicates() -> Result<()> {
        ensure!(dedupe_names(&["John", "Paul", "John"]) == "John, Paul");
        ensure!(dedupe_names(&["John"]) == "John");
        ensure!(dedupe_names(&[] as &[&str]) == "");
        Ok(())
    }

    #[test]
    fn special_characters_preserved() -> Result<()> {
        let json = r#"{"id":4,"title":"Café Résumé — Über","track_number":1}"#;
        let track: Track = from_str(json)?;
        let meta = extract_comprehensive_metadata(&track, None, None);
        ensure!(meta.title.as_deref() == Some("Café Résumé — Über"));
        Ok(())
    }
}
