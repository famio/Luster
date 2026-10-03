//! Paths and the measurements taken of them.

use kurbo::{Affine, BezPath, ParamCurve, PathEl, Point, Rect};
use crate::math;

/// Closed polygons: one per subpath, without the repeated closing point.
pub type Contours = Vec<Vec<Point>>;

/// How finely curves are flattened, in the units of the path being flattened.
/// A badge is 1 across, so this is about a thousandth of it: finer than the
/// rasters anything is traced on.
pub const TOLERANCE: f64 = 0.0006;

/// Area a path encloses, signed by winding: holes wound the other way subtract.
///
/// Curves are sampled at eight steps each.
pub fn signed_area(path: &BezPath) -> f64 {
    let mut total = 0.0;
    let mut pen = Point::ZERO;
    let mut start = Point::ZERO;
    let mut edge = |a: Point, b: Point| total += a.x * b.y - b.x * a.y;
    for element in path.elements() {
        match *element {
            PathEl::MoveTo(p) => {
                pen = p;
                start = p;
            }
            PathEl::LineTo(p) => {
                edge(pen, p);
                pen = p;
            }
            PathEl::QuadTo(c, p) => {
                let curve = kurbo::QuadBez::new(pen, c, p);
                let mut last = pen;
                for step in 1..=8 {
                    let next = curve.eval(f64::from(step) / 8.0);
                    edge(last, next);
                    last = next;
                }
                pen = p;
            }
            PathEl::CurveTo(c1, c2, p) => {
                let curve = kurbo::CubicBez::new(pen, c1, c2, p);
                let mut last = pen;
                for step in 1..=8 {
                    let next = curve.eval(f64::from(step) / 8.0);
                    edge(last, next);
                    last = next;
                }
                pen = p;
            }
            PathEl::ClosePath => {
                edge(pen, start);
                pen = start;
            }
        }
    }
    total / 2.0
}

/// The most a flattened curve may turn across one of its segments.
///
/// The tolerance alone lets a small curve through in a few long segments: a
/// disc a hundredth of the badge across comes out an octagon, well inside the
/// tolerance, and the light on its walls shows every side.
const MAX_TURN: f64 = 10.0 * std::f64::consts::PI / 180.0;

/// Splits each curve into pieces that turn no more than `MAX_TURN`, so that
/// flattening gives it at least a segment each. A curve is not cut finer than
/// `tolerance` along, so a tiny one is not ground into dust.
fn bounded_turns(path: &BezPath, tolerance: f64) -> BezPath {
    // How far the control polygon turns, which bounds the curve's own turn,
    // and how long it is.
    let measure = |points: &[Point]| -> (f64, f64) {
        let legs: Vec<kurbo::Vec2> =
            points.windows(2).map(|w| w[1] - w[0]).filter(|leg| leg.hypot2() > 1e-24).collect();
        let turn = legs.windows(2).map(|w| math::atan2(w[0].cross(w[1]), w[0].dot(w[1])).abs()).sum();
        (turn, legs.iter().map(|leg| math::length(*leg)).sum())
    };
    let pieces = |points: &[Point]| -> usize {
        let (turn, length) = measure(points);
        let by_turn = (turn / MAX_TURN).ceil();
        let by_length = (length / tolerance).floor().max(1.0);
        by_turn.min(by_length).max(1.0) as usize
    };
    let mut out = BezPath::new();
    let mut pen = Point::ZERO;
    for element in path.elements() {
        match *element {
            PathEl::QuadTo(c, p) => {
                let n = pieces(&[pen, c, p]);
                let curve = kurbo::QuadBez::new(pen, c, p);
                for i in 0..n {
                    let piece = curve.subsegment(i as f64 / n as f64..(i + 1) as f64 / n as f64);
                    out.quad_to(piece.p1, if i + 1 == n { p } else { piece.p2 });
                }
                pen = p;
            }
            PathEl::CurveTo(c1, c2, p) => {
                let n = pieces(&[pen, c1, c2, p]);
                let curve = kurbo::CubicBez::new(pen, c1, c2, p);
                for i in 0..n {
                    let piece = curve.subsegment(i as f64 / n as f64..(i + 1) as f64 / n as f64);
                    out.curve_to(piece.p1, piece.p2, if i + 1 == n { p } else { piece.p3 });
                }
                pen = p;
            }
            PathEl::MoveTo(p) | PathEl::LineTo(p) => {
                out.push(*element);
                pen = p;
            }
            PathEl::ClosePath => out.close_path(),
        }
    }
    out
}

