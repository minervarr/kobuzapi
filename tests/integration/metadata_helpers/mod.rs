//! Helpers for metadata comparison integration tests.

pub mod metadata_report;

use std::{
    collections::{HashMap, HashSet},
    fs::{create_dir_all, remove_file, rename, write},
    path::{Path, PathBuf},
    process::Command,
};

use {
    anyhow::{Error, Result, anyhow, bail},
    reqwest::Client,
    serde_json::to_string_pretty,
    tokio::runtime::Runtime,
    tracing::info,
};

use qobuz_api_rust_refactor::{
    api::service::QobuzApiService,
    metadata::{
        config::MetadataConfig, embedder::embed_metadata_in_file,
        extractor::extract_comprehensive_metadata,
    },
    models::{file_url::quality::MP3_320, track::Track},
    sanitize::sanitize_filename,
};

/// Constructs a [`TestTrack`] from artist, title, track ID, and C# base name.
macro_rules! track {
    ($artist:expr, $title:expr, $id:expr, $csharp:expr) => {
        TestTrack {
            artist: $artist,
            title: $title,
            track_id: $id,
            csharp_base: $csharp,
        }
    };
}

/// Metadata fields related to file name and directory paths (always differ, ignored).
pub const DIRECTORY_FILENAME_IGNORED: &[&str] = &["File Name", "Directory"];

/// Metadata fields related to duration and sample counts (encoding-dependent, ignored).
pub const DURATION_IGNORED: &[&str] = &["Duration", "Total Samples"];

/// Metadata fields related to file system timestamps (always differ, ignored).
pub const FILE_DATE_TIME_IGNORED: &[&str] = &[
    "File Modification Date/Time",
    "File Access Date/Time",
    "File Inode Change Date/Time",
];

/// Metadata fields related to file and ID3 tag sizes (always differ, ignored).
pub const FILE_SIZE_IGNORED: &[&str] = &["File Size", "ID3 Size"];

/// LAME encoder-specific metadata fields (MP3-only, encoding-dependent, ignored).
pub const LAME_IGNORED: &[&str] = &[
    "Encoder",
    "Lame Bitrate",
    "Lame Low Pass Filter",
    "Lame Method",
    "Lame Quality",
    "Lame Stereo Mode",
    "Lame VBR Quality",
    "MS Stereo",
];
/// Root directory for metadata test artifacts.
pub const METADATA_TEST_DIR: &str = "metadata_tests";

/// Picture dimension metadata fields (always differ due to resizing, ignored).
pub const PICTURE_IGNORED: &[&str] = &["Picture Bits Per Pixel", "Picture Height", "Picture Width"];

/// `ExifTool` version field (tool-dependent, ignored).
pub const VERSION_IGNORED: &[&str] = &["ExifTool Version Number"];

