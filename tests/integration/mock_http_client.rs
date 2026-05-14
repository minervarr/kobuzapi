//! Shared mock HTTP client for integration tests.

use std::{future::Future, pin::Pin};

use reqwest::Response;

use qobuz_api::{api::http_client::HttpClient, errors::QobuzApiError};

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