/// Flattens a path into closed polygons, one per subpath.
///
/// Repeated points are dropped and a subpath that ends where it started is not
/// given the point twice, so every contour is ready to be walked as a ring.
/// Curves are held to `MAX_TURN` a segment as well as to `tolerance`.
pub fn contours(path: &BezPath, tolerance: f64) -> Contours {
    let path = bounded_turns(path, tolerance);
    let mut all: Contours = Vec::new();
    let mut current: Vec<Point> = Vec::new();
    let push = |current: &mut Vec<Point>, p: Point| {
        if current.last().is_none_or(|last| last.distance_squared(p) > 1e-24) {
            current.push(p);
        }
    };
    let finish = |current: &mut Vec<Point>, all: &mut Contours| {
        while current.len() > 1 && current[0].distance_squared(*current.last().unwrap()) <= 1e-24 {
            current.pop();
        }
        if current.len() >= 3 {
            all.push(std::mem::take(current));
        } else {
            current.clear();
        }
    };
    kurbo::flatten(path.elements().iter().copied(), tolerance, |element| match element {
        PathEl::MoveTo(p) => {
            finish(&mut current, &mut all);
            current.push(p);
        }
        PathEl::LineTo(p) => push(&mut current, p),
        PathEl::ClosePath => finish(&mut current, &mut all),
        _ => unreachable!("flatten emits only moves, lines and closes"),
    });
    finish(&mut current, &mut all);
    all
}

/// A path from closed polygons.
pub fn from_contours(contours: &[Vec<Point>]) -> BezPath {
    let mut path = BezPath::new();
    for contour in contours {
        let Some((first, rest)) = contour.split_first() else { continue };
        path.move_to(*first);
        for p in rest {
            path.line_to(*p);
        }
        path.close_path();
    }
    path
}

/// Maps a path from the artwork's 0...1 y-down square into badge units: a
/// square centered on the origin, 1 across, y up.
pub fn to_badge_units() -> Affine {
    Affine::new([1.0, 0.0, 0.0, -1.0, -0.5, 0.5])
}

/// Whether `p` is inside the contours under `even_odd`, by winding number.
pub fn inside(p: Point, contours: &[Vec<Point>], even_odd: bool) -> bool {
    let mut winding = 0i32;
    for contour in contours {
        for (i, a) in contour.iter().enumerate() {
            let b = contour[(i + 1) % contour.len()];
            let cross = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
            if a.y <= p.y {
                if b.y > p.y && cross > 0.0 {
                    winding += 1;
                }
            } else if b.y <= p.y && cross < 0.0 {
                winding -= 1;
            }
        }
    }
    if even_odd { winding % 2 != 0 } else { winding != 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(side: f64) -> BezPath {
        let mut path = BezPath::new();
        path.move_to((0.0, 0.0));
        path.line_to((side, 0.0));
        path.line_to((side, side));
        path.line_to((0.0, side));
        path.close_path();
        path
    }

    #[test]
    fn a_square_encloses_its_area_wound_one_way_or_the_other() {
        assert_eq!(signed_area(&square(2.0)), 4.0);
        let mut reversed = BezPath::new();
        reversed.move_to((0.0, 0.0));
        reversed.line_to((0.0, 2.0));
        reversed.line_to((2.0, 2.0));
        reversed.line_to((2.0, 0.0));
        reversed.close_path();
        assert_eq!(signed_area(&reversed), -4.0);
    }

    #[test]
    fn a_circles_area_is_close_to_pi() {
        let circle = kurbo::Shape::to_path(&kurbo::Circle::new((0.0, 0.0), 1.0), 1e-6);
        // Eight samples per curve: the inscribed polygon falls a little short
        // of the circle, by about 0.1%.
        let area = signed_area(&circle);
        assert!((area - std::f64::consts::PI).abs() < 4e-3, "{area}");
        assert!(area < std::f64::consts::PI);
    }

    #[test]
    fn contours_close_and_drop_repeats() {
        let mut path = square(1.0);
        path.line_to((0.0, 0.0)); // back to the start, then closed
        path.close_path();
        let contours = contours(&path, TOLERANCE);
        assert_eq!(contours.len(), 1);
        assert_eq!(contours[0].len(), 4);
    }

    #[test]
    fn winding_tells_inside_from_outside() {
        let outer = vec![
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(4.0, 4.0),
            Point::new(0.0, 4.0),
        ];
        // A hole, wound the other way.
        let hole = vec![
            Point::new(1.0, 1.0),
            Point::new(1.0, 3.0),
            Point::new(3.0, 3.0),
            Point::new(3.0, 1.0),
        ];
        let shape = vec![outer, hole];
        assert!(inside(Point::new(0.5, 2.0), &shape, false));
        assert!(!inside(Point::new(2.0, 2.0), &shape, false));
        assert!(!inside(Point::new(5.0, 2.0), &shape, false));
    }
}

/// The box round every point of the rings, if they span anything.
pub fn bounds(rings: &Contours) -> Option<Rect> {
    let mut box_: Option<Rect> = None;
    for point in rings.iter().flatten() {
        let next = Rect::new(point.x, point.y, point.x, point.y);
        box_ = Some(box_.map_or(next, |b: Rect| b.union(next)));
    }
    box_.filter(|b| b.width() > 0.0 || b.height() > 0.0)
}
