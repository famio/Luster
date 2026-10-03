//! Thinning a traced outline: Ramer-Douglas-Peucker.

use kurbo::Point;

use crate::math;

/// Ramer-Douglas-Peucker over a closed loop. RDP always keeps its end points,
/// so the loop is anchored at the point farthest from its centroid rather than
/// at the arbitrary trace start, which could pin a kink onto a flat run.
pub fn closed(loop_: &[Point], tolerance: f64) -> Vec<Point> {
    if loop_.len() <= 3 {
        return loop_.to_vec();
    }
    let centre = loop_.iter().fold(Point::ZERO, |sum, p| Point::new(sum.x + p.x, sum.y + p.y));
    let centre = Point::new(centre.x / loop_.len() as f64, centre.y / loop_.len() as f64);
    let anchor = (0..loop_.len())
        .max_by(|&a, &b| math::distance(centre, loop_[a]).total_cmp(&math::distance(centre, loop_[b])))
        .unwrap_or(0);
    let mut turned: Vec<Point> = loop_[anchor..].iter().chain(&loop_[..anchor]).copied().collect();
    turned.push(turned[0]);
    let mut kept = open(&turned, tolerance);
    if kept.len() > 1 && kept[0] == kept[kept.len() - 1] {
        kept.pop();
    }
    kept
}

/// Ramer-Douglas-Peucker on an open polyline.
pub fn open(line: &[Point], tolerance: f64) -> Vec<Point> {
    if line.len() <= 3 {
        return line.to_vec();
    }
    let mut keep = vec![false; line.len()];
    keep[0] = true;
    keep[line.len() - 1] = true;
    let mut stack = vec![(0usize, line.len() - 1)];
    while let Some((first, last)) = stack.pop() {
        if last <= first + 1 {
            continue;
        }
        let mut worst = 0.0;
        let mut index = first;
        for i in first + 1..last {
            let d = distance(line[i], line[first], line[last]);
            if d > worst {
                worst = d;
                index = i;
            }
        }
        if worst > tolerance {
            keep[index] = true;
            stack.push((first, index));
            stack.push((index, last));
        }
    }
    line.iter().zip(keep).filter_map(|(p, k)| k.then_some(*p)).collect()
}

fn distance(point: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let length = (dx * dx + dy * dy).sqrt();
    if length == 0.0 {
        return math::distance(point, a);
    }
    (dy * point.x - dx * point.y + b.x * a.y - b.y * a.x).abs() / length
}
