//! Walking a usvg tree into the regions a document paints.
//!
//! A stroke remembers where its ends are, because a line that stops on a fill
//! of its own colour is treated differently from one that runs across it.

use kurbo::{Affine, BezPath, Point, Rect};

use crate::geom::boolean::{self, Rule};
use crate::geom::path::{self, Contours};
use crate::geom::stroke::{self, LineCap, LineJoin};
use crate::{CancelToken, Error, model::Rgba, svg::paint::{Gradient, Paint, Spread, Stop}};

/// Limits for untrusted input.
pub const MAX_BYTES: usize = 20 << 20;
pub const MAX_ELEMENTS: usize = 5_000;

/// Where a stroke ends, and how wide it is there.
#[derive(Clone, Copy, Debug)]
pub struct Cap {
    pub point: Point,
    pub radius: f64,
}

/// One region the document paints, in the document's units.
#[derive(Clone, Debug)]
pub struct Element {
    /// Disjoint rings: outers wound one way, holes the other.
    pub rings: Contours,
    /// What paints it, with every opacity already folded into the alpha.
    pub paint: Option<Paint>,
    /// The flat colour that stands in for that paint while cells are built.
    pub color: Option<Rgba>,
    /// A stroke, which the badge raises as metal rather than filling as enamel.
    pub is_stroke: bool,
    pub ends: Vec<Cap>,
}

/// What a document paints, in paint order.
#[derive(Clone, Debug)]
pub struct Document {
    pub elements: Vec<Element>,
    /// The canvas the art was placed in: the viewBox, as usvg resolves it.
    pub frame: Rect,
}

impl Document {
    /// Flattening tolerance for this document, the same fraction of the canvas
    /// whatever units it is drawn in.
    pub fn tolerance(&self) -> f64 {
        self.frame.width().max(self.frame.height()) * path::TOLERANCE
    }
}

pub fn read(data: &[u8], cancel: &CancelToken) -> Result<Document, Error> {
    if data.len() > MAX_BYTES {
        return Err(Error::InputTooComplex(format!("{} bytes", data.len())));
    }
    // No text: usvg is built without it (Cargo.toml), which keeps the font
    // machinery and its megabytes out of the engine.
    let options = usvg::Options { ..usvg::Options::default() };
    let tree = usvg::Tree::from_data(data, &options).map_err(|e| Error::InvalidSvg(e.to_string()))?;
    cancel.check()?;

    let size = tree.size();
    let frame = Rect::new(0.0, 0.0, f64::from(size.width()), f64::from(size.height()));
    let tolerance = frame.width().max(frame.height()) * path::TOLERANCE;
    let mut reader = Reader { elements: Vec::new(), tolerance, cancel };
    reader.group(tree.root(), 1.0, None)?;
    Ok(Document { elements: reader.elements, frame })
}

struct Reader<'a> {
    elements: Vec<Element>,
    tolerance: f64,
    cancel: &'a CancelToken,
}

