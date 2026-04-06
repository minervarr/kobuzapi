# Feature Specification: Qobuz API Client Refactor

**Feature Branch**: `001-qobuz-api-refactor`  
**Created**: 2026-04-06  
**Status**: Draft  
**Input**: User description: "Build an application that acts as a refactor of `/home/arch/Downloads/github/qobuz-api-rust`"

## Clarifications

### Session 2026-04-06

- Q: What is the scope boundary for this refactor? → A: In scope: auth + search + download + metadata; out: streaming, social features
- Q: Are Favorites and CLI in scope? → A: Both Favorites and CLI are in scope
- Q: How should credentials be stored? → A: .env file only, with chmod 600 advisory / auto-set permissions
- Q: What is the download concurrency model? → A: Configurable concurrent limit with a sensible default of 4
- Q: How should rate limiting be handled? → A: Automatic retry with exponential backoff (capped at ~3 retries)

## Out of Scope

- Real-time audio streaming / playback
- Social features (sharing, public playlists, recommendations, following)

**Confirmed in scope**: Authentication, search, download, metadata embedding, favorites management, and interactive CLI.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Authenticate with Qobuz (Priority: P1)

A developer using the library needs to authenticate with the Qobuz music streaming service before performing any operations. They provide credentials (email and password, or user ID and auth token) and the library establishes a valid session. The library also supports automatically reading credentials from environment variables or a `.env` file for convenience during development.

**Why this priority**: Authentication is the foundation — every other feature depends on a valid session. Without it, no API calls succeed.

**Independent Test**: Can be fully tested by providing valid credentials and verifying that the session is established, token is stored, and subsequent API calls are authorized. Delivers value as a standalone authentication module.

**Acceptance Scenarios**:

1. **Given** the user provides a valid email and password, **When** authentication is attempted, **Then** the library establishes a session and stores the auth token
2. **Given** the user provides a valid user ID and auth token, **When** token-based authentication is attempted, **Then** the library validates the token and stores it for subsequent requests
3. **Given** environment variables contain valid credentials, **When** automatic authentication is triggered, **Then** the library reads credentials and establishes a session without manual input
4. **Given** invalid credentials are provided, **When** authentication is attempted, **Then** a clear error message indicates the authentication failure reason
5. **Given** the user has an active session, **When** app credentials expire or become invalid, **Then** the library automatically refreshes them from the Qobuz web player without requiring user intervention (see FR-004)

---

### User Story 2 - Search Music Catalog (Priority: P1)

A user wants to find music on Qobuz by searching for albums, artists, tracks, or playlists. They enter a search query and receive structured results matching their query, with relevant metadata for each result (name, artist, album art, duration, etc.).

**Why this priority**: Search is the primary discovery mechanism and the most common entry point for users. Without search, users cannot find content to interact with.

**Independent Test**: Can be fully tested by submitting search queries and verifying structured results are returned for each content type. Delivers value as a standalone search capability.

**Acceptance Scenarios**:

1. **Given** a valid session exists, **When** the user searches for albums by name, **Then** results contain matching albums with title, artist, release date, and cover art URL
2. **Given** a valid session exists, **When** the user searches for artists, **Then** results contain matching artists with name, biography summary, and image
3. **Given** a valid session exists, **When** the user searches for tracks, **Then** results contain matching tracks with title, artist, album, duration, and availability
4. **Given** a valid session exists, **When** the user searches for playlists, **Then** results contain matching playlists with name, description, track count, and creator
5. **Given** a search query returns no results, **When** the response is processed, **Then** an empty result set is returned without errors
6. **Given** the user performs a broad catalog search, **When** results span multiple content types, **Then** results are grouped by type with relevant metadata for each

---

### User Story 3 - Browse Content Details (Priority: P2)

A user wants to view detailed information about a specific album, artist, track, or playlist. They request details by ID and receive comprehensive metadata including track listings, artist credits, release information, genre, label, and related content.

**Why this priority**: Browsing details is essential for users to decide what to download or favorite, but it depends on search (P1) to find content IDs first.

**Independent Test**: Can be fully tested by requesting details for known content IDs and verifying complete metadata is returned. Delivers value as a standalone content browsing capability.

