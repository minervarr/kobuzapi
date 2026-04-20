//! Shared deserialization helpers for API responses.

use {
    serde::{Deserialize, Deserializer, de::Error},
    serde_json::{
        Value::{self, Null, Object, String as SerdeString},
        from_value,
    },
};

use crate::models::album::Image;

/// Deserializes an optional JSON value.
///
/// # Arguments
///
/// * `deserializer` - The serde deserializer
///
/// # Returns
///
/// An optional JSON value.
///
/// # Errors
///
/// Returns a deserialization error if deserialization fails.
fn deserialize_optional_value<'de, D>(deserializer: D) -> Result<Option<Value>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<Value>::deserialize(deserializer)
}

/// Maps a deserialized value to a string, handling common cases.
///
/// # Arguments
///
/// * `value` - Optional JSON value to convert
///
/// # Returns
///
/// A string representation if conversion succeeds, or `None`.
fn map_value_to_string(value: Option<Value>) -> Option<String> {
    match value {
        None | Some(Null) => None,
        Some(SerdeString(s)) => Some(s),
        Some(v) => Some(v.to_string()),
    }
}

/// Deserializes fields that the API returns as either a plain string or `{"display":"Name"}`.
///
/// # Arguments
///
/// * `deserializer` - The serde deserializer
///
/// # Errors
///
/// Returns a deserialization error if the value is an invalid type.
///
/// # Returns
///
/// `Ok(Some(string))` for string values or objects with a `display` field,
/// `Ok(None)` for null/missing.
pub fn deserialize_flexible_name<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = deserialize_optional_value(deserializer)?;
    match value {
        None | Some(Null) => Ok(None),
        Some(SerdeString(s)) => Ok(Some(s)),
        Some(Object(map)) => map
            .get("display")
            .and_then(Value::as_str)
            .map(|s| Some(s.to_string()))
            .ok_or_else(|| Error::custom("object missing 'display' field")),
        Some(v) => Ok(map_value_to_string(Some(v))),
    }
}

/// Deserializes fields that the API returns as either a string, number, or other value.
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
pub fn deserialize_flexible_string_id<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = deserialize_optional_value(deserializer)?;
    Ok(map_value_to_string(value))
}

/// Deserializes picture/image fields the API returns as either a string URL, null, or an Image
/// object.
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
pub fn deserialize_picture<'de, D>(deserializer: D) -> Result<Option<Image>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = deserialize_optional_value(deserializer)?;
    match value {
        None | Some(Null | SerdeString(_)) => Ok(None),
        Some(v) => from_value(v).map_err(Error::custom),
    }
}
