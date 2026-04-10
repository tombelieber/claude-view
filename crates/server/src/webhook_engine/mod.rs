//! Webhook notification engine.
//!
//! Subscribes to `broadcast::Sender<SessionEvent>`, formats events,
//! and delivers HMAC-signed HTTP POST requests to configured endpoints.

pub mod config;
pub mod debounce;
pub mod delivery;
pub mod formatters;