**Acceptance Scenarios**:

1. **Given** a valid album ID, **When** album details are requested, **Then** the response includes title, artist, track list, release date, genre, label, cover art, and audio quality options
2. **Given** a valid artist ID, **When** artist details are requested, **Then** the response includes name, biography, image, and list of releases
3. **Given** a valid track ID, **When** track details are requested, **Then** the response includes title, artist, album, duration, track number, composers, performers, and audio quality
4. **Given** a valid playlist ID, **When** playlist details are requested, **Then** the response includes name, description, track list, creator, and creation date
5. **Given** an invalid or non-existent content ID, **When** details are requested, **Then** a clear error indicates the resource was not found

---

### User Story 4 - Download Music (Priority: P2)

A user wants to download individual tracks or entire albums from Qobuz at a chosen audio quality level (e.g., MP3 320kbps, FLAC, Hi-Res). Downloaded files are saved to a specified location with properly formatted filenames and organized directory structure.

**Why this priority**: Downloading is the primary value proposition for many users. It depends on authentication and content browsing but delivers the core utility.

**Independent Test**: Can be fully tested by downloading a known track or album at a specific quality and verifying the file is saved correctly with the right format and size.

**Acceptance Scenarios**:

1. **Given** a valid track ID and quality level, **When** download is initiated, **Then** the audio file is downloaded and saved to the specified path with a sanitized filename
2. **Given** a valid album ID and quality level, **When** album download is initiated, **Then** all tracks are downloaded sequentially into a directory named after the album and artist
3. **Given** a download fails due to expired credentials, **When** the library detects a signature error, **Then** credentials are automatically refreshed and the download is retried (see FR-009)
4. **Given** a chosen quality level (MP3, FLAC, Hi-Res 24-bit), **When** the file URL is requested, **Then** the correct format URL matching the requested quality is returned
5. **Given** a download is in progress, **When** a network interruption occurs, **Then** the error is reported with context about which track failed and whether retry is possible
6. **Given** a partially downloaded file exists on disk, **When** download is re-initiated for the same track, **Then** the download resumes from the last received byte using HTTP range requests (see FR-023)

---

### User Story 5 - Embed Metadata in Audio Files (Priority: P3)

A user wants downloaded audio files to contain complete, accurate metadata tags (title, artist, album, genre, composer, conductor, cover art, etc.) that display correctly in music players. The library embeds this metadata during the download process.

**Why this priority**: Metadata embedding enhances the download experience but is secondary to the actual download capability. Users can still listen to files without embedded metadata.

**Independent Test**: Can be fully tested by downloading a track and verifying the embedded metadata matches expected values using an external metadata reader.

**Acceptance Scenarios**:

1. **Given** a downloaded FLAC file, **When** metadata is embedded, **Then** Vorbis Comments contain title, artist, album, album artist, genre, date, composer, conductor, performer, track number, disc number, and cover art
2. **Given** a downloaded MP3 file, **When** metadata is embedded, **Then** ID3v2 tags contain the equivalent fields with format-appropriate tag names
3. **Given** a track with multiple artists and performers, **When** metadata is embedded, **Then** artists are deduplicated and formatted correctly (comma-separated for FLAC, slash-separated for MP3)
4. **Given** a classical music track with conductor and orchestra information, **When** metadata is embedded, **Then** the conductor is prioritized as the album artist for FLAC files
5. **Given** cover art is available in multiple resolutions, **When** metadata is embedded, **Then** the highest available resolution artwork is embedded in the audio file
6. **Given** a user wants to control which metadata fields are written, **When** a metadata configuration is provided, **Then** only the specified fields are embedded while others are omitted

---

### User Story 6 - Manage Favorites (Priority: P3)

A user wants to add albums, artists, and tracks to their Qobuz favorites list, remove items from favorites, and retrieve their current favorites collection.

**Why this priority**: Favorites management enhances the user experience but is not core to the download/search workflow. It enriches the library's feature set.

**Independent Test**: Can be fully tested by adding a favorite, verifying it appears in the favorites list, then removing it and confirming removal.