/// Canonical set of tracks used for metadata comparison testing.
static TEST_TRACKS: [TestTrack; 20] = [
    track!("Adele", "Hello", 28_014_963, "Adele - Hello"),
    track!(
        "Daft Punk",
        "Around the World",
        1_065_482,
        "Daft Punk - Around the World"
    ),
    track!(
        "Glenn Gould",
        "Aria _ Sarabande",
        26_890_072,
        "Glenn Gould - Aria _ Sarabande (Remastered)"
    ),
    track!(
        "Herbie Hancock",
        "Chameleon",
        37_905,
        "Herbie Hancock - Chameleon (Album Version)"
    ),
    track!(
        "Kendrick Lamar",
        "BLOOD.",
        40_128_300,
        "Kendrick Lamar - BLOOD"
    ),
    track!(
        "Madonna",
        "Like a Virgin",
        6_004_880,
        "Madonna - Like a Virgin"
    ),
    track!(
        "Miles Davis",
        "So What",
        13_176_083,
        "Miles Davis - So What"
    ),
    track!(
        "Nirvana",
        "Smells Like Teen Spirit",
        14_158_543,
        "Nirvana - Smells Like Teen Spirit (Album Version)"
    ),
    track!(
        "Staatskapelle Dresden",
        "Symphony No. 40 in G Minor, K. 550: I. Molto Allegro",
        162_673_929,
        "Staatskapelle Dresden - Symphony No. 40 in G Minor, K. 550_ I. Molto Allegro (Remastered)"
    ),
    track!(
        "The Beatles",
        "Hey Jude",
        235_803_319,
        "The Beatles - Hey Jude (Remastered 2015)"
    ),
    track!(
        "Johnny Cash",
        "Hurt",
        33_839_468,
        "Johnny Cash - Hurt (Album Version)"
    ),
    track!(
        "Stevie Wonder",
        "Superstition",
        16_012_386,
        "Stevie Wonder - Superstition (Album Version)"
    ),
    track!(
        "Bob Marley & The Wailers",
        "Three Little Birds",
        52_323_780,
        "Bob Marley & The Wailers - Three Little Birds"
    ),
    track!(
        "B.B. King",
        "The Thrill Is Gone",
        52_371_130,
        "B.B. King - The Thrill Is Gone"
    ),
    track!(
        "Metallica",
        "Enter Sandman",
        123_452_387,
        "Metallica - Enter Sandman (Remastered 2021)"
    ),
    track!(
        "Bob Dylan",
        "Like a Rolling Stone",
        15_215_741,
        "Bob Dylan - Like a Rolling Stone"
    ),
    track!(
        "Donna Summer",
        "I Feel Love",
        52_482_680,
        "Donna Summer - I Feel Love"
    ),
    track!(
        "Sex Pistols",
        "Anarchy In The UK",
        262_708_929,
        "Sex Pistols - Anarchy In The UK"
    ),
    track!(
        "Buena Vista Social Club",
        "Chan Chan",
        387_387_265,
        "Buena Vista Social Club - Chan Chan (Remastered 2021)"
    ),
    track!("Hans Zimmer", "Time", 2_206_801, "Hans Zimmer - Time"),
];

/// A single key-value entry parsed from `exiftool -G1` output.
pub struct ExifEntry {
    /// `ExifTool` field name (e.g., "ID3v2.3:Album").
    pub field: String,
    /// Field value as a string.
    pub value: String,
}

/// Describes a difference between Rust and C# metadata for a single field.
#[derive(Clone, Debug)]
pub enum FieldDifference {
    /// Field exists in both but values differ.
    Differs {
        /// Field name.
        field: String,
        /// Value embedded by Rust implementation.
        rust_value: String,
        /// Value embedded by C# reference implementation.
        csharp_value: String,
    },
    /// Field exists only in the Rust output.
    OnlyInRust {
        /// Field name.
        field: String,
        /// Value in Rust output.
        value: String,
    },
    /// Field exists only in the C# output.
    OnlyInCSharp {
        /// Field name.
        field: String,
        /// Value in C# output.
        value: String,
    },
}

/// Aggregated comparison results across all tracks for one format.
#[derive(Default)]
pub struct ReportSummary {
    /// Differences grouped by field name, each with file header and diff details.
    pub differences: HashMap<String, Vec<(String, FieldDifference)>>,
    /// Count of ignored differences per field name.
    pub ignored_counts: HashMap<String, usize>,
}

/// Descriptor for a test track used in metadata comparison.
pub struct TestTrack {
    /// Primary artist name.
    pub artist: &'static str,
    /// Track title.
    pub title: &'static str,
    /// Qobuz track ID.
    pub track_id: i32,
    /// Base filename of the C# reference metadata file.
    pub csharp_base: &'static str,
}

/// Returns the canonical set of tracks used for metadata comparison testing.
///
/// # Returns
///
/// A static reference to the array of test tracks.
pub fn test_tracks() -> &'static [TestTrack] {
    &TEST_TRACKS
}

/// Builds a file header string for diff reports comparing Rust vs C# output.
///
/// # Arguments
///
/// * `track` - The test track descriptor
/// * `format_label` - Format label (e.g., "FLAC", "MP3")
///
/// # Returns
///
/// A formatted header string with full paths.
pub fn build_file_header(track: &TestTrack, format_label: &str) -> String {
    let format_lower = format_label.to_lowercase();
    let rust_base = track_filename_base(track, format_label);
    format!(
        "@/qobuz-api-rust-refactor/metadata_tests/metadata/C#-songs/{format_lower}/{}.txt vs \
         @/qobuz-api-rust-refactor/metadata_tests/metadata/{format_lower}/{rust_base}.txt",
        track.csharp_base,
    )
}

