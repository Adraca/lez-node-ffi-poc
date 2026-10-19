#![allow(clippy::missing_safety_doc)]

#[cfg(panic = "abort")]
compile_error!(
    "lez-node-ffi-poc requires panic = \"unwind\" to ensure the FFI catch_unwind boundary protects the foreign host. \
    Compiling with panic = \"abort\" causes foreign process termination on panic."
);

pub mod admin;
pub mod api;
pub mod api_history;
pub mod core;
pub mod history;
pub mod indexer;
pub mod queries;
pub mod storage;

pub use admin::*;
pub use api::*;
pub use api_history::*;
pub use core::*;
pub use indexer::*;
pub use storage::*;
