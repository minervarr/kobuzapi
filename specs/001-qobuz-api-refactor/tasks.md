# Tasks: Qobuz API Client Refactor

**Input**: Design documents from `/specs/001-qobuz-api-refactor/`
**Prerequisites**: plan.md, spec.md, data-model.md, contracts/public-api.md, research.md, quickstart.md

**Tests**: Unit tests at bottom of files per AGENTS.md conventions. Test tasks are included per constitution Principle II (Test-First Engineering: Red-Green-Refactor). Per the constitution, test tasks are listed FIRST in each implementation section — write tests (Red), implement (Green), then refactor.

**Organization**: Tasks grouped by user story to enable independent implementation and testing.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2)
- **Sub-task IDs**: Letter suffixes (e.g., `T002a`, `T043a`) indicate tasks inserted after initial numbering to avoid renumbering cascade. They are full tasks with the same standing as numeric IDs
- Include exact file paths in descriptions

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization, Cargo.toml configuration, and module scaffold

- [x] T001 Configure `Cargo.toml` with all dependencies per plan.md: `reqwest` (stream, json), `tokio` (full), `serde`/`serde_json`, `lofty`, `thiserror`, `anyhow`, `tracing`/`tracing-subscriber`, `md5`, `base64`, `regex`, `dotenvy`, `tokio-stream`, `rayon`, `parking-lot`, `criterion`, `tempfile` in `Cargo.toml`
- [x] T002 Set `edition = "2024"` and configure `lints` section with clippy pedantic warnings as deny in `Cargo.toml`
- [x] T002a Create `clippy.toml` at repository root with `excessive-nesting-threshold = 3` per constitution Performance & Reliability Standards table
- [x] T003 [P] Create `src/lib.rs` with module declarations, placeholder re-exports per contracts/public-api.md, and `#![forbid(unsafe_code)]` attribute at crate root per constitution Principle I
- [x] T004 [P] Create `src/main.rs` with minimal `tokio::main` entry point that initializes tracing subscriber
- [x] T005 [P] Create `src/errors.rs` with `QobuzApiError` enum per data-model.md (all 13 variants with `thiserror` derives, `Send + Sync + 'static`)
- [x] T006 [P] Create `src/signing.rs` with MD5-based request signature generation functions per research.md section 4 (general signed GET and track file URL signatures)
- [x] T007 [P] Create `src/credentials.rs` with `.env` file I/O (`dotenvy`), permission setting (`0o600`), and web player JS credential extraction (`regex` patterns) per research.md section 5
- [x] T008 [P] Create `src/sanitize.rs` with `sanitize_filename()` function for cross-platform filename sanitization per plan.md; unit tests MUST cover: path separators (`/`, `\`), control characters, Unicode, filenames exceeding 255 bytes (truncation), and names that become empty after sanitization

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core models and HTTP primitives that ALL user stories depend on

**CRITICAL**: No user story work can begin until this phase is complete

- [x] T009 Create `src/models/mod.rs` with module declarations and re-exports for all model submodules per plan.md
- [x] T010 [P] Create `src/models/credential.rs` with `Credential` struct (all `Option` fields) per data-model.md
- [x] T011 [P] Create `src/models/file_url.rs` with `FileUrl` struct and quality format ID constants per data-model.md
- [x] T012 [P] Create `src/models/album.rs` with `Album`, `Image`, `Genre`, `Label` structs per data-model.md
- [x] T013 [P] Create `src/models/artist.rs` with `Artist`, `Biography` structs per data-model.md
- [x] T014 [P] Create `src/models/track.rs` with `Track`, `AudioInfo` structs per data-model.md
- [x] T015 [P] Create `src/models/playlist.rs` with `Playlist` struct per data-model.md
- [x] T016 [P] Create `src/models/search.rs` with `SearchResult<T>`, `ItemSearchResult<T>` structs per data-model.md
- [x] T016a [P] Create `src/models/favorites.rs` with `UserFavorites` struct per data-model.md
- [x] T017 [P] Create `src/models/subscription.rs` with `Subscription`, `User` structs per data-model.md
- [x] T018 Create `src/api/mod.rs` with module declarations for `http_client`, `service`, `requests`, `auth`, `content/`, `favorites` per plan.md
- [x] T019 Create `src/api/requests.rs` with HTTP primitives: `get()`, `post()`, `signed_get()` (consumes signature functions from `src/signing.rs`), response parsing, and retry-with-backoff wrapper (configurable retry limit, default 3 retries, exponential backoff) per research.md sections 1 and 6
- [x] T020 Create `src/api/service.rs` with `QobuzApiService` struct definition (fields: `app_id`, `app_secret`, `user_auth_token`, `client`, `credentials_refreshed`), `new()` and `with_credentials()` constructors per data-model.md and contracts/public-api.md; initialize `reqwest::Client` with connection pooling and timeout defaults (connect timeout: 10s, request timeout: 30s); use a plain `bool` for `credentials_refreshed` since the public contract uses `&mut self` for auth methods (exclusive access), not interior mutability
- [x] T074 [P] Create `src/api/http_client.rs` with `HttpClient` trait definition (`get`, `post`, `signed_get`, `bytes_stream`) and `ReqwestClient` implementation wrapping `reqwest::Client` per research.md section 8 and contracts/public-api.md (deterministic testing via trait abstraction)
- [x] T075 [P] Create declarative macros in `src/api/requests.rs` (`search_endpoint!`, `get_endpoint!`) to eliminate duplication across search (T027-T030) and get (T034-T038) method patterns per constitution Principle I
- [x] T076 [P] Create `tests/integration/` directory with `auth_tests.rs`, `search_tests.rs`, `download_tests.rs`, `favorites_tests.rs`, `metadata_tests.rs` scaffolding (empty test modules with `MockHttpClient` stub) per plan.md project structure
- [x] T033 Create `src/api/content/mod.rs` with module declarations for albums, artists, tracks, playlists, catalog per plan.md

**Checkpoint**: Foundation ready — all models, HTTP primitives, test infrastructure, and deduplication macros in place. User story implementation can begin.

---

## Phase 3: User Story 1 - Authenticate with Qobuz (Priority: P1) MVP

**Goal**: Establish authentication with Qobuz via email/password, token, env vars, and auto-refresh from web player JS

**Independent Test**: Provide valid credentials and verify session is established, token is stored, subsequent API calls are authorized

### Implementation for User Story 1

- [x] T077 [US1] Write unit tests in `src/api/auth.rs` (`#[cfg(test)] mod tests`) for: env auth with valid/invalid vars (including `QOBUZ_USERNAME` alias for `QOBUZ_EMAIL`), login success/failure, token auth success/failure, refresh-per-session constraint, credential refresh flow, and web player credential extraction failure (verifying that a clear `CredentialsError` is returned indicating manual configuration is needed) per constitution Principle II
- [x] T021 [US1] Implement `authenticate_with_env()` in `src/api/auth.rs` — reads `QOBUZ_USER_ID`/`QOBUZ_USER_AUTH_TOKEN` or `QOBUZ_EMAIL`/`QOBUZ_PASSWORD`/`QOBUZ_USERNAME` (alias for `QOBUZ_EMAIL`) env vars, delegates to `login()` or `login_with_token()` per contracts/public-api.md
- [x] T022 [US1] Implement `login(email, password)` in `src/api/auth.rs` — MD5-hashes password, POSTs to `/user/login`, stores `user_auth_token` per contracts/public-api.md
- [x] T023 [US1] Implement `login_with_token(user_id, auth_token)` in `src/api/auth.rs` — POSTs to `/user/login` with token credentials, stores `user_auth_token` per contracts/public-api.md
- [x] T024 [US1] Implement `refresh_app_credentials()` in `src/api/auth.rs` — re-extracts from web player JS, writes to `.env`, enforces single-refresh-per-session constraint per contracts/public-api.md
- [x] T025 [US1] Wire auth methods into `QobuzApiService` in `src/api/service.rs` — expose `authenticate_with_env()`, `login()`, `login_with_token()`, `refresh_app_credentials()` as public methods delegating to `src/api/auth.rs`
- [x] T026 [US1] Add structured `tracing` instrumentation to all auth methods in `src/api/auth.rs` per AGENTS.md error handling rules

**Checkpoint**: Authentication fully functional and independently testable. Users can authenticate via any method.

---

## Phase 4: User Story 2 - Search Music Catalog (Priority: P1)

**Goal**: Search albums, artists, tracks, playlists, and catalog by text query with structured results

**Independent Test**: Submit search queries and verify structured results returned for each content type with correct metadata

### Implementation for User Story 2

- [x] T078 [US2] Write unit tests in each content module (`albums.rs`, `artists.rs`, `tracks.rs`, `playlists.rs`, `catalog.rs`) for: search result deserialization, empty result handling, pagination params, and error responses per constitution Principle II
- [x] T027 [P] [US2] Implement `search_albums()` in `src/api/content/albums.rs` — GET `/album/search` with query params, deserialize into `ItemSearchResult<Box<Album>>` per contracts/public-api.md
- [x] T028 [P] [US2] Implement `search_artists()` in `src/api/content/artists.rs` — GET `/artist/search` with query params, deserialize into `ItemSearchResult<Box<Artist>>` per contracts/public-api.md
- [x] T029 [P] [US2] Implement `search_tracks()` in `src/api/content/tracks.rs` — GET `/track/search` with query params, deserialize into `ItemSearchResult<Box<Track>>` per contracts/public-api.md
- [x] T030 [P] [US2] Implement `search_playlists()` in `src/api/content/playlists.rs` — GET `/playlist/search` with query params, deserialize into `ItemSearchResult<Box<Playlist>>` per contracts/public-api.md
- [x] T031 [US2] Implement `search_catalog()` in `src/api/content/catalog.rs` — searches all content types, returns grouped `SearchResult` per contracts/public-api.md
- [x] T032 [US2] Wire search methods into `QobuzApiService` in `src/api/service.rs` — expose all search methods as public API

**Checkpoint**: Search fully functional. Users can search for any content type and receive structured results.

---

## Phase 5: User Story 3 - Browse Content Details (Priority: P2)

**Goal**: Retrieve detailed information for albums, artists, tracks, and playlists by unique ID

**Independent Test**: Request details for known content IDs and verify complete metadata is returned

### Implementation for User Story 3

- [x] T079 [US3] Write unit tests in each content module for: get-by-ID with valid/invalid IDs, `ResourceNotFoundError` on 404, optional `extra` param handling per constitution Principle II
- [x] T034 [P] [US3] Implement `get_album()` in `src/api/content/albums.rs` — GET `/album/get` with optional `extra` param, returns `Album` per contracts/public-api.md
- [x] T035 [P] [US3] Implement `get_artist()` in `src/api/content/artists.rs` — GET `/artist/get` with optional `extra` param, returns `Artist` per contracts/public-api.md
- [x] T036 [P] [US3] Implement `get_track()` in `src/api/content/tracks.rs` — GET `/track/get`, returns `Track` per contracts/public-api.md
- [x] T037 [P] [US3] Implement `get_playlist()` in `src/api/content/playlists.rs` — GET `/playlist/get` with optional `extra` param, returns `Playlist` per contracts/public-api.md
- [x] T038 [P] [US3] Implement `get_release_list()` in `src/api/content/artists.rs` — GET `/artist/getReleasesList`, returns `ItemSearchResult<Box<Album>>` per contracts/public-api.md
- [x] T039 [US3] Wire content browsing methods into `QobuzApiService` in `src/api/service.rs` — expose all get methods as public API
- [x] T040 [US3] Add `ResourceNotFoundError` handling for non-existent content IDs in `src/api/requests.rs` response parsing

**Checkpoint**: Content browsing fully functional. Users can retrieve detailed metadata for any content by ID.

---

## Phase 6: User Story 4 - Download Music (Priority: P2)

**Goal**: Download individual tracks and complete albums at configurable quality levels with concurrent downloads

**Independent Test**: Download a known track or album at a specific quality and verify file is saved correctly

### Implementation for User Story 4

- [x] T080 [US4] Write unit tests in `src/api/content/tracks.rs` and `src/api/content/albums.rs` for: single track download, album download with concurrency, signature error recovery, partial resume, filename formatting, network error context, regional/subscription unavailability error surfacing (verifying `DownloadError` or `ApiErrorResponse` with unavailability message) per constitution Principle II
- [x] T041 [US4] Implement `get_track_file_url()` in `src/api/content/tracks.rs` — GET `/track/getFileUrl` with signed request (track file URL signature per research.md), returns `FileUrl` per contracts/public-api.md
- [x] T042 [US4] Implement streaming download function in `src/api/requests.rs` — `bytes_stream()` with `tokio::io::BufWriter` for efficient disk I/O per research.md section 1
- [x] T043 [US4] Implement `download_track()` in `src/api/content/tracks.rs` — gets file URL, streams to disk, formats filename as `{NN}. {title}.{ext}`, handles signature error recovery with credential refresh per contracts/public-api.md
- [x] T043a [US4] Implement HTTP range-request resume logic in `src/api/requests.rs` — detect partial file on disk, send `Range` header to resume from last byte, verify `Content-Range` response per FR-023 and constitution Principle IV (resumable downloads)
- [x] T044 [US4] Implement `download_album()` in `src/api/content/albums.rs` — fetches album details, creates `{artist}/{album_title}/` directory, downloads all tracks with `tokio::sync::Semaphore` for bounded concurrency (default 4) per contracts/public-api.md
- [x] T045 [US4] Add download progress tracing and error context (track ID, album ID, quality) in `src/api/content/tracks.rs` and `src/api/content/albums.rs`
- [x] T046 [US4] Wire download methods into `QobuzApiService` in `src/api/service.rs` — expose `get_track_file_url()`, `download_track()`, `download_album()` as public API

**Checkpoint**: Downloads fully functional. Users can download tracks and albums at any quality level with concurrent downloads.

---

## Phase 7: User Story 5 - Embed Metadata in Audio Files (Priority: P3)

**Goal**: Embed comprehensive metadata (tags + cover art) into downloaded FLAC and MP3 files using `lofty`

**Independent Test**: Download a track and verify embedded metadata matches expected values

### Implementation for User Story 5

- [ ] T081 [US5] Write unit tests in `src/metadata/extractor.rs` and `src/metadata/embedder.rs` for: comprehensive field extraction, artist deduplication, classical music conductor priority, FLAC/MP3 tag writing, separator formatting, `MetadataConfig` field toggling, special character encoding in metadata values (Unicode, control characters, very long strings) per constitution Principle II
- [ ] T081a [US4] Write unit test in `src/api/content/tracks.rs` for retry policy stacking per FR-018c: mock a download that fails with a signature error (triggering FR-009 credential refresh + single retry), then the retried request returns HTTP 429 (triggering FR-018b rate limit retry), verify both policies apply independently and the download eventually succeeds or surfaces `RateLimitError` after FR-018b retries are exhausted
- [ ] T047 [P] [US5] Create `src/metadata/mod.rs` with module declarations and re-exports per plan.md
- [ ] T048 [P] [US5] Create `src/metadata/config.rs` with `MetadataConfig` struct (all boolean fields, `Default` impl with `comment: false`, rest `true`) per data-model.md
- [ ] T049 [US5] Create `src/metadata/extractor.rs` with `extract_comprehensive_metadata()` — extracts all metadata fields from API models (`Track`, `Album`, `Artist`) into a structured intermediate representation; uses `rayon::par_iter` for batch extraction when processing album tracks per contracts/public-api.md and constitution Principle IV
- [ ] T050 [US5] Implement artist deduplication logic in `src/metadata/extractor.rs` — deduplicates when multiple roles reference the same person per FR-012
- [ ] T051 [US5] Implement classical music metadata handling in `src/metadata/extractor.rs` — prioritizes conductor as album artist, handles orchestra information per FR-022
- [ ] T052 [US5] Create `src/metadata/embedder.rs` with `embed_metadata_in_file()` — writes tags using `lofty`: Vorbis Comments for FLAC, ID3v2 for MP3, cover art via `Picture`, respects `MetadataConfig` field toggles; uses `rayon::par_iter` for batch embedding; formats multi-artist fields with comma separator for FLAC (Vorbis Comments) and slash separator for MP3 (ID3v2) per spec.md US5 acceptance scenario 3; correctly encodes special characters and very long values in metadata fields per spec.md edge cases per research.md section 3 and contracts/public-api.md
- [ ] T053 [US5] Integrate metadata embedding into `download_track()` and `download_album()` in `src/api/content/tracks.rs` and `src/api/content/albums.rs` — call embedder when `config` is provided per contracts/public-api.md
- [ ] T054 [US5] Wire metadata re-exports into `src/lib.rs` per contracts/public-api.md

**Checkpoint**: Metadata embedding fully functional. Downloaded files display complete metadata in music players.

---

## Phase 8: User Story 6 - Manage Favorites (Priority: P3)

**Goal**: Add, remove, and retrieve favorites (albums, artists, tracks)

**Independent Test**: Add a favorite, verify it appears, remove it, confirm removal

### Implementation for User Story 6

- [ ] T082 [US6] Write unit tests in `src/api/favorites.rs` for: add favorite, remove favorite, get favorites list, get favorite IDs, signed request generation per constitution Principle II
- [ ] T055 [US6] Create `src/api/favorites.rs` with `add_user_favorites()` — POST `/favorite/create` with signed request, accepts `item_ids` slice and `item_type` string per contracts/public-api.md
- [ ] T056 [US6] Implement `delete_user_favorites()` in `src/api/favorites.rs` — POST `/favorite/delete` with signed request per contracts/public-api.md
- [ ] T057 [US6] Implement `get_user_favorites()` in `src/api/favorites.rs` — GET `/favorite/getUserFavorites` with signed request, returns `UserFavorites` per contracts/public-api.md
- [ ] T058 [US6] Implement `get_user_favorite_ids()` in `src/api/favorites.rs` — GET `/favorite/getUserFavorites` with IDs-only mode, returns `UserFavorites` with populated `*_ids` fields per contracts/public-api.md
- [ ] T059 [US6] Wire favorites methods into `QobuzApiService` in `src/api/service.rs` — expose all favorites methods as public API
- [ ] T060 [US6] Add structured tracing for favorites operations in `src/api/favorites.rs`

**Checkpoint**: Favorites management fully functional. Users can add, remove, and list favorites.

---

## Phase 9: User Story 7 - Interactive CLI (Priority: P3)

**Goal**: Interactive REPL for searching, browsing, and downloading music without writing code

**Independent Test**: Run CLI, enter search query, select result, view details, initiate download

### Implementation for User Story 7

- [ ] T083 [US7] Write unit tests in `src/cli/interactive.rs` for: command parsing, search output formatting, download handler with quality selection, favorites commands, error output curation (no raw error dumps) per constitution Principle II and Principle III
- [ ] T061 [US7] Create `src/cli/mod.rs` with module declarations per plan.md
- [ ] T062 [US7] Create `src/cli/interactive.rs` with REPL loop — reads from `std::io::stdin`, parses commands (search, browse, download, favorites, quit), dispatches to `QobuzApiService`; all user-facing error output must use curated messages (no raw error dumps), with structured `tracing` internally per constitution Principle III per research.md section 9
- [ ] T063 [US7] Implement search command handler in `src/cli/interactive.rs` — displays numbered results with metadata (title, artist, album) per spec.md acceptance scenario 1
- [ ] T064 [US7] Implement browse/detail command handler in `src/cli/interactive.rs` — shows detailed info for selected item per spec.md acceptance scenario 2
- [ ] T065 [US7] Implement download command handler in `src/cli/interactive.rs` — initiates download with quality selection, progress indication, and completion confirmation per spec.md acceptance scenario 3
- [ ] T066 [US7] Implement favorites command handler in `src/cli/interactive.rs` — add/remove/list favorites from REPL
- [ ] T067 [US7] Wire CLI entry point in `src/main.rs` — initialize service, authenticate, launch REPL loop

**Checkpoint**: CLI fully functional. Users can interactively search, browse, download, and manage favorites.

---

## Phase 10: Polish & Cross-Cutting Concerns

**Purpose**: Final validation, cleanup, and documentation

- [ ] T068 Update `src/lib.rs` re-exports to match final public API surface per contracts/public-api.md
- [ ] T069 [P] Run `cargo clippy --fix --allow-dirty --all-targets -- -W clippy::pedantic` and resolve all warnings
- [ ] T070 [P] Run `cargo fmt` and verify formatting
- [ ] T071 Verify all files are under 400-line limit per plan.md constraint
- [ ] T072 Validate quickstart.md examples compile and run correctly against the implemented library
- [ ] T073 Run `cargo test` and ensure all unit tests pass
- [ ] T073a Run `cargo bench` and verify no benchmark regression exceeds 10% in throughput or allocation count for hot paths per constitution Principle IV and Performance & Reliability Standards table
- [ ] T084 [P] Create `benches/` directory with `criterion` benchmarks for hot paths per research.md section 10: album download pipeline, metadata embedding (FLAC + MP3), search result deserialization, request signature generation (MD5), authentication handshake timing (validate SC-001: auth within 5s on ≥10 Mbps/≤100ms latency). Add `[[bench]]` entries in `Cargo.toml` per constitution Principle IV
- [ ] T085 [P] Audit all public items for `///` documentation (including `# Arguments` and `# Returns` where applicable) and all modules for `//!` module-level docs per constitution Principle I

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately
- **Foundational (Phase 2)**: Depends on Phase 1 completion — BLOCKS all user stories
- **User Stories (Phase 3-9)**: All depend on Phase 2 completion
  - US1 (Phase 3): No dependencies on other stories — MVP
  - US2 (Phase 4): Depends on US1 (requires authenticated session for API calls)
  - US3 (Phase 5): Depends on US1 (requires authenticated session; models from Phase 2)
  - US4 (Phase 6): Depends on US1 + US3 (needs track details, file URLs)
  - US5 (Phase 7): Depends on US4 (metadata embedding integrated into download)
  - US6 (Phase 8): Depends on US1 (requires authenticated session, signed requests)
  - US7 (Phase 9): Depends on all prior stories (US2, US3, US4, US5, US6) — CLI wraps library
- **Polish (Phase 10)**: Depends on all user stories complete

### User Story Dependencies

```
US1 (Auth)
├── US2 (Search) → US3 (Browse) → US4 (Download) → US5 (Metadata)
├── US6 (Favorites)
└── US7 (CLI) — depends on US2, US3, US4, US5, US6
```

### Within Each User Story

- Write tests FIRST (Red-Green-Refactor per constitution Principle II)
- Models before services (where applicable)
- Core implementation until tests pass (Green), then refactor
- Wire into `QobuzApiService` last (expose public API)
- Add tracing after core logic is working

### Parallel Opportunities

- All Phase 1 tasks marked [P] can run in parallel (T003-T008)
- All model files in Phase 2 (T010-T017) can run in parallel
- Within US2: search methods for each content type (T027-T030) can run in parallel
- Within US3: all get methods (T034-T038) can run in parallel
- US6 (Favorites) can run in parallel with US3/US4/US5 (independent of search/browse/download); however, the sequential phase numbering reflects a deliberate implementation order — developers may reorder phases 5–8 as needed provided US1 completes first

---

## Parallel Example: Phase 1

```
T003: src/lib.rs          ┐
T004: src/main.rs         │
T005: src/errors.rs       ├─ All in parallel (different files)
T006: src/signing.rs      │
T007: src/credentials.rs  │
T008: src/sanitize.rs     ┘
```

## Parallel Example: Phase 2 Models

```
T010: src/models/credential.rs    ┐
T011: src/models/file_url.rs      │
T012: src/models/album.rs         │
T013: src/models/artist.rs        ├─ All in parallel (different files)
T014: src/models/track.rs         │
T015: src/models/playlist.rs      │
T016: src/models/search.rs        │
T017: src/models/subscription.rs  ┘
```

## Parallel Example: User Story 2 (Search)

```
T027: search_albums    ┐
T028: search_artists   ├─ All in parallel (different files, independent endpoints)
T029: search_tracks    │
T030: search_playlists ┘
→ Then T031 (catalog aggregates above), T032 (wire into service)
```

## Parallel Example: User Story 3 (Browse)

```
T034: get_album         ┐
T035: get_artist        ├─ All in parallel (different files, independent endpoints)
T036: get_track         │
T037: get_playlist      │
T038: get_release_list  ┘
→ Then T039 (wire into service), T040 (error handling)
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (models + HTTP primitives)
3. Complete Phase 3: User Story 1 (Authentication)
4. **STOP and VALIDATE**: Test authentication independently
5. Can demo login/token/env auth and credential refresh

### Incremental Delivery

1. Setup + Foundational → Foundation ready
2. Add US1 (Auth) → Test → MVP deployed
3. Add US2 (Search) → Test → Users can search music
4. Add US3 (Browse) → Test → Users can view details
5. Add US4 (Download) → Test → Core value delivered
6. Add US5 (Metadata) → Test → Files tagged correctly
7. Add US6 (Favorites) → Test → Full feature set
8. Add US7 (CLI) → Test → Interactive interface
9. Polish → Production ready

---

## Notes

- [P] tasks = different files, no dependencies on incomplete work
- [Story] label maps task to specific user story for traceability
- Each user story is independently completable and testable
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- All public items must have `///` documentation per AGENTS.md
- All errors use structured `tracing` with fields per AGENTS.md
- Max 400 lines per file, zero clippy pedantic warnings, no unsafe code
- Test tasks (T077-T083) follow constitution Principle II (Test-First Engineering): listed FIRST in each implementation section to enforce Red-Green-Refactor — write tests, implement until green, then refactor
- Benchmark tasks (T084) cover hot paths identified in research.md section 10 per constitution Principle IV
- `HttpClient` trait (T074) enables deterministic testing per research.md section 8 recommendation
