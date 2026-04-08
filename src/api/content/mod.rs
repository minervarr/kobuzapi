//! Content API operations: search and browse for albums, artists, tracks, playlists.

pub mod albums;
pub mod artists;
pub mod catalog;
pub mod playlists;
pub mod tracks;

use serde::de::DeserializeOwned;

use crate::{
    api::{
        requests::{self, RequestAuth},
        service::QobuzApiService,
    },
    errors::QobuzApiError,
};

/// Retrieves the auth token from the service and sends a signed GET request.
///
/// # Arguments
///
/// * `service` - Authenticated API service
/// * `endpoint` - API endpoint path
/// * `params` - Key-value parameter pairs (will be sorted for signing)
///
/// # Returns
///
/// The deserialized response of type `T`.
///
/// # Errors
///
/// Returns a `QobuzApiError` if not authenticated or the API request fails.
async fn do_signed_get<T: DeserializeOwned>(
    service: &QobuzApiService,
    endpoint: &str,
    params: &mut Vec<(String, String)>,
) -> Result<T, QobuzApiError> {
    let token = service.require_auth_token()?;

    requests::signed_get(
        service.http_client(),
        service.base_url(),
        endpoint,
        params,
        &RequestAuth {
            app_id: &service.app_id,
            app_secret: service.app_secret(),
            user_auth_token: token,
        },
    )
    .await
}

/// Sends a search request with query, limit, and offset parameters.
///
/// # Arguments
///
/// * `service` - Authenticated API service
/// * `endpoint` - API endpoint path (e.g., `"/album/search"`)
/// * `query` - Search query string
/// * `limit` - Maximum number of results to return
/// * `offset` - Pagination offset
///
/// # Returns
///
/// The deserialized response of type `T`.
///
/// # Errors
///
/// Returns a `QobuzApiError` if not authenticated or the API request fails.
pub async fn search<T: DeserializeOwned>(
    service: &QobuzApiService,
    endpoint: &str,
    query: &str,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<T, QobuzApiError> {
    let mut params = vec![("query".to_string(), query.to_string())];
    requests::push_pagination_params(&mut params, limit, offset);

    do_signed_get(service, endpoint, &mut params).await
}

/// Sends a get-by-ID request with an optional extra parameter.
///
/// # Arguments
///
/// * `service` - Authenticated API service
/// * `endpoint` - API endpoint path (e.g., `"/album/get"`)
/// * `id_field` - Parameter name for the ID (e.g., `"album_id"`)
/// * `id` - Resource identifier
/// * `extra` - Optional extra fields to include
///
/// # Returns
///
/// The deserialized response of type `T`.
///
/// # Errors
///
/// Returns a `QobuzApiError` if not authenticated or the API request fails.
pub async fn get_by_id<T: DeserializeOwned, I: ToString>(
    service: &QobuzApiService,
    endpoint: &str,
    id_field: &str,
    id: I,
    extra: Option<&str>,
) -> Result<T, QobuzApiError> {
    let mut params = vec![(id_field.to_string(), id.to_string())];
    if let Some(e) = extra {
        params.push(("extra".to_string(), e.to_string()));
    }

    do_signed_get(service, endpoint, &mut params).await
}

/// Sends a paginated request by ID with limit and offset parameters.
///
/// # Arguments
///
/// * `service` - Authenticated API service
/// * `endpoint` - API endpoint path (e.g., `"/artist/getReleasesList"`)
/// * `id_field` - Parameter name for the ID (e.g., `"artist_id"`)
/// * `id` - Resource identifier
/// * `limit` - Maximum number of results to return
/// * `offset` - Pagination offset
///
/// # Returns
///
/// The deserialized response of type `T`.
///
/// # Errors
///
/// Returns a `QobuzApiError` if not authenticated or the API request fails.
pub async fn paginated<T: DeserializeOwned, I: ToString>(
    service: &QobuzApiService,
    endpoint: &str,
    id_field: &str,
    id: I,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<T, QobuzApiError> {
    let mut params = vec![(id_field.to_string(), id.to_string())];
    requests::push_pagination_params(&mut params, limit, offset);

    do_signed_get(service, endpoint, &mut params).await
}
