# Data Model: Qobuz API Client Refactor

**Feature**: `001-qobuz-api-refactor` | **Date**: 2026-04-06

## Entity Overview

```text
┌─────────────────────┐     ┌─────────────────────┐
│   QobuzApiService   │────→│     Credential      │
│   (central service) │     │   (auth state)      │
└─────────┬───────────┘     └─────────────────────┘
          │ uses
          ▼
┌─────────────────────┐     ┌─────────────────────┐
│      FileUrl        │     │    MetadataConfig   │
│   (download URL)    │     │   (tag preferences) │
└─────────────────────┘     └─────────────────────┘
          │
          │ describes
          ▼
┌─────────────────────┐     ┌─────────────────────┐
│       Track         │────→│       Album         │
│                     │     │                     │
└─────────┬───────────┘     └─────────┬───────────┘
          │ belongs to                │ by
          ▼                           ▼
┌─────────────────────┐     ┌─────────────────────┐
│      Playlist       │     │       Artist        │
│                     │     │                     │
└─────────────────────┘     └─────────────────────┘
          │
          ▼
┌─────────────────────┐
│    SearchResult     │
│  (grouped results)  │
└─────────────────────┘
          │
          ▼
┌─────────────────────┐
│    UserFavorites    │
│  (favorites list)   │
└─────────────────────┘
```

---

## Entities

### QobuzApiService

The central service holding authentication state and providing all API operations.

| Field | Type | Description |
|-------|------|-------------|
| `app_id` | `String` | Qobuz application ID (extracted from web player or env) |
| `app_secret` | `String` | Qobuz application secret (extracted from web player or env) |
| `user_auth_token` | `Option<String>` | User authentication token (set after login) |
| `client` | `Box<dyn HttpClient>` | HTTP client abstraction (`ReqwestClient` in production, `MockHttpClient` in tests). Backed by a single `reqwest::Client` with connection pooling. See research.md section 8 for trait definition. |
| `credentials_refreshed` | `bool` | Whether credentials have been refreshed this session |

**State transitions**:
1. `new()` → Unauthenticated (no user_auth_token)
2. `authenticate_with_env()` / `login()` / `login_with_token()` → Authenticated (user_auth_token set)
3. `refresh_app_credentials()` → Credentials refreshed (credentials_refreshed = true)

**Validation rules**:
- `app_id` and `app_secret` must be non-empty after construction
- `user_auth_token` must be `Some` for signed endpoints
- `credentials_refreshed` can only transition `false → true` once per session (constitution: ≤1 refresh)

---

### Album

A music album with full metadata.

| Field | Type | Description |
|-------|------|-------------|
| `id` | `String` | Unique album identifier |
| `title` | `Option<String>` | Album title |
| `version` | `Option<String>` | Version/subtitle |
| `upc` | `Option<String>` | Universal Product Code |
| `url` | `Option<String>` | Qobuz web URL |
| `artist` | `Option<Box<Artist>>` | Primary artist |
| `artists` | `Option<Vec<Box<Artist>>>` | All artists |
| `composer` | `Option<Box<Artist>>` | Primary composer |
| `label` | `Option<Label>` | Record label |
| `genre` | `Option<Genre>` | Primary genre |
| `genres` | `Option<Vec<Genre>>` | All genres |
| `image` | `Option<Image>` | Cover art URLs (multiple sizes) |
| `duration` | `Option<i32>` | Total duration in seconds |
| `tracks_count` | `Option<i32>` | Number of tracks |
| `media_count` | `Option<i32>` | Number of discs |
| `release_date_original` | `Option<String>` | Original release date |
| `release_date_stream` | `Option<String>` | Streaming availability date |
| `release_date_download` | `Option<String>` | Download availability date |
| `product_type` | `Option<String>` | Product type identifier |
| `release_type` | `Option<String>` | Release type (album, single, etc.) |
| `hires` | `Option<bool>` | Hi-Res audio available |
| `hires_streamable` | `Option<bool>` | Hi-Res streaming available |
| `downloadable` | `Option<bool>` | Download available |
| `streamable` | `Option<bool>` | Streaming available |
| `track_ids` | `Option<Vec<i32>>` | List of track IDs (when requested with extra) |
| `copyright` | `Option<String>` | Copyright notice |
| `product_sales_factors` | `Option<Value>` | Sales metadata |
| `maximum_bit_depth` | `Option<i32>` | Maximum available bit depth |
| `maximum_sampling_rate` | `Option<f64>` | Maximum available sample rate |
| `maximum_channel_count` | `Option<i32>` | Maximum channel count |

**Relationships**: Contains `Artist`, `Label`, `Genre`, `Image`. Referenced by `Track`.

---

### Artist

A music artist.

