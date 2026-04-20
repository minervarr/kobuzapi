# Implementation Plan: Qobuz API Client Refactor

**Branch**: `001-qobuz-api-refactor` | **Date**: 2026-04-06 | **Spec**: `specs/001-qobuz-api-refactor/spec.md`
**Input**: Feature specification from `specs/001-qobuz-api-refactor/spec.md`

## Summary

Refactor the `qobuz-api-rust` library into a clean, high-performance Rust crate and CLI binary. The refactor covers authentication (email/password, token, env vars, auto-refresh from web player JS), search across albums/artists/tracks/playlists, content browsing by ID, track/album downloads with configurable concurrency and automatic credential refresh, metadata embedding via `lofty` (Vorbis Comments for FLAC, ID3v2 for MP3), favorites management, and an interactive CLI. The project enforces pedantic clippy, 400-line file limits, test-first development, and structured tracing throughout.

## Technical Context

**Language/Version**: Rust 2024 edition (latest stable toolchain)
**Primary Dependencies**: `reqwest` (HTTP), `tokio` (async runtime), `serde`/`serde_json` (serialization), `lofty` (audio metadata), `thiserror` (error types), `anyhow` (binary errors), `tracing`/`tracing-subscriber` (observability), `md5` (request signatures), `base64` (app secret decoding), `regex` (web player credential extraction), `dotenvy` (`.env` file loading), `tokio-stream` (chunked downloads), `rayon` (CPU-bound metadata parallelism), `parking-lot` (locks)
**Storage**: `.env` file for credential persistence (with `0600` permissions); filesystem for downloaded audio files
**Testing**: `cargo test` (unit tests at bottom of files), `tempfile` for filesystem fixtures, deterministic mocks for API integration tests, `criterion` for benchmarks
**Target Platform**: Linux (primary), cross-platform compatible
**Project Type**: Library crate + CLI binary
**Performance Goals**: Single `reqwest::Client` with connection pooling (connect timeout: 10s, request timeout: 30s); credential refresh at most once per session; configurable concurrent downloads (default 4); configurable retry limit (default 3) with exponential backoff; `tokio` for I/O-bound, `rayon` for CPU-bound metadata tagging
**Constraints**: Max 400 lines per file; zero clippy pedantic warnings; no unsafe code; no `unwrap`/`expect`/`panic`; max 3 levels nesting; all public items documented
**Scale/Scope**: ~15-20 source modules; 7 user stories; 23 functional requirements; single-user desktop client

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Zero-Compromise Code Quality | PASS | Plan enforces pedantic clippy (68 deny lints), 400-line limit, no unsafe, no `.ui/.xml/.blp`, `macro_rules!` for dedup, `thiserror`/`anyhow` error handling, full documentation |
| II. Test-First Engineering | PASS | Plan mandates Red-Green-Refactor, unit tests at bottom of files, deterministic mocks, `tempfile` for fixtures, `cargo test` before commit |
| III. Consistent User Experience | CONDITIONAL PASS | Plan specifies MusicBrainz Picard naming (`Artist/Album/NN. Title.ext`), actionable error messages, uniform quality interface across album/track downloads and file URL retrieval, structured tracing internally, curated user output. **Note**: Constitution mandates uniform quality interface "across streaming operations" — streaming is explicitly deferred per spec Out of Scope section. The streaming uniformity requirement will be addressed in a future feature. Quality uniformity for this feature covers album downloads, track downloads, and file URL retrieval only. |
| IV. Performance & Reliability | PASS | Single `reqwest::Client` with connection pooling, credential refresh ≤1 per session, `tokio`/`rayon` split, configurable retries with exponential backoff, `criterion` benchmarks |

**Gate Result**: ALL PASS — proceeding to Phase 0.

### Post-Design Re-Check (after Phase 1)

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Zero-Compromise Code Quality | PASS | Data model uses `Option<T>` fields for safe partial deserialization; error type is flat enum with `thiserror`; module structure groups by capability/domain; no file exceeds 400 lines in design; all public items documented in contracts |
| II. Test-First Engineering | PASS | Contract specifies pre/post-conditions for all public methods; integration test files defined per domain (auth, search, download); deterministic mocking strategy documented in research |
| III. Consistent User Experience | CONDITIONAL PASS | Quickstart demonstrates MusicBrainz Picard naming; error categories are user-actionable; quality levels present uniform interface across album/track downloads and file URL retrieval; CLI provides REPL for all operations. **Note**: Streaming uniformity deferred per spec Out of Scope — see initial constitution check above. |
| IV. Performance & Reliability | PASS | Single `reqwest::Client` enforced in contract; `credentials_refreshed` flag prevents >1 refresh per session; `tokio::sync::Semaphore` for bounded concurrency; retry with exponential backoff specified; HTTP range-request resume for partial downloads (FR-023) |

