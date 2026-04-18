//! Metadata embedding into audio files using `lofty`.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use {
    lofty::{
        config::WriteOptions,
        file::{FileType::Flac, TaggedFileExt},
        ogg::VorbisComments,
        picture::{MimeType, Picture, PictureType::CoverFront},
        probe::Probe,
        tag::{
            Accessor,
            ItemKey::{
                AlbumArtist, CommercialInformationUrl, Composer, CopyrightMessage, Isrc, Label,
                MusicianCredits, OriginalMediaType, Producer, RecordingDate, ReleaseDate, Year,
            },
            ItemValue, Tag, TagExt, TagItem,
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
                Album, AlbumArtist as FieldAlbumArtist, Artist, Composer as FieldComposer,
                Copyright, CoverArt, DiscNumber, DiscTotal, Explicit, Genre, InvolvedPeople,
                Isrc as FieldIsrc, Label as FieldLabel, MediaType, Producer as FieldProducer,
                ReleaseDate as FieldReleaseDate, ReleaseYear, Title, TrackNumber, TrackTotal, Upc,
                Url as FieldUrl,
            },
        },
        extractor::ComprehensiveMetadata,
    },
};

/// Embeds metadata into an audio file.
///
/// Writes tags using `lofty` with format-specific handling:
/// - FLAC: Vorbis Comments
/// - MP3: `ID3v2`
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

    let is_flac = tagged_file.file_type() == Flac;

    let tag = if let Some(t) = tagged_file.primary_tag_mut() {
        t
    } else {
        let tag_type = tagged_file.primary_tag_type();
        tagged_file.insert_tag(Tag::new(tag_type));
        tagged_file
            .primary_tag_mut()
            .ok_or_else(|| MetadataError("No tag created".into()))?
    };

    tag.clear();

    apply_title(tag, metadata, config);
    apply_album(tag, metadata, config);
    apply_album_artist(tag, metadata, config, is_flac);
    apply_artist(tag, metadata, config, is_flac);
    apply_involved_people(tag, metadata, config);
    apply_composer(tag, metadata, config, is_flac);
    apply_producer(tag, metadata, config, is_flac);
    apply_label(tag, metadata, config);
    apply_genre(tag, metadata, config);
    apply_track_numbers(tag, metadata, config);
    apply_disc_numbers(tag, metadata, config);
    apply_copyright(tag, metadata, config);
    apply_isrc(tag, metadata, config);
    apply_dates(tag, metadata, config, is_flac);
    apply_url(tag, metadata, config);
    apply_media_type(tag, metadata, config);
    apply_cover_art(tag, metadata, config);

    if is_flac {
        let mut vc: VorbisComments = tag.clone().into();
        apply_flac_custom_keys(&mut vc, metadata, config);
        vc.save_to_path(path, WriteOptions::default())
            .map_err(|e| MetadataError(format!("Save failed: {e}")))?;
    } else {
        tag.save_to_path(path, WriteOptions::default())
            .map_err(|e| MetadataError(format!("Save failed: {e}")))?;
    }

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

/// Writes the track title, appending version if present.
///
/// # Arguments
///
/// * `tag` - Target tag to write into
/// * `meta` - Source metadata
/// * `config` - Field toggle configuration
fn apply_title(tag: &mut Tag, meta: &ComprehensiveMetadata, config: &MetadataConfig) {
    if !config.is_enabled(Title) {
        return;
    }
    if let Some(title) = meta.title.as_ref() {
        let full = match &meta.track_version {
            Some(v) if !v.is_empty() => format!("{title} ({v})"),
            _ => title.clone(),
        };
        tag.set_title(full);
    }
}

/// Writes the album title, appending version if present.
///
/// # Arguments
///
/// * `tag` - Target tag to write into
/// * `meta` - Source metadata
/// * `config` - Field toggle configuration
fn apply_album(tag: &mut Tag, meta: &ComprehensiveMetadata, config: &MetadataConfig) {
    if !config.is_enabled(Album) {
        return;
    }
    if let Some(album_title) = meta.album.as_ref() {
        let full = match &meta.album_version {
            Some(v) if !v.is_empty() => format!("{album_title} ({v})"),
            _ => album_title.clone(),
        };
        tag.set_album(full);
    }
}

