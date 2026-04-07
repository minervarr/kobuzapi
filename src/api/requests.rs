//! HTTP request primitives: GET, POST, signed GET, response parsing, retry-with-backoff.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use {
    reqwest::Response,
    serde::de::DeserializeOwned,
    serde_json::{Value, from_str},
    tokio::time::sleep,
};

use crate::{
    api::http_client::HttpClient,
    errors::QobuzApiError::{
        self, ApiErrorResponse, ApiResponseParseError, DownloadError, RateLimitError,
        ResourceNotFoundError,
    },
    signing::sign_request,
};

/// Base URL for all Qobuz API v0.2 endpoints.
const BASE_URL: &str = "https://www.qobuz.com/api.json/0.2";
/// Maximum number of retry attempts on rate limiting.
const MAX_RETRIES: u32 = 3;
/// Base delay in milliseconds for exponential backoff.
const BASE_BACKOFF_MS: u64 = 500;

/// Bundles application credentials and user token for signed API requests.
pub struct RequestAuth<'a> {
    /// Application ID.
    pub app_id: &'a str,
    /// Application secret for request signing.
    pub app_secret: &'a str,
    /// User authentication token.
    pub user_auth_token: &'a str,
}

/// Appends optional `limit` and `offset` pagination parameters to a params vector.
///
/// # Arguments
///
/// * `params` - Parameter vector to modify
/// * `limit` - Maximum number of results (if provided)
/// * `offset` - Pagination offset (if provided)
pub fn push_pagination_params(
    params: &mut Vec<(String, String)>,
    limit: Option<i32>,
    offset: Option<i32>,
) {
    if let Some(l) = limit {
        params.push(("limit".to_string(), l.to_string()));
    }
    if let Some(o) = offset {
        params.push(("offset".to_string(), o.to_string()));
    }
}

/// Appends `app_id`, `request_ts`, and `request_sig` to params for request signing.
///
/// # Arguments
///
/// * `params` - Parameter vector to modify
/// * `method` - HTTP method (e.g., `"GET"`, `"POST"`)
/// * `endpoint` - API endpoint path
/// * `auth` - Application credentials for signing
fn append_signature(
    params: &mut Vec<(String, String)>,
    method: &str,
    endpoint: &str,
    auth: &RequestAuth<'_>,
) {
    params.push(("app_id".to_string(), auth.app_id.to_string()));
    params.push(("request_ts".to_string(), timestamp()));
    let sig = sign_request(method, endpoint, params, auth.app_secret);
    params.push(("request_sig".to_string(), sig));
}

/// Executes a POST request with form parameters and parses the response.
///
/// # Arguments
///
/// * `client` - HTTP client implementation
/// * `endpoint` - API endpoint path
/// * `params` - Key-value form parameters
///
/// # Returns
///
/// Parsed JSON response of type `T`.
///
/// # Errors
///
/// Returns a `QobuzApiError` on HTTP failures or JSON parse errors.
async fn execute_post<T: DeserializeOwned>(
    client: &dyn HttpClient,
    endpoint: &str,
    params: &[(String, String)],
) -> Result<T, QobuzApiError> {
    let url = format!("{BASE_URL}{endpoint}");
    let param_refs: Vec<(&str, &str)> = params
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    let response = client.post_form(&url, &param_refs).await?;

    parse_response::<T>(response, endpoint).await
}

/// Sends a signed GET request and parses the JSON response.
///
/// # Arguments
///
/// * `client` - HTTP client implementation
/// * `endpoint` - API endpoint path (e.g., `"/album/search"`)
/// * `params` - Key-value parameter pairs (will be sorted for signing)
/// * `auth` - Application credentials and user authentication token
///
/// # Returns
///
/// Parsed JSON response of type `T`.
///
/// # Errors
///
/// Returns a `QobuzApiError` on HTTP failures, rate limiting, or JSON parse errors.
pub async fn signed_get<T: DeserializeOwned>(
    client: &dyn HttpClient,
    endpoint: &str,
    params: &mut Vec<(String, String)>,
    auth: &RequestAuth<'_>,
) -> Result<T, QobuzApiError> {
    append_signature(params, "GET", endpoint, auth);

    let url = build_url_with_params(endpoint, params);
    let response = retry_with_backoff(client, &url, auth.user_auth_token).await?;

    parse_response::<T>(response, endpoint).await
}

/// Sends a POST request with form parameters and parses the JSON response.
///
/// # Arguments
///
/// * `client` - HTTP client implementation
/// * `endpoint` - API endpoint path
/// * `params` - Key-value form parameters
/// * `app_id` - Application ID
/// * `user_auth_token` - User authentication token (may be empty for login)
///
/// # Returns
///
/// Parsed JSON response of type `T`.
///
/// # Errors
///
/// Returns a `QobuzApiError` on HTTP failures or JSON parse errors.
pub async fn post<T: DeserializeOwned>(
    client: &dyn HttpClient,
    endpoint: &str,
    params: &mut Vec<(String, String)>,
    app_id: &str,
    user_auth_token: &str,
) -> Result<T, QobuzApiError> {
    params.push(("app_id".to_string(), app_id.to_string()));
    if !user_auth_token.is_empty() {
        params.push(("user_auth_token".to_string(), user_auth_token.to_string()));
    }

    execute_post::<T>(client, endpoint, params).await
}

