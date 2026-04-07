//! Authentication methods for the Qobuz API.

use std::{env::var, path::Path};

use {
    md5::{Digest, Md5},
    serde::Deserialize,
    tokio::runtime::Runtime,
    tracing::info,
};

use crate::{
    api::{http_client::HttpClient, requests, service::QobuzApiService},
    credentials::{extract_from_web_player, save_app_credentials},
    errors::QobuzApiError::{self, AuthenticationError, CredentialsError},
    signing::to_hex,
};

/// Response from the Qobuz login endpoint.
#[derive(Deserialize)]
struct LoginResponse {
    /// User authentication token returned on successful login.
    user_auth_token: Option<String>,
}

/// Authenticates using environment variables.
///
/// Reads credentials in priority order:
/// 1. `QOBUZ_USER_ID` + `QOBUZ_USER_AUTH_TOKEN` → token-based auth
/// 2. `QOBUZ_EMAIL` + `QOBUZ_PASSWORD` → email/password auth
/// 3. `QOBUZ_USERNAME` + `QOBUZ_PASSWORD` → username/password auth
///
/// # Arguments
///
/// * `service` - Mutable reference to the API service to authenticate
///
/// # Returns
///
/// `Ok(())` on successful authentication.
///
/// # Errors
///
/// Returns a `QobuzApiError` if no valid credentials are found or the login request fails.
pub fn authenticate_with_env(service: &mut QobuzApiService) -> Result<(), QobuzApiError> {
    if let (Ok(user_id), Ok(token)) = (var("QOBUZ_USER_ID"), var("QOBUZ_USER_AUTH_TOKEN")) {
        info!("Using token-based authentication from environment");

        let rt = Runtime::new()?;
        rt.block_on(login_with_token_inner(
            service.http_client(),
            &service.app_id,
            user_id.trim(),
            token.trim(),
        ))?;

        service.set_auth_token(token.trim().to_string());
        return Ok(());
    }

    let email = var("QOBUZ_EMAIL")
        .or_else(|_| var("QOBUZ_USERNAME"))
        .map_err(|var_error| AuthenticationError {
            message: format!(
                "No QOBUZ_USER_ID/QOBUZ_USER_AUTH_TOKEN or QOBUZ_EMAIL/QOBUZ_PASSWORD environment \
                 variables found: {var_error}"
            ),
        })?;

    let password = var("QOBUZ_PASSWORD").map_err(|var_error| AuthenticationError {
        message: format!("QOBUZ_PASSWORD environment variable not found: {var_error}"),
    })?;

    info!("Using email/password authentication from environment");

    let rt = Runtime::new()?;
    let token = rt.block_on(login_inner(
        service.http_client(),
        &service.app_id,
        email.trim(),
        password.trim(),
    ))?;

    service.set_auth_token(token);
    Ok(())
}

/// Authenticates with email and MD5-hashed password via `POST /user/login`.
///
/// # Arguments
///
/// * `service` - Mutable reference to the API service to authenticate
/// * `email` - User email address
/// * `password` - User password (will be MD5-hashed before sending)
///
/// # Returns
///
/// `Ok(())` on successful authentication.
///
/// # Errors
///
/// Returns a `QobuzApiError` if the login request fails or no token is returned.
pub fn login(
    service: &mut QobuzApiService,
    email: &str,
    password: &str,
) -> Result<(), QobuzApiError> {
    let rt = Runtime::new()?;
    let token = rt.block_on(login_inner(
        service.http_client(),
        &service.app_id,
        email,
        password,
    ))?;

    service.set_auth_token(token);
    Ok(())
}

/// Inner login logic: hashes the password and sends the login POST request.
///
/// # Arguments
///
/// * `client` - HTTP client implementation
/// * `app_id` - Application ID
/// * `email` - User email address
/// * `password` - User password (will be MD5-hashed)
///
/// # Returns
///
/// The user authentication token on successful login.
///
/// # Errors
///
/// Returns a `QobuzApiError` if the HTTP request fails or the response contains no token.
async fn login_inner(
    client: &dyn HttpClient,
    app_id: &str,
    email: &str,
    password: &str,
) -> Result<String, QobuzApiError> {
    let hashed = to_hex(&Md5::digest(password.as_bytes()));

    let mut params = vec![
        ("email".to_string(), email.to_string()),
        ("password".to_string(), hashed),
    ];

    let response: LoginResponse =
        requests::post(client, "/user/login", &mut params, app_id, "").await?;

    response.user_auth_token.ok_or_else(|| AuthenticationError {
        message: "Login succeeded but no user_auth_token in response".to_string(),
    })
}