/// Writes the album artist tag.
///
/// # Arguments
///
/// * `tag` - Target tag to write into
/// * `meta` - Source metadata
/// * `config` - Field toggle configuration
/// * `is_flac` - Whether the output file is FLAC (affects separator and selection logic)
fn apply_album_artist(
    tag: &mut Tag,
    meta: &ComprehensiveMetadata,
    config: &MetadataConfig,
    is_flac: bool,
) {
    if !config.is_enabled(FieldAlbumArtist) {
        return;
    }
    let name = if is_flac {
        build_flac_album_artist(meta)
    } else {
        build_mp3_album_artist(meta)
    };
    if !name.is_empty() {
        tag.push(TagItem::new(AlbumArtist, ItemValue::Text(name)));
    }
}

/// Builds album artist for FLAC: prefers conductor with main-artist role if present in performers.
///
/// # Arguments
///
/// * `meta` - Source metadata
///
/// # Returns
///
/// The resolved album artist name string.
fn build_flac_album_artist(meta: &ComprehensiveMetadata) -> String {
    if let Some(artists) = meta.performers.as_ref() {
        let conductor = meta.album_artists.iter().find(|a| {
            let has_main_artist_role = a
                .roles
                .as_ref()
                .is_some_and(|roles| roles.contains(&"main-artist".to_string()));
            let name_matches_conductor = a
                .name
                .as_ref()
                .is_some_and(|name| artists.contains(&format!("{name}, Conductor")));
            has_main_artist_role && name_matches_conductor
        });
        if let Some(a) = conductor {
            return a.name.clone().unwrap_or_default();
        }
    }
    meta.album_artist_name.clone().unwrap_or_default()
}

/// Builds album artist for MP3: collects all main-artist names, slash-separated.
///
/// # Arguments
///
/// * `meta` - Source metadata
///
/// # Returns
///
/// Slash-separated artist names string.
fn build_mp3_album_artist(meta: &ComprehensiveMetadata) -> String {
    let mut main = Vec::new();
    for a in &meta.album_artists {
        let has_role = a
            .roles
            .as_ref()
            .is_some_and(|roles| roles.contains(&"main-artist".to_string()));
        if has_role && let Some(name) = a.name.as_ref() {
            main.push(name.clone());
        }
    }
    if main.is_empty()
        && let Some(name) = meta.album_artist_name.as_ref()
    {
        main.push(name.clone());
    }
    main.join("/")
}

/// Collects artist names from the performers string into the output list.
///
/// # Arguments
///
/// * `meta` - Source metadata
/// * `names` - Output vector to append deduplicated artist names to
/// * `seen` - Set of already-seen names for deduplication
fn collect_performer_artists(
    meta: &ComprehensiveMetadata,
    names: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    let Some(performers) = meta.performers.as_ref() else {
        return;
    };
    for name in extract_artist_names_from_performers(performers, seen) {
        if !seen.contains(&name) {
            names.push(name.clone());
            seen.insert(name);
        }
    }
}

/// Writes the artist tag by extracting names from performers, performer name, and album artists.
///
/// # Arguments
///
/// * `tag` - Target tag to write into
/// * `meta` - Source metadata
/// * `config` - Field toggle configuration
/// * `is_flac` - Whether the output file is FLAC (affects separator)
fn apply_artist(
    tag: &mut Tag,
    meta: &ComprehensiveMetadata,
    config: &MetadataConfig,
    is_flac: bool,
) {
    if !config.is_enabled(Artist) {
        return;
    }
    let mut names = Vec::new();
    let mut seen = HashSet::new();
    collect_performer_artists(meta, &mut names, &mut seen);
    if let Some(name) = meta.performer_name.as_ref()
        && !seen.contains(name)
    {
        names.push(name.clone());
        seen.insert(name.clone());
    }
    for a in &meta.album_artists {
        if let Some(name) = a.name.as_ref()
            && !name.is_empty()
            && !seen.contains(name)
        {
            names.push(name.clone());
            seen.insert(name.clone());
        }
    }
    if !names.is_empty() {
        let combined = if is_flac {
            names.join(", ")
        } else {
            names.join("/")
        };
        tag.set_artist(combined);
    }
}

