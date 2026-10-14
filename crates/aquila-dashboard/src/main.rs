#![forbid(unsafe_code)]

//! The Trunk binary entry point for the Aquila dashboard.
//!
//! Trunk builds this binary for `wasm32-unknown-unknown` and runs
//! `wasm-bindgen` over the result. On the host it is an inert stub so
//! `cargo test --workspace` can build the crate without the browser stack
//! (ticket #10 build gate).

#[cfg(target_arch = "wasm32")]
fn main() {
    aquila_dashboard::start();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!(
        "aquila-dashboard: the management UI is a WASM single-page app; \
         build it with `trunk build --release` (ticket #10)."
    );
}
