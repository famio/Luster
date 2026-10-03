//! Tracing a coverage field back into a path.
//!
//! Marching squares with interpolated crossings: a crossing sits where the
//! field passes the level, not on a cell boundary, so the trace is sub-pixel
//! from the start. Corners are found on the dense trace, before it is thinned,
//! because a thinned loop cannot tell a corner from a small round end.

use kurbo::{BezPath, Point, Rect};
use rayon::prelude::*;
use crate::math;

/// Distance either side of a point over which its turn is measured, in pixels.
/// Short enough to keep small round ends round, longer than antialiasing blur.
const CORNER_REACH: f64 = 3.0;

/// Turn across `CORNER_REACH` above which a point is a corner.
const CORNER_TURN: f64 = 45.0 * std::f64::consts::PI / 180.0;

/// Rows of cells scanned together.
const BAND: usize = 32;

/// Traces the `level` contour of a coverage field (one value per pixel, rows
/// from the top) into `frame`, in the art's units.
///
/// The result is wound however the loops came out, so callers that care resolve
/// it with `geom::boolean::simplify` under the even-odd rule: nesting means
/// holes, not winding.
pub fn outline(
    field: &[f32],
    width: usize,
    height: usize,
    frame: Rect,
    level: f32,
    tolerance: f64,
) -> Option<BezPath> {
    let mut path = BezPath::new();
    for (placed, corners) in thinned(field, width, height, frame, level, tolerance)? {
        add_curve(&placed, &corners, &mut path);
    }
    (!path.elements().is_empty()).then_some(path)
}

/// The same, in straight segments through the thinned points rather than
/// curves, so that every point of it lies within `tolerance` of the contour.
/// A curve through them rounds a corner too slight to be flagged, by as much
/// as the straight runs either side of it are long.
pub fn outline_straight(
    field: &[f32],
    width: usize,
    height: usize,
    frame: Rect,
    level: f32,
    tolerance: f64,
) -> Option<BezPath> {
    let mut path = BezPath::new();
    for (placed, _) in thinned(field, width, height, frame, level, tolerance)? {
        path.move_to(placed[0]);
        for p in &placed[1..] {
            path.line_to(*p);
        }
        path.close_path();
    }
    (!path.elements().is_empty()).then_some(path)
}

/// Each traced loop thinned, in `frame`, with its corners flagged.
fn thinned(
    field: &[f32],
    width: usize,
    height: usize,
    frame: Rect,
    level: f32,
    tolerance: f64,
) -> Option<Vec<(Vec<Point>, Vec<bool>)>> {
    if width == 0 || height == 0 || field.len() != width * height {
        return None;
    }
    if !(frame.width() > 0.0 && frame.height() > 0.0) {
        return None;
    }
    let loops = march(field, width, height, level);
    if loops.is_empty() {
        return None;
    }
    let sx = frame.width() / width as f64;
    let sy = frame.height() / height as f64;
    Some(
        loops
            .iter()
            .map(|loop_| fitted(loop_, tolerance))
            .filter(|(points, _)| points.len() >= 3)
            .map(|(points, corners)| {
                // Samples sit at pixel centers, and the field is traced with a
                // one-sample empty border around it.
                let placed = points
                    .iter()
                    .map(|p| Point::new(frame.x0 + (p.x - 0.5) * sx, frame.y0 + (p.y - 0.5) * sy))
                    .collect();
                (placed, corners)
            })
            .collect(),
    )
}

