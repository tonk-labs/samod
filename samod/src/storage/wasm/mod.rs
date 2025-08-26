//! WebAssembly storage implementations
//!
//! This module provides various storage backends for WebAssembly environments:
//! - OPFS (Origin Private File System) for modern browsers
//! - IndexedDB for broader browser compatibility
//! - Node.js filesystem bindings for server-side WASM
//! - WASI filesystem for WASI-compatible runtimes

#[cfg(all(target_arch = "wasm32", feature = "wasm-browser"))]
pub mod opfs;

#[cfg(all(target_arch = "wasm32", feature = "wasm-browser"))]
pub mod indexeddb;

#[cfg(all(target_arch = "wasm32", feature = "wasm-node"))]
pub mod node;

#[cfg(all(target_arch = "wasm32", feature = "wasi"))]
pub mod wasi;

// Re-export storage types based on available features
#[cfg(all(target_arch = "wasm32", feature = "wasm-browser"))]
pub use opfs::OpfsStorage;

#[cfg(all(target_arch = "wasm32", feature = "wasm-browser"))]
pub use indexeddb::IndexedDbStorage;

#[cfg(all(target_arch = "wasm32", feature = "wasm-node"))]
pub use node::NodeFsStorage;

#[cfg(all(target_arch = "wasm32", feature = "wasi"))]
pub use wasi::WasiStorage;
