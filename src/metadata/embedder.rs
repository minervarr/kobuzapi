//! Metadata embedding into audio files using `lofty`.

use std::path::{Path, PathBuf};

use {
    lofty::{
        config::WriteOptions,
        file::TaggedFileExt,
        picture::{Picture, PictureType::CoverFront},
        probe::Probe,
        tag::{
            Accessor,
            ItemKey::{
                AlbumArtist, Comment, Composer, Conductor, Isrc, Label, Performer, Producer,
                RecordingDate, Year,
            },
            Tag, TagExt,
        },
    },
    rayon::iter::{IntoParallelRefIterator, ParallelIterator},
    tracing::debug,
};

use crate::{
    errors::QobuzApiError::{self, MetadataError},
    metadata::{
        config::{
            MetadataConfig,
            MetadataField::{
                Album, AlbumArtist as FieldAlbumArtist, Artist, Comment as FieldComment,
                Composer as FieldComposer, Conductor as FieldConductor, Copyright, CoverArt, Date,
                DiscNumber, Genre, Isrc as FieldIsrc, Label as FieldLabel,
                Performer as FieldPerformer, Producer as FieldProducer, Title, TrackNumber,
            },
        },
        extractor::ComprehensiveMetadata,
    },
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
    debug!(path = %path.display(), "Embedding metadata");

    let mut tagged_file = Probe::open(path)
        .map_err(|e| MetadataError(format!("Probe open: {e}")))?
        .read()
        .map_err(|e| MetadataError(format!("Probe read: {e}")))?;

    let tag = if let Some(t) = tagged_file.primary_tag_mut() {
        t
    } else {
        let tag_type = tagged_file.primary_tag_type();
        tagged_file.insert_tag(Tag::new(tag_type));
        tagged_file
            .primary_tag_mut()
            .ok_or_else(|| MetadataError("No tag created".into()))?
    };

    apply_metadata_fields(tag, metadata, config);

    if let Some(data) = &metadata.cover_art_data
        && config.is_enabled(CoverArt)
    {
        let picture = Picture::unchecked(data.clone())
            .pic_type(CoverFront)
            .build();
        tag.push_picture(picture);
    }

    tag.save_to_path(path, WriteOptions::default())
        .map_err(|e| MetadataError(format!("Save failed: {e}")))?;

    debug!(path = %path.display(), "Metadata embedded");
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
        .par_iter()
        .map(|(path, metadata)| embed_metadata_in_file(path, metadata, config))
        .collect()
}