/// Marching squares with interpolated crossings, chained into closed loops.
/// Points are in sample space, the empty border included.
fn march(field: &[f32], width: usize, height: usize, level: f32) -> Vec<Vec<Point>> {
    let padded = width + 2;
    let tall = height + 2;
    let value = |x: usize, y: usize| -> f32 {
        if x >= 1 && y >= 1 && x <= width && y <= height {
            field[(y - 1) * width + (x - 1)]
        } else {
            0.0
        }
    };
    // A crossing is named by the grid edge it lies on (running right or down
    // from a sample), so the two cells that share an edge agree on it exactly.
    let across = |x: usize, y: usize| (y * padded + x) * 2;
    let down = |x: usize, y: usize| (y * padded + x) * 2 + 1;
    let point = |edge: usize| -> Point {
        let (x, y) = ((edge / 2) % padded, (edge / 2) / padded);
        let from = value(x, y);
        let to = if edge % 2 == 0 { value(x + 1, y) } else { value(x, y + 1) };
        let span = to - from;
        let t = if span.abs() > 1e-6 {
            f64::from(((level - from) / span).clamp(0.0, 1.0))
        } else {
            0.5
        };
        if edge % 2 == 0 {
            Point::new(x as f64 + t, y as f64)
        } else {
            Point::new(x as f64, y as f64 + t)
        }
    };

    // Each band of rows is scanned on its own; the links come out in the
    // same order a single scan would give them.
    let bands: Vec<Vec<(usize, usize)>> = (0..(tall - 1).div_ceil(BAND))
        .into_par_iter()
        .map(|band| {
            let mut links: Vec<(usize, usize)> = Vec::new();
            // Two rows of samples, border included.
            let mut upper = vec![0.0f32; padded];
            let mut lower = vec![0.0f32; padded];
            let fill = |row: &mut [f32], y: usize| {
                if y >= 1 && y <= height {
                    row[1..=width].copy_from_slice(&field[(y - 1) * width..y * width]);
                } else {
                    row.fill(0.0);
                }
            };
            for y in band * BAND..((band + 1) * BAND).min(tall - 1) {
                fill(&mut upper, y);
                fill(&mut lower, y + 1);
                for x in 0..padded - 1 {
                    let (a, b) = (upper[x], upper[x + 1]);
                    let (c, d) = (lower[x + 1], lower[x]);
                    let code = u8::from(a >= level) << 3
                        | u8::from(b >= level) << 2
                        | u8::from(c >= level) << 1
                        | u8::from(d >= level);
                    if code == 0 || code == 15 {
                        continue;
                    }
                    let (top, bottom) = (across(x, y), across(x, y + 1));
                    let (left, right) = (down(x, y), down(x + 1, y));
                    // Directed so the covered side stays on one hand. A saddle
                    // is resolved by the cell's mean value.
                    let joined = (a + b + c + d) / 4.0 >= level;
                    let mut link = |from: usize, to: usize| links.push((from, to));
                    match code {
                        1 => link(left, bottom),
                        2 => link(bottom, right),
                        3 => link(left, right),
                        4 => link(right, top),
                        5 => {
                            if joined {
                                link(left, top);
                                link(right, bottom);
                            } else {
                                link(left, bottom);
                                link(right, top);
                            }
                        }
                        6 => link(bottom, top),
                        7 => link(left, top),
                        8 => link(top, left),
                        9 => link(top, bottom),
                        10 => {
                            if joined {
                                link(top, right);
                                link(bottom, left);
                            } else {
                                link(top, left);
                                link(bottom, right);
                            }
                        }
                        11 => link(top, right),
                        12 => link(right, left),
                        13 => link(right, bottom),
                        14 => link(bottom, left),
                        _ => {}
                    }
                }
            }
            links
        })
        .collect();
    // Sorted by where each link starts, so the same field always gives the
    // same path. Should one crossing be linked twice, the later link stands.
    let mut next: Vec<(usize, usize)> = bands.concat();
    next.sort_by_key(|&(from, _)| from);
    next.reverse();
    next.dedup_by_key(|&mut (from, _)| from);
    next.reverse();
    let mut used = vec![false; next.len()];
    let find = |edge: usize| next.binary_search_by_key(&edge, |&(from, _)| from).ok();

    let mut loops: Vec<Vec<Point>> = Vec::new();
    for first in 0..next.len() {
        if used[first] {
            continue;
        }
        let start = next[first].0;
        let mut loop_: Vec<Point> = Vec::new();
        let mut edge = start;
        while let Some(at) = find(edge).filter(|&at| !used[at]) {
            used[at] = true;
            let here = point(edge);
            // Skip duplicates: a crossing on a sample is shared by its edges.
            if loop_.last().is_none_or(|last| math::distance(*last, here) >= 1e-3) {
                loop_.push(here);
            }
            edge = next[at].1;
            if edge == start {
                break;
            }
        }
        if loop_.len() >= 3 {
            loops.push(loop_);
        }
    }
    loops
}