/// Sends a signed POST request with form parameters and parses the JSON response.
///
/// # Arguments
///
/// * `client` - HTTP client implementation
/// * `endpoint` - API endpoint path
/// * `params` - Key-value form parameters (will be sorted for signing)
/// * `auth` - Application credentials and user authentication token
///
/// # Returns
///
/// Parsed JSON response of type `T`.
///
/// # Errors
///
/// Returns a `QobuzApiError` on HTTP failures, rate limiting, or JSON parse errors.
pub async fn signed_post<T: DeserializeOwned>(
    client: &dyn HttpClient,
    endpoint: &str,
    params: &mut Vec<(String, String)>,
    auth: &RequestAuth<'_>,
) -> Result<T, QobuzApiError> {
    params.push((
        "user_auth_token".to_string(),
        auth.user_auth_token.to_string(),
    ));
    append_signature(params, "POST", endpoint, auth);

    execute_post::<T>(client, endpoint, params).await
}

/// Sends a GET request to an authenticated endpoint for binary download.
///
/// # Arguments
///
/// * `client` - HTTP client implementation
/// * `url` - Full download URL
/// * `token` - User auth token
/// * `range` - Optional Range header value (e.g., `"bytes=1024-"`)
///
/// # Returns
///
/// The raw `reqwest::Response` for streaming.
///
/// # Errors
///
/// Returns a `QobuzApiError` on HTTP failures, 404, or non-success status codes.
pub async fn download_stream(
    client: &dyn HttpClient,
    url: &str,
    token: &str,
    range: Option<&str>,
) -> Result<Response, QobuzApiError> {
    let response = client.get_with_auth(url, token, range).await?;

    let status = response.status();
    if status.is_success() || status.as_u16() == 206 {
        Ok(response)
    } else if status.as_u16() == 404 {
        Err(ResourceNotFoundError {
            resource_type: "file".to_string(),
            resource_id: url.to_string(),
        })
    } else {
        Err(DownloadError {
            message: format!("HTTP {status}"),
        })
    }
}

/// Retries a GET request with exponential backoff on rate limiting (HTTP 429).
///
/// # Arguments
///
/// * `client` - HTTP client implementation
/// * `url` - Full URL with query parameters
/// * `user_auth_token` - User authentication token
///
/// # Returns
///
/// The successful HTTP response.
///
/// # Errors
///
/// Returns a `QobuzApiError::RateLimitError` if all retries are exhausted.
async fn retry_with_backoff(
    client: &dyn HttpClient,
    url: &str,
    user_auth_token: &str,
) -> Result<Response, QobuzApiError> {
    let mut last_error: Option<QobuzApiError> = None;

    for attempt in 0..=MAX_RETRIES {
        let response = client.get_with_auth(url, user_auth_token, None).await?;

        let status = response.status();
        if status.as_u16() == 429 {
            let delay = BASE_BACKOFF_MS * 2u64.pow(attempt);
            sleep(Duration::from_millis(delay)).await;
            last_error = Some(RateLimitError {
                message: format!("Rate limited, retry {attempt}/{MAX_RETRIES}"),
            });
            continue;
        }

        return Ok(response);
    }

    Err(last_error.unwrap_or_else(|| RateLimitError {
        message: "Rate limited: retries exhausted".to_string(),
    }))
}

/// Builds a full URL with query parameters from an endpoint path.
///
/// # Arguments
///
/// * `endpoint` - API endpoint path (e.g., `"/album/search"`)
/// * `params` - Key-value parameter pairs
///
/// # Returns
///
/// A fully formed URL string with encoded query parameters.
fn build_url_with_params(endpoint: &str, params: &[(String, String)]) -> String {
    let mut url = format!("{BASE_URL}{endpoint}");
    if !params.is_empty() {
        let query: String = params
            .iter()
            .map(|(k, v)| format!("{}={}", urlencoding(k), urlencoding(v)))
            .collect::<Vec<_>>()
            .join("&");
        url.push('?');
        url.push_str(&query);
    }
    url
}

/// URL-encodes special characters in a string.
///
/// # Arguments
///
/// * `s` - The string to encode
///
/// # Returns
///
/// A URL-encoded string with spaces as `+` and special chars as `%XX`.
fn urlencoding(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            ' ' => "+".to_string(),
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            _ => format!("%{:02X}", c as u32),
        })
        .collect()
}

/// Parses an API response, handling error status codes and JSON deserialization.
///
/// # Arguments
///
/// * `response` - The HTTP response to parse
/// * `endpoint` - API endpoint path for error context
///
/// # Returns
///
/// The deserialized response body of type `T`.
///
/// # Errors
///
/// Returns a `QobuzApiError` on non-success status codes or JSON deserialization failure.
async fn parse_response<T: DeserializeOwned>(
    response: Response,
    endpoint: &str,
) -> Result<T, QobuzApiError> {
    let status = response.status();
    let body = response.text().await?;

    if status.is_success() {
        return from_str::<T>(&body).map_err(|e| ApiResponseParseError {
            content: truncate(&body, 500),
            source_info: Some(format!("{e}")),
        });
    }

    if let Ok(err) = from_str::<Value>(&body) {
        let code = i32::try_from(err.get("code").and_then(Value::as_i64).unwrap_or_default())
            .unwrap_or_default();
        let message = err
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown error");

        if status.as_u16() == 404 {
            return Err(ResourceNotFoundError {
                resource_type: endpoint.trim_start_matches('/').to_string(),
                resource_id: message.to_string(),
            });
        }

        return Err(ApiErrorResponse {
            code,
            message: message.to_string(),
            status: status.to_string(),
        });
    }

    Err(ApiErrorResponse {
        code: i32::from(status.as_u16()),
        message: truncate(&body, 200),
        status: status.to_string(),
    })
}

/// Generates a Unix timestamp string for request signing.
///
/// # Returns
///
/// The current Unix timestamp as a string.
fn timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    now.as_secs().to_string()
}

/// Truncates a string to `max_len` characters with ellipsis.
///
/// # Arguments
///
/// * `s` - The string to truncate
/// * `max_len` - Maximum character length before truncation
///
/// # Returns
///
/// The original string if short enough, or a truncated version with `...`.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}
