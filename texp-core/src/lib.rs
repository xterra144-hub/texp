#![allow(unsafe_op_in_unsafe_fn)]
pub mod app;
pub mod art;
pub mod config;
pub mod event;
pub mod fdsearch;
pub mod grep;
pub mod indexer;
pub mod state;
pub mod suggestions;
#[cfg(windows)]
pub mod winapi_calls;
#[cfg(not(windows))]
pub mod linux_calls;