**Post-Design Gate Result**: ALL PASS — design is constitution-compliant.

## Project Structure

### Documentation (this feature)

```text
specs/001-qobuz-api-refactor/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   └── public-api.md    # Public library API contract
└── tasks.md             # Phase 2 output (NOT created by this command)
```

### Source Code (repository root)

```text
src/
├── lib.rs                   # Library root, re-exports
├── main.rs                  # CLI binary entry point
├── api/                     # API client layer
│   ├── mod.rs               # Module declarations
│   ├── service.rs           # QobuzApiService struct, constructors, auth state
│   ├── http_client.rs       # HttpClient trait definition and ReqwestClient implementation (trait abstraction for deterministic testing per research.md section 8)
│   ├── requests.rs          # HTTP primitives (get, post, signed_get, signature generation)
│   ├── auth.rs              # Login, token auth, env auth, credential refresh
│   ├── content/
│   │   ├── mod.rs           # Content submodule declarations
│   │   ├── albums.rs        # Album search, get, download
│   │   ├── artists.rs       # Artist search, get, releases
│   │   ├── tracks.rs        # Track search, get, file URL, download
│   │   ├── playlists.rs     # Playlist search, get
│   │   └── catalog.rs       # Catalog search
│   └── favorites.rs         # Favorites CRUD
├── models/                  # Data structures
│   ├── mod.rs               # Module declarations, re-exports
│   ├── album.rs             # Album, Image, Genre, Label
│   ├── artist.rs            # Artist, Biography
│   ├── track.rs             # Track, AudioInfo
│   ├── playlist.rs          # Playlist
│   ├── search.rs            # SearchResult, ItemSearchResult
│   ├── favorites.rs         # UserFavorites
│   ├── credential.rs        # Credential, Login
│   ├── file_url.rs          # FileUrl
│   └── subscription.rs      # Subscription, User
├── metadata/                # Audio metadata layer
│   ├── mod.rs               # Module declarations, re-exports
│   ├── config.rs            # MetadataConfig
│   ├── extractor.rs         # Metadata extraction from API models
│   └── embedder.rs          # Metadata embedding into audio files (FLAC/MP3)
├── errors.rs                # QobuzApiError enum (thiserror)
├── signing.rs               # Request signature generation (MD5)
├── credentials.rs           # .env I/O, web player credential extraction
├── sanitize.rs              # Filename sanitization, path formatting
└── cli/                     # CLI interface
    ├── mod.rs               # CLI module root
    └── interactive.rs       # Interactive REPL loop

tests/
├── integration/
│   ├── auth_tests.rs        # Authentication integration tests (mocked)
│   ├── search_tests.rs      # Search integration tests (mocked)
│   ├── download_tests.rs    # Download integration tests (mocked)
│   ├── favorites_tests.rs   # Favorites integration tests (mocked)
│   └── metadata_tests.rs    # Metadata integration tests (mocked)
```

**Structure Decision**: Single-project structure grouped by capability/domain. The `api/` module contains the HTTP client layer split by endpoint domain. `models/` contains pure data structures. `metadata/` handles audio file tagging. `cli/` provides the interactive interface. Shared utilities (`signing`, `credentials`, `sanitize`, `errors`) are top-level modules in `src/`. This follows the constitution's "group by capability/domain" rule and avoids the forbidden `models/handlers/utils` anti-pattern.

## CLI Command Grammar

The interactive REPL (US7) accepts the following commands:

**Search**: `search <query> [limit <N>]`
- Searches all content types (albums, artists, tracks, playlists)
- Results displayed as numbered lists grouped by type
- `limit` defaults to 10 if omitted

**Browse**: `browse <type> <id>`
- `<type>`: `album`, `artist`, `track`, `playlist`
- Displays full metadata for the selected item

**Download**: `download <type> <id> [quality <level>] [output <path>]`
- `<type>`: `track` or `album`
- `<level>`: `mp3` (5), `flac` (6), `hires96` (7), `hires192` (27); defaults to `flac`
- `output` defaults to current directory
- Shows progress indication and completion confirmation

**Favorites**: `fav <action> <type> <id...>`
- `<action>`: `add`, `remove`, `list`, `ids`
- `<type>`: `album`, `artist`, `track`
- Multiple IDs accepted for `add`/`remove`
- `list` and `ids` ignore `<id>` and return all favorites

**Quit**: `quit` or `exit`
- Exits the REPL

**Quality Level Mapping**:

| Name | Format ID | Description |
|------|-----------|-------------|
| `mp3` | 5 | MP3 320kbps |
| `flac` | 6 | FLAC 16-bit/44.1kHz |
| `hires96` | 7 | FLAC 24-bit/96kHz |
| `hires192` | 27 | FLAC 24-bit/192kHz |

## Complexity Tracking

No violations to justify — all constitution gates pass.