/// Writes the involved people / musician credits from the performers string.
///
/// # Arguments
///
/// * `tag` - Target tag to write into
/// * `meta` - Source metadata
/// * `config` - Field toggle configuration
fn apply_involved_people(tag: &mut Tag, meta: &ComprehensiveMetadata, config: &MetadataConfig) {
    if !config.is_enabled(InvolvedPeople) {
        return;
    }
    if let Some(performers) = meta.performers.as_ref()
        && !performers.is_empty()
    {
        tag.push(TagItem::new(
            MusicianCredits,
            ItemValue::Text(performers.clone()),
        ));
    }
}

/// Writes the composer tag with format-specific logic.
///
/// # Arguments
///
/// * `tag` - Target tag to write into
/// * `meta` - Source metadata
/// * `config` - Field toggle configuration
/// * `is_flac` - Whether the output file is FLAC (affects composer selection)
fn apply_composer(
    tag: &mut Tag,
    meta: &ComprehensiveMetadata,
    config: &MetadataConfig,
    is_flac: bool,
) {
    if !config.is_enabled(FieldComposer) {
        return;
    }
    let composers = if is_flac {
        build_flac_composers(meta)
    } else {
        build_mp3_composers(meta)
    };
    if !composers.is_empty() {
        tag.push(TagItem::new(Composer, ItemValue::Text(composers.join("/"))));
    }
}

/// Builds composer list for FLAC: takes the last composer from performers.
///
/// # Arguments
///
/// * `meta` - Source metadata
///
/// # Returns
///
/// A vector of composer names (single entry for FLAC).
fn build_flac_composers(meta: &ComprehensiveMetadata) -> Vec<String> {
    let from_performers = meta
        .performers
        .as_ref()
        .map(|p| extract_composers_from_performers(p))
        .unwrap_or_default();
    if let Some(last) = from_performers.last()
        && last != "Various Composers"
    {
        return vec![last.clone()];
    }
    if let Some(name) = meta.track_composer_name.as_ref()
        && name != "Various Composers"
    {
        return vec![name.clone()];
    }
    if let Some(name) = meta.album_composer_name.as_ref()
        && name != "Various Composers"
    {
        return vec![name.clone()];
    }
    Vec::new()
}

/// Builds composer list for MP3: uses performers-derived composers, falling back to
/// track/album composer only when no performers have composer roles.
///
/// # Arguments
///
/// * `meta` - Source metadata
///
/// # Returns
///
/// A deduplicated vector of composer names.
fn build_mp3_composers(meta: &ComprehensiveMetadata) -> Vec<String> {
    let mut composers = Vec::new();
    let mut normalized = HashSet::new();
    collect_performer_composers(meta, &mut composers, &mut normalized);
    if !composers.is_empty() {
        return composers;
    }
    if let Some(name) = meta.track_composer_name.as_ref()
        && name != "Various Composers"
    {
        return vec![name.clone()];
    }
    if let Some(name) = meta.album_composer_name.as_ref()
        && name != "Various Composers"
    {
        return vec![name.clone()];
    }
    Vec::new()
}

/// Collects composers from performers string into the output list with deduplication.
///
/// # Arguments
///
/// * `meta` - Source metadata
/// * `composers` - Output vector to append composer names to
/// * `normalized` - Set of normalized names for deduplication
fn collect_performer_composers(
    meta: &ComprehensiveMetadata,
    composers: &mut Vec<String>,
    normalized: &mut HashSet<String>,
) {
    let Some(performers) = meta.performers.as_ref() else {
        return;
    };
    for c in extract_composers_from_performers(performers) {
        if c != "Various Composers" && !is_duplicate_composer(&c, normalized) {
            composers.push(c.clone());
            normalized.insert(normalize_composer_name(&c));
        }
    }
}

