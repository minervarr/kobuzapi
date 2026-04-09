//! API client layer: HTTP primitives, authentication, content operations, and favorites.

pub mod auth;
pub mod content;
pub mod favorites;
pub mod http_client;
pub mod requests;
pub mod service;

#[cfg(test)]
mod test_support;
