//! Shared mock HTTP client for integration tests.

use std::{future::Future, pin::Pin};

use qobuz_api_rust_refactor::{api::http_client::HttpClient, errors::QobuzApiError};

use reqwest::Response;

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub struct MockHttpClient;

impl HttpClient for MockHttpClient {
    fn get(
        &self,
        _url: &str,
        _params: &[(&str, &str)],
    ) -> BoxFuture<'_, Result<Response, QobuzApiError>> {
        unimplemented!("mock not yet configured")
    }

    fn post_form(
        &self,
        _url: &str,
        _params: &[(&str, &str)],
    ) -> BoxFuture<'_, Result<Response, QobuzApiError>> {
        unimplemented!("mock not yet configured")
    }

    fn get_with_auth(
        &self,
        _url: &str,
        _token: &str,
        _range: Option<&str>,
    ) -> BoxFuture<'_, Result<Response, QobuzApiError>> {
        unimplemented!("mock not yet configured")
    }
}

#[cfg(test)]
mod tests {
    use super::{HttpClient, MockHttpClient};

    #[test]
    fn mock_http_client_implements_trait() {
        fn assert_impl<T: HttpClient>() {}
        assert_impl::<MockHttpClient>();
    }
}
