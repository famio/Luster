//! Painting the document to find out what colour each cell shows.
//!
//! Art is often built from translucent glazes over solid layers, so the
//! document is painted once and each cell's colour is read back from the
//! pixels. The badge's edge is traced from the same painting's alpha: unioning
//! the shapes instead would keep a notch at every seam between them.

use kurbo::{Affine, BezPath, Rect};
use tiny_skia::{FillRule, GradientStop, Paint as SkiaPaint, Pixmap, Shader, SpreadMode, Transform};

use crate::model::Rgba;
use crate::geom::path::{self, Contours};
use crate::raster::trace;
use crate::svg::cells::Cell;
use crate::svg::paint::{Gradient, Paint, Spread};
use crate::svg::read::Element;
use crate::{CancelToken, Error};

/// Longest side of the colour-sampling raster, in pixels.
const RESOLUTION: usize = 512;
/// Longest side of the coverage raster the outline is traced from.
const OUTLINE_RESOLUTION: usize = 1024;
/// Simplification tolerance for the traced outline, in pixels.
const OUTLINE_TOLERANCE: f64 = 0.5;

/// The cells, repainted with what the document composites inside each of
/// their interiors, and the outline of everything it paints.
pub struct Reading {
    pub cells: Vec<Cell>,
    pub outline: Option<BezPath>,
}

/// Two paintings of the document at full resolution; `cancel` is looked at
/// between the steps of each.
pub fn sampling(cells: Vec<Cell>, elements: &[Element], frame: Rect, cancel: &CancelToken) -> Result<Reading, Error> {
    // The outline is traced from a painting of its own, alongside this one.
    let (outline, cells) = rayon::join(
        || if cancel.is_cancelled() { None } else { outline(elements, frame) },
        || colours(cells, elements, frame, cancel),
    );
    cancel.check()?;
    Ok(Reading { cells, outline })
}

/// The cells, each repainted with what the document composites inside it.
fn colours(cells: Vec<Cell>, elements: &[Element], frame: Rect, cancel: &CancelToken) -> Vec<Cell> {
    // One byte of index per cell, and 0 means "no cell".
    if elements.is_empty() || cells.is_empty() || cells.len() >= 255 {
        return cells;
    }
    let Some((mut art, scale, width, height)) = canvas(frame, RESOLUTION) else {
        return cells;
    };
    paint(elements, &mut art, frame, scale);
    if cancel.is_cancelled() {
        return cells;
    }

    // Index raster: each cell filled with its own number, without antialiasing
    // so that edges do not blend into neighbouring indices.
    let Some((mut index, _, _, _)) = canvas(frame, RESOLUTION) else {
        return cells;
    };
    let transform = placement(frame, scale);
    for (number, cell) in cells.iter().enumerate() {
        let mut paint = SkiaPaint::default();
        paint.anti_alias = false;
        let grey = (number + 1) as u8;
        paint.set_color_rgba8(grey, grey, grey, 255);
        if let Some(path) = skia_path(&cell.rings) {
            index.fill_path(&path, &paint, FillRule::Winding, transform, None);
        }
    }

    if cancel.is_cancelled() {
        return cells;
    }
    // Only a cell's interior is sampled: a pixel whose neighbours belong to
    // the same cell. An edge pixel is a blend of the cell and whatever lies
    // beside it, and on a cell a few pixels across those blends would decide
    // its colour.
    let slots: Vec<u8> = index.pixels().iter().map(|p| p.red()).collect();
    let interior = |x: usize, y: usize, number: u8| -> bool {
        x > 0
            && y > 0
            && x + 1 < width
            && y + 1 < height
            && slots[y * width + x - 1] == number
            && slots[y * width + x + 1] == number
            && slots[(y - 1) * width + x] == number
            && slots[(y + 1) * width + x] == number
    };

    let mut totals = vec![(0.0f64, 0.0f64, 0.0f64, 0.0f64); cells.len()];
    for (at, (pixel, slot)) in art.pixels().iter().zip(&slots).enumerate() {
        let number = *slot as usize;
        if number == 0 || number > cells.len() {
            continue;
        }
        if !interior(at % width, at / width, *slot) {
            continue;
        }
        let alpha = f64::from(pixel.alpha()) / 255.0;
        if alpha <= 0.5 {
            continue;
        }
        // The pixmap is premultiplied, so undo it before averaging.
        let total = &mut totals[number - 1];
        total.0 += f64::from(pixel.red()) / 255.0 / alpha;
        total.1 += f64::from(pixel.green()) / 255.0 / alpha;
        total.2 += f64::from(pixel.blue()) / 255.0 / alpha;
        total.3 += 1.0;
    }

    cells
        .into_iter()
        .zip(totals)
        .map(|(cell, total)| {
            if total.3 < 4.0 {
                return cell;
            }
            Cell {
                color: Rgba {
                    r: (total.0 / total.3).min(1.0) as f32,
                    g: (total.1 / total.3).min(1.0) as f32,
                    b: (total.2 / total.3).min(1.0) as f32,
                    a: 1.0,
                },
                ..cell
            }
        })
        .collect()
}