impl Reader<'_> {
    fn group(&mut self, group: &usvg::Group, opacity: f32, clip: Option<&Contours>) -> Result<(), Error> {
        let opacity = opacity * group.opacity().get();
        // A group's clip and mask apply to everything inside it, on top of
        // whatever its parents already cut it by.
        let mut region = clip.cloned();
        if let Some(path) = group.clip_path() {
            region = Some(self.narrow(region, self.clip_region(path)));
        }
        if let Some(mask) = group.mask() {
            region = Some(self.narrow(region, self.mask_region(mask)));
        }

        for node in group.children() {
            self.cancel.check()?;
            match node {
                usvg::Node::Group(inner) => self.group(inner, opacity, region.as_ref())?,
                usvg::Node::Path(path) => self.path(path, opacity, region.as_ref())?,
                // Text is resolved away by usvg; a lone image paints nothing a
                // badge can be struck from. One inside a mask is traced in
                // `contents`.
                usvg::Node::Image(_) | usvg::Node::Text(_) => {}
            }
        }
        Ok(())
    }

    fn path(&mut self, path: &usvg::Path, opacity: f32, clip: Option<&Contours>) -> Result<(), Error> {
        if !path.is_visible() {
            return Ok(());
        }
        let transform = convert_transform(path.abs_transform());
        if let Some(fill) = path.fill() {
            let rule = match fill.rule() {
                usvg::FillRule::EvenOdd => Rule::EvenOdd,
                usvg::FillRule::NonZero => Rule::NonZero,
            };
            let mut shape = convert_path(path.data());
            shape.apply_affine(transform);
            let rings = boolean::simplify(&path::contours(&shape, self.tolerance), rule);
            self.emit(rings, fill.paint(), opacity * fill.opacity().get(), false, Vec::new(), clip)?;
        }
        if let Some(stroke) = path.stroke() {
            // Stroked in local units, so the pen scales with the transform.
            let local = convert_path(path.data());
            let mut outline = stroke::outline(
                &local,
                f64::from(stroke.width().get()),
                match stroke.linecap() {
                    usvg::LineCap::Butt => LineCap::Butt,
                    usvg::LineCap::Round => LineCap::Round,
                    usvg::LineCap::Square => LineCap::Square,
                },
                match stroke.linejoin() {
                    usvg::LineJoin::Round => LineJoin::Round,
                    usvg::LineJoin::Bevel => LineJoin::Bevel,
                    _ => LineJoin::Miter,
                },
                f64::from(stroke.miterlimit().get()),
            );
            outline.apply_affine(transform);
            let rings = boolean::simplify(&path::contours(&outline, self.tolerance), Rule::NonZero);
            // The cap's radius in document units: half the width, scaled as the
            // transform scales area.
            let radius = f64::from(stroke.width().get()) / 2.0 * transform.determinant().abs().sqrt();
            let ends = ends(&local)
                .into_iter()
                .map(|point| Cap { point: transform * point, radius })
                .collect();
            self.emit(rings, stroke.paint(), opacity * stroke.opacity().get(), true, ends, clip)?;
        }
        Ok(())
    }

    fn emit(
        &mut self,
        rings: Contours,
        paint: &usvg::Paint,
        opacity: f32,
        is_stroke: bool,
        ends: Vec<Cap>,
        clip: Option<&Contours>,
    ) -> Result<(), Error> {
        let rings = match clip {
            Some(clip) => boolean::intersection(&rings, clip, Rule::NonZero),
            None => rings,
        };
        if rings.is_empty() {
            return Ok(());
        }
        let paint = convert_paint(paint).map(|p| p.faded(opacity));
        let color = paint.as_ref().and_then(Paint::flat);
        self.elements.push(Element { rings, paint, color, is_stroke, ends });
        if self.elements.len() > MAX_ELEMENTS {
            return Err(Error::InputTooComplex(format!("over {MAX_ELEMENTS} painted shapes")));
        }
        Ok(())
    }

    /// Both regions at once, or the new one when there was none.
    fn narrow(&self, region: Option<Contours>, next: Contours) -> Contours {
        match region {
            Some(region) => boolean::intersection(&region, &next, Rule::NonZero),
            None => next,
        }
    }

    /// A clipPath as a region: everything its contents cover.
    fn clip_region(&self, clip: &usvg::ClipPath) -> Contours {
        let mut region = self.contents(clip.root());
        if let Some(inner) = clip.clip_path() {
            region = boolean::intersection(&region, &self.clip_region(inner), Rule::NonZero);
        }
        region
    }

    /// A mask as a region: what its contents draw, limited to its box. A
    /// luminance mask shows what it draws, not its whole declared box, so the
    /// contents win when there are any.
    fn mask_region(&self, mask: &usvg::Mask) -> Contours {
        let rect = mask.rect();
        let box_ = rectangle(Rect::new(
            f64::from(rect.x()),
            f64::from(rect.y()),
            f64::from(rect.x() + rect.width()),
            f64::from(rect.y() + rect.height()),
        ));
        let contents = self.contents(mask.root());
        let mut region = if contents.is_empty() {
            box_
        } else {
            boolean::intersection(&contents, &box_, Rule::NonZero)
        };
        if let Some(inner) = mask.mask() {
            region = boolean::intersection(&region, &self.mask_region(inner), Rule::NonZero);
        }
        region
    }

    /// The bright part of an embedded image, as a region.
    fn image_region(&self, image: &usvg::Image) -> Option<Contours> {
        if !image.is_visible() {
            return None;
        }
        let (luminance, width, height) = sample(image.kind())?;
        // The image occupies its own size, placed by its transform.
        let size = image.size();
        let frame = Rect::new(0.0, 0.0, f64::from(size.width()), f64::from(size.height()));
        let mut traced = crate::raster::trace::image(
            &luminance,
            width,
            height,
            frame,
            crate::raster::trace::IMAGE_TOLERANCE,
        )?;
        traced.apply_affine(convert_transform(image.abs_transform()));
        // Nesting means holes, not winding: the tracer says nothing about it.
        Some(boolean::simplify(&path::contours(&traced, self.tolerance), Rule::EvenOdd))
    }

    /// The union of everything a group's paths cover, fills and strokes alike.
    fn contents(&self, group: &usvg::Group) -> Contours {
        let mut region: Contours = Vec::new();
        for node in group.children() {
            let next = match node {
                usvg::Node::Group(inner) => self.contents(inner),
                usvg::Node::Path(path) => {
                    let transform = convert_transform(path.abs_transform());
                    let mut shape = convert_path(path.data());
                    shape.apply_affine(transform);
                    let rule = match path.fill().map(usvg::Fill::rule) {
                        Some(usvg::FillRule::EvenOdd) => Rule::EvenOdd,
                        _ => Rule::NonZero,
                    };
                    boolean::simplify(&path::contours(&shape, self.tolerance), rule)
                }
                // A design tool may export a vector mask as an embedded
                // image: what it shows is traced back into a region.
                usvg::Node::Image(image) => match self.image_region(image) {
                    Some(region) => region,
                    None => continue,
                },
                usvg::Node::Text(_) => continue,
            };
            region = if region.is_empty() { next } else { boolean::union(&region, &next, Rule::NonZero) };
        }
        region
    }
}

