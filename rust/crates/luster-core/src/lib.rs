//! The Luster engine: SVG artwork in, badge meshes out.
//!
//! Everything here is plain Rust. The bindings for Swift, Kotlin and Dart live
//! in their own crates and only translate types.

pub mod cancel;
pub mod badge;
pub mod export;
pub mod mints;
pub mod model;
pub mod style;
pub mod texture;

// The stages a badge is made in. Their insides are the engine's own; the CLI
// reaches the ones it dumps through `inspect`.
pub(crate) mod cull;
pub(crate) mod design;
pub(crate) mod geom;
pub(crate) mod math;
pub(crate) mod mesh;
pub(crate) mod mint;
pub(crate) mod paint;
pub(crate) mod raster;
pub(crate) mod svg;
pub(crate) mod work;

/// How a vertex is laid out in a submesh's buffer.
pub mod vertex {
    pub use crate::mesh::{NORMAL_OFFSET, POSITION_OFFSET, TANGENT_OFFSET, UV_OFFSET, VERTEX_STRIDE};
}

/// What the engine makes of a document on the way to a badge, for the CLI's
/// dumps and parity checks. Not a stable API.
#[doc(hidden)]
pub mod inspect {
    pub use crate::design::{area, artwork, placing};
    pub use crate::geom::path::{Contours, contours};
    pub use crate::mint::strike;
    pub use crate::svg::cells::cells;
    pub use crate::svg::composite::sampling;
    pub use crate::svg::read::read;
}

pub use cancel::CancelToken;
/// The geometry crate the engine's paths are built with, for callers that
/// measure them.
pub use kurbo;
pub use badge::{Badge, Material, MaterialRole, MintOptions, Rgba, Submesh, mint, mint_with};

/// The engine's version, as the bindings report it.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Why a badge could not be minted.
#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    #[error("the SVG could not be read: {0}")]
    InvalidSvg(String),
    #[error("the SVG is too complex: {0}")]
    InputTooComplex(String),
    #[error("the SVG has nothing to mint")]
    NothingToMint,
    #[error("cancelled")]
    Cancelled,
    /// The engine itself went wrong: a bug, not the document's doing.
    #[error("the engine failed: {0}")]
    Internal(String),
}

#[cfg(test)]
mod tests {
    /// Scripts/release.sh moves the version, so what is checked is its form.
    #[test]
    fn version_is_major_minor_patch() {
        let parts: Vec<&str> = super::version().split('.').collect();
        assert_eq!(parts.len(), 3, "{}", super::version());
        assert!(parts.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())));
    }
}