/// Returns the path to the C# reference metadata directory for a given format.
///
/// # Arguments
///
/// * `format` - Format subdirectory name (e.g., "flac", "mp3")
///
/// # Returns
///
/// A `PathBuf` pointing to the C# reference directory.
pub fn csharp_metadata_dir(format: &str) -> PathBuf {
    Path::new(METADATA_TEST_DIR)
        .join("metadata")
        .join("C#-songs")
        .join(format)
}

/// Returns the path to the downloads directory for a given format.
///
/// # Arguments
///
/// * `format` - Format subdirectory name (e.g., "flac", "mp3")
///
/// # Returns
///
/// A `PathBuf` pointing to the downloads directory.
pub fn downloads_dir(format: &str) -> PathBuf {
    Path::new(METADATA_TEST_DIR).join("downloads").join(format)
}

/// Returns the path to the JSON metadata directory.
///
/// # Returns
///
/// A `PathBuf` pointing to the JSON output directory.
pub fn json_dir() -> PathBuf {
    Path::new(METADATA_TEST_DIR).join("metadata").join("json")
}

/// Returns the path to the Rust metadata output directory for a given format.
///
/// # Arguments
///
/// * `format` - Format subdirectory name (e.g., "flac", "mp3")
///
/// # Returns
///
/// A `PathBuf` pointing to the metadata output directory.
pub fn metadata_dir(format: &str) -> PathBuf {
    Path::new(METADATA_TEST_DIR).join("metadata").join(format)
}

/// Returns the path to the reports output directory.
///
/// # Returns
///
/// A `PathBuf` pointing to the reports directory.
pub fn reports_dir() -> PathBuf {
    Path::new(METADATA_TEST_DIR).join("reports")
}

/// Builds a sanitized base filename for a test track and format.
///
/// # Arguments
///
/// * `track` - The test track descriptor
/// * `format_label` - Format label (e.g., "FLAC", "MP3")
///
/// # Returns
///
/// A sanitized filename string in the form `{artist} - {title} - {format} - {id}`.
pub fn track_filename_base(track: &TestTrack, format_label: &str) -> String {
    sanitize_filename(&format!(
        "{} - {} - {format_label} - {}",
        track.artist, track.title, track.track_id
    ))
}

/// Compares `ExifTool` metadata entries between Rust and C# outputs.
///
/// Filters out known-ignored fields and returns both meaningful differences
/// and ignored field counts.
///
/// # Arguments
///
/// * `rust_entries` - Metadata entries from the Rust-embedded file
/// * `csharp_entries` - Metadata entries from the C# reference file
/// * `is_mp3` - Whether the format is MP3 (enables additional LAME ignored fields)
///
/// # Returns
///
/// A tuple of (differences, `ignored_field_counts`).
pub fn compare_exif_metadata(
    rust_entries: &[ExifEntry],
    csharp_entries: &[ExifEntry],
    is_mp3: bool,
) -> (Vec<FieldDifference>, Vec<(String, String)>) {
    let rust_map: HashMap<&str, &str> = rust_entries
        .iter()
        .map(|e| (e.field.as_str(), e.value.as_str()))
        .collect();
    let csharp_map: HashMap<&str, &str> = csharp_entries
        .iter()
        .map(|e| (e.field.as_str(), e.value.as_str()))
        .collect();
    let mut diffs = Vec::new();
    let mut ignored = Vec::new();
    let all_keys: HashSet<&&str> = rust_map.keys().chain(csharp_map.keys()).collect();
    for &key in &all_keys {
        let rust_val = rust_map.get(key).copied();
        let csharp_val = csharp_map.get(key).copied();
        let is_ignored = is_ignored_field(key, is_mp3);
        if is_ignored && rust_val != csharp_val {
            ignored.push((key.to_string(), "ignored (always different)".to_string()));
        }
        if is_ignored {
            continue;
        }
        if let Some(diff) = diff_field(key, rust_val, csharp_val) {
            diffs.push(diff);
        }
    }
    (diffs, ignored)
}

