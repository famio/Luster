//! Working a badge's metal out on a raster.
//!
//! The engine decides where metal goes by drawing the art on a scratch raster
//! and measuring it: distances to its edges, the pieces it falls into, the gaps
//! between them. A raster cannot hold anything thinner than a pixel, which is
//! the point: booleans leave slivers and tangential contacts that no extruder
//! can build.

pub mod field;
pub mod sheet;
pub mod trace;

pub use sheet::Sheet;
