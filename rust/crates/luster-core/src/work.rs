//! How many threads the engine takes.
//!
//! Minting is a background job in every host that runs it, and the host's own
//! thread has a screen to keep up. Rayon would take every core by default;
//! the engine leaves one, once, however many badges are minted.

/// Sizes the thread pool the engine's parallel passes run on. Called at the
/// start of a mint; after the first it costs nothing. WebAssembly has no pool:
/// its passes run on the caller (see `par`).
pub fn ready() {
    #[cfg(not(target_family = "wasm"))]
    {
        static SET: std::sync::Once = std::sync::Once::new();
        SET.call_once(|| {
            let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2);
            let _ = rayon::ThreadPoolBuilder::new()
                .num_threads((cores - 1).max(1))
                .thread_name(|i| format!("luster-{i}"))
                .build_global();
        });
    }
}
