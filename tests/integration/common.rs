//! Shared test helpers for integration tests.

use std::{env::var, path::Path};

use {
    anyhow::{Result, anyhow, bail},
    dotenvy::from_path,
};

/// Validates that user credentials exist in the environment.
///
/// Loads the `.env` file and checks for either email/password or user ID/token credentials.
///
/// # Returns
///
/// `Ok(())` if valid credentials are found.
///
/// # Errors
///
/// Returns an error if `.env` is missing, cannot be parsed, or has no user credentials.
pub fn ensure_env_credentials() -> Result<()> {
    let env_path = Path::new(".env");

    if !env_path.exists() {
        bail!(
            "No .env file found. Copy .env.example to .env and fill in your Qobuz \
             credentials.\nSet either QOBUZ_EMAIL + QOBUZ_PASSWORD or QOBUZ_USER_ID + \
             QOBUZ_USER_AUTH_TOKEN."
        );
    }

    from_path(env_path).map_err(|e| anyhow!("Failed to parse .env: {e}"))?;

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

    Ok(())
}