| Field | Type | Description |
|-------|------|-------------|
| `id` | `Option<i32>` | Unique artist identifier |
| `name` | `Option<String>` | Artist name |
| `slug` | `Option<String>` | URL-friendly name |
| `picture` | `Option<Image>` | Artist image |
| `image` | `Option<Image>` | Artist image (alternate field) |
| `biography` | `Option<Biography>` | Biography text |
| `albums_count` | `Option<i32>` | Number of albums |
| `roles` | `Option<Vec<String>>` | Artist roles (main-artist, composer, etc.) |
| `albums` | `Option<ItemSearchResult<Box<Album>>>` | Associated albums |

**Relationships**: Referenced by `Album`, `Track`. Contains `Image`, `Biography`.

#### Biography
| Field | Type | Description |
|-------|------|-------------|
| `text` | `Option<String>` | Full biography text (may contain HTML) |
| `summary` | `Option<String>` | Short biography summary |
| `lang` | `Option<String>` | Language code (e.g., "en") |

**Note**: The Qobuz API returns biography as a nested object with variable field names. All fields should be `Option` for safe partial deserialization.

---

### Track

An individual music track.

| Field | Type | Description |
|-------|------|-------------|
| `id` | `Option<i32>` | Unique track identifier |
| `title` | `Option<String>` | Track title |
| `version` | `Option<String>` | Version subtitle |
| `isrc` | `Option<String>` | ISRC code |
| `track_number` | `Option<i32>` | Track position in album |
| `duration` | `Option<i32>` | Duration in seconds |
| `media_number` | `Option<i32>` | Disc number |
| `work` | `Option<String>` | Classical work title |
| `album` | `Option<Box<Album>>` | Parent album |
| `performer` | `Option<Box<Artist>>` | Primary performer |
| `performers` | `Option<String>` | All performers (formatted string) |
| `composer` | `Option<Box<Artist>>` | Primary composer |
| `audio_info` | `Option<AudioInfo>` | Audio technical details |
| `copyright` | `Option<String>` | Copyright notice |
| `streamable` | `Option<bool>` | Streaming available |
| `downloadable` | `Option<bool>` | Download available |
| `hires` | `Option<bool>` | Hi-Res available |
| `maximum_bit_depth` | `Option<i32>` | Max bit depth |
| `maximum_sampling_rate` | `Option<f64>` | Max sample rate |
| `maximum_channel_count` | `Option<i32>` | Max channel count |
| `release_date_original` | `Option<String>` | Original release date |
| `release_date_stream` | `Option<String>` | Streaming date |
| `parental_warning` | `Option<bool>` | Explicit content flag |
| `product_sales_factors` | `Option<Value>` | Sales metadata |

**Relationships**: Belongs to `Album`. References `Artist` (performer, composer). Has `AudioInfo`.

---

### Playlist

A curated list of tracks.

| Field | Type | Description |
|-------|------|-------------|
| `id` | `Option<String>` | Unique playlist identifier |
| `name` | `Option<String>` | Playlist name |
| `description` | `Option<String>` | Description text |
| `tracks_count` | `Option<i32>` | Number of tracks |
| `duration` | `Option<i32>` | Total duration in seconds |
| `is_public` | `Option<bool>` | Public visibility |
| `creator` | `Option<User>` | Playlist creator |
| `image` | `Option<Image>` | Playlist cover art |
| `tracks` | `Option<ItemSearchResult<Box<Track>>>` | Contained tracks |
| `created_at` | `Option<i64>` | Creation timestamp |
| `updated_at` | `Option<i64>` | Last update timestamp |

---

### SearchResult

Grouped search results across content types.

| Field | Type | Description |
|-------|------|-------------|
| `albums` | `Option<ItemSearchResult<Box<Album>>>` | Matching albums |
| `artists` | `Option<ItemSearchResult<Box<Artist>>>` | Matching artists |
| `tracks` | `Option<ItemSearchResult<Box<Track>>>` | Matching tracks |
| `playlists` | `Option<ItemSearchResult<Box<Playlist>>>` | Matching playlists |

### ItemSearchResult\<T\>

Generic paginated result container.

| Field | Type | Description |
|-------|------|-------------|
| `items` | `Option<Vec<T>>` | Result items |
| `total` | `Option<i32>` | Total matching items |
| `limit` | `Option<i32>` | Items per page |
| `offset` | `Option<i32>` | Current page offset |

---

### FileUrl

Download URL for a track at a specific quality.

| Field | Type | Description |
|-------|------|-------------|
| `track_id` | `Option<i32>` | Track identifier |
| `duration` | `Option<f64>` | Track duration |
| `url` | `Option<String>` | Download URL |
| `format_id` | `Option<i32>` | Quality format ID |
| `mime_type` | `Option<String>` | MIME type |
| `sampling_rate` | `Option<f64>` | Sample rate |
| `bit_depth` | `Option<i32>` | Bit depth |
| `status` | `Option<i32>` | Response status code |
| `message` | `Option<String>` | Error message if applicable |
| `code` | `Option<String>` | Error code if applicable |

**Quality format IDs**:
| ID | Format |
|----|--------|
| 5 | MP3 320kbps |
| 6 | FLAC 16-bit/44.1kHz |
| 7 | FLAC 24-bit/96kHz |
| 27 | FLAC 24-bit/192kHz |

