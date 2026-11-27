//! The modules behind the `monitor-lizard` CLI, as a library too: a Tauri shell (`src-tauri/`) links
//! against this crate instead of recompiling every module from its own copy of the source via
//! `#[path]` includes, which keeps the two binaries' code from drifting apart over time.
pub mod actions;
pub mod baseline;
pub mod classify;
pub mod collect;
pub mod group;
pub mod guide;
pub mod history;
pub mod iokit;
pub mod llm;
pub mod llm_server;
pub mod model;
pub mod model_store;
pub mod netif;
pub mod notify;
pub mod paths;
pub mod persistence;
pub mod privilege;
pub mod procusage;
pub mod rules;
pub mod sampler;
pub mod server;
pub mod settings;
pub mod snapshot;
pub mod storage;
pub mod trash_route;
pub mod watcher;
