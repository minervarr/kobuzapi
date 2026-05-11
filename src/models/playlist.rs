//! Playlist data model.

use serde::{Deserialize, Serialize};

use crate::models::{
    album::Image, deserialization::deserialize_flexible_string_id, search::ItemSearchResult,
    subscription::User, track::Track,
};

/// A curated list of tracks.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
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
    /// Playlist creator (full user object, from `/playlist/get`).
    pub creator: Option<User>,
    /// Playlist owner (simpler object with name, from search endpoints).
    pub owner: Option<PlaylistOwner>,
    /// Playlist cover art.
    pub image: Option<Image>,
    /// Contained tracks.
    pub tracks: Option<ItemSearchResult<Box<Track>>>,
    /// Creation timestamp.
    pub created_at: Option<i64>,
    /// Last update timestamp.
    pub updated_at: Option<i64>,
}

impl Playlist {
    /// Returns the display name of the playlist creator/owner, checking
    /// `creator.display_name` first (from `/playlist/get`), then `owner.name`
    /// (from search endpoints).
    ///
    /// # Returns
    ///
    /// `Some(&str)` with the display name if available, `None` otherwise.
    #[must_use]
    pub fn creator_name(&self) -> Option<&str> {
        self.creator
            .as_ref()
            .and_then(|c| c.display_name.as_deref())
            .or_else(|| {
                let o = self.owner.as_ref()?;
                o.name.as_deref()
            })
    }
}

/// Playlist owner (returned by search endpoints).
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct PlaylistOwner {
    /// Owner user ID.
    pub id: Option<i32>,
    /// Owner display name.
    pub name: Option<String>,
}
