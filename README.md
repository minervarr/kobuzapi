# Qobuz API Rust Client

An unofficial Rust client library for the Qobuz music streaming API, migrated from the C# implementation by [`DJDoubleD`](https://github.com/DJDoubleD). Provides comprehensive access to Qobuz's features including authentication, content search and retrieval (albums, artists, tracks, playlists), user favorites management, streaming URL generation, track/album/artist/playlist downloads with automatic metadata embedding, and web player credential extraction.

## Overview

This project is a migration of an existing C# Qobuz API library to Rust. The original C# project is written in .NET Framework 4.8.1, which is not compatible with Linux, making this Rust implementation particularly valuable for cross-platform use. The goal is to leverage Rust's strengths in memory safety, performance, and concurrency while maintaining the full functionality of the original C# version.

## Features

- **Authentication:**
  - Email/password, user ID/auth token, and automatic environment variable-based authentication
  - Web player credential extraction and caching
  - Automatic credential refresh on signature expiry
- **Content Search & Browse:**
  - Unified catalog search and dedicated search for albums, artists, tracks, playlists, and articles
  - Fetch detailed information for albums, artists, tracks, and playlists
  - Artist release list retrieval
- **User Favorites:**
  - Add, delete, and retrieve favorites
  - Retrieve favorite IDs
- **Downloads:**
  - Download individual tracks
  - Download entire albums, artist discographies, and playlists with concurrent fetching
  - Cancellation support via `AtomicBool`
  - Automatic retry with exponential backoff on rate limiting
  - Track file URL generation for streaming
- **Metadata Embedding:**
  - Comprehensive metadata (artist, album, track details, cover art, performer roles) embedded into downloaded audio files
  - Configurable metadata options
- **Error Handling:**
  - 12 typed error variants via `thiserror` covering auth, HTTP, I/O, API errors, rate limiting, cancellation, and more
  - All errors satisfy `Send + Sync + 'static` for use in async Tokio tasks
- **Testability:**
  - `HttpClient` trait abstraction enables deterministic testing with mock HTTP servers
  - Integration tests with a sequential mock server
  - Benchmark suite with `criterion`

## Architecture

```
src/
├── api/              # API client layer
│   ├── auth.rs       # Authentication methods
│   ├── content/      # Content operations (search, browse, download)
│   ├── favorites.rs  # User favorites management
│   ├── http_client.rs# HTTP client trait + reqwest implementation
│   ├── macros.rs     # delegate! / delegate_with_retry! macros
│   ├── requests.rs   # Signed request primitives, retry with backoff
│   ├── response.rs   # Response parsing, error handling
│   ├── service.rs    # Central QobuzApiService
│   └── service_download.rs # Download delegates
├── credentials/      # .env I/O and web player credential extraction
├── errors.rs         # QobuzApiError enum (12 variants)
├── metadata/         # Audio metadata extraction and embedding
│   ├── config.rs     # Metadata configuration
│   ├── embedder/     # Tag embedding (artist, basic, performer fields)
│   └── extractor.rs  # Metadata extraction from API responses
├── models/           # Serde data models for API responses
├── sanitize.rs       # Cross-platform filename sanitization
└── signing.rs        # MD5-based request signature generation
```

Key design decisions:

- **Synchronous API surface:** The service exposes synchronous methods. Async API functions are called via a per-call Tokio runtime (`Runtime::new().block_on(...)`). This keeps the public API simple while enabling concurrent downloads internally.
- **`HttpClient` trait:** All HTTP requests go through a `dyn HttpClient` trait, enabling mock-based deterministic tests without network I/O.
- **Delegate macros:** `delegate!` and `delegate_with_retry!` generate the sync-to-async bridge, with the latter automatically refreshing web player credentials on `"Invalid Request Signature"` errors.
- **Cancellation:** Long-running downloads accept an `Option<&AtomicBool>` for cooperative cancellation.

## Dependencies

