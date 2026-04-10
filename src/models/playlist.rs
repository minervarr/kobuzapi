//! Playlist data model.

use {
    serde::{Deserialize, Deserializer},
    serde_json::Value::{self, Null},
};

use crate::models::{album::Image, search::ItemSearchResult, subscription::User, track::Track};

/// A curated list of tracks.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Playlist {
    /// Unique playlist identifier (may be a string or number from the API).
    #[serde(default, deserialize_with = "deserialize_flexible_string_id")]
    pub id: Option<String>,
    /// Playlist name.
    pub name: Option<String>,
    /// Description text.
    pub description: Option<String>,
    /// Number of tracks.
    pub tracks_count: Option<i32>,
    /// Total duration in seconds.
    pub duration: Option<i32>,
    /// Public visibility.
    pub is_public: Option<bool>,
    /// Playlist creator.
    pub creator: Option<User>,
    /// Playlist cover art.
    pub image: Option<Image>,
    /// Contained tracks.
    pub tracks: Option<ItemSearchResult<Box<Track>>>,
    /// Creation timestamp.
    pub created_at: Option<i64>,
    /// Last update timestamp.
    pub updated_at: Option<i64>,
}

/// Deserializes an ID that may be a string or number from the API.
///
/// # Arguments
///
/// * `deserializer` - The serde deserializer
///
/// # Errors
///
/// Returns a deserialization error if the value cannot be deserialized.
///
/// # Returns
///
/// `Ok(Some(string))` for string/number values, `Ok(None)` for null/missing.
fn deserialize_flexible_string_id<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    match value {
        None | Some(Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s)),
        Some(Value::Number(n)) => Ok(Some(n.to_string())),
        Some(v) => Ok(Some(v.to_string())),
    }
}