/// Where a path's open subpaths start and stop. A closed subpath has no ends.
fn ends(path: &BezPath) -> Vec<Point> {
    use kurbo::PathEl;
    let mut out = Vec::new();
    let (mut start, mut last): (Option<Point>, Option<Point>) = (None, None);
    let finish = |start: &mut Option<Point>, last: &mut Option<Point>, out: &mut Vec<Point>| {
        if let (Some(a), Some(b)) = (*start, *last) {
            if a != b {
                out.push(a);
                out.push(b);
            }
        }
        *start = None;
        *last = None;
    };
    for element in path.elements() {
        match *element {
            PathEl::MoveTo(p) => {
                finish(&mut start, &mut last, &mut out);
                start = Some(p);
                last = Some(p);
            }
            PathEl::LineTo(p) | PathEl::QuadTo(_, p) | PathEl::CurveTo(_, _, p) => last = Some(p),
            PathEl::ClosePath => {
                start = None;
                last = None;
            }
        }
    }
    finish(&mut start, &mut last, &mut out);
    out
}

/// Decodes an image and samples its luminance, gated by alpha: a transparent
/// pixel reads as black, which is outside the mask. The sample grid is at most
/// `IMAGE_RESOLUTION` on its longer side, with an empty border, so that a shape
/// reaching the image's edge still traces as a closed region.
fn sample(kind: &usvg::ImageKind) -> Option<(Vec<f32>, usize, usize)> {
    let data = match kind {
        usvg::ImageKind::PNG(data) | usvg::ImageKind::JPEG(data) | usvg::ImageKind::GIF(data) => {
            data.as_ref()
        }
        // WebP is not compiled in, and a nested SVG is read as a tree, not an
        // image; neither has appeared in badge artwork.
        usvg::ImageKind::WEBP(_) | usvg::ImageKind::SVG(_) => return None,
    };
    let decoded = image::load_from_memory(data).ok()?.into_rgba8();
    let (source_width, source_height) = (decoded.width() as usize, decoded.height() as usize);
    if source_width == 0 || source_height == 0 {
        return None;
    }
    let resolution = crate::raster::trace::IMAGE_RESOLUTION;
    let longest = source_width.max(source_height);
    let scale = (resolution as f64 / longest as f64).min(1.0);
    let width = ((source_width as f64 * scale).round() as usize).max(2);
    let height = ((source_height as f64 * scale).round() as usize).max(2);

    let mut out = vec![0.0f32; width * height];
    for y in 0..height {
        for x in 0..width {
            // Nearest sample: the mask is a shape, not a photograph.
            let sx = ((x as f64 + 0.5) / width as f64 * source_width as f64) as u32;
            let sy = ((y as f64 + 0.5) / height as f64 * source_height as f64) as u32;
            let pixel = decoded.get_pixel(sx.min(decoded.width() - 1), sy.min(decoded.height() - 1));
            let alpha = f32::from(pixel[3]) / 255.0;
            let luma = (0.2126 * f32::from(pixel[0])
                + 0.7152 * f32::from(pixel[1])
                + 0.0722 * f32::from(pixel[2]))
                / 255.0;
            out[y * width + x] = luma * alpha;
        }
    }
    Some((out, width, height))
}

