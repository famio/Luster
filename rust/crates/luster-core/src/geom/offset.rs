//! Moving an outline in, point by point.

use kurbo::Point;

use crate::geom::path::Contours;

/// Every ring of a region moved `d` towards the material.
pub fn inset(rings: &Contours, d: f64) -> Contours {
    rings.iter().map(|ring| inset_ring(ring, d)).collect()
}

/// Moves every point of a ring `d` towards the material, along the bisector of
/// its two edges.
///
/// Where the offset would fold the ring over itself — at a notch narrower than
/// twice `d` — the fold is pinched shut instead, which leaves degenerate
/// triangles rather than an inside-out piece.
pub fn inset_ring(ring: &[Point], d: f64) -> Vec<Point> {
    let n = ring.len();
    if n < 3 || d <= 0.0 {
        return ring.to_vec();
    }
    // Inward is the left of the direction of travel: outers and holes are
    // wound the other way round, so this moves both towards the material.
    let normal = |a: Point, b: Point| {
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len = (dx * dx + dy * dy).sqrt().max(1e-12);
        (-dy / len, dx / len)
    };
    let normals: Vec<(f64, f64)> =
        (0..n).map(|i| normal(ring[i], ring[(i + 1) % n])).collect();

    let mut moved: Vec<Point> = (0..n)
        .map(|i| {
            let before = normals[(i + n - 1) % n];
            let here = normals[i];
            let sum = (before.0 + here.0, before.1 + here.1);
            let along = 1.0 + (before.0 * here.0 + before.1 * here.1);
            // A miter at a sharp corner runs away; clamp it to twice the inset.
            let scale = if along > 1e-6 { (d / along).min(2.0 * d) } else { d };
            Point::new(ring[i].x + sum.0 * scale, ring[i].y + sum.1 * scale)
        })
        .collect();

    // Pinch shut whatever folded: an edge that now runs backwards. Pinching
    // one edge can turn its neighbour back, so a corner cut into many short
    // edges takes passes until nothing is left folded.
    for _ in 0..n {
        let mut folded = false;
        for i in 0..n {
            let j = (i + 1) % n;
            let was = (ring[j].x - ring[i].x, ring[j].y - ring[i].y);
            let now = (moved[j].x - moved[i].x, moved[j].y - moved[i].y);
            if was.0 * now.0 + was.1 * now.1 < 0.0 {
                let mid = Point::new((moved[i].x + moved[j].x) / 2.0, (moved[i].y + moved[j].y) / 2.0);
                moved[i] = mid;
                moved[j] = mid;
                folded = true;
            }
        }
        if !folded {
            break;
        }
    }
    moved
}