/// Writes producer tags from performers string (FLAC only).
///
/// # Arguments
///
/// * `tag` - Target tag to write into
/// * `meta` - Source metadata
/// * `config` - Field toggle configuration
/// * `is_flac` - Whether the output file is FLAC (producer tags are FLAC-only)
fn apply_producer(
    tag: &mut Tag,
    meta: &ComprehensiveMetadata,
    config: &MetadataConfig,
    is_flac: bool,
) {
    if !config.is_enabled(FieldProducer) || !is_flac {
        return;
    }
    if let Some(performers) = meta.performers.as_ref() {
        for producer in extract_producers_from_performers(performers) {
            tag.push(TagItem::new(Producer, ItemValue::Text(producer)));
        }
    }
}

/// Writes the label / publisher tag.
///
/// # Arguments
///
/// * `tag` - Target tag to write into
/// * `meta` - Source metadata
/// * `config` - Field toggle configuration
fn apply_label(tag: &mut Tag, meta: &ComprehensiveMetadata, config: &MetadataConfig) {
    if !config.is_enabled(FieldLabel) {
        return;
    }
    if let Some(label) = meta.label.as_ref() {
        tag.push(TagItem::new(Label, ItemValue::Text(label.clone())));
    }
}

/// Writes the genre tag if enabled.
///
/// # Arguments
///
/// * `tag` - Target tag to write into
/// * `meta` - Source metadata
/// * `config` - Field toggle configuration
fn apply_genre(tag: &mut Tag, meta: &ComprehensiveMetadata, config: &MetadataConfig) {
    if !config.is_enabled(Genre) {
        return;
    }
    if let Some(genre) = meta.genre.as_ref() {
        tag.set_genre(genre.clone());
    }
}

/// Writes track number and track total tags.
///
/// # Arguments
///
/// * `tag` - Target tag to write into
/// * `meta` - Source metadata
/// * `config` - Field toggle configuration
fn apply_track_numbers(tag: &mut Tag, meta: &ComprehensiveMetadata, config: &MetadataConfig) {
    if config.is_enabled(TrackNumber)
        && let Some(n) = meta.track_number
    {
        tag.set_track(n.cast_unsigned());
    }
    if config.is_enabled(TrackTotal)
        && let Some(n) = meta.track_total
    {
        tag.set_track_total(n.cast_unsigned());
    }
}

/// Writes disc number and disc total tags.
///
/// # Arguments
///
/// * `tag` - Target tag to write into
/// * `meta` - Source metadata
/// * `config` - Field toggle configuration
fn apply_disc_numbers(tag: &mut Tag, meta: &ComprehensiveMetadata, config: &MetadataConfig) {
    if config.is_enabled(DiscNumber)
        && let Some(n) = meta.disc_number
    {
        tag.set_disk(n.cast_unsigned());
    }
    if config.is_enabled(DiscTotal)
        && let Some(n) = meta.disc_total
    {
        tag.set_disk_total(n.cast_unsigned());
    }
}

/// Writes the copyright message tag.
///
/// # Arguments
///
/// * `tag` - Target tag to write into
/// * `meta` - Source metadata
/// * `config` - Field toggle configuration
fn apply_copyright(tag: &mut Tag, meta: &ComprehensiveMetadata, config: &MetadataConfig) {
    if !config.is_enabled(Copyright) {
        return;
    }
    if let Some(copyright) = meta.copyright.as_ref() {
        tag.push(TagItem::new(
            CopyrightMessage,
            ItemValue::Text(copyright.clone()),
        ));
    }
}

/// Writes the ISRC tag.
///
/// # Arguments
///
/// * `tag` - Target tag to write into
/// * `meta` - Source metadata
/// * `config` - Field toggle configuration
fn apply_isrc(tag: &mut Tag, meta: &ComprehensiveMetadata, config: &MetadataConfig) {
    if !config.is_enabled(FieldIsrc) {
        return;
    }
    if let Some(isrc) = meta.isrc.as_ref() {
        tag.push(TagItem::new(Isrc, ItemValue::Text(isrc.clone())));
    }
}

