//! WASM runtime implementation for running in WebAssembly environments

use std::any::Any;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::runtime::{JoinError, RuntimeHandle};

/// A runtime handle for WASM environments
/// 
/// This runtime handle spawns tasks using wasm-bindgen-futures for browser
/// environments. Note that in WASM, all tasks run on the same thread.
#[derive(Clone)]
pub struct WasmRuntime;

impl WasmRuntime {
    /// Create a new WASM runtime handle
    pub fn new() -> Self {
        Self
    }
}

impl Default for WasmRuntime {
    fn default() -> Self {
        Self::new()
    }
}

/// Join handle for WASM spawned tasks
pub struct WasmJoinHandle<O> {
    _phantom: std::marker::PhantomData<O>,
}

// Manually implement Unpin for WasmJoinHandle
impl<O> Unpin for WasmJoinHandle<O> {}

impl<O> Future for WasmJoinHandle<O> {
    type Output = Result<O, WasmJoinError>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        // In WASM, we can't really join spawned tasks
        // This is a limitation of the current wasm-bindgen-futures implementation
        Poll::Pending
    }
}

/// Error type for WASM join operations
#[derive(Debug)]
pub struct WasmJoinError;

impl std::fmt::Display for WasmJoinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WASM join error")
    }
}

impl std::error::Error for WasmJoinError {}

impl JoinError for WasmJoinError {
    fn is_panic(&self) -> bool {
        false
    }

    fn into_panic(self) -> Box<dyn Any + Send + 'static> {
        Box::new("WASM join error")
    }
}

impl RuntimeHandle for WasmRuntime {
    type JoinErr = WasmJoinError;
    type JoinFuture<O: Send + 'static> = WasmJoinHandle<O>;

    fn spawn<O, F>(&self, f: F) -> Self::JoinFuture<O>
    where
        O: Send + 'static,
        F: Future<Output = O> + Send + 'static,
    {
        #[cfg(target_arch = "wasm32")]
        {
            // Spawn the future using wasm-bindgen-futures for all WASM targets
            // Note: We can't get the output value in WASM, so we just spawn and forget
            wasm_bindgen_futures::spawn_local(async move {
                let _ = f.await;
            });
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            // This should not happen as this runtime is only for WASM
            panic!("WasmRuntime used outside of WASM target");
        }

        WasmJoinHandle {
            _phantom: std::marker::PhantomData,
        }
    }
}