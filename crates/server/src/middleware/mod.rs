//! V1-hardening M1.2 — request-scoped middleware.
//!
//! Provides the cross-cutting layers that previously lived either as
//! ad-hoc per-handler code (auth) or not at all (request IDs, timeouts,
//! body limits).
//!
//! See `docs/superpowers/specs/2026-04-06-v1-release-hardening-design.md`.

pub mod request_id;

pub use request_id::{set_request_id, RequestId, SetRequestIdLayer};
