//! Auth unit tests with a local mock HTTP server for deterministic testing.

use std::{
    collections::HashMap,
    env::VarError::{self, NotPresent},
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    thread::{sleep, spawn},
    time::Duration,
};

use anyhow::{Result, anyhow, ensure};

use crate::{
    api::{
        auth::{authenticate_with_env_from, login, login_with_token, refresh_app_credentials},
        http_client::ReqwestClient,
        service::QobuzApiService,
    },
    errors::QobuzApiError::{ApiErrorResponse, CredentialsError},
};

struct MockServer {
    addr: SocketAddr,
}

impl MockServer {
    fn start(status: u16, body: &str) -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: \
             {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        spawn_server_loop(listener, response.into_bytes());
        sleep(Duration::from_millis(50));
        Ok(Self { addr })
    }

    fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.addr.port())
    }
}

fn serve_response(mut stream: TcpStream, response: &[u8]) {
    let mut buf = [0u8; 8192];
    drop(stream.read(&mut buf));
    drop(stream.write_all(response));
    drop(stream.flush());
}

fn accept_and_serve(listener: &TcpListener, bytes: &[u8]) {
    if let Ok(s) = listener.accept().map(|(s, _)| s) {
        serve_response(s, bytes);
    }
}

fn serve_loop(listener: &TcpListener, bytes: &[u8]) {
    for _ in 0..4 {
        accept_and_serve(listener, bytes);
    }
}

fn spawn_server_loop(listener: TcpListener, bytes: Vec<u8>) {
    spawn(move || serve_loop(&listener, &bytes));
}

fn mock_env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

fn env_reader(env: &HashMap<String, String>) -> impl Fn(&str) -> Result<String, VarError> + '_ {
    move |key| env.get(key).cloned().ok_or(NotPresent)
}

fn make_test_service(base_url: &str) -> Result<QobuzApiService> {
    let client = ReqwestClient::new()?;
    Ok(QobuzApiService::new_test(client.into_boxed(), base_url))
}

#[test]
fn env_auth_prefers_token_over_email() -> Result<()> {
    let env = mock_env(&[
        ("QOBUZ_USER_ID", "42"),
        ("QOBUZ_USER_AUTH_TOKEN", "tok"),
        ("QOBUZ_EMAIL", "e@x.com"),
        ("QOBUZ_PASSWORD", "p"),
    ]);
    let server = MockServer::start(200, r#"{"user_auth_token":"tok","user":{"id":"42"}}"#)?;
    let mut service = make_test_service(&server.base_url())?;
    authenticate_with_env_from(&mut service, env_reader(&env))?;
    ensure!(service.require_auth_token()? == "tok");
    Ok(())
}

#[test]
fn env_auth_uses_email_with_password() -> Result<()> {
    let env = mock_env(&[("QOBUZ_EMAIL", "e@x.com"), ("QOBUZ_PASSWORD", "secret")]);
    let server = MockServer::start(200, r#"{"user_auth_token":"email-tok","user":{"id":"1"}}"#)?;
    let mut service = make_test_service(&server.base_url())?;
    authenticate_with_env_from(&mut service, env_reader(&env))?;
    ensure!(service.require_auth_token()? == "email-tok");
    Ok(())
}

#[test]
fn env_auth_uses_username_alias() -> Result<()> {
    let env = mock_env(&[("QOBUZ_USERNAME", "e@x.com"), ("QOBUZ_PASSWORD", "secret")]);
    let server = MockServer::start(200, r#"{"user_auth_token":"uname-tok","user":{"id":"1"}}"#)?;
    let mut service = make_test_service(&server.base_url())?;
    authenticate_with_env_from(&mut service, env_reader(&env))?;
    ensure!(service.require_auth_token()? == "uname-tok");
    Ok(())
}

fn expect_auth_error(env: &HashMap<String, String>, expected_substring: &str) -> Result<()> {
    let mut service = QobuzApiService::with_credentials("id", "secret")?;
    let err = authenticate_with_env_from(&mut service, env_reader(env))
        .err()
        .ok_or_else(|| anyhow!("expected error"))?;
    ensure!(err.to_string().contains(expected_substring));
    Ok(())
}

#[test]
fn env_auth_fails_without_vars() -> Result<()> {
    expect_auth_error(&mock_env(&[]), "environment variables found")
}

#[test]
fn env_auth_fails_email_no_password() -> Result<()> {
    expect_auth_error(&mock_env(&[("QOBUZ_EMAIL", "e@x.com")]), "QOBUZ_PASSWORD")
}

#[test]
fn login_success_stores_token() -> Result<()> {
    let server = MockServer::start(200, r#"{"user_auth_token":"login-tok","user":{"id":"1"}}"#)?;
    let mut service = make_test_service(&server.base_url())?;
    login(&mut service, "user@example.com", "password")?;
    ensure!(service.require_auth_token()? == "login-tok");
    Ok(())
}

#[test]
fn login_failure_returns_error() -> Result<()> {
    let server = MockServer::start(401, r#"{"status":"error","code":401,"message":"Invalid"}"#)?;
    let mut service = make_test_service(&server.base_url())?;
    let err = login(&mut service, "bad@x.com", "wrong")
        .err()
        .ok_or_else(|| anyhow!("expected error"))?;
    ensure!(matches!(err, ApiErrorResponse { .. }));
    Ok(())
}

#[test]
fn token_auth_success_stores_token() -> Result<()> {
    let server = MockServer::start(200, r#"{"user_auth_token":"tok","user":{"id":"42"}}"#)?;
    let mut service = make_test_service(&server.base_url())?;
    login_with_token(&mut service, "42", "tok")?;
    ensure!(service.require_auth_token()? == "tok");
    Ok(())
}

#[test]
fn token_auth_failure_returns_error() -> Result<()> {
    let server = MockServer::start(
        401,
        r#"{"status":"error","code":401,"message":"Bad token"}"#,
    )?;
    let mut service = make_test_service(&server.base_url())?;
    ensure!(login_with_token(&mut service, "99", "bad").is_err());
    Ok(())
}

#[test]
fn refresh_rejects_double_refresh() -> Result<()> {
    let mut service = QobuzApiService::with_credentials("id", "secret")?;
    service.mark_credentials_refreshed();
    let err = refresh_app_credentials(&mut service)
        .err()
        .ok_or_else(|| anyhow!("expected error"))?;
    ensure!(matches!(err, CredentialsError { .. }));
    ensure!(err.to_string().contains("once per session"));
    Ok(())
}
