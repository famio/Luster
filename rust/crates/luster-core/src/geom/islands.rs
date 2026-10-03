//! Grouping contours into islands: one outer ring and the holes inside it.
//!
//! The extruder builds one piece per island, so a badge with separate pieces
//! (a disc and a ribbon, say) keeps them apart, and a hole in one of them is
//! not filled by another's outline.

use kurbo::Point;

use super::path::{self, Contours};

/// An outer ring and the rings enclosed by it.
#[derive(Clone, Debug)]
pub struct Island {
    pub outer: Vec<Point>,
    pub holes: Contours,
}

impl Island {
    /// The rings, outer first: the shape as a fill.
    pub fn rings(&self) -> Contours {
        let mut rings = vec![self.outer.clone()];
        rings.extend(self.holes.iter().cloned());
        rings
    }
}

/// Splits disjoint rings into islands. Rings are assumed already resolved (as
/// `boolean::simplify` leaves them): they do not cross, so a ring is a hole of
/// the smallest ring that contains it.
///
/// Rings smaller than `speck` in area are dropped: booleans and traces leave
/// hairline scraps that nothing can be extruded from.
pub fn split(rings: &Contours, speck: f64) -> Vec<Island> {
    let kept: Vec<&Vec<Point>> = rings
        .iter()
        .filter(|ring| ring.len() >= 3 && area(ring).abs() > speck)
        .collect();

    // Largest first, so a ring's container is already an island when it is
    // reached.
    let mut order: Vec<usize> = (0..kept.len()).collect();
    order.sort_by(|&a, &b| area(kept[b]).abs().total_cmp(&area(kept[a]).abs()));

    let mut islands: Vec<Island> = Vec::new();
    for index in order {
        let ring = kept[index];
        let point = interior(ring);
        // The smallest island that contains this ring owns it as a hole.
        let owner = islands
            .iter_mut()
            .filter(|island| path::inside(point, &island.rings(), true))
            .min_by(|a, b| area(&a.outer).abs().total_cmp(&area(&b.outer).abs()));
        match owner {
            Some(island) => island.holes.push(ring.clone()),
            None => islands.push(Island { outer: ring.clone(), holes: Vec::new() }),
        }
    }
    islands
}

fn area(ring: &[Point]) -> f64 {
    let mut total = 0.0;
    for (i, a) in ring.iter().enumerate() {
        let b = ring[(i + 1) % ring.len()];
        total += a.x * b.y - b.x * a.y;
    }
    total / 2.0
}

/// A point inside the ring: its centroid, or failing that the midpoint of a
/// diagonal that stays inside, which a star or a crescent also has.
fn interior(ring: &[Point]) -> Point {
    let centroid = ring.iter().fold(Point::ZERO, |sum, p| Point::new(sum.x + p.x, sum.y + p.y));
    let centroid = Point::new(centroid.x / ring.len() as f64, centroid.y / ring.len() as f64);
    if path::inside(centroid, std::slice::from_ref(&ring.to_vec()), true) {
        return centroid;
    }
    for i in 0..ring.len() {
        for j in (i + 2)..ring.len() {
            let mid = Point::new((ring[i].x + ring[j].x) / 2.0, (ring[i].y + ring[j].y) / 2.0);
            if path::inside(mid, std::slice::from_ref(&ring.to_vec()), true) {
                return mid;
            }
        }
    }
    centroid
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::boolean::{self, Rule};

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
        vec![
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ]
    }

    #[test]
    fn a_hole_goes_to_the_ring_that_contains_it() {
        let rings = boolean::simplify(&vec![rect(0.0, 0.0, 4.0, 4.0), rect(1.0, 1.0, 3.0, 3.0)], Rule::EvenOdd);
        let islands = split(&rings, 1e-9);
        assert_eq!(islands.len(), 1);
        assert_eq!(islands[0].holes.len(), 1);
    }

    #[test]
    fn separate_shapes_are_separate_islands() {
        let rings = vec![rect(0.0, 0.0, 1.0, 1.0), rect(2.0, 0.0, 3.0, 1.0)];
        assert_eq!(split(&rings, 1e-9).len(), 2);
    }

    #[test]
    fn a_shape_inside_a_hole_is_an_island_of_its_own() {
        let rings = boolean::simplify(
            &vec![
                rect(0.0, 0.0, 9.0, 9.0),
                rect(2.0, 2.0, 7.0, 7.0),
                rect(3.0, 3.0, 6.0, 6.0),
            ],
            Rule::EvenOdd,
        );
        let islands = split(&rings, 1e-9);
        assert_eq!(islands.len(), 2, "the outer ring and the shape in its hole");
        assert_eq!(islands[0].holes.len(), 1);
    }

    #[test]
    fn specks_are_dropped() {
        let rings = vec![rect(0.0, 0.0, 4.0, 4.0), rect(0.0, 0.0, 0.001, 0.001)];
        assert_eq!(split(&rings, 1e-5).len(), 1);
    }
}
