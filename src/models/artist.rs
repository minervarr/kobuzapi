//! Artist data model.

use serde::Deserialize;

use crate::models::{
    album::{Album, Image},
    search::ItemSearchResult,
};

/// A music artist.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Artist {
    /// Unique artist identifier.
    pub id: Option<i32>,
    /// Artist name.
    pub name: Option<String>,
    /// URL-friendly name.
    pub slug: Option<String>,
    /// Artist image.
    pub picture: Option<Image>,
    /// Artist image (alternate field).
    pub image: Option<Image>,
    /// Biography text.
    pub biography: Option<Biography>,
    /// Number of albums.
    pub albums_count: Option<i32>,
    /// Artist roles (main-artist, composer, etc.).
    pub roles: Option<Vec<String>>,
    /// Associated albums.
    pub albums: Option<ItemSearchResult<Box<Album>>>,
}

/// Artist biography text.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Biography {
    /// Biography text content.
    pub text: Option<String>,
    /// Summary text.
    pub summary: Option<String>,
}
