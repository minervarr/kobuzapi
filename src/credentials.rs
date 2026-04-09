//! Credential management: `.env` file I/O, web player extraction.

use std::{
    fs::{Permissions, read_to_string, set_permissions, write},
    os::unix::fs::PermissionsExt,
    path::Path,
    string::ToString,
};

use {dotenvy::from_path_iter, regex::Regex, reqwest::Client};

use crate::errors::QobuzApiError::{self, CredentialsError};

/// Reads `QOBUZ_APP_ID` and `QOBUZ_APP_SECRET` from a `.env` file.
///
/// # Arguments
///
/// * `path` - Path to the `.env` file
///
/// # Returns
///
/// `Some((app_id, app_secret))` if both values are found, `None` otherwise.
///
/// # Errors
///
/// Returns `QobuzApiError::CredentialsError` if the file cannot be read or parsed.
pub fn load_app_credentials(path: &Path) -> Result<Option<(String, String)>, QobuzApiError> {
    if !path.exists() {
        return Ok(None);
    }

    let mut app_id = None;
    let mut app_secret = None;

    for item in from_path_iter(path).map_err(|e| CredentialsError {
        message: format!("Failed to read .env file: {e}"),
    })? {
        let (key, value) = item.map_err(|e| CredentialsError {
            message: format!("Failed to parse .env entry: {e}"),
        })?;
        match key.as_str() {
            "QOBUZ_APP_ID" => app_id = Some(value),
            "QOBUZ_APP_SECRET" => app_secret = Some(value),
            _ => {}
        }
    }

    match (app_id, app_secret) {
        (Some(id), Some(secret)) => Ok(Some((id, secret))),
        _ => Ok(None),
    }
}

/// Writes `QOBUZ_APP_ID` and `QOBUZ_APP_SECRET` to a `.env` file with `0600` permissions.
///
/// # Arguments
///
/// * `path` - Path to the `.env` file
/// * `app_id` - Application ID to store
/// * `app_secret` - Application secret to store
///
/// # Returns
///
/// `Ok(())` on success.
///
/// # Errors
///
/// Returns `QobuzApiError::IoError` if the file cannot be written or permissions set.
pub fn save_app_credentials(
    path: &Path,
    app_id: &str,
    app_secret: &str,
) -> Result<(), QobuzApiError> {
    let existing = if path.exists() {
        read_to_string(path)?
    } else {
        String::new()
    };

    let lines: Vec<&str> = existing.lines().collect();
    let mut updated_lines: Vec<String> = lines.iter().map(ToString::to_string).collect();
    let mut id_written = false;
    let mut secret_written = false;

    for line in &mut updated_lines {
        if line.starts_with("QOBUZ_APP_ID=") || line == "# QOBUZ_APP_ID=" {
            *line = format!("QOBUZ_APP_ID={app_id}");
            id_written = true;
        } else if line.starts_with("QOBUZ_APP_SECRET=") || line == "# QOBUZ_APP_SECRET=" {
            *line = format!("QOBUZ_APP_SECRET={app_secret}");
            secret_written = true;
        } else {
            // preserve other lines unchanged
        }
    }

    if !id_written {
        updated_lines.push(format!("QOBUZ_APP_ID={app_id}"));
    }
    if !secret_written {
        updated_lines.push(format!("QOBUZ_APP_SECRET={app_secret}"));
    }

    let content = updated_lines.join("\n");
    write(path, content)?;

    set_file_permissions(path)?;

    Ok(())
}

/// Extracts `app_id` and `app_secret` from the Qobuz web player JavaScript bundle.
///
/// # Returns
///
/// A tuple of `(app_id, app_secret)` extracted from the web player.
///
/// # Errors
///
/// Returns `QobuzApiError::CredentialsError` if extraction fails or `QobuzApiError::HttpError` on
/// network failure.
pub async fn extract_from_web_player() -> Result<(String, String), QobuzApiError> {
    let client = Client::builder().user_agent("Mozilla/5.0").build()?;

    let login_page = client
        .get("https://play.qobuz.com/login")
        .send()
        .await?
        .text()
        .await?;

    let bundle_url = extract_bundle_url(&login_page)?;

    let bundle_js = client.get(bundle_url).send().await?.text().await?;

    extract_app_credentials(&bundle_js)
}

