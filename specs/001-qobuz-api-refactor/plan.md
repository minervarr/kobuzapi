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
**Performance Goals**: Single `reqwest::Client` with connection pooling; credential refresh at most once per session; configurable concurrent downloads (default 4); `tokio` for I/O-bound, `rayon` for CPU-bound metadata tagging
**Constraints**: Max 400 lines per file; zero clippy pedantic warnings; no unsafe code; no `unwrap`/`expect`/`panic`; max 3 levels nesting; all public items documented
**Scale/Scope**: ~15-20 source modules; 7 user stories; 22 functional requirements; single-user desktop client

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Zero-Compromise Code Quality | PASS | Plan enforces pedantic clippy (68 deny lints), 400-line limit, no unsafe, no `.ui/.xml/.blp`, `macro_rules!` for dedup, `thiserror`/`anyhow` error handling, full documentation |
| II. Test-First Engineering | PASS | Plan mandates Red-Green-Refactor, unit tests at bottom of files, deterministic mocks, `tempfile` for fixtures, `cargo test` before commit |
| III. Consistent User Experience | PASS | Plan specifies MusicBrainz Picard naming (`Artist/Album/NN. Title.ext`), actionable error messages, uniform quality interface, structured tracing internally, curated user output |
| IV. Performance & Reliability | PASS | Single `reqwest::Client` with connection pooling, credential refresh ≤1 per session, `tokio`/`rayon` split, configurable retries with exponential backoff, `criterion` benchmarks |

**Gate Result**: ALL PASS — proceeding to Phase 0.

### Post-Design Re-Check (after Phase 1)

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Zero-Compromise Code Quality | PASS | Data model uses `Option<T>` fields for safe partial deserialization; error type is flat enum with `thiserror`; module structure groups by capability/domain; no file exceeds 400 lines in design; all public items documented in contracts |
| II. Test-First Engineering | PASS | Contract specifies pre/post-conditions for all public methods; integration test files defined per domain (auth, search, download); deterministic mocking strategy documented in research |
| III. Consistent User Experience | PASS | Quickstart demonstrates MusicBrainz Picard naming; error categories are user-actionable; quality levels present uniform interface; CLI provides REPL for all operations |
| IV. Performance & Reliability | PASS | Single `reqwest::Client` enforced in contract; `credentials_refreshed` flag prevents >1 refresh per session; `tokio::sync::Semaphore` for bounded concurrency; retry with exponential backoff specified |

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
│   ├── search.rs            # SearchResult, ItemSearchResult, UserFavorites
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
│   └── download_tests.rs    # Download integration tests (mocked)
```

**Structure Decision**: Single-project structure grouped by capability/domain. The `api/` module contains the HTTP client layer split by endpoint domain. `models/` contains pure data structures. `metadata/` handles audio file tagging. `cli/` provides the interactive interface. Shared utilities (`signing`, `credentials`, `sanitize`, `errors`) are top-level modules in `src/`. This follows the constitution's "group by capability/domain" rule and avoids the forbidden `models/handlers/utils` anti-pattern.

## Complexity Tracking

No violations to justify — all constitution gates pass.
