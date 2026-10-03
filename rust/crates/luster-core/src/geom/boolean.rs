//! Boolean operations on regions.
//!
//! i_overlay works on integer coordinates internally, so the same operands
//! always give the same result, whatever the platform. Curves are flattened
//! first: every boolean in the engine is between regions that a raster or an
//! extrusion will consume as polygons anyway.

use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::simplify::SimplifyShape;
use i_overlay::float::single::SingleFloatOverlay;
use kurbo::Point;

use super::path::Contours;

/// Which side of a fill counts as inside.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    NonZero,
    EvenOdd,
}

impl From<Rule> for FillRule {
    fn from(rule: Rule) -> Self {
        match rule {
            Rule::NonZero => FillRule::NonZero,
            Rule::EvenOdd => FillRule::EvenOdd,
        }
    }
}

/// Shapes as i_overlay returns them: an outer ring and its holes, per shape.
type Shapes = Vec<Vec<Vec<[f64; 2]>>>;

fn lower(contours: &Contours) -> Vec<Vec<[f64; 2]>> {
    contours.iter().map(|c| c.iter().map(|p| [p.x, p.y]).collect()).collect()
}

fn lift(shapes: Shapes) -> Contours {
    shapes.into_iter().flatten().map(|ring| ring.into_iter().map(|p| Point::new(p[0], p[1])).collect()).collect()
}

/// Resolves a region's own overlaps and windings into disjoint rings: outers
/// wound one way, holes the other.
pub fn simplify(contours: &Contours, rule: Rule) -> Contours {
    lift(lower(contours).simplify_shape(rule.into()))
}

/// A region shrunk by `distance`: every point of it at least that far inside.
/// Where the region is narrower than twice the distance it comes apart or
/// goes, rather than folding over itself. `contours` must be wound as
/// `simplify` leaves them.
pub fn inset(contours: &Contours, distance: f64) -> Contours {
    use i_overlay::mesh::float::outline::offset::OutlineOffset;
    use i_overlay::mesh::float::style::{LineJoin, OutlineStyle};
    // Round where the shrunk outline turns a corner of the region's inside,
    // in steps of at most a tenth of a radian.
    let style = OutlineStyle::new(-distance).line_join(LineJoin::Round(0.1));
    lift(lower(contours).outline(&style))
}

/// A region with every gap in it narrower than twice `gap` closed: grown by
/// `gap`, then shrunk by it again. Its outline is thinned first, by a small
/// fraction of the gap: offsets of an outline in steps far finer than the
/// offset make each offset edge cross dozens of its neighbours, and crawl.
pub fn close(contours: &Contours, gap: f64) -> Contours {
    let thinned: Contours = contours
        .iter()
        .map(|ring| crate::geom::simplify::closed(ring, gap / 20.0))
        .filter(|ring| ring.len() >= 3)
        .collect();
    inset(&inset(&thinned, -gap), gap)
}

/// A region with every part of it narrower than twice `width` taken away:
/// shrunk by `width`, then grown by it again. Thinned first, as `close` is,
/// and thinned again once shrunk, more finely: the shrink rounds the
/// region's inside corners in steps far finer than `width`, and growing
/// those back crawls as offsetting the unthinned outline would.
pub fn open(contours: &Contours, width: f64) -> Contours {
    let thinned: Contours = contours
        .iter()
        .map(|ring| crate::geom::simplify::closed(ring, width / 20.0))
        .filter(|ring| ring.len() >= 3)
        .collect();
    let shrunk: Contours = inset(&thinned, width)
        .iter()
        .map(|ring| crate::geom::simplify::closed(ring, width / 100.0))
        .filter(|ring| ring.len() >= 3)
        .collect();
    inset(&shrunk, -width)
}

fn combine(a: &Contours, b: &Contours, overlay: OverlayRule, rule: Rule) -> Contours {
    lift(lower(a).overlay(&lower(b), overlay, rule.into()))
}

pub fn union(a: &Contours, b: &Contours, rule: Rule) -> Contours {
    combine(a, b, OverlayRule::Union, rule)
}

pub fn difference(a: &Contours, b: &Contours, rule: Rule) -> Contours {
    combine(a, b, OverlayRule::Difference, rule)
}

pub fn intersection(a: &Contours, b: &Contours, rule: Rule) -> Contours {
    combine(a, b, OverlayRule::Intersect, rule)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::path::signed_area;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Contours {
        vec![vec![
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ]]
    }

    fn area(contours: &Contours) -> f64 {
        signed_area(&crate::geom::path::from_contours(contours)).abs()
    }

    #[test]
    fn overlapping_squares_union_into_one_region() {
        let joined = union(&rect(0.0, 0.0, 2.0, 2.0), &rect(1.0, 1.0, 3.0, 3.0), Rule::NonZero);
        assert_eq!(joined.len(), 1);
        assert!((area(&joined) - 7.0).abs() < 1e-9);
    }

    #[test]
    fn a_hole_is_cut_and_kept_as_its_own_ring() {
        let holed = difference(&rect(0.0, 0.0, 4.0, 4.0), &rect(1.0, 1.0, 3.0, 3.0), Rule::NonZero);
        assert_eq!(holed.len(), 2, "an outer ring and its hole");
        assert!((area(&holed) - 12.0).abs() < 1e-9);
    }

    #[test]
    fn simplify_resolves_an_even_odd_hole() {
        // Two rings wound the same way: even-odd makes the inner one a hole.
        let mut rings = rect(0.0, 0.0, 4.0, 4.0);
        rings.extend(rect(1.0, 1.0, 3.0, 3.0));
        assert!((area(&simplify(&rings, Rule::EvenOdd)) - 12.0).abs() < 1e-9);
        assert!((area(&simplify(&rings, Rule::NonZero)) - 16.0).abs() < 1e-9);
    }

    #[test]
    fn separate_regions_stay_separate() {
        let apart = union(&rect(0.0, 0.0, 1.0, 1.0), &rect(2.0, 2.0, 3.0, 3.0), Rule::NonZero);
        assert_eq!(apart.len(), 2);
    }

    #[test]
    fn closing_joins_across_a_hairline_and_keeps_a_gap() {
        // Two squares a hair apart, and a third well clear of them.
        let mut rings = rect(0.0, 0.0, 1.0, 1.0);
        rings.extend(rect(1.000001, 0.0, 2.0, 1.0));
        rings.extend(rect(2.1, 0.0, 3.0, 1.0));
        let closed = close(&rings, 0.01);
        assert_eq!(closed.len(), 2, "the hairline is closed, the gap kept: {closed:?}");
        // No bigger than it was, give or take the thinning.
        assert!((area(&closed) - area(&rings)).abs() < 0.01, "{}", area(&closed));
    }

    #[test]
    fn opening_drops_a_hairline_and_keeps_a_body() {
        // A hairline strip, and a square well wider than the opening.
        let mut rings = rect(0.0, 0.0, 1.0, 0.0005);
        rings.extend(rect(0.0, 1.0, 1.0, 2.0));
        let opened = open(&rings, 0.01);
        assert_eq!(opened.len(), 1, "the hairline is gone, the square kept: {opened:?}");
        assert!((area(&opened) - 1.0).abs() < 0.01, "{}", area(&opened));
    }
}