/// Traces the document's alpha coverage into one closed outline.
pub fn outline(elements: &[Element], frame: Rect) -> Option<BezPath> {
    if elements.is_empty() {
        return None;
    }
    let (mut coverage, scale, width, height) = canvas(frame, OUTLINE_RESOLUTION)?;
    // Paint the real paints, not a flat flood: a fill that fades to
    // transparent stops being visible before its path ends.
    paint(elements, &mut coverage, frame, scale);
    // Trace fractional alpha rather than a thresholded mask: a thresholded
    // outline wanders by half a pixel, which shows as ripples once it is
    // offset.
    let alpha: Vec<f32> =
        coverage.pixels().iter().map(|p| f32::from(p.alpha()) / 255.0).collect();
    trace::outline(&alpha, width, height, frame, 0.5, OUTLINE_TOLERANCE)
}

/// Paints the elements in document order, so each glaze lands on what is
/// beneath it.
fn paint(elements: &[Element], canvas: &mut Pixmap, frame: Rect, scale: f64) {
    paint_through(elements, canvas, Affine::scale(scale) * Affine::translate((-frame.x0, -frame.y0)), None);
}

/// The same, through any transform, and optionally clipped.
pub fn paint_through(
    elements: &[Element],
    canvas: &mut Pixmap,
    placement: Affine,
    mask: Option<&tiny_skia::Mask>,
) {
    let transform = skia_transform(placement);
    for element in elements {
        let Some(paint) = &element.paint else { continue };
        let Some(path) = skia_path(&element.rings) else { continue };
        let mut skia = SkiaPaint::default();
        skia.anti_alias = true;
        match paint {
            Paint::Solid(color) => skia.set_color(tiny_skia::Color::from_rgba(
                color.r.clamp(0.0, 1.0),
                color.g.clamp(0.0, 1.0),
                color.b.clamp(0.0, 1.0),
                color.a.clamp(0.0, 1.0),
            ).unwrap_or(tiny_skia::Color::TRANSPARENT)),
            Paint::Gradient(gradient) => match shader(gradient) {
                Some(shader) => skia.shader = shader,
                None => continue,
            },
        }
        canvas.fill_path(&path, &skia, FillRule::Winding, transform, mask);
    }
}

fn shader(gradient: &Gradient) -> Option<Shader<'static>> {
    let stops = |stops: &[crate::svg::paint::Stop]| -> Vec<GradientStop> {
        stops
            .iter()
            .map(|s| {
                GradientStop::new(
                    s.offset,
                    tiny_skia::Color::from_rgba(
                        s.color.r.clamp(0.0, 1.0),
                        s.color.g.clamp(0.0, 1.0),
                        s.color.b.clamp(0.0, 1.0),
                        s.color.a.clamp(0.0, 1.0),
                    )
                    .unwrap_or(tiny_skia::Color::TRANSPARENT),
                )
            })
            .collect()
    };
    let mode = |spread: Spread| match spread {
        Spread::Pad => SpreadMode::Pad,
        Spread::Reflect => SpreadMode::Reflect,
        Spread::Repeat => SpreadMode::Repeat,
    };
    let point = |(x, y): (f64, f64)| tiny_skia::Point::from_xy(x as f32, y as f32);
    match gradient {
        Gradient::Linear { from, to, stops: s, transform, spread } => {
            tiny_skia::LinearGradient::new(
                point(*from),
                point(*to),
                stops(s),
                mode(*spread),
                skia_transform(*transform),
            )
        }
        Gradient::Radial { centre, radius, focus, stops: s, transform, spread } => {
            // The focal point starts the ramp with no radius of its own, as
            // SVG's fx/fy do.
            tiny_skia::RadialGradient::new(
                point(*focus),
                0.0,
                point(*centre),
                *radius as f32,
                stops(s),
                mode(*spread),
                skia_transform(*transform),
            )
        }
    }
}

/// A pixmap covering `frame` whose longer side is `resolution` pixels.
fn canvas(frame: Rect, resolution: usize) -> Option<(Pixmap, f64, usize, usize)> {
    if !(frame.width() > 0.0 && frame.height() > 0.0) {
        return None;
    }
    let scale = resolution as f64 / frame.width().max(frame.height());
    let width = ((frame.width() * scale).round() as usize).max(1);
    let height = ((frame.height() * scale).round() as usize).max(1);
    let pixmap = Pixmap::new(width as u32, height as u32)?;
    Some((pixmap, scale, width, height))
}

/// Document units into raster pixels.
fn placement(frame: Rect, scale: f64) -> Transform {
    Transform::from_scale(scale as f32, scale as f32)
        .pre_translate(-frame.x0 as f32, -frame.y0 as f32)
}

fn skia_transform(affine: Affine) -> Transform {
    let c = affine.as_coeffs();
    Transform::from_row(c[0] as f32, c[1] as f32, c[2] as f32, c[3] as f32, c[4] as f32, c[5] as f32)
}

