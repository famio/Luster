use std::sync::atomic::{AtomicBool, Ordering};

use crate::Error;

/// Lets the caller stop a mint in flight. The engine checks it between stages
/// and per piece, and returns `Error::Cancelled`.
#[derive(Debug, Default)]
pub struct CancelToken(AtomicBool);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    /// `Err(Cancelled)` once cancelled: for `?` at checkpoints.
    pub fn check(&self) -> Result<(), Error> {
        if self.is_cancelled() { Err(Error::Cancelled) } else { Ok(()) }
    }
}