/// Authenticates with user ID and auth token via `POST /user/login`.
///
/// # Arguments
///
/// * `service` - Mutable reference to the API service to authenticate
/// * `user_id` - Qobuz user ID
/// * `auth_token` - User authentication token
///
/// # Returns
///
/// `Ok(())` on successful authentication.
///
/// # Errors
///
/// Returns a `QobuzApiError` if the login request fails or no token is confirmed.
pub fn login_with_token(
    service: &mut QobuzApiService,
    user_id: &str,
    auth_token: &str,
) -> Result<(), QobuzApiError> {
    let rt = Runtime::new()?;
    rt.block_on(login_with_token_inner(
        service.http_client(),
        &service.app_id,
        user_id,
        auth_token,
    ))?;

    service.set_auth_token(auth_token.to_string());
    Ok(())
}

/// Inner token login logic: sends the login POST request with user ID and token.
///
/// # Arguments
///
/// * `client` - HTTP client implementation
/// * `app_id` - Application ID
/// * `user_id` - Qobuz user ID
/// * `auth_token` - User authentication token
///
/// # Returns
///
/// `Ok(())` on successful token validation.
///
/// # Errors
///
/// Returns a `QobuzApiError` if the HTTP request fails or the response contains no token.
async fn login_with_token_inner(
    client: &dyn HttpClient,
    app_id: &str,
    user_id: &str,
    auth_token: &str,
) -> Result<(), QobuzApiError> {
    let mut params = vec![
        ("user_id".to_string(), user_id.to_string()),
        ("user_auth_token".to_string(), auth_token.to_string()),
    ];

    let response: LoginResponse =
        requests::post(client, "/user/login", &mut params, app_id, auth_token).await?;

    if response.user_auth_token.is_none() {
        return Err(AuthenticationError {
            message: "Token login succeeded but no user_auth_token in response".to_string(),
        });
    }

    Ok(())
}

/// Re-extracts app credentials from the Qobuz web player.
///
/// Can only be called once per session. Returns error on subsequent calls.
///
/// # Arguments
///
/// * `service` - Mutable reference to the API service to refresh
///
/// # Returns
///
/// `Ok(())` on successful credential refresh.
///
/// # Errors
///
/// Returns a `QobuzApiError` if credentials have already been refreshed or extraction fails.
pub fn refresh_app_credentials(service: &mut QobuzApiService) -> Result<(), QobuzApiError> {
    if service.is_credentials_refreshed() {
        return Err(CredentialsError {
            message: "Credentials can only be refreshed once per session".to_string(),
        });
    }

    info!("Refreshing app credentials from web player");

    let rt = Runtime::new()?;
    let (app_id, app_secret) = rt.block_on(extract_from_web_player())?;

    let env_path = Path::new(".env");
    save_app_credentials(env_path, &app_id, &app_secret)?;

    service.app_id = app_id;
    service.app_secret = app_secret;
    service.mark_credentials_refreshed();

    info!("App credentials refreshed successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, ensure};

    use crate::api::service::QobuzApiService;

    #[test]
    fn with_credentials_rejects_empty_app_id() -> Result<()> {
        let result = QobuzApiService::with_credentials("", "secret");
        ensure!(result.is_err(), "should reject empty app_id");
        Ok(())
    }

    #[test]
    fn with_credentials_rejects_empty_app_secret() -> Result<()> {
        let result = QobuzApiService::with_credentials("123", "");
        ensure!(result.is_err(), "should reject empty app_secret");
        Ok(())
    }

    #[test]
    fn require_auth_token_fails_when_not_authenticated() -> Result<()> {
        let service = QobuzApiService::with_credentials("123", "secret")?;
        let result = service.require_auth_token();
        ensure!(result.is_err(), "should fail when not authenticated");
        Ok(())
    }

    #[test]
    fn set_and_require_auth_token_works() -> Result<()> {
        let mut service = QobuzApiService::with_credentials("123", "secret")?;
        service.set_auth_token("my-token".to_string());
        let token = service.require_auth_token()?;
        ensure!(token == "my-token", "token mismatch");
        Ok(())
    }

    #[test]
    fn credentials_refresh_enforced_once_per_session() -> Result<()> {
        let mut service = QobuzApiService::with_credentials("123", "secret")?;
        ensure!(!service.is_credentials_refreshed());
        service.mark_credentials_refreshed();
        ensure!(service.is_credentials_refreshed());
        Ok(())
    }
}
