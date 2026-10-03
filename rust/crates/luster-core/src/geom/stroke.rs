//! Strokes as regions: the engine treats drawn line work as an area to fill,
//! never as a pen.

use kurbo::{BezPath, Cap, Join, Stroke, StrokeOpts};

use super::path::TOLERANCE;

/// How a stroke ends and turns, as the SVG attributes give it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

/// The region a stroke of `width` covers.
pub fn outline(path: &BezPath, width: f64, cap: LineCap, join: LineJoin, miter_limit: f64) -> BezPath {
    let style = Stroke {
        width,
        join: match join {
            LineJoin::Miter => Join::Miter,
            LineJoin::Round => Join::Round,
            LineJoin::Bevel => Join::Bevel,
        },
        miter_limit,
        start_cap: cap.into(),
        end_cap: cap.into(),
        dash_pattern: Default::default(),
        dash_offset: 0.0,
    };
    kurbo::stroke(path.elements().iter().copied(), &style, &StrokeOpts::default(), TOLERANCE)
}

/// A round-ended, round-joined stroke: what the engine grows shapes with.
pub fn round(path: &BezPath, width: f64) -> BezPath {
    outline(path, width, LineCap::Round, LineJoin::Round, 4.0)
}

impl From<LineCap> for Cap {
    fn from(cap: LineCap) -> Self {
        match cap {
            LineCap::Butt => Cap::Butt,
            LineCap::Round => Cap::Round,
            LineCap::Square => Cap::Square,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::path::signed_area;

    #[test]
    fn a_stroked_line_covers_its_rectangle_and_its_round_ends() {
        let mut line = BezPath::new();
        line.move_to((0.0, 0.0));
        line.line_to((10.0, 0.0));
        let area = signed_area(&round(&line, 2.0)).abs();
        let expected = 10.0 * 2.0 + std::f64::consts::PI;
        assert!((area - expected).abs() < 0.05, "{area} vs {expected}");
    }

    #[test]
    fn a_butt_cap_stops_at_the_end() {
        let mut line = BezPath::new();
        line.move_to((0.0, 0.0));
        line.line_to((10.0, 0.0));
        let area = signed_area(&outline(&line, 2.0, LineCap::Butt, LineJoin::Miter, 4.0)).abs();
        assert!((area - 20.0).abs() < 0.05, "{area}");
    }
}