---

### MetadataConfig

Configuration for which metadata fields to embed.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `title` | `bool` | `true` | Track title |
| `artist` | `bool` | `true` | Artist name |
| `album` | `bool` | `true` | Album title |
| `album_artist` | `bool` | `true` | Album artist |
| `genre` | `bool` | `true` | Genre |
| `date` | `bool` | `true` | Release date |
| `composer` | `bool` | `true` | Composer |
| `conductor` | `bool` | `true` | Conductor |
| `performer` | `bool` | `true` | Performer |
| `track_number` | `bool` | `true` | Track number |
| `disc_number` | `bool` | `true` | Disc number |
| `cover_art` | `bool` | `true` | Cover art image |
| `isrc` | `bool` | `true` | ISRC code |
| `copyright` | `bool` | `true` | Copyright notice |
| `label` | `bool` | `true` | Record label |
| `media` | `bool` | `true` | Original media type |
| `comment` | `bool` | `false` | Comment field |
| `producer` | `bool` | `true` | Producer |

**Validation**: All fields are booleans; no complex validation needed.

---

### UserFavorites

Collection of user's favorited items.

| Field | Type | Description |
|-------|------|-------------|
| `albums` | `Option<ItemSearchResult<Box<Album>>>` | Favorite albums |
| `artists` | `Option<ItemSearchResult<Box<Artist>>>` | Favorite artists |
| `tracks` | `Option<ItemSearchResult<Box<Track>>>` | Favorite tracks |
| `article_ids` | `Option<Vec<i32>>` | Favorite article IDs |
| `artist_ids` | `Option<Vec<i32>>` | Favorite artist IDs |
| `album_ids` | `Option<Vec<i32>>` | Favorite album IDs |
| `track_ids` | `Option<Vec<i32>>` | Favorite track IDs |

---

### Credential

User authentication credentials.

| Field | Type | Description |
|-------|------|-------------|
| `user_id` | `Option<String>` | Qobuz user ID |
| `user_auth_token` | `Option<String>` | Authentication token |
| `email` | `Option<String>` | Email address |
| `password` | `Option<String>` | MD5-hashed password |
| `app_id` | `Option<String>` | Application ID |
| `app_secret` | `Option<String>` | Application secret |

---

### Supporting Types

#### Image
| Field | Type | Description |
|-------|------|-------------|
| `small` | `Option<String>` | Small thumbnail URL |
| `thumbnail` | `Option<String>` | Thumbnail URL |
| `medium` | `Option<String>` | Medium size URL |
| `large` | `Option<String>` | Large size URL |
| `extralarge` | `Option<String>` | Extra-large URL |
| `mega` | `Option<String>` | Highest resolution URL |
| `back` | `Option<String>` | Back cover URL |

#### Genre
| Field | Type | Description |
|-------|------|-------------|
| `id` | `Option<i32>` | Genre ID |
| `name` | `Option<String>` | Genre name |
| `slug` | `Option<String>` | URL-friendly name |
| `color` | `Option<String>` | Display color |

#### Label
| Field | Type | Description |
|-------|------|-------------|
| `id` | `Option<i32>` | Label ID |
| `name` | `Option<String>` | Label name |
| `slug` | `Option<String>` | URL-friendly name |

#### AudioInfo
| Field | Type | Description |
|-------|------|-------------|
| `bit_depth` | `Option<i32>` | Bit depth |
| `sampling_rate` | `Option<f64>` | Sample rate in kHz |
| `channels` | `Option<i32>` | Channel count |
| `codec` | `Option<String>` | Audio codec |

#### User
| Field | Type | Description |
|-------|------|-------------|
| `id` | `Option<i32>` | User ID |
| `credential` | `Option<Credential>` | User credentials/capabilities |
| `subscription` | `Option<Subscription>` | Subscription details |
| `display_name` | `Option<String>` | Display name |

---

## QobuzApiError (Error Type)

```rust
#[derive(Error, Debug)]
pub enum QobuzApiError {
    #[error("Authentication failed: {message}")]
    AuthenticationError { message: String },
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("API error {code}: {message}")]
    ApiErrorResponse { code: i32, message: String, status: String },
    #[error("Failed to parse API response: {content}")]
    ApiResponseParseError { content: String, source: Option<String> },
    #[error("Service initialization failed: {message}")]
    InitializationError { message: String },
    #[error("Invalid credentials: {message}")]
    CredentialsError { message: String },
    #[error("Download failed: {message}")]
    DownloadError { message: String },
    #[error("Metadata error: {0}")]
    MetadataError(String),
    #[error("{resource_type} not found: {resource_id}")]
    ResourceNotFoundError { resource_type: String, resource_id: String },
    #[error("Rate limited: {message}")]
    RateLimitError { message: String },
    #[error("Invalid parameter: {message}")]
    InvalidParameterError { message: String },
    #[error("Unexpected API response: {message}")]
    UnexpectedApiResponseError { message: String },
}
```