fn rectangle(rect: Rect) -> Contours {
    vec![vec![
        Point::new(rect.x0, rect.y0),
        Point::new(rect.x1, rect.y0),
        Point::new(rect.x1, rect.y1),
        Point::new(rect.x0, rect.y1),
    ]]
}

fn convert_transform(t: usvg::Transform) -> Affine {
    Affine::new([
        f64::from(t.sx),
        f64::from(t.ky),
        f64::from(t.kx),
        f64::from(t.sy),
        f64::from(t.tx),
        f64::from(t.ty),
    ])
}

fn convert_path(path: &usvg::tiny_skia_path::Path) -> BezPath {
    use usvg::tiny_skia_path::PathSegment;
    let mut out = BezPath::new();
    let p = |p: usvg::tiny_skia_path::Point| Point::new(f64::from(p.x), f64::from(p.y));
    for segment in path.segments() {
        match segment {
            PathSegment::MoveTo(a) => out.move_to(p(a)),
            PathSegment::LineTo(a) => out.line_to(p(a)),
            PathSegment::QuadTo(c, a) => out.quad_to(p(c), p(a)),
            PathSegment::CubicTo(c1, c2, a) => out.curve_to(p(c1), p(c2), p(a)),
            PathSegment::Close => out.close_path(),
        }
    }
    out
}

fn convert_paint(paint: &usvg::Paint) -> Option<Paint> {
    match paint {
        usvg::Paint::Color(c) => Some(Paint::Solid(Rgba {
            r: f32::from(c.red) / 255.0,
            g: f32::from(c.green) / 255.0,
            b: f32::from(c.blue) / 255.0,
            a: 1.0,
        })),
        usvg::Paint::LinearGradient(g) => Some(Paint::Gradient(Box::new(Gradient::Linear {
            from: (f64::from(g.x1()), f64::from(g.y1())),
            to: (f64::from(g.x2()), f64::from(g.y2())),
            stops: stops(g.stops()),
            transform: convert_transform(g.transform()),
            spread: spread(g.spread_method()),
        }))),
        usvg::Paint::RadialGradient(g) => Some(Paint::Gradient(Box::new(Gradient::Radial {
            centre: (f64::from(g.cx()), f64::from(g.cy())),
            radius: f64::from(g.r().get()),
            focus: (f64::from(g.fx()), f64::from(g.fy())),
            stops: stops(g.stops()),
            transform: convert_transform(g.transform()),
            spread: spread(g.spread_method()),
        }))),
        // A pattern is artwork of its own; the badge does not read one.
        usvg::Paint::Pattern(_) => None,
    }
}

fn stops(stops: &[usvg::Stop]) -> Vec<Stop> {
    stops
        .iter()
        .map(|s| Stop {
            offset: s.offset().get(),
            color: Rgba {
                r: f32::from(s.color().red) / 255.0,
                g: f32::from(s.color().green) / 255.0,
                b: f32::from(s.color().blue) / 255.0,
                a: s.opacity().get(),
            },
        })
        .collect()
}

