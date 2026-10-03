//! Plane geometry: paths, boolean operations, strokes and islands.
//!
//! Paths are `kurbo::BezPath` in badge units (the artwork's larger side is 1,
//! centered on the origin) or in the artwork's own 0...1 y-down square, as each
//! stage says. Booleans and islands work on flattened contours; curves survive
//! only as far as the first boolean.

pub mod boolean;
pub mod islands;
pub mod offset;
pub mod path;
pub mod simplify;
pub mod stroke;

