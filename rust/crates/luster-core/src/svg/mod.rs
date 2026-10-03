//! Reading an SVG document the way a badge needs it.
//!
//! usvg does the SVG part: XML, CSS, `use`, units, inherited presentation
//! attributes. What it leaves to us is everything the badge cares about: that
//! regions are unioned rather than merged, that a clip or a mask is a region
//! and never paint, that a stroke is a region too, and that a nearly
//! transparent fill is a glaze which colours what lies beneath instead of
//! owning any area of its own.

pub mod cells;
pub mod composite;
pub mod paint;
pub mod read;