/// Writes release date and year tags with format-specific keys.
///
/// # Arguments
///
/// * `tag` - Target tag to write into
/// * `meta` - Source metadata
/// * `config` - Field toggle configuration
/// * `is_flac` - Whether the output file is FLAC (affects tag key selection)
fn apply_dates(
    tag: &mut Tag,
    meta: &ComprehensiveMetadata,
    config: &MetadataConfig,
    is_flac: bool,
) {
    let (date_full, year) = determine_primary_date(meta);
    if config.is_enabled(ReleaseYear)
        && let Some(y) = year
    {
        if is_flac {
            tag.push(TagItem::new(Year, ItemValue::Text(y.to_string())));
        } else {
            tag.push(TagItem::new(RecordingDate, ItemValue::Text(y.to_string())));
        }
    }
    if !config.is_enabled(FieldReleaseDate) {
        return;
    }
    if let Some(date_str) = date_full.as_ref() {
        if is_flac {
            tag.push(TagItem::new(
                RecordingDate,
                ItemValue::Text(date_str.clone()),
            ));
        } else {
            tag.push(TagItem::new(ReleaseDate, ItemValue::Text(date_str.clone())));
        }
    }
}

/// Writes the commercial information URL tag.
///
/// # Arguments
///
/// * `tag` - Target tag to write into
/// * `meta` - Source metadata
/// * `config` - Field toggle configuration
fn apply_url(tag: &mut Tag, meta: &ComprehensiveMetadata, config: &MetadataConfig) {
    if !config.is_enabled(FieldUrl) {
        return;
    }
    if let Some(url) = meta.product_url.as_ref() {
        let full = if url.starts_with("http") {
            url.clone()
        } else {
            format!("https://www.qobuz.com{url}")
        };
        tag.push(TagItem::new(
            CommercialInformationUrl,
            ItemValue::Text(full),
        ));
    }
}

/// Writes the original media type tag.
///
/// # Arguments
///
/// * `tag` - Target tag to write into
/// * `meta` - Source metadata
/// * `config` - Field toggle configuration
fn apply_media_type(tag: &mut Tag, meta: &ComprehensiveMetadata, config: &MetadataConfig) {
    if !config.is_enabled(MediaType) {
        return;
    }
    let media = meta.release_type.as_ref().or(meta.product_type.as_ref());
    if let Some(mt) = media {
        tag.push(TagItem::new(OriginalMediaType, ItemValue::Text(mt.clone())));
    }
}

/// Writes embedded cover art.
///
/// # Arguments
///
/// * `tag` - Target tag to write into
/// * `meta` - Source metadata containing cover art binary data
/// * `config` - Field toggle configuration
fn apply_cover_art(tag: &mut Tag, meta: &ComprehensiveMetadata, config: &MetadataConfig) {
    if !config.is_enabled(CoverArt) {
        return;
    }
    if let Some(data) = &meta.cover_art_data {
        let picture = Picture::unchecked(data.clone())
            .pic_type(CoverFront)
            .mime_type(MimeType::Jpeg)
            .build();
        tag.push_picture(picture);
    }
}

/// Writes FLAC-specific `VorbisComment` keys that have no `lofty` `ItemKey` mapping.
///
/// These keys (`INVOLVEDPEOPLE`, `ITUNESADVISORY`, `ORGANIZATION`, `UPC`, `URL`)
/// are written by the C# reference implementation but silently dropped by lofty's
/// generic `Tag` → `VorbisComments` conversion.
///
/// # Arguments
///
/// * `vc` - Target `VorbisComments` tag
/// * `meta` - Source metadata
/// * `config` - Field toggle configuration
fn apply_flac_custom_keys(
    vc: &mut VorbisComments,
    meta: &ComprehensiveMetadata,
    config: &MetadataConfig,
) {
    if config.is_enabled(InvolvedPeople)
        && let Some(performers) = meta.performers.as_ref()
        && !performers.is_empty()
    {
        let formatted = performers
            .replace("\r\n", ". ")
            .replace('\r', ". ")
            .trim()
            .to_string();
        vc.push("INVOLVEDPEOPLE".to_string(), formatted);
    }
    if config.is_enabled(Explicit)
        && let Some(warning) = meta.parental_warning
    {
        let val = if warning { "1" } else { "0" };
        vc.push("ITUNESADVISORY".to_string(), val.to_string());
    }
    if config.is_enabled(FieldLabel)
        && let Some(label) = meta.label.as_ref()
    {
        vc.push("ORGANIZATION".to_string(), label.clone());
    }
    if config.is_enabled(Upc)
        && let Some(upc) = meta.upc.as_ref()
    {
        vc.push("UPC".to_string(), upc.clone());
    }
    if config.is_enabled(FieldUrl)
        && let Some(url) = meta.product_url.as_ref()
    {
        let full = if url.starts_with("http") {
            url.clone()
        } else {
            format!("https://www.qobuz.com{url}")
        };
        vc.push("URL".to_string(), full);
    }
}

