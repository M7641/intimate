//! Storage backend connectors

#[cfg(feature = "s3")]
pub mod s3;

#[cfg(feature = "local")]
pub mod local;
