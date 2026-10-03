//! What a region is painted with: a colour, or a gradient.

use kurbo::Affine;

use crate::model::Rgba;

/// A gradient stop, in sRGB with straight alpha.
#[derive(Clone, Copy, Debug)]
pub struct Stop {
    pub offset: f32,
    pub color: Rgba,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spread {
    Pad,
    Reflect,
    Repeat,
}

#[derive(Clone, Debug)]
pub enum Gradient {
    /// From `(x1, y1)` to `(x2, y2)`, in the document's units.
    Linear { from: (f64, f64), to: (f64, f64), stops: Vec<Stop>, transform: Affine, spread: Spread },
    /// Centre, radius and focal point, in the document's units.
    Radial {
        centre: (f64, f64),
        radius: f64,
        focus: (f64, f64),
        stops: Vec<Stop>,
        transform: Affine,
        spread: Spread,
    },
}

impl Gradient {
    pub fn stops(&self) -> &[Stop] {
        match self {
            Gradient::Linear { stops, .. } | Gradient::Radial { stops, .. } => stops,
        }
    }

    /// The gradient's flat average: what stands in for it while cells are
    /// built, before the document is painted and each cell's colour read back.
    pub fn average(&self) -> Option<Rgba> {
        average(self.stops())
    }
}

/// The average of a run of stops: each weighted by its own opacity, and the
/// alpha the mean of the opacities. A ramp that fades out is mostly the colour
/// that stays.
pub fn average(stops: &[Stop]) -> Option<Rgba> {
    if stops.is_empty() {
        return None;
    }
    let (mut r, mut g, mut b, mut weight) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for stop in stops {
        let w = f64::from(stop.color.a).max(0.001);
        r += f64::from(stop.color.r) * w;
        g += f64::from(stop.color.g) * w;
        b += f64::from(stop.color.b) * w;
        weight += w;
    }
    if weight <= 0.0 {
        return None;
    }
    let alpha = stops.iter().map(|s| f64::from(s.color.a)).sum::<f64>() / stops.len() as f64;
    Some(Rgba {
        r: (r / weight) as f32,
        g: (g / weight) as f32,
        b: (b / weight) as f32,
        a: alpha.clamp(0.0, 1.0) as f32,
    })
}

impl From<Stop> for Rgba {
    fn from(stop: Stop) -> Rgba {
        stop.color
    }
}

/// How a region is painted.
#[derive(Clone, Debug)]
pub enum Paint {
    Solid(Rgba),
    Gradient(Box<Gradient>),
}

impl Paint {
    /// The colour that stands in for this paint while cells are built.
    pub fn flat(&self) -> Option<Rgba> {
        match self {
            Paint::Solid(color) => Some(*color),
            Paint::Gradient(gradient) => gradient.average(),
        }
    }

    /// Multiplies the paint's alpha, as `fill-opacity` and a group's opacity do.
    pub fn faded(&self, by: f32) -> Paint {
        match self {
            Paint::Solid(color) => Paint::Solid(Rgba { a: color.a * by, ..*color }),
            Paint::Gradient(gradient) => {
                let mut gradient = gradient.clone();
                let stops = match gradient.as_mut() {
                    Gradient::Linear { stops, .. } | Gradient::Radial { stops, .. } => stops,
                };
                for stop in stops.iter_mut() {
                    stop.color.a *= by;
                }
                Paint::Gradient(gradient)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stop(offset: f32, v: f32) -> Stop {
        Stop { offset, color: Rgba { r: v, g: v, b: v, a: 1.0 } }
    }

    #[test]
    fn one_stop_is_its_own_average() {
        assert_eq!(average(&[stop(0.0, 0.25)]).unwrap().r, 0.25);
    }

    #[test]
    fn a_ramp_averages_to_its_middle() {
        let flat = average(&[stop(0.0, 0.0), stop(1.0, 1.0)]).unwrap();
        assert!((flat.r - 0.5).abs() < 1e-6, "{}", flat.r);
    }

    #[test]
    fn a_stop_is_weighted_by_its_own_opacity() {
        // A ramp from opaque black to transparent white is mostly black.
        let clear = Stop { offset: 1.0, color: Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.0 } };
        let flat = average(&[stop(0.0, 0.0), clear]).unwrap();
        assert!(flat.r < 0.01, "{}", flat.r);
        assert!((flat.a - 0.5).abs() < 1e-6, "the alpha is the mean of the stops'");
    }

    #[test]
    fn no_stops_is_no_colour() {
        assert!(average(&[]).is_none());
    }
}