| Crate | Purpose |
|-------|---------|
| [`reqwest`](https://crates.io/crates/reqwest) | Async HTTP client with connection pooling |
| [`serde`](https://crates.io/crates/serde) + [`serde_json`](https://crates.io/crates/serde_json) | JSON serialization/deserialization |
| [`tokio`](https://crates.io/crates/tokio) | Async runtime (fs, sync, time) |
| [`lofty`](https://crates.io/crates/lofty) | Audio metadata reading/writing |
| [`md-5`](https://crates.io/crates/md-5) | MD5 hashing for request signing |
| [`regex`](https://crates.io/crates/regex) | Web player JS bundle parsing |
| [`base64`](https://crates.io/crates/base64) | Decoding during credential extraction |
| [`thiserror`](https://crates.io/crates/thiserror) | Typed error derivation |
| [`tracing`](https://crates.io/crates/tracing) + `tracing-subscriber` | Structured observability |
| [`rayon`](https://crates.io/crates/rayon) | Data parallelism for concurrent downloads |
| [`anyhow`](https://crates.io/crates/anyhow) | Binary-level error context |

Development: [`criterion`](https://crates.io/crates/criterion) (benchmarks), [`tempfile`](https://crates.io/crates/tempfile) (test fixtures).

## Usage

### Instantiation

```rust
use qobuz_api::QobuzApiService;

// Auto-detect credentials (from .env or web player extraction)
let mut service = QobuzApiService::new()?;

// Or provide credentials explicitly
let mut service = QobuzApiService::with_credentials("YOUR_APP_ID", "YOUR_APP_SECRET")?;
```

### Authentication

```rust
// From environment variables (QOBUZ_USER_ID + QOBUZ_USER_AUTH_TOKEN,
// or QOBUZ_EMAIL + QOBUZ_PASSWORD, or QOBUZ_USERNAME + QOBUZ_PASSWORD)
service.authenticate_with_env()?;

// With explicit credentials
service.login("email@example.com", "md5_hashed_password")?;
service.login_with_token("user_id", "auth_token")?;
```

### Search

```rust
let albums = service.search_albums("Miles Davis", Some(10), None)?;
let tracks = service.search_tracks("Kendrick Lamar", Some(10), None)?;
let catalog = service.search_catalog("Pink Floyd", Some(5), None)?;
```

### Browse

```rust
let album = service.get_album("12345", Some("tracks,artist"))?;
let artist = service.get_artist(123, Some("albums"))?;
let track = service.get_track(40128300)?;
let releases = service.get_release_list(123, Some(10), None)?;
```

### Download

Track format IDs: `5` (MP3 320), `6` (FLAC Lossless), `7` (FLAC Hi-Res ≤96 kHz), `27` (FLAC Hi-Res >96 kHz).

```rust
use std::path::Path;

// Download a single track
let path = service.download_track("40128300", 6, Path::new("downloads/Artist/Album"), None)?;

// Download an entire album with concurrent fetching
let paths = service.download_album("12345", 6, Path::new("downloads"), None, Some(4))?;

// Download with cancellation
use std::sync::{Arc, atomic::AtomicBool};
let cancel = Arc::new(AtomicBool::new(false));
let cancel_clone = cancel.clone();
std::thread::spawn(move || {
    std::thread::sleep(std::time::Duration::from_secs(10));
    cancel_clone.store(true, std::sync::atomic::Ordering::Relaxed);
});
let result = service.download_album_cancellable("12345", 6, Path::new("downloads"), None, Some(4), cancel);
```

### Favorites

```rust
service.add_user_favorites(&[40128300], "track")?;
let favorites = service.get_user_favorites("track", Some(50), None)?;
service.delete_user_favorites(&[40128300], "track")?;
```

## Testing

```bash
# Unit tests (no network)
cargo test

# Integration tests with mock HTTP server
cargo test --test integration

# Live integration tests (requires valid .env credentials)
cargo test --features live-tests

# Benchmarks
cargo bench
```

## Acknowledgements

This project is inspired by and built upon the excellent work of [`DJDoubleD`](https://github.com/DJDoubleD). Special thanks to him and his projects:

*   **DJDoubleD/QobuzApiSharp:** The original C# Qobuz API library that served as the foundation and migration source for this Rust client.
    *   [https://github.com/DJDoubleD/QobuzApiSharp](https://github.com/DJDoubleD/QobuzApiSharp)
*   **DJDoubleD/QobuzDownloaderX-MOD:** For insights into Qobuz API interactions and related tooling.
    *   [https://github.com/DJDoubleD/QobuzDownloaderX-MOD](https://github.com/DJDoubleD/QobuzDownloaderX-MOD)

## License

GNU General Public License v3.0 — see [`LICENSE`](LICENSE) for details.
