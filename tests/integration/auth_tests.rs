//! Integration tests for authentication against the live Qobuz API.
//!
//! These tests read real credentials from a `.env` file and verify that
//! authentication actually works against the Qobuz API.
//!
//! **Tests FAIL if credentials are missing or wrong.** There is no silent skip.
//!
//! Setup: copy `.env.example` to `.env` and fill in your credentials, then:
//!
//! `cargo test --test auth-integration`
//!
//! In CI without credentials, run `cargo test` (without `--test auth-integration`) to run only
//! unit tests and the mock integration.

use std::{env::var, path::Path};

use qobuz_api_rust_refactor::api::service::QobuzApiService;

use anyhow::{Result, anyhow, bail};

struct UserCredentials {
    email: Option<String>,
    password: Option<String>,
    user_id: Option<String>,
    user_auth_token: Option<String>,
}

/// Loads the `.env` file and returns user credentials.
///
/// App credentials (`QOBUZ_APP_ID`/`QOBUZ_APP_SECRET`) are optional —
/// `QobuzApiService::new()` auto-extracts them from the Qobuz web player.
///
/// # Errors
///
/// Returns an error if `.env` is missing, cannot be parsed, or has no user credentials at all.
fn require_user_credentials() -> Result<UserCredentials> {
    let env_path = Path::new(".env");

    if !env_path.exists() {
        bail!(
            "No .env file found. Copy .env.example to .env and fill in your Qobuz \
             credentials.\nSet either QOBUZ_EMAIL + QOBUZ_PASSWORD or QOBUZ_USER_ID + \
             QOBUZ_USER_AUTH_TOKEN."
        );
    }

    dotenvy::from_path(env_path).map_err(|e| anyhow!("Failed to parse .env: {e}"))?;

    let email = var("QOBUZ_EMAIL").or_else(|_| var("QOBUZ_USERNAME")).ok();
    let password = var("QOBUZ_PASSWORD").ok();
    let user_id = var("QOBUZ_USER_ID").ok();
    let user_auth_token = var("QOBUZ_USER_AUTH_TOKEN").ok();

    let has_email_auth = email.is_some() && password.is_some();
    let has_token_auth = user_id.is_some() && user_auth_token.is_some();

    if !has_email_auth && !has_token_auth {
        bail!(
            "No user credentials in .env. Provide one of:\n- QOBUZ_EMAIL (or QOBUZ_USERNAME) + \
             QOBUZ_PASSWORD\n- QOBUZ_USER_ID + QOBUZ_USER_AUTH_TOKEN"
        );
    }

    if email.is_some() && password.is_none() {
        bail!("QOBUZ_EMAIL is set but QOBUZ_PASSWORD is missing in .env");
    }

    Ok(UserCredentials {
        email,
        password,
        user_id,
        user_auth_token,
    })
}

/// Creates a `QobuzApiService` using `new()`, which auto-extracts app credentials from the web
/// player (or reads them from `.env` if present).
///
/// # Errors
///
/// Returns an error if app credential extraction fails and no `.env` app credentials exist.
fn create_service() -> Result<QobuzApiService> {
    QobuzApiService::new().map_err(|e| anyhow!("Failed to create service: {e}"))
}

#[cfg(test)]
mod tests {
    use qobuz_api_rust_refactor::errors::QobuzApiError::{ApiErrorResponse, AuthenticationError};

    use {
        anyhow::{Result, anyhow, bail, ensure},
        tracing::info,
    };

    use super::{create_service, require_user_credentials};

    /// Verifies that email/password login succeeds with correct credentials.
    ///
    /// # Errors
    ///
    /// Returns an error if `.env` is missing, email/password are not set, or login fails.
    #[test]
    fn live_email_password_login_succeeds() -> Result<()> {
        let creds = require_user_credentials()?;

        let (Some(email), Some(password)) = (&creds.email, &creds.password) else {
            bail!(
                "This test requires QOBUZ_EMAIL (or QOBUZ_USERNAME) and QOBUZ_PASSWORD in \
                 .env.\nAlternatively, this test is not applicable to token-only auth setups."
            );
        };

        let mut service = create_service()?;

        service.login(email, password)?;

        let token = service.require_auth_token().map_err(|original| {
            anyhow!("Login succeeded but no auth token was stored: {original}")
        })?;
        ensure!(!token.is_empty(), "Auth token should not be empty");

        info!("email/password login succeeded against live API");
        Ok(())
    }

    /// Verifies that wrong email/password credentials are rejected by the API.
    ///
    /// # Errors
    ///
    /// Returns an error if `.env` is missing or if wrong credentials are incorrectly accepted.
    #[test]
    fn live_email_password_wrong_credentials_fail() -> Result<()> {
        require_user_credentials()?;

        let mut service = create_service()?;

        let result = service.login("nonexistent@example.com", "wrongpassword123");

        ensure!(result.is_err(), "Login with wrong credentials should fail");

        let Err(err) = result else {
            bail!("Expected error for wrong credentials");
        };

        ensure!(
            matches!(err, ApiErrorResponse { .. } | AuthenticationError { .. }),
            "Wrong credentials should return ApiErrorResponse or AuthenticationError, got: {err:?}"
        );

        info!("wrong credentials correctly rejected by live API");
        Ok(())
    }

    /// Verifies that token-based login succeeds with correct credentials.
    ///
    /// # Errors
    ///
    /// Returns an error if `.env` is missing, `USER_ID`/`AUTH_TOKEN` are not set, or login fails.
    #[test]
    fn live_token_login_succeeds() -> Result<()> {
        let creds = require_user_credentials()?;

        let (Some(user_id), Some(auth_token)) = (&creds.user_id, &creds.user_auth_token) else {
            bail!(
                "This test requires QOBUZ_USER_ID and QOBUZ_USER_AUTH_TOKEN in \
                 .env.\nAlternatively, this test is not applicable to email/password-only auth \
                 setups."
            );
        };

        let mut service = create_service()?;

        service.login_with_token(user_id, auth_token)?;

        let stored_token = service.require_auth_token().map_err(|original| {
            anyhow!("Token login succeeded but no auth token was stored: {original}")
        })?;
        ensure!(!stored_token.is_empty(), "Auth token should not be empty");

        info!("token login succeeded against live API");
        Ok(())
    }

    /// Verifies that env-based authentication succeeds with credentials from `.env`.
    ///
    /// # Errors
    ///
    /// Returns an error if `.env` is missing or authentication fails.
    #[test]
    fn live_env_auth_succeeds() -> Result<()> {
        require_user_credentials()?;

        let mut service = create_service()?;
        service.authenticate_with_env()?;

        let token = service.require_auth_token().map_err(|original| {
            anyhow!("Env auth succeeded but no auth token was stored: {original}")
        })?;
        ensure!(!token.is_empty(), "Auth token should not be empty");

        info!("env-based authentication succeeded against live API");
        Ok(())
    }

    /// Verifies the full `QobuzApiService::new()` + `authenticate_with_env()` flow.
    ///
    /// # Errors
    ///
    /// Returns an error if `.env` is missing or the full flow fails.
    #[test]
    fn live_service_new_reads_env_and_authenticates() -> Result<()> {
        require_user_credentials()?;

        let mut service = create_service()?;
        service.authenticate_with_env()?;

        let token = service.require_auth_token()?;
        ensure!(!token.is_empty(), "Auth token should not be empty");

        info!("full service::new() + authenticate_with_env() succeeded against live API");
        Ok(())
    }
}
