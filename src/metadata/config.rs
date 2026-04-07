//! Configuration for which metadata fields to embed.

use std::collections::HashSet;

/// Configuration controlling which metadata fields to embed in audio files.
///
/// Uses a set of `MetadataField` variants. `Default` enables all fields except `Comment`.
/// Use `is_enabled()` to check whether a field should be embedded.
#[derive(Clone, Debug, PartialEq)]
pub struct MetadataConfig {
    /// Set of enabled metadata fields.
    enabled: HashSet<MetadataField>,
}

impl MetadataConfig {
    /// Creates a config with all fields enabled.
    ///
    /// # Returns
    ///
    /// A `MetadataConfig` with every `MetadataField` enabled.
    #[must_use]
    pub fn all() -> Self {
        Self {
            enabled: HashSet::from([
                MetadataField::Title,
                MetadataField::Artist,
                MetadataField::Album,
                MetadataField::AlbumArtist,
                MetadataField::Genre,
                MetadataField::Date,
                MetadataField::Composer,
                MetadataField::Conductor,
                MetadataField::Performer,
                MetadataField::TrackNumber,
                MetadataField::DiscNumber,
                MetadataField::CoverArt,
                MetadataField::Isrc,
                MetadataField::Copyright,
                MetadataField::Label,
                MetadataField::Media,
                MetadataField::Comment,
                MetadataField::Producer,
            ]),
        }
    }

    /// Returns whether a specific field is enabled for embedding.
    ///
    /// # Arguments
    ///
    /// * `field` - The metadata field to check
    ///
    /// # Returns
    ///
    /// `true` if the field is enabled.
    #[must_use]
    pub fn is_enabled(&self, field: MetadataField) -> bool {
        self.enabled.contains(&field)
    }

    /// Enables or disables a specific field.
    ///
    /// # Arguments
    ///
    /// * `field` - The metadata field to toggle
    /// * `enabled` - `true` to enable, `false` to disable
    pub fn set(&mut self, field: MetadataField, enabled: bool) {
        if enabled {
            self.enabled.insert(field);
        } else {
            self.enabled.remove(&field);
        }
    }
}

impl Default for MetadataConfig {
    fn default() -> Self {
        let mut config = Self::all();
        config.enabled.remove(&MetadataField::Comment);
        config
    }
}

/// Metadata fields that can be embedded in audio files.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MetadataField {
    /// Track title.
    Title,
    /// Artist name.
    Artist,
    /// Album title.
    Album,
    /// Album artist.
    AlbumArtist,
    /// Genre.
    Genre,
    /// Release date.
    Date,
    /// Composer.
    Composer,
    /// Conductor.
    Conductor,
    /// Performer.
    Performer,
    /// Track number.
    TrackNumber,
    /// Disc number.
    DiscNumber,
    /// Cover art image.
    CoverArt,
    /// ISRC code.
    Isrc,
    /// Copyright notice.
    Copyright,
    /// Record label.
    Label,
    /// Original media type.
    Media,
    /// Comment field.
    Comment,
    /// Producer.
    Producer,
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, ensure};

    use crate::metadata::config::{
        MetadataConfig,
        MetadataField::{Comment, Producer, Title},
    };

    #[test]
    fn default_excludes_comment() -> Result<()> {
        let config = MetadataConfig::default();
        ensure!(config.is_enabled(Title));
        ensure!(!config.is_enabled(Comment));
        Ok(())
    }

    #[test]
    fn all_enables_every_field() -> Result<()> {
        let config = MetadataConfig::all();
        ensure!(config.is_enabled(Comment));
        ensure!(config.is_enabled(Producer));
        Ok(())
    }

    #[test]
    fn set_toggles_field() -> Result<()> {
        let mut config = MetadataConfig::default();
        ensure!(!config.is_enabled(Comment));
        config.set(Comment, true);
        ensure!(config.is_enabled(Comment));
        config.set(Comment, false);
        ensure!(!config.is_enabled(Comment));
        Ok(())
    }
}