**Acceptance Scenarios**:

1. **Given** a valid session, **When** the user adds an album, artist, or track to favorites, **Then** the item appears in the user's favorites list
2. **Given** a valid session, **When** the user removes an item from favorites, **Then** the item no longer appears in the favorites list
3. **Given** a valid session, **When** the user requests their favorites, **Then** all favorited items are returned with type, ID, and basic metadata
4. **Given** a valid session, **When** the user requests just the favorite IDs, **Then** a list of numeric IDs grouped by content type is returned

---

### User Story 7 - Interactive CLI (Priority: P3)

A developer or end-user wants an interactive command-line interface to search for music, browse results, and download tracks or albums without writing code. The CLI provides a simple text-based loop for entering queries and selecting results.

**Why this priority**: The CLI is a convenience interface on top of the library. It demonstrates the library's capabilities but the library itself (P1-P2 features) is the primary deliverable.

**Independent Test**: Can be fully tested by running the CLI, entering search queries, selecting results, and initiating downloads, verifying the output at each step.

**Acceptance Scenarios**:

1. **Given** the application is launched, **When** the user enters a search query, **Then** matching results are displayed with numbered options
2. **Given** search results are displayed, **When** the user selects an item, **Then** detailed information about the selected item is shown
3. **Given** a selected album or track, **When** the user chooses to download, **Then** the download begins with progress indication and completion confirmation
4. **Given** the REPL is active, **When** the user issues a favorites command (add, remove, list), **Then** the operation completes with confirmation feedback

---

### Edge Cases

- What happens when the Qobuz API returns rate limit errors? The library should automatically retry with exponential backoff, up to a maximum of 3 retries, before surfacing a clear rate limit error (see FR-018b).
- How does the system handle network timeouts or connection failures during downloads? Errors should be reported with context about the operation that failed.
- What happens when the web player credential extraction fails (e.g., Qobuz changes their web player structure)? A clear error should indicate that automatic credential refresh is unavailable and manual configuration is needed.
- How does the system handle tracks or albums that are not available in the user's region or subscription tier? An appropriate error should indicate unavailability.
- What happens when metadata fields contain special characters or very long values? Filenames should be sanitized and metadata should be encoded correctly for each audio format.
- How does the system handle concurrent download requests? Each download should complete independently without interfering with others, bounded by a configurable concurrency limit (default: 4 simultaneous downloads).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The library MUST authenticate users with the Qobuz API using email/password credentials
- **FR-002**: The library MUST authenticate users with the Qobuz API using user ID and auth token
- **FR-003**: The library MUST support automatic authentication from environment variables (`QOBUZ_USER_ID`, `QOBUZ_USER_AUTH_TOKEN`, `QOBUZ_EMAIL`, `QOBUZ_PASSWORD`). `QOBUZ_USERNAME` is treated as an alias for `QOBUZ_EMAIL` for backward compatibility
- **FR-004**: The library MUST automatically extract and refresh application credentials (app ID and app secret) from the Qobuz web player JavaScript bundle
- **FR-005**: The library MUST search for albums, artists, tracks, and playlists by text query, and MUST support a combined catalog search that returns results grouped by content type via `SearchResult`
- **FR-006**: The library MUST retrieve detailed information for albums, artists, tracks, and playlists by their unique identifiers
- **FR-007**: The library MUST download individual tracks at user-selected quality levels (MP3 320kbps, FLAC, Hi-Res 24-bit/96kHz, Hi-Res 24-bit/192kHz)
- **FR-008**: The library MUST download complete albums with all tracks organized into artist/album directory structure, using a configurable concurrency limit (default: 4 simultaneous downloads)
- **FR-009**: The library MUST automatically retry downloads when credential-related signature errors occur, by invoking the credential refresh mechanism defined in FR-004 and retrying
- **FR-010**: The library MUST embed comprehensive metadata into downloaded audio files, including title, artist, album, album artist, genre, date, composer, conductor, performer, track number, disc number, and cover art
- **FR-011**: The library MUST handle format-specific metadata tagging (Vorbis Comments for FLAC, ID3v2 for MP3)
- **FR-012**: The library MUST deduplicate artist and composer names when multiple roles reference the same person
- **FR-013**: The library MUST allow users to configure which metadata fields are embedded via a metadata configuration object
- **FR-014**: The library MUST download and embed the highest available resolution cover art
- **FR-015**: The library MUST add and remove items from the user's Qobuz favorites list
- **FR-016**: The library MUST retrieve the user's favorites list and favorite IDs
- **FR-017**: The library MUST sign API requests with MD5-based request signatures when required
- **FR-018a**: The library MUST provide clear, structured error types for all failure scenarios (authentication, network, API, metadata, download, rate limiting, resource not found)
- **FR-018b**: The library MUST automatically retry rate-limited requests with exponential backoff (max 3 retries)
- **FR-019**: The library MUST sanitize filenames for cross-platform compatibility
- **FR-020**: The project MUST provide an interactive CLI binary for searching, browsing, and downloading music
- **FR-021**: The library MUST support reading and writing application credentials to `.env` files for persistence, and MUST set file permissions to `0600` (owner read/write only) when creating or writing the file
- **FR-022**: The library MUST handle classical music metadata with conductor and orchestra information, prioritizing conductor as album artist
- **FR-023**: The library MUST support resumable partial downloads, allowing interrupted downloads to continue from the last received byte using HTTP range requests

