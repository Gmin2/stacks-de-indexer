//! HTTP event listener, GraphQL API, and Prometheus metrics.
//!
//! This crate runs two HTTP servers:
//!
//! 1. **Event listener** — receives `POST /new_block`, `/new_burn_block`,
//!    `/new_mempool_tx`, `/drop_mempool_tx` from a stacks-core node configured
//!    with `[[events_observer]]`.
//!
//! 2. **API server** — serves `GET /health`, `GET /metrics` (Prometheus),
//!    and `POST /graphql` (auto-generated schema from config).

pub mod graphql;
pub mod http;
pub mod metrics;

pub use http::run;