/// Applies metadata fields to a tag based on config toggles.
fn apply_metadata_fields(tag: &mut Tag, meta: &ComprehensiveMetadata, config: &MetadataConfig) {
    if config.is_enabled(Title)
        && let Some(v) = &meta.title
    {
        tag.set_title(v.clone());
    }
    if config.is_enabled(Artist)
        && let Some(v) = &meta.artist
    {
        tag.set_artist(v.clone());
    }
    if config.is_enabled(Album)
        && let Some(v) = &meta.album
    {
        tag.set_album(v.clone());
    }
    if config.is_enabled(FieldAlbumArtist)
        && let Some(v) = &meta.album_artist
    {
        tag.insert_text(AlbumArtist, v.clone());
    }
    if config.is_enabled(Genre)
        && let Some(v) = &meta.genre
    {
        tag.set_genre(v.clone());
    }
    if config.is_enabled(Date)
        && let Some(v) = &meta.date
    {
        tag.insert_text(RecordingDate, v.clone());
        if let Ok(year_str) = v.chars().take(4).collect::<String>().parse::<u32>() {
            tag.insert_text(Year, year_str.to_string());
        }
    }
    if config.is_enabled(FieldComposer)
        && let Some(v) = &meta.composer
    {
        tag.insert_text(Composer, v.clone());
    }
    if config.is_enabled(FieldConductor)
        && let Some(v) = &meta.conductor
    {
        tag.insert_text(Conductor, v.clone());
    }
    if config.is_enabled(FieldPerformer)
        && let Some(v) = &meta.performer
    {
        tag.insert_text(Performer, v.clone());
    }
    if config.is_enabled(TrackNumber)
        && let Some(n) = meta.track_number
    {
        tag.set_track(n.cast_unsigned());
    }
    if config.is_enabled(DiscNumber)
        && let Some(n) = meta.disc_number
    {
        tag.set_disk(n.cast_unsigned());
    }
    if config.is_enabled(FieldIsrc)
        && let Some(v) = &meta.isrc
    {
        tag.insert_text(Isrc, v.clone());
    }
    if config.is_enabled(FieldLabel)
        && let Some(v) = &meta.label
    {
        tag.insert_text(Label, v.clone());
    }
    if config.is_enabled(FieldComment)
        && let Some(v) = &meta.performer
    {
        tag.set_comment(v.clone());
    }
    if config.is_enabled(Copyright)
        && let Some(v) = &meta.copyright
    {
        tag.insert_text(Comment, format!("Copyright: {v}"));
    }
    if config.is_enabled(FieldProducer)
        && let Some(v) = &meta.composer
    {
        tag.insert_text(Producer, v.clone());
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::write, io::Result, path::Path};

    use {
        anyhow::{Result as AnyhowResult, anyhow, ensure},
        lofty::{
            file::TaggedFileExt,
            picture::PictureType::CoverFront,
            probe::Probe,
            tag::{Accessor, Tag},
        },
        tempfile::TempDir,
    };

    use crate::metadata::{
        config::{
            MetadataConfig,
            MetadataField::{Artist, CoverArt, Title},
        },
        embedder::embed_metadata_in_file,
        extractor::ComprehensiveMetadata,
    };

    fn create_minimal_flac(path: &Path) -> Result<()> {
        let mut data = b"fLaC\x80\x00\x00\x22".to_vec();
        let streaminfo = [
            0x10u8, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0A, 0xC4, 0x40, 0xF0,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        data.extend_from_slice(&streaminfo);
        write(path, data)
    }

    fn get_tag(path: &Path) -> AnyhowResult<Tag> {
        let tagged_file = Probe::open(path)
            .map_err(|e| anyhow!("probe: {e}"))?
            .read()
            .map_err(|e| anyhow!("read: {e}"))?;
        tagged_file
            .primary_tag()
            .cloned()
            .ok_or_else(|| anyhow!("no tag"))
    }

    fn cover_art_metadata() -> ComprehensiveMetadata {
        let jpeg_bytes = vec![0xFFu8, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x00, 0x00];
        ComprehensiveMetadata {
            cover_art_data: Some(jpeg_bytes),
            ..sample_metadata()
        }
    }

    fn sample_metadata() -> ComprehensiveMetadata {
        ComprehensiveMetadata {
            title: Some("Test Song".into()),
            artist: Some("Test Artist".into()),
            album: Some("Test Album".into()),
            album_artist: Some("Album Artist".into()),
            genre: Some("Rock".into()),
            date: Some("2024-01-15".into()),
            composer: Some("Composer Name".into()),
            conductor: None,
            performer: Some("Performer (guitar)".into()),
            track_number: Some(1),
            disc_number: Some(1),
            isrc: Some("USXXX2400001".into()),
            copyright: Some("(C) 2024 Label".into()),
            label: Some("Test Label".into()),
            cover_art_url: None,
            cover_art_data: None,
        }
    }

    #[test]
    fn embed_flac_all_fields() -> AnyhowResult<()> {
        let dir = TempDir::new()?;
        let path = dir.path().join("test.flac");
        create_minimal_flac(&path)?;

        let meta = sample_metadata();
        let config = MetadataConfig::all();
        embed_metadata_in_file(&path, &meta, &config)?;
        let tag = get_tag(&path)?;
        ensure!(tag.title().as_deref() == Some("Test Song"));
        ensure!(tag.artist().as_deref() == Some("Test Artist"));
        ensure!(tag.album().as_deref() == Some("Test Album"));
        ensure!(tag.genre().as_deref() == Some("Rock"));
        ensure!(tag.track() == Some(1));
        ensure!(tag.disk() == Some(1));
        Ok(())
    }

    #[test]
    fn embed_flac_selective_fields() -> AnyhowResult<()> {
        let dir = TempDir::new()?;
        let path = dir.path().join("test.flac");
        create_minimal_flac(&path)?;

        let meta = sample_metadata();
        let mut config = MetadataConfig::default();
        config.set(Title, false);
        config.set(Artist, false);

        embed_metadata_in_file(&path, &meta, &config)?;
        let tag = get_tag(&path)?;
        ensure!(tag.title().is_none());
        ensure!(tag.artist().is_none());
        ensure!(tag.album().as_deref() == Some("Test Album"));
        Ok(())
    }

    #[test]
    fn embed_cover_art() -> AnyhowResult<()> {
        let dir = TempDir::new()?;
        let path = dir.path().join("test.flac");
        create_minimal_flac(&path)?;

        let meta = cover_art_metadata();
        embed_metadata_in_file(&path, &meta, &MetadataConfig::all())?;

        let tag = get_tag(&path)?;
        ensure!(!tag.pictures().is_empty());
        ensure!(tag.pictures()[0].pic_type() == CoverFront);
        Ok(())
    }

    #[test]
    fn embed_cover_art_skipped_when_disabled() -> AnyhowResult<()> {
        let dir = TempDir::new()?;
        let path = dir.path().join("test.flac");
        create_minimal_flac(&path)?;

        let meta = cover_art_metadata();
        let mut config = MetadataConfig::all();
        config.set(CoverArt, false);

        embed_metadata_in_file(&path, &meta, &config)?;
        let tag = get_tag(&path)?;

        ensure!(tag.pictures().is_empty());
        Ok(())
    }

    #[test]
    fn embed_unicode_values() -> AnyhowResult<()> {
        let dir = TempDir::new()?;
        let path = dir.path().join("test.flac");
        create_minimal_flac(&path)?;

        let meta = ComprehensiveMetadata {
            title: Some("Café Résumé — Über".into()),
            artist: Some("Björk".into()),
            album: Some("日本語タイトル".into()),
            ..sample_metadata()
        };
        let config = MetadataConfig::all();

        embed_metadata_in_file(&path, &meta, &config)?;
        let tag = get_tag(&path)?;
        ensure!(tag.title().as_deref() == Some("Café Résumé — Über"));
        ensure!(tag.artist().as_deref() == Some("Björk"));
        ensure!(tag.album().as_deref() == Some("日本語タイトル"));
        Ok(())
    }
}