### Key Entities

- **QobuzApiService**: The central service that holds authentication state (app ID, app secret, auth token) and provides all API operations
- **Album**: A music album with title, artist, track list, release date, genre, label, cover art, and audio quality information
- **Artist**: A music artist with name, biography, image, and associated releases
- **Track**: An individual music track with title, artist, album, duration, track number, composers, performers, and audio quality
- **Playlist**: A curated list of tracks with name, description, creator, and track count
- **SearchResult**: A collection of search results grouped by content type (albums, artists, tracks, playlists)
- **FileUrl**: A download URL for a track at a specific quality level
- **MetadataConfig**: Configuration object specifying which metadata fields to embed in audio files
- **UserFavorites**: The user's collection of favorited albums, artists, and tracks
- **Credential**: Authentication credential data including user ID, `user_auth_token`, email, and hashed password
- **QobuzApiError**: Structured error type covering authentication, network, API response, metadata, download, and rate limiting failures

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Users can authenticate with Qobuz and perform API operations within 5 seconds of providing credentials on a connection with ≥10 Mbps downstream and ≤100ms latency to the Qobuz API
- **SC-002**: Search queries return structured results for all supported content types (albums, artists, tracks, playlists)
- **SC-003**: Downloads complete successfully with files saved in the correct format and quality level as selected by the user
- **SC-004**: Downloaded audio files display complete metadata (title, artist, album, cover art, genre, composer, etc.) correctly in any player supporting Vorbis Comments (FLAC) or ID3v2 (MP3)
- **SC-005**: The library recovers automatically from expired credentials without user intervention, completing downloads that would otherwise fail
- **SC-006**: All error scenarios produce clear, actionable error messages that indicate the specific failure reason
- **SC-007**: The codebase passes all lint checks with zero warnings
- **SC-008**: The codebase includes unit tests for core functionality (authentication, search, download, metadata)
- **SC-009**: The codebase follows consistent module organization grouped by capability/domain, with no file exceeding 400 lines

## Assumptions

- Users have a valid Qobuz subscription that permits streaming and downloading at their requested quality levels
- Users have stable internet connectivity for API calls and file downloads
- The Qobuz web player structure remains compatible for automatic credential extraction; if it changes, manual credential configuration serves as fallback
- The `.env` file format is acceptable for local credential persistence during development; the library auto-sets `0600` permissions on the file
- The refactored application targets Linux as the primary platform
- The existing API endpoint structure (`https://www.qobuz.com/api.json/0.2/`) remains stable
- Environment variable names (`QOBUZ_USER_ID`, `QOBUZ_USER_AUTH_TOKEN`, `QOBUZ_EMAIL`, `QOBUZ_PASSWORD`) remain unchanged for backward compatibility; `QOBUZ_USERNAME` is supported as an alias for `QOBUZ_EMAIL`
