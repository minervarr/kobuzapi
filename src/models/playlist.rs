//! Playlist data model.

use serde::Deserialize;

use crate::models::{album::Image, search::ItemSearchResult, subscription::User, track::Track};

/// A curated list of tracks.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Playlist {
    /// Unique playlist identifier.
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
