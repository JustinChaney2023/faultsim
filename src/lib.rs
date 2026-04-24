//! faultsim — discrete-event simulator for failure-detection research.
//!
//! The simulator models a cluster of nodes exchanging heartbeat messages over a
//! configurable network. Pluggable [`detector::FailureDetector`] implementations
//! observe message patterns and make liveness decisions. Scenarios are defined
//! in TOML config files; results are exported as CSV or JSON.
//!
//! # Entry points
//!
//! - [`scenario::load_config`] — parse a scenario TOML file
//! - [`scenario::build_engine`] — wire config into a runnable [`engine::Engine`]
//! - [`engine::Engine::run`] — execute the simulation
//! - [`scenario::print_summary`] — human-readable results to stdout

pub mod aggregate;
pub mod clock;
pub mod config;
pub mod detector;
pub mod engine;
pub mod event;
pub mod metrics;
pub mod network;
pub mod node;
pub mod scenario;