/// Thins a dense loop to the points a curve is fitted through, and flags the
/// corners, which are kept unmoved.
fn fitted(loop_: &[Point], tolerance: f64) -> (Vec<Point>, Vec<bool>) {
    let count = loop_.len();
    if count <= 8 {
        return (loop_.to_vec(), vec![true; count]);
    }
    // Cumulative arc length at each point.
    let mut along = vec![0.0f64; count + 1];
    for index in 0..count {
        along[index + 1] = along[index] + math::distance(loop_[index], loop_[(index + 1) % count]);
    }
    let length = along[count];
    if length <= 4.0 * CORNER_REACH {
        return (loop_.to_vec(), vec![false; count]);
    }
    // The point at arc length `distance` from the loop's start; wraps both ways.
    let at = |distance: f64| -> Point {
        let mut d = distance % length;
        if d < 0.0 {
            d += length;
        }
        let (mut low, mut high) = (0usize, count);
        while high - low > 1 {
            let mid = (low + high) / 2;
            if along[mid] <= d { low = mid } else { high = mid }
        }
        let (a, b) = (loop_[low], loop_[(low + 1) % count]);
        let span = along[low + 1] - along[low];
        let t = if span > 0.0 { (d - along[low]) / span } else { 0.0 };
        Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
    };
    // Turn at each point, measured across `CORNER_REACH`.
    let turn: Vec<f64> = (0..count)
        .map(|index| {
            let here = loop_[index];
            let before = at(along[index] - CORNER_REACH);
            let after = at(along[index] + CORNER_REACH);
            let into = (here.x - before.x, here.y - before.y);
            let outof = (after.x - here.x, after.y - here.y);
            let lengths = (into.0 * into.0 + into.1 * into.1).sqrt()
                * (outof.0 * outof.0 + outof.1 * outof.1).sqrt();
            if lengths <= 1e-12 {
                return 0.0;
            }
            math::acos(((into.0 * outof.0 + into.1 * outof.1) / lengths).clamp(-1.0, 1.0))
        })
        .collect();

    // A corner is a local peak of the turn; its neighbours see most of the
    // same turn, so only the peak is kept.
    let ring = |distance: f64| {
        let d = distance % length;
        if d < 0.0 { d + length } else { d }
    };
    let mut corners: Vec<usize> = Vec::new();
    for index in 0..count {
        if turn[index] <= CORNER_TURN {
            continue;
        }
        let mut peak = true;
        let mut other = (index + 1) % count;
        while peak && other != index && ring(along[other] - along[index]) <= CORNER_REACH {
            if turn[other] > turn[index] {
                peak = false;
            }
            other = (other + 1) % count;
        }
        other = (index + count - 1) % count;
        while peak && other != index && ring(along[index] - along[other]) <= CORNER_REACH {
            if turn[other] >= turn[index] {
                peak = false;
            }
            other = (other + count - 1) % count;
        }
        if peak {
            corners.push(index);
        }
    }

    if corners.is_empty() {
        let kept = crate::geom::simplify::closed(loop_, tolerance);
        let count = kept.len();
        return (kept, vec![false; count]);
    }
    // Thin each run between corners separately, so every corner survives.
    let mut points: Vec<Point> = Vec::new();
    let mut flags: Vec<bool> = Vec::new();
    for (number, &first) in corners.iter().enumerate() {
        let last = corners[(number + 1) % corners.len()];
        let mut run = vec![loop_[first]];
        let mut index = (first + 1) % count;
        while index != last {
            run.push(loop_[index]);
            index = (index + 1) % count;
        }
        run.push(loop_[last]);
        let kept = crate::geom::simplify::open(&run, tolerance);
        for (offset, point) in kept.iter().take(kept.len() - 1).enumerate() {
            points.push(*point);
            flags.push(offset == 0);
        }
    }
    (points, flags)
}