/// Compares Rust and C# field values and returns a `FieldDifference` if they differ.
///
/// # Arguments
///
/// * `key` - The field name being compared
/// * `rust_val` - Value from the Rust implementation (may be `None`)
/// * `csharp_val` - Value from the C# reference (may be `None`)
///
/// # Returns
///
/// `Some(FieldDifference)` if the values differ or one is missing, `None` if identical.
fn diff_field(
    key: &str,
    rust_val: Option<&str>,
    csharp_val: Option<&str>,
) -> Option<FieldDifference> {
    match (rust_val, csharp_val) {
        (Some(rv), Some(cv)) if !values_equivalent(rv, cv) => Some(FieldDifference::Differs {
            field: key.to_string(),
            rust_value: rv.to_string(),
            csharp_value: cv.to_string(),
        }),
        (Some(rv), None) => Some(FieldDifference::OnlyInRust {
            field: key.to_string(),
            value: rv.to_string(),
        }),
        (None, Some(cv)) => Some(FieldDifference::OnlyInCSharp {
            field: key.to_string(),
            value: cv.to_string(),
        }),
        _ => None,
    }
}

/// Downloads a test track, embeds metadata, and saves to the downloads directory.
///
/// # Arguments
///
/// * `service` - Authenticated API service
/// * `track` - Test track descriptor
/// * `format_id` - Quality format ID
///
/// # Returns
///
/// The path to the downloaded and tagged file.
pub fn download_test_track(
    service: &mut QobuzApiService,
    track: &TestTrack,
    format_id: i32,
) -> Result<PathBuf> {
    let format_label = if format_id == MP3_320 { "MP3" } else { "FLAC" };
    let ext = if format_id == MP3_320 { "mp3" } else { "flac" };
    let dl_dir = downloads_dir(&format_label.to_lowercase());
    let base = track_filename_base(track, format_label);
    let filename = format!("{base}.{ext}");
    let file_path = dl_dir.join(&filename);
    if file_path.exists() {
        remove_file(&file_path)?;
    }
    let downloaded = service.download_track(track.track_id, format_id, &dl_dir, None)?;
    let track_details: Track = service.get_track(track.track_id).map_err(Error::from)?;
    let album = track_details.album.as_deref();
    let mut meta = extract_comprehensive_metadata(&track_details, album, None);
    if let Some(url) = meta.cover_art_url.as_ref() {
        if let Ok(data) = download_cover_art(url) {
            meta.cover_art_data = Some(data);
        } else {
            info!(url = %url, "Failed to download cover art");
        }
    }
    let metadata_config = MetadataConfig::default();
    embed_metadata_in_file(&downloaded, &meta, &metadata_config)?;
    if downloaded != file_path {
        rename(&downloaded, &file_path)?;
    }
    info!(track = %track.title, format = format_label, "Downloaded and tagged");
    Ok(file_path)
}

/// Downloads cover art image data from a URL.
///
/// # Arguments
///
/// * `url` - URL of the cover art image
///
/// # Returns
///
/// `Ok(Vec<u8>)` on success with the raw image bytes, or a network error.
fn download_cover_art(url: &str) -> Result<Vec<u8>> {
    let rt = Runtime::new()?;
    rt.block_on(async {
        let client = Client::new();
        let resp = client.get(url).send().await?;
        resp.bytes().await.map(|b| b.to_vec()).map_err(Error::from)
    })
}

/// Creates all required directories for metadata test artifacts.
///
/// # Returns
///
/// `Ok(())` on success, or an I/O error.
pub fn ensure_directories() -> Result<()> {
    for dir in [
        downloads_dir("flac"),
        downloads_dir("mp3"),
        metadata_dir("flac"),
        metadata_dir("mp3"),
        json_dir(),
        reports_dir(),
    ] {
        create_dir_all(&dir)?;
    }
    Ok(())
}

/// Extracts metadata from an audio file using `ExifTool` and saves the raw output to disk.
///
/// # Arguments
///
/// * `file_path` - Path to the audio file
/// * `output_path` - Path to write the raw `ExifTool` output
///
/// # Returns
///
/// Parsed `ExifTool` entries.
pub fn extract_and_save_metadata(file_path: &Path, output_path: &Path) -> Result<Vec<ExifEntry>> {
    let entries = extract_metadata_exiftool(file_path)?;
    let output = Command::new("exiftool")
        .arg("-G1")
        .arg(file_path)
        .output()
        .map_err(|e| anyhow!("exiftool: {e}"))?;
    write(
        output_path,
        String::from_utf8_lossy(&output.stdout).as_bytes(),
    )?;
    Ok(entries)
}