/// A path for tiny-skia, for callers outside this module.
pub fn skia_path_of(path: &BezPath) -> Option<tiny_skia::Path> {
    skia_path(&path::contours(path, 1e-4))
}

fn skia_path(rings: &Contours) -> Option<tiny_skia::Path> {
    let mut builder = tiny_skia::PathBuilder::new();
    for ring in rings {
        let Some((first, rest)) = ring.split_first() else { continue };
        builder.move_to(first.x as f32, first.y as f32);
        for p in rest {
            builder.line_to(p.x as f32, p.y as f32);
        }
        builder.close();
    }
    builder.finish()
}

/// The bounds of everything the elements paint.
pub fn bounds(elements: &[Element]) -> Rect {
    let mut box_: Option<Rect> = None;
    for point in elements.iter().flat_map(|e| e.rings.iter().flatten()) {
        let next = Rect::new(point.x, point.y, point.x, point.y);
        box_ = Some(box_.map_or(next, |b: Rect| b.union(next)));
    }
    box_.unwrap_or(Rect::ZERO)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CancelToken;
    use crate::svg::{cells, read};

    fn reading(svg: &str) -> (Reading, Rect) {
        let document = read::read(svg.as_bytes(), &CancelToken::new()).expect("readable");
        let cells = cells::cells(&document.elements, document.frame);
        (sampling(cells, &document.elements, document.frame, &CancelToken::new()).unwrap(), document.frame)
    }

    #[test]
    fn a_cancel_stops_the_painting() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="#c33"/></svg>"##;
        let document = read::read(svg.as_bytes(), &CancelToken::new()).expect("readable");
        let cells = cells::cells(&document.elements, document.frame);
        let cancel = CancelToken::new();
        cancel.cancel();
        assert!(matches!(
            sampling(cells, &document.elements, document.frame, &cancel),
            Err(Error::Cancelled)
        ));
    }

    /// A glaze tints what is beneath it, and the cell is given the colour it
    /// ends up showing.
    #[test]
    fn a_translucent_fill_tints_what_is_beneath() {
        let (reading, _) = reading(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
                <rect x="0" y="0" width="100" height="100" fill="#FFFFFF"/>
                <rect x="0" y="0" width="100" height="50" fill="#0000FF" fill-opacity="0.5"/>
            </svg>"##,
        );
        let mut colors: Vec<Rgba> = reading.cells.iter().map(|c| c.color).collect();
        colors.sort_by(|a, b| a.r.total_cmp(&b.r));
        assert_eq!(colors.len(), 2);
        // Half blue over white: a pale blue, and the bare white.
        assert!(colors[0].r > 0.45 && colors[0].r < 0.55 && colors[0].b > 0.95, "{:?}", colors[0]);
        assert!(colors[1].r > 0.99 && colors[1].b > 0.99, "{:?}", colors[1]);
    }

    /// A gradient is painted, not averaged: each side of a ramp reads its own
    /// colour.
    #[test]
    fn a_gradient_is_painted_not_averaged() {
        let (reading, _) = reading(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
                <defs><linearGradient id="g" x1="0" y1="0" x2="100" y2="0" gradientUnits="userSpaceOnUse">
                    <stop offset="0" stop-color="#000000"/><stop offset="1" stop-color="#FFFFFF"/>
                </linearGradient></defs>
                <rect x="0" y="0" width="40" height="100" fill="url(#g)"/>
                <rect x="60" y="0" width="40" height="100" fill="url(#g)"/>
            </svg>"##,
        );
        // Two cells of one colour key, each sampled where it sits on the ramp.
        let mut reds: Vec<f32> = reading.cells.iter().map(|c| c.color.r).collect();
        reds.sort_by(f32::total_cmp);
        assert_eq!(reds.len(), 2);
        assert!(reds[0] < 0.3, "the left shape is near the ramp's black end: {}", reds[0]);
        assert!(reds[1] > 0.7, "the right shape is near its white end: {}", reds[1]);
    }

    #[test]
    fn the_outline_follows_what_is_painted() {
        let (reading, _) = reading(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
                <circle cx="50" cy="50" r="40" fill="#7040a0"/>
            </svg>"##,
        );
        let outline = reading.outline.expect("an outline");
        let area = path::signed_area(&outline).abs();
        let disc = std::f64::consts::PI * 1600.0;
        assert!((area - disc).abs() < disc * 0.01, "{area} vs {disc}");
    }

    #[test]
    fn two_shapes_that_touch_trace_as_one_outline() {
        let (reading, _) = reading(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
                <rect x="10" y="40" width="45" height="20" fill="#c33"/>
                <rect x="45" y="40" width="45" height="20" fill="#3c3"/>
            </svg>"##,
        );
        let outline = reading.outline.expect("an outline");
        let rings = path::contours(&outline, 0.05);
        assert_eq!(rings.len(), 1, "one outline, with no notch at the seam");
    }
}