/// Extracts the bundle JavaScript URL from the login page HTML.
///
/// # Arguments
///
/// * `html` - Raw HTML of the Qobuz login page
///
/// # Returns
///
/// The full URL of the bundle JavaScript file.
///
/// # Errors
///
/// Returns `QobuzApiError::CredentialsError` if the bundle URL cannot be found.
fn extract_bundle_url(html: &str) -> Result<String, QobuzApiError> {
    let re = Regex::new(r#"src="(/[^"]*bundle[^"]*\.js)""#).map_err(|e| CredentialsError {
        message: format!("Invalid regex: {e}"),
    })?;

    let caps = re.captures(html).ok_or_else(|| CredentialsError {
        message: "Could not find bundle.js URL in login page".to_string(),
    })?;

    let path = caps.get(1).ok_or_else(|| CredentialsError {
        message: "Could not extract bundle.js URL from capture group".to_string(),
    })?;

    Ok(format!("https://play.qobuz.com{}", path.as_str()))
}

/// Extracts the application ID and secret from the production API config in the bundle JS.
///
/// Matches the `production:{api:{appId:"...",appSecret:"..."}` pattern.
///
/// # Arguments
///
/// * `js` - JavaScript source of the Qobuz web player bundle
///
/// # Returns
///
/// A tuple of `(app_id, app_secret)`.
///
/// # Errors
///
/// Returns `QobuzApiError::CredentialsError` if either value cannot be found.
fn extract_app_credentials(js: &str) -> Result<(String, String), QobuzApiError> {
    let re =
        Regex::new(r#"production:\{api:\{appId:"(\d+)",appSecret:"([^"]+)""#).map_err(|e| {
            CredentialsError {
                message: format!("Invalid regex: {e}"),
            }
        })?;

    let caps = re.captures(js).ok_or_else(|| CredentialsError {
        message: "Could not find production appId/appSecret in bundle JavaScript".to_string(),
    })?;

    let app_id = caps
        .get(1)
        .ok_or_else(|| CredentialsError {
            message: "Could not extract appId from capture group".to_string(),
        })?
        .as_str()
        .to_string();

    let app_secret = caps
        .get(2)
        .ok_or_else(|| CredentialsError {
            message: "Could not extract appSecret from capture group".to_string(),
        })?
        .as_str()
        .to_string();

    Ok((app_id, app_secret))
}

