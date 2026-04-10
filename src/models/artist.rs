//! Artist data model.

use {
    serde::{Deserialize, Deserializer, de::Error},
    serde_json::{
        Value::{self, Null},
        from_value,
    },
};

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
    /// Artist picture (may be a string URL or an Image object from the API).
    #[serde(default, deserialize_with = "deserialize_picture")]
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

/// Deserializes `picture` which the API returns as either a string URL, null, or an Image object.
///
/// # Arguments
///
/// * `deserializer` - The serde deserializer
///
/// # Errors
///
/// Returns a deserialization error if the value is an invalid image object.
///
/// # Returns
///
/// `Ok(None)` for string URLs and null, `Ok(Some(Image))` for valid image objects.
fn deserialize_picture<'de, D>(deserializer: D) -> Result<Option<Image>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    match value {
        None | Some(Null | Value::String(_)) => Ok(None),
        Some(v) => from_value(v).map_err(Error::custom),
    }
}
