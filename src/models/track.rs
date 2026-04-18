//! Track data model.

use {
    serde::{Deserialize, Serialize},
    serde_json::Value,
};

use crate::models::{album::Album, artist::Artist};

/// Audio technical details.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct AudioInfo {
    /// Bit depth.
    pub bit_depth: Option<i32>,
    /// Sample rate in kHz.
    pub sampling_rate: Option<f64>,
    /// Channel count.
    pub channels: Option<i32>,
    /// Audio codec.
    pub codec: Option<String>,
}

/// An individual music track.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Track {
    /// Unique track identifier.
    pub id: Option<i32>,
    /// Track title.
    pub title: Option<String>,
    /// Version subtitle.
    pub version: Option<String>,
    /// ISRC code.
    pub isrc: Option<String>,
    /// Track position in album.
    pub track_number: Option<i32>,
    /// Duration in seconds.
    pub duration: Option<i32>,
    /// Disc number.
    pub media_number: Option<i32>,
    /// Classical work title.
    pub work: Option<String>,
    /// Parent album.
    pub album: Option<Box<Album>>,
    /// Primary performer.
    pub performer: Option<Box<Artist>>,
    /// All performers (formatted string).
    pub performers: Option<String>,
    /// Primary composer.
    pub composer: Option<Box<Artist>>,
    /// Audio technical details.
    pub audio_info: Option<AudioInfo>,
    /// Copyright notice.
    pub copyright: Option<String>,
    /// Streaming available.
    pub streamable: Option<bool>,
    /// Download available.
    pub downloadable: Option<bool>,
    /// Hi-Res available.
    pub hires: Option<bool>,
    /// Max bit depth.
    pub maximum_bit_depth: Option<i32>,
    /// Max sample rate.
    pub maximum_sampling_rate: Option<f64>,
    /// Max channel count.
    pub maximum_channel_count: Option<i32>,
    /// Original release date.
    pub release_date_original: Option<String>,
    /// Streaming date.
    pub release_date_stream: Option<String>,
    /// Explicit content flag.
    pub parental_warning: Option<bool>,
    /// Sales metadata.
    pub product_sales_factors: Option<Value>,
}