/// Sets file permissions to owner-only read/write (`0600`) on Unix systems.
///
/// # Arguments
///
/// * `path` - Path to the file
///
/// # Returns
///
/// `Ok(())` on success.
///
/// # Errors
///
/// Returns `QobuzApiError::IoError` if permissions cannot be set.
fn set_file_permissions(path: &Path) -> Result<(), QobuzApiError> {
    #[cfg(unix)]
    {
        set_permissions(path, Permissions::from_mode(0o600))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{fs::read_to_string, io::Write, path::Path};

    use {
        anyhow::{Result, anyhow, bail, ensure},
        tempfile::{NamedTempFile, tempdir},
    };

    use crate::credentials::{
        extract_app_credentials, extract_bundle_url, load_app_credentials, save_app_credentials,
    };

    fn create_temp_env(contents: &str) -> Result<NamedTempFile> {
        let mut file = NamedTempFile::new()?;
        write!(file, "{contents}")?;
        Ok(file)
    }

    #[test]
    fn load_app_credentials_reads_existing_file() -> Result<()> {
        let file = create_temp_env("QOBUZ_APP_ID=12345\nQOBUZ_APP_SECRET=secret123\n")?;
        let result = load_app_credentials(file.path())?;
        let (id, secret) = result.ok_or_else(|| anyhow!("expected Some"))?;
        ensure!(id == "12345", "id mismatch");
        ensure!(secret == "secret123", "secret mismatch");
        Ok(())
    }

    #[test]
    fn load_app_credentials_returns_none_for_missing_fields() -> Result<()> {
        let file = create_temp_env("QOBUZ_APP_ID=12345\n")?;
        let result = load_app_credentials(file.path())?;
        ensure!(result.is_none(), "expected None when fields missing");
        Ok(())
    }

    #[test]
    fn load_app_credentials_returns_none_for_nonexistent_file() -> Result<()> {
        let result = load_app_credentials(Path::new("/nonexistent/.env"))?;
        ensure!(result.is_none(), "expected None for nonexistent file");
        Ok(())
    }

    #[test]
    fn save_app_credentials_creates_new_file() -> Result<()> {
        let dir = tempdir()?;
        let path = dir.path().join(".env");

        save_app_credentials(&path, "app123", "secret456")?;

        let content = read_to_string(&path)?;
        ensure!(content.contains("QOBUZ_APP_ID=app123"), "missing app_id");
        ensure!(
            content.contains("QOBUZ_APP_SECRET=secret456"),
            "missing app_secret"
        );
        Ok(())
    }

    #[test]
    fn save_app_credentials_updates_existing_values() -> Result<()> {
        let file = create_temp_env("QOBUZ_APP_ID=old\nQOBUZ_APP_SECRET=oldsecret\n")?;
        save_app_credentials(file.path(), "new", "newsecret")?;

        let content = read_to_string(file.path())?;
        ensure!(
            content.contains("QOBUZ_APP_ID=new"),
            "should contain new id"
        );
        ensure!(
            content.contains("QOBUZ_APP_SECRET=newsecret"),
            "should contain new secret"
        );
        ensure!(!content.contains("old"), "should not contain old values");
        Ok(())
    }

    #[test]
    fn save_app_credentials_replaces_commented_placeholders() -> Result<()> {
        let file = create_temp_env("# QOBUZ_APP_ID=\n# QOBUZ_APP_SECRET=\nQOBUZ_USER_ID=123\n")?;
        save_app_credentials(file.path(), "app_id_val", "secret_val")?;

        let content = read_to_string(file.path())?;
        ensure!(
            content.contains("QOBUZ_APP_ID=app_id_val"),
            "should contain app_id"
        );
        ensure!(
            content.contains("QOBUZ_APP_SECRET=secret_val"),
            "should contain app_secret"
        );
        ensure!(
            !content.contains("# QOBUZ_APP_ID="),
            "should not contain commented placeholder"
        );
        ensure!(
            !content.contains("# QOBUZ_APP_SECRET="),
            "should not contain commented placeholder"
        );
        ensure!(
            content.contains("QOBUZ_USER_ID=123"),
            "should preserve other lines"
        );
        Ok(())
    }

    #[test]
    fn extract_bundle_url_finds_js_url() -> Result<()> {
        let html = r#"<script src="/resources/8.1.0-b019/bundle.js"></script>"#;
        let url = extract_bundle_url(html)?;
        ensure!(
            url == "https://play.qobuz.com/resources/8.1.0-b019/bundle.js",
            "url mismatch: got {url}"
        );
        Ok(())
    }

    #[test]
    fn extract_bundle_url_fails_on_missing_url() -> Result<()> {
        let html = "<html>No scripts here</html>";
        if extract_bundle_url(html).is_ok() {
            bail!("expected error for missing bundle URL");
        }
        Ok(())
    }

    #[test]
    fn extract_app_credentials_finds_production_config() -> Result<()> {
        let js = r#"integration:{api:{appId:"123",appSecret:"abc"}},production:{api:{appId:"798273057",appSecret:"05a4851e74ee47fda346f50cfdfc4f09"},braze:f}"#;
        let (id, secret) = extract_app_credentials(js)?;
        ensure!(id == "798273057", "id mismatch: got {id}");
        ensure!(
            secret == "05a4851e74ee47fda346f50cfdfc4f09",
            "secret mismatch: got {secret}"
        );
        Ok(())
    }

    #[test]
    fn extract_app_credentials_fails_on_missing_config() -> Result<()> {
        let js = r#"integration:{api:{appId:"123",appSecret:"abc"}}"#;
        if extract_app_credentials(js).is_ok() {
            bail!("expected error when production config is missing");
        }
        Ok(())
    }

    #[test]
    fn extract_app_credentials_fails_on_missing_secret() -> Result<()> {
        let js = r#"production:{api:{appId:"798273057"}}"#;
        if extract_app_credentials(js).is_ok() {
            bail!("expected error when appSecret is missing");
        }
        Ok(())
    }
}