/// Resolves the primary date using priority: album download > album original > track original >
/// `released_at` timestamp.
///
/// # Arguments
///
/// * `meta` - Source metadata
///
/// # Returns
///
/// A tuple of `(full_date_string, year)`, either of which may be `None`.
fn determine_primary_date(meta: &ComprehensiveMetadata) -> (Option<String>, Option<u32>) {
    if let Some(d) = meta.album_release_date_download.as_ref() {
        return (Some(d.clone()), parse_year(d));
    }
    if let Some(d) = meta.album_release_date_original.as_ref() {
        return (Some(d.clone()), parse_year(d));
    }
    if let Some(d) = meta.track_release_date_original.as_ref() {
        return (Some(d.clone()), parse_year(d));
    }
    if let Some(ts) = meta.released_at {
        return timestamp_to_date_and_year(ts);
    }
    (None, None)
}

/// Parses a 4-digit year from a date string.
///
/// # Arguments
///
/// * `date` - Date string in `YYYY-MM-DD` or similar format
///
/// # Returns
///
/// The year as `Some(u32)`, or `None` if parsing fails.
fn parse_year(date: &str) -> Option<u32> {
    date.split('-').next()?.parse().ok()
}

/// Converts a Unix timestamp to a date string and year.
///
/// # Arguments
///
/// * `timestamp` - Unix timestamp in seconds
///
/// # Returns
///
/// A tuple of `(formatted_date_string, year)`.
fn timestamp_to_date_and_year(timestamp: i64) -> (Option<String>, Option<u32>) {
    let days = timestamp.div_euclid(86400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (
        Some(format!("{y:04}-{m:02}-{d:02}")),
        Some(y.try_into().unwrap_or(0)),
    )
}

/// Parses a performers string into `(person_name, roles)` pairs.
///
/// # Arguments
///
/// * `performers_str` - Raw performers string from the API (e.g., `"Name, Role - Other, Role2"`)
///
/// # Returns
///
/// A vector of `(person_name, role_list)` tuples.
fn parse_performers(performers_str: &str) -> Vec<(&str, Vec<&str>)> {
    performers_str
        .split(" - ")
        .filter_map(|group| {
            let group = group.trim();
            let mut parts: Vec<&str> = group.split(',').map(str::trim).collect();
            if parts.is_empty() {
                return None;
            }
            let person_name = parts.remove(0).trim();
            Some((person_name, parts))
        })
        .collect()
}

/// Extracts artist names from the performers string, filtering by relevant roles.
///
/// # Arguments
///
/// * `performers_str` - Raw performers string from the API
/// * `existing` - Set of already-known names to skip
///
/// # Returns
///
/// A vector of artist name strings matching performance roles.
fn extract_artist_names_from_performers(
    performers_str: &str,
    existing: &HashSet<String>,
) -> Vec<String> {
    let mut names = Vec::new();
    for (person_name, roles) in parse_performers(performers_str) {
        let has_role = roles.iter().any(|role| {
            *role == "Performer" || role.contains("MainArtist") || role.contains("FeaturedArtist")
        });
        if has_role && !existing.contains(person_name) && !names.contains(&person_name.to_string())
        {
            names.push(person_name.to_string());
        }
    }
    names
}

/// Extracts composer names from the performers string.
///
/// # Arguments
///
/// * `performers_str` - Raw performers string from the API
///
/// # Returns
///
/// A deduplicated vector of composer name strings.
fn extract_composers_from_performers(performers_str: &str) -> Vec<String> {
    let mut composers = Vec::new();
    for (person_name, roles) in parse_performers(performers_str) {
        let is_composer = roles
            .iter()
            .any(|r| r.contains("Composer") || r.contains("Lyricist"));
        if is_composer && !composers.contains(&person_name.to_string()) {
            composers.push(person_name.to_string());
        }
    }
    composers
}

/// Extracts producer names from the performers string.
///
/// # Arguments
///
/// * `performers_str` - Raw performers string from the API
///
/// # Returns
///
/// A vector of producer name strings.
fn extract_producers_from_performers(performers_str: &str) -> Vec<String> {
    let mut producers = Vec::new();
    for (person_name, roles) in parse_performers(performers_str) {
        if roles.contains(&"Producer") {
            producers.push(person_name.to_string());
        }
    }
    producers
}

/// Normalizes a composer name for comparison purposes.
///
/// # Arguments
///
/// * `name` - The raw composer name
///
/// # Returns
///
/// A lowercased, punctuation-normalized version of the name.
fn normalize_composer_name(name: &str) -> String {
    name.to_lowercase()
        .trim()
        .replace(['.', ','], "")
        .replace('-', " ")
        .replace("  ", " ")
        .trim()
        .to_string()
}

/// Checks if a composer name is a duplicate of an existing one.
///
/// # Arguments
///
/// * `name` - The composer name to check
/// * `existing` - Set of already-normalized composer names
///
/// # Returns
///
/// `true` if the name matches an existing entry after normalization.
fn is_duplicate_composer(name: &str, existing: &HashSet<String>) -> bool {
    let normalized = normalize_composer_name(name);
    if existing.contains(&normalized) {
        return true;
    }
    for e in existing {
        if e.contains(&normalized) || normalized.contains(e) {
            return true;
        }
    }
    false
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

    fn sample_metadata() -> ComprehensiveMetadata {
        ComprehensiveMetadata {
            title: Some("Test Song".into()),
            album: Some("Test Album".into()),
            performer_name: Some("Test Artist".into()),
            genre: Some("Rock".into()),
            album_release_date_original: Some("2024-01-15".into()),
            track_composer_name: Some("Composer Name".into()),
            track_number: Some(1),
            disc_number: Some(1),
            track_total: Some(10),
            disc_total: Some(1),
            isrc: Some("USXXX2400001".into()),
            copyright: Some("(C) 2024 Label".into()),
            label: Some("Test Label".into()),
            ..ComprehensiveMetadata::default()
        }
    }

    fn cover_art_metadata() -> ComprehensiveMetadata {
        ComprehensiveMetadata {
            cover_art_data: Some(vec![0xFFu8, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x00, 0x00]),
            ..sample_metadata()
        }
    }

    #[test]
    fn embed_flac_all_fields() -> AnyhowResult<()> {
        let dir = TempDir::new()?;
        let path = dir.path().join("test.flac");
        create_minimal_flac(&path)?;
        embed_metadata_in_file(&path, &sample_metadata(), &MetadataConfig::all())?;
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
        let mut config = MetadataConfig::default();
        config.set(Title, false);
        config.set(Artist, false);
        embed_metadata_in_file(&path, &sample_metadata(), &config)?;
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
        embed_metadata_in_file(&path, &cover_art_metadata(), &MetadataConfig::all())?;
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
        let mut config = MetadataConfig::all();
        config.set(CoverArt, false);
        embed_metadata_in_file(&path, &cover_art_metadata(), &config)?;
        ensure!(get_tag(&path)?.pictures().is_empty());
        Ok(())
    }

    #[test]
    fn embed_title_with_version() -> AnyhowResult<()> {
        let dir = TempDir::new()?;
        let path = dir.path().join("test.flac");
        create_minimal_flac(&path)?;
        let meta = ComprehensiveMetadata {
            track_version: Some("Remastered 2015".into()),
            ..sample_metadata()
        };
        embed_metadata_in_file(&path, &meta, &MetadataConfig::all())?;
        ensure!(get_tag(&path)?.title().as_deref() == Some("Test Song (Remastered 2015)"));
        Ok(())
    }
}
