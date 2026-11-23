//! RunIt's engine: measure the machine, predict model speed, find and fetch models.
pub mod app;
pub mod bench;
pub mod catalog;
pub mod conditions;
pub mod creature;
pub mod download;
pub mod engine;
pub mod gguf;
pub mod hardware;
pub mod http;
pub mod items;
pub mod predict;
pub mod quality;
pub mod recommend;
pub mod run;
pub mod store;

use std::sync::Arc;

/// Called with progress and findings, from any thread.
pub type Emit = Arc<dyn Fn(serde_json::Value) + Send + Sync>;