/// Extracts metadata from an audio file using `exiftool -G1`.
///
/// # Arguments
///
/// * `file_path` - Path to the audio file
///
/// # Returns
///
/// Parsed `ExifTool` entries.
pub fn extract_metadata_exiftool(file_path: &Path) -> Result<Vec<ExifEntry>> {
    let output = Command::new("exiftool")
        .arg("-G1")
        .arg(file_path)
        .output()
        .map_err(|e| anyhow!("exiftool failed to start: {e}"))?;
    if !output.status.success() {
        bail!(
            "exiftool exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(parse_exiftool_output(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

/// Returns whether a metadata field should be ignored during comparison.
///
/// # Arguments
///
/// * `field` - The `ExifTool` field name to check
/// * `is_mp3` - Whether the format is MP3 (enables additional LAME ignored fields)
///
/// # Returns
///
/// `true` if the field is in an ignored category.
pub fn is_ignored_field(field: &str, is_mp3: bool) -> bool {
    let f = |list: &[&str]| list.contains(&field);
    f(FILE_DATE_TIME_IGNORED)
        || f(FILE_SIZE_IGNORED)
        || f(DIRECTORY_FILENAME_IGNORED)
        || f(PICTURE_IGNORED)
        || f(DURATION_IGNORED)
        || f(VERSION_IGNORED)
        || (is_mp3 && f(LAME_IGNORED))
}

/// Parses raw `exiftool -G1` output into a list of entries.
///
/// # Arguments
///
/// * `content` - Raw stdout from `exiftool -G1`
///
/// # Returns
///
/// A vector of parsed `ExifEntry` instances.
pub fn parse_exiftool_output(content: &str) -> Vec<ExifEntry> {
    let mut entries = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some(colon_pos) = trimmed.find(':') else {
            continue;
        };
        let before = &trimmed[..colon_pos];
        let value = trimmed[colon_pos + 1..].trim();
        let field = before
            .rfind(']')
            .map_or_else(|| before.trim(), |pos| before[pos + 1..].trim());
        if field.is_empty() || value.is_empty() {
            continue;
        }
        entries.push(ExifEntry {
            field: field.to_string(),
            value: value.to_string(),
        });
    }
    entries
}

/// Saves track details from the API as pretty-printed JSON for reference.
///
/// # Arguments
///
/// * `service` - Authenticated API service
/// * `track` - Test track descriptor
///
/// # Returns
///
/// `Ok(())` on success, or an API / I/O error.
pub fn save_track_json(service: &QobuzApiService, track: &TestTrack) -> Result<()> {
    let dir = json_dir();
    let base = sanitize_filename(&format!(
        "{} - {} - {}",
        track.artist, track.title, track.track_id
    ));
    let path = dir.join(format!("{base}.json"));
    let track_details: Track = service.get_track(track.track_id).map_err(Error::from)?;
    let json = to_string_pretty(&track_details)?;
    write(&path, json)?;
    info!(track = %track.title, "JSON saved");
    Ok(())
}

/// Groups two related fields (e.g., "Musician Credits" / "Involved People") in the diff list.
///
/// If both fields exist and their values match, the pair is moved to the ignored list
/// under the `grouped_name`. Otherwise, they are merged under `grouped_name` as a diff.
///
/// # Arguments
///
/// * `diffs` - Vector of field differences
/// * `ignored` - Vector of ignored field differences
/// * `field_a` - First field name to check
/// * `field_b` - Second field name to check
/// * `grouped_name` - Name to use for the grouped result
pub fn group_field_pair(
    diffs: &mut Vec<FieldDifference>,
    ignored: &mut Vec<(String, String)>,
    field_a: &str,
    field_b: &str,
    grouped_name: &str,
) {
    let a_diff = extract_diff_by_field(diffs, field_a);
    let b_diff = extract_diff_by_field(diffs, field_b);

    match (a_diff, b_diff) {
        (Some(a), Some(b)) => {
            let val_a = diff_value(&a);
            let val_b = diff_value(&b);
            if values_equivalent(&val_a, &val_b) {
                ignored.push((
                    grouped_name.to_string(),
                    "ignored (matching values)".to_string(),
                ));
            } else {
                diffs.push(FieldDifference::Differs {
                    field: grouped_name.to_string(),
                    rust_value: val_a,
                    csharp_value: val_b,
                });
            }
        }
        (Some(diff), None) | (None, Some(diff)) => {
            diffs.push(rename_field(diff, grouped_name));
        }
        (None, None) => {}
    }
}

/// Extracts and removes a field difference from the list by field name.
///
/// # Arguments
///
/// * `diffs` - Vector of field differences to search
/// * `field` - The field name to find
///
/// # Returns
///
/// `Some(FieldDifference)` if found and removed, `None` otherwise.
fn extract_diff_by_field(diffs: &mut Vec<FieldDifference>, field: &str) -> Option<FieldDifference> {
    let idx = diffs.iter().position(|d| diff_field_name(d) == field)?;
    Some(diffs.remove(idx))
}

/// Extracts the field name from a `FieldDifference`.
///
/// # Arguments
///
/// * `diff` - The field difference to extract from
///
/// # Returns
///
/// The field name as a string slice.
fn diff_field_name(diff: &FieldDifference) -> &str {
    match diff {
        FieldDifference::Differs { field, .. }
        | FieldDifference::OnlyInRust { field, .. }
        | FieldDifference::OnlyInCSharp { field, .. } => field,
    }
}

/// Extracts the Rust-side value from a `FieldDifference`.
///
/// # Arguments
///
/// * `diff` - The field difference to extract from
///
/// # Returns
///
/// The Rust value as a `String`.
fn diff_value(diff: &FieldDifference) -> String {
    match diff {
        FieldDifference::Differs { rust_value, .. } => rust_value.clone(),
        FieldDifference::OnlyInRust { value, .. } | FieldDifference::OnlyInCSharp { value, .. } => {
            value.clone()
        }
    }
}

/// Checks if two metadata values are equivalent, accounting for reordering of
/// dash-separated credits (e.g., "A - B" vs "B - A").
///
/// # Arguments
///
/// * `a` - First value to compare
/// * `b` - Second value to compare
///
/// # Returns
///
/// `true` if the values are identical or contain the same credits in any order.
fn values_equivalent(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let norm_a = normalize_whitespace(a);
    let norm_b = normalize_whitespace(b);
    if norm_a == norm_b {
        return true;
    }
    let mut parts_a: Vec<String> = norm_a.split(" - ").map(normalize_credit).collect();
    let mut parts_b: Vec<String> = norm_b.split(" - ").map(normalize_credit).collect();
    parts_a.sort_unstable();
    parts_b.sort_unstable();
    parts_a == parts_b
}

/// Collapses consecutive whitespace into a single space and trims.
///
/// # Arguments
///
/// * `s` - The string to normalize
///
/// # Returns
///
/// A string with consecutive whitespace collapsed into single spaces and trimmed.
fn normalize_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<&str>>().join(" ")
}

/// Normalizes a single credit entry by sorting its comma-separated roles
/// while keeping the name (first element) in place.
///
/// # Arguments
///
/// * `credit` - A credit string like "Name, Role1, Role2"
///
/// # Returns
///
/// A normalized string with sorted roles, e.g. "Name, Role2, Role1".
fn normalize_credit(credit: &str) -> String {
    let mut parts: Vec<&str> = credit.split(", ").collect();
    if parts.len() > 1 {
        let name = parts.remove(0);
        parts.sort_unstable();
        parts.insert(0, name);
    }
    parts.join(", ")
}

/// Creates a new `FieldDifference` with a different field name, preserving all values.
///
/// # Arguments
///
/// * `diff` - The original field difference
/// * `new_name` - The new field name to use
///
/// # Returns
///
/// A new `FieldDifference` with the updated field name.
fn rename_field(diff: FieldDifference, new_name: &str) -> FieldDifference {
    match diff {
        FieldDifference::Differs {
            rust_value,
            csharp_value,
            ..
        } => FieldDifference::Differs {
            field: new_name.to_string(),
            rust_value,
            csharp_value,
        },
        FieldDifference::OnlyInRust { value, .. } => FieldDifference::OnlyInRust {
            field: new_name.to_string(),
            value,
        },
        FieldDifference::OnlyInCSharp { value, .. } => FieldDifference::OnlyInCSharp {
            field: new_name.to_string(),
            value,
        },
    }
}