fn spread(method: usvg::SpreadMethod) -> Spread {
    match method {
        usvg::SpreadMethod::Pad => Spread::Pad,
        usvg::SpreadMethod::Reflect => Spread::Reflect,
        usvg::SpreadMethod::Repeat => Spread::Repeat,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::path::{from_contours, signed_area};
    use kurbo::Shape;

    fn read_str(svg: &str) -> Document {
        read(svg.as_bytes(), &CancelToken::new()).expect("readable")
    }

    fn area(element: &Element) -> f64 {
        signed_area(&from_contours(&element.rings)).abs()
    }

    #[test]
    fn a_fill_and_a_stroke_are_separate_regions() {
        let document = read_str(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
                <rect x="10" y="10" width="80" height="80" fill="#c33" stroke="#000" stroke-width="4"/>
            </svg>"##,
        );
        assert_eq!(document.elements.len(), 2);
        assert!(!document.elements[0].is_stroke);
        assert!(document.elements[1].is_stroke);
        assert!((area(&document.elements[0]) - 6400.0).abs() < 1.0);
    }

    #[test]
    fn the_frame_is_the_view_box() {
        let document = read_str(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 20"><rect width="10" height="10" fill="#000"/></svg>"##,
        );
        assert_eq!((document.frame.width(), document.frame.height()), (40.0, 20.0));
    }

    #[test]
    fn a_clip_path_cuts_what_it_contains_and_paints_nothing_itself() {
        let document = read_str(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
                <defs><clipPath id="c"><rect x="0" y="0" width="50" height="100"/></clipPath></defs>
                <g clip-path="url(#c)"><rect x="0" y="0" width="100" height="100" fill="#000"/></g>
            </svg>"##,
        );
        assert_eq!(document.elements.len(), 1, "the clip itself paints nothing");
        assert!((area(&document.elements[0]) - 5000.0).abs() < 1.0);
    }

    #[test]
    fn a_mask_is_its_contents_limited_to_its_box() {
        let document = read_str(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
                <mask id="m"><circle cx="50" cy="50" r="25" fill="white"/></mask>
                <g mask="url(#m)"><rect width="100" height="100" fill="#000"/></g>
            </svg>"##,
        );
        let area = area(&document.elements[0]);
        let disc = std::f64::consts::PI * 625.0;
        assert!((area - disc).abs() < disc * 0.01, "{area} vs {disc}");
    }

    #[test]
    fn a_translucent_fill_keeps_its_alpha() {
        let document = read_str(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
                <rect width="100" height="100" fill="#0000ff" fill-opacity="0.5"/>
            </svg>"##,
        );
        let color = document.elements[0].color.unwrap();
        assert!((color.a - 0.5).abs() < 0.01, "{}", color.a);
        assert!(color.b > 0.9 && color.r < 0.1);
    }

    #[test]
    fn a_gradient_stands_in_as_its_average() {
        let document = read_str(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
                <defs><linearGradient id="g" x1="0" y1="0" x2="100" y2="0" gradientUnits="userSpaceOnUse">
                    <stop offset="0" stop-color="#000000"/><stop offset="1" stop-color="#ffffff"/>
                </linearGradient></defs>
                <rect width="100" height="100" fill="url(#g)"/>
            </svg>"##,
        );
        let color = document.elements[0].color.unwrap();
        assert!((color.r - 0.5).abs() < 0.02, "{}", color.r);
        assert!(matches!(document.elements[0].paint, Some(Paint::Gradient(_))));
    }

    #[test]
    fn an_open_stroke_reports_both_of_its_ends() {
        let document = read_str(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
                <path d="M10,50 L90,50" fill="none" stroke="#000" stroke-width="6"/>
            </svg>"##,
        );
        let ends = &document.elements[0].ends;
        assert_eq!(ends.len(), 2);
        assert!((ends[0].radius - 3.0).abs() < 1e-6);
        assert!((ends[0].point.x - 10.0).abs() < 1e-6 && (ends[1].point.x - 90.0).abs() < 1e-6);
    }

    #[test]
    fn a_closed_stroke_has_no_ends() {
        let document = read_str(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
                <circle cx="50" cy="50" r="30" fill="none" stroke="#000" stroke-width="6"/>
            </svg>"##,
        );
        assert!(document.elements[0].ends.is_empty());
    }

    #[test]
    fn nested_transforms_compose() {
        let document = read_str(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
                <g transform="translate(40,40)"><g transform="scale(2)">
                    <path d="M0,0 H10 V10 H0 Z" fill="#000"/>
                </g></g>
            </svg>"##,
        );
        let bounds = from_contours(&document.elements[0].rings).bounding_box();
        assert!((bounds.x0 - 40.0).abs() < 1e-6 && (bounds.width() - 20.0).abs() < 1e-6, "{bounds:?}");
    }
}