/// Adds a closed centripetal Catmull-Rom spline through the points, as cubics.
///
/// Centripetal, so a long segment does not make a short one overshoot, and the
/// curve runs straight into and out of a corner instead of through it.
fn add_curve(loop_: &[Point], corners: &[bool], path: &mut BezPath) {
    let count = loop_.len();
    if count < 4 {
        path.move_to(loop_[0]);
        for p in &loop_[1..] {
            path.line_to(*p);
        }
        path.close_path();
        return;
    }
    let point = |index: isize| loop_[index.rem_euclid(count as isize) as usize];
    let corner = |index: isize| corners[index.rem_euclid(count as isize) as usize];
    // Centripetal knot spacing: the square root of the distance.
    let span = |a: Point, b: Point| math::distance(a, b).sqrt().max(1e-6);

    path.move_to(loop_[0]);
    for index in 0..count as isize {
        let (p0, p1) = (point(index - 1), point(index));
        let (p2, p3) = (point(index + 1), point(index + 2));
        let (d1, d2, d3) = (span(p0, p1), span(p1, p2), span(p2, p3));
        // A corner's handle points along its own segment, so an edge between
        // two corners stays straight.
        let leaving = if corner(index) { d2 / (3.0 * (d2 + d2)) } else { d2 / (3.0 * (d1 + d2)) };
        let arriving =
            if corner(index + 1) { d2 / (3.0 * (d2 + d2)) } else { d2 / (3.0 * (d2 + d3)) };
        let back = if corner(index) { p1 } else { p0 };
        let ahead = if corner(index + 1) { p2 } else { p3 };
        // Handles are clamped to 0.45 of their segment (a third-of-a-circle arc
        // needs 0.44): a handle is sized from its neighbours, so a long
        // neighbouring segment would otherwise make the curve overshoot.
        let room = math::distance(p1, p2) * 0.45;
        let held = |x: f64, y: f64| -> (f64, f64) {
            let length = (x * x + y * y).sqrt();
            if length > room && length > 0.0 { (x * room / length, y * room / length) } else { (x, y) }
        };
        let out = held((p2.x - back.x) * leaving, (p2.y - back.y) * leaving);
        let into = held((ahead.x - p1.x) * arriving, (ahead.y - p1.y) * arriving);
        path.curve_to(
            Point::new(p1.x + out.0, p1.y + out.1),
            Point::new(p2.x - into.0, p2.y - into.1),
            p2,
        );
    }
    path.close_path();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::path::signed_area;
    use crate::raster::Sheet;

    /// A field of a disc of `radius` art units, drawn on its own sheet.
    fn disc_field(radius: f64) -> (Sheet, Vec<f32>) {
        let sheet = Sheet::around(Rect::new(-radius, -radius, radius, radius), radius * 0.2, 256)
            .expect("a sheet");
        let path = kurbo::Shape::to_path(&kurbo::Circle::new((0.0, 0.0), radius), 1e-6);
        let cover = sheet.lay(&path, 0.0);
        (sheet, cover)
    }

    #[test]
    fn a_disc_traces_back_to_its_area() {
        let (sheet, cover) = disc_field(1.0);
        let path = outline(&cover, sheet.width, sheet.height, sheet.frame, 0.5, 0.6).unwrap();
        let area = signed_area(&path).abs();
        assert!((area - std::f64::consts::PI).abs() < 0.02, "{area}");
    }

    #[test]
    fn a_square_keeps_its_corners() {
        let sheet = Sheet::around(Rect::new(-1.0, -1.0, 1.0, 1.0), 0.2, 256).unwrap();
        let mut square = BezPath::new();
        square.move_to((-1.0, -1.0));
        square.line_to((1.0, -1.0));
        square.line_to((1.0, 1.0));
        square.line_to((-1.0, 1.0));
        square.close_path();
        let cover = sheet.lay(&square, 0.0);
        let path = outline(&cover, sheet.width, sheet.height, sheet.frame, 0.5, 0.6).unwrap();
        let area = signed_area(&path).abs();
        assert!((area - 4.0).abs() < 0.05, "{area}");
        // Corners are kept unmoved, so the traced box reaches them.
        let box_ = kurbo::Shape::bounding_box(&path);
        assert!((box_.width() - 2.0).abs() < 0.03 && (box_.height() - 2.0).abs() < 0.03, "{box_:?}");
    }

    #[test]
    fn a_hole_traces_as_its_own_loop() {
        let sheet = Sheet::around(Rect::new(-1.0, -1.0, 1.0, 1.0), 0.2, 256).unwrap();
        let mut ring = kurbo::Shape::to_path(&kurbo::Circle::new((0.0, 0.0), 1.0), 1e-6);
        let inner = kurbo::Shape::to_path(&kurbo::Circle::new((0.0, 0.0), 0.5), 1e-6);
        ring.extend(inner.iter());
        // Even-odd on the raster: the inner disc is drawn, then cut out.
        let mut cover = sheet.lay(&ring, 0.0);
        let hole = sheet.lay(&kurbo::Shape::to_path(&kurbo::Circle::new((0.0, 0.0), 0.5), 1e-6), 0.0);
        for (c, h) in cover.iter_mut().zip(&hole) {
            *c = (*c - *h).max(0.0);
        }
        let path = outline(&cover, sheet.width, sheet.height, sheet.frame, 0.5, 0.6).unwrap();
        let rings = crate::geom::path::contours(&path, 0.001);
        assert_eq!(rings.len(), 2, "an outer loop and the hole");
        let resolved = crate::geom::boolean::simplify(&rings, crate::geom::boolean::Rule::EvenOdd);
        let area = signed_area(&crate::geom::path::from_contours(&resolved)).abs();
        let expected = std::f64::consts::PI * (1.0 - 0.25);
        assert!((area - expected).abs() < 0.03, "{area} vs {expected}");
    }

    #[test]
    fn an_empty_field_traces_nothing() {
        let sheet = Sheet::around(Rect::new(0.0, 0.0, 1.0, 1.0), 0.1, 64).unwrap();
        let empty = vec![0.0f32; sheet.pixels()];
        assert!(outline(&empty, sheet.width, sheet.height, sheet.frame, 0.5, 0.6).is_none());
    }

    /// A 4x4-supersampled coverage field of whatever `inside` says is filled.
    fn coverage(side: usize, inside: impl Fn(f64, f64) -> bool) -> Vec<f32> {
        let mut field = vec![0.0f32; side * side];
        for y in 0..side {
            for x in 0..side {
                let mut hits = 0;
                for sy in 0..4 {
                    for sx in 0..4 {
                        if inside(x as f64 + (sx as f64 + 0.5) / 4.0, y as f64 + (sy as f64 + 0.5) / 4.0) {
                            hits += 1;
                        }
                    }
                }
                field[y * side + x] = hits as f32 / 16.0;
            }
        }
        field
    }

    /// The path's points, with each curve sampled at eight steps.
    fn points(path: &BezPath) -> Vec<Point> {
        use kurbo::{ParamCurve, PathEl};
        let mut out = Vec::new();
        let mut last = Point::ZERO;
        for element in path.elements() {
            match *element {
                PathEl::MoveTo(p) | PathEl::LineTo(p) => {
                    last = p;
                    out.push(p);
                }
                PathEl::CurveTo(c1, c2, p) => {
                    let curve = kurbo::CubicBez::new(last, c1, c2, p);
                    for step in 1..=8 {
                        out.push(curve.eval(f64::from(step) / 8.0));
                    }
                    last = p;
                }
                _ => {}
            }
        }
        out
    }

    /// A counter as narrow as a few pixels keeps its round end instead of
    /// coming to a point.
    #[test]
    fn a_small_round_end_stays_round() {
        let side = 96usize;
        let (centre, across, along) = (48.0f64, 6.0f64, 20.0f64);
        let field = coverage(side, |x, y| {
            ((x - centre) / across).powi(2) + ((y - centre) / along).powi(2) <= 1.0
        });
        let frame = Rect::new(0.0, 0.0, side as f64, side as f64);
        let traced = outline(&field, side, side, frame, 0.5, 0.5).unwrap();
        let all = points(&traced);
        assert!(all.len() > 8);
        for point in &all {
            // Approximate distance from the ellipse: the level-set value over
            // the gradient's length.
            let (dx, dy) = (point.x - centre, point.y - centre);
            let value = (dx / across).powi(2) + (dy / along).powi(2) - 1.0;
            let slope = 2.0 * ((dx / (across * across)).powi(2) + (dy / (along * along)).powi(2)).sqrt();
            assert!(value.abs() / slope < 0.35, "the trace leaves the counter at {point:?}");
        }
        // Round: one pixel short of the end, the path is still several wide.
        let near_end: Vec<f64> = all
            .iter()
            .filter(|p| (p.y - (centre - along + 1.0)).abs() < 0.6)
            .map(|p| p.x)
            .collect();
        let reach = near_end.iter().fold(f64::MIN, |m, &v| m.max(v))
            - near_end.iter().fold(f64::MAX, |m, &v| m.min(v));
        assert!(reach > 3.0, "the counter's end came to a point");
    }

    /// Corners stay sharp and in place, and straight edges stay straight.
    #[test]
    fn a_corner_stays_where_it_was_drawn() {
        let side = 96usize;
        let (low, high) = (20.3f64, 70.7f64);
        let field = coverage(side, |x, y| x >= low && x <= high && y >= low && y <= high);
        let frame = Rect::new(0.0, 0.0, side as f64, side as f64);
        let traced = outline(&field, side, side, frame, 0.5, 0.5).unwrap();
        let all = points(&traced);
        for corner in [(low, low), (high, low), (high, high), (low, high)] {
            let corner = Point::new(corner.0, corner.1);
            let nearest = all.iter().map(|p| math::distance(*p, corner)).fold(f64::MAX, f64::min);
            assert!(nearest < 0.6, "the corner at {corner:?} was rounded off");
        }
        for point in &all {
            let off = (point.x - low)
                .abs()
                .min((point.x - high).abs())
                .min((point.y - low).abs())
                .min((point.y - high).abs());
            assert!(off < 0.4, "an edge of the square bows out at {point:?}");
        }
    }

    #[test]
    fn tracing_is_repeatable() {
        let (sheet, cover) = disc_field(1.0);
        let once = outline(&cover, sheet.width, sheet.height, sheet.frame, 0.5, 0.6).unwrap();
        let twice = outline(&cover, sheet.width, sheet.height, sheet.frame, 0.5, 0.6).unwrap();
        assert_eq!(once.to_svg(), twice.to_svg());
    }
}

/// Traces the bright part of a decoded image into `frame`.
///
/// A design tool may export a vector mask as an embedded image; tracing what
/// it shows gives an outline the reader can use like any other region. The
/// image is sampled small, read by luminance (a transparent pixel is black, so
/// outside), and traced at the half level.
pub fn image(
    luminance: &[f32],
    width: usize,
    height: usize,
    frame: Rect,
    tolerance: f64,
) -> Option<BezPath> {
    outline(luminance, width, height, frame, 0.5, tolerance)
}

/// Longest side an image mask is sampled at, in samples.
pub const IMAGE_RESOLUTION: usize = 256;
/// Simplification tolerance for a traced image, in samples.
pub const IMAGE_TOLERANCE: f64 = 0.3;
