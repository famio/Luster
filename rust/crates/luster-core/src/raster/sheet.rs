//! The scratch raster: a grid over a shape plus some room, drawn on in the
//! art's own units and read back as per-pixel coverage.

use kurbo::{BezPath, PathEl, Rect};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

use crate::geom::stroke::{LineCap, LineJoin};

pub struct Sheet {
    /// The area covered, in the art's units.
    pub frame: Rect,
    /// Pixels per unit.
    pub scale: f64,
    pub width: usize,
    pub height: usize,
}

impl Sheet {
    /// A sheet around `frame` with `room` to spare on every side, whose longer
    /// side is `resolution` pixels.
    pub fn around(frame: Rect, room: f64, resolution: usize) -> Option<Sheet> {
        let frame = frame.inflate(room, room);
        if !(frame.width() > 0.0 && frame.height() > 0.0) {
            return None;
        }
        let scale = resolution as f64 / frame.width().max(frame.height());
        Some(Sheet {
            width: ((frame.width() * scale).round() as usize).max(1),
            height: ((frame.height() * scale).round() as usize).max(1),
            frame,
            scale,
        })
    }

    /// A sheet over exactly this many pixels of `frame`, for callers that
    /// have already decided the grid.
    pub fn exactly(frame: Rect, width: usize, height: usize) -> Option<Sheet> {
        if width == 0 || height == 0 || !(frame.width() > 0.0) {
            return None;
        }
        Some(Sheet { scale: width as f64 / frame.width(), frame, width, height })
    }

    pub fn pixels(&self) -> usize {
        self.width * self.height
    }

    /// Per-pixel coverage (0...1) of what `draw` lays down, rows from the top.
    pub fn coverage(&self, draw: impl FnOnce(&mut Canvas)) -> Vec<f32> {
        let mut canvas = Canvas::new(self);
        draw(&mut canvas);
        canvas.read()
    }

    /// The same, as the bytes it is read from, which `level` reads as
    /// `coverage` does: a quarter of the memory, for coverage that is held
    /// while other work goes on.
    pub fn coverage_bytes(&self, draw: impl FnOnce(&mut Canvas)) -> Vec<u8> {
        let mut canvas = Canvas::new(self);
        draw(&mut canvas);
        canvas.pixmap.pixels().iter().map(|p| p.alpha()).collect()
    }

    /// The coverage of one path, optionally grown by `grown_by` on every side.
    pub fn lay(&self, path: &BezPath, grown_by: f64) -> Vec<f32> {
        self.coverage(|canvas| canvas.lay(path, grown_by))
    }

    /// Traces a coverage field on this sheet back to a path, in the art's
    /// units. `tolerance` is in pixels.
    pub fn outline(&self, field: &[f32], tolerance: f64) -> Option<BezPath> {
        super::trace::outline(field, self.width, self.height, self.frame, 0.5, tolerance)
    }

    /// The same, in straight segments; see `trace::outline_straight`.
    pub fn outline_straight(&self, field: &[f32], tolerance: f64) -> Option<BezPath> {
        super::trace::outline_straight(field, self.width, self.height, self.frame, 0.5, tolerance)
    }
}

/// A sheet being drawn on. Everything is drawn opaque white; only coverage is
/// read back.
pub struct Canvas {
    pixmap: Pixmap,
    transform: Transform,
    paint: Paint<'static>,
}

impl Canvas {
    fn new(sheet: &Sheet) -> Canvas {
        let mut paint = Paint::default();
        paint.anti_alias = true;
        paint.set_color(tiny_skia::Color::WHITE);
        Canvas {
            pixmap: Pixmap::new(sheet.width as u32, sheet.height as u32)
                .expect("a sheet is at least one pixel"),
            // Art units into pixels; the raster is y-down, like the art.
            transform: Transform::from_scale(sheet.scale as f32, sheet.scale as f32)
                .pre_translate(-sheet.frame.x0 as f32, -sheet.frame.y0 as f32),
            paint,
        }
    }

    /// Fills every path at once, adding coverage rather than compositing it.
    ///
    /// Two regions that share an edge each half-cover the pixels along it,
    /// which source-over would composite to three quarters of a pixel and
    /// leave a seam; added, they come to a whole one.
    pub fn add(&mut self, paths: impl IntoIterator<Item = BezPath>) {
        let mut paint = self.paint.clone();
        paint.blend_mode = tiny_skia::BlendMode::Plus;
        for path in paths {
            let Some(skia) = convert(&path) else { continue };
            self.pixmap.fill_path(&skia, &paint, FillRule::Winding, self.transform, None);
        }
    }

    /// Clears a band of `width` centred on `path`: what a stroke would cover,
    /// taken back out again.
    pub fn erase(&mut self, path: &BezPath, width: f64) {
        let Some(skia) = convert(path) else { return };
        let mut paint = self.paint.clone();
        paint.blend_mode = tiny_skia::BlendMode::Clear;
        let stroke = Stroke {
            width: width as f32,
            line_cap: LineCap::Round.into(),
            line_join: LineJoin::Round.into(),
            ..Stroke::default()
        };
        self.pixmap.stroke_path(&skia, &paint, &stroke, self.transform, None);
    }

    /// Fills `path`, then, when `grown_by` is positive, strokes it twice that
    /// wide with round ends and joins: the shape grown by that much.
    pub fn lay(&mut self, path: &BezPath, grown_by: f64) {
        let Some(skia) = convert(path) else { return };
        self.pixmap.fill_path(&skia, &self.paint, FillRule::Winding, self.transform, None);
        if grown_by > 0.0 {
            let stroke = Stroke {
                width: (2.0 * grown_by) as f32,
                line_cap: LineCap::Round.into(),
                line_join: LineJoin::Round.into(),
                ..Stroke::default()
            };
            self.pixmap.stroke_path(&skia, &self.paint, &stroke, self.transform, None);
        }
    }

    fn read(&self) -> Vec<f32> {
        self.pixmap.pixels().iter().map(|p| level(p.alpha())).collect()
    }
}

/// A byte of coverage as a fraction, 0...1.
pub fn level(byte: u8) -> f32 {
    f32::from(byte) / 255.0
}

impl From<LineCap> for tiny_skia::LineCap {
    fn from(cap: LineCap) -> Self {
        match cap {
            LineCap::Butt => tiny_skia::LineCap::Butt,
            LineCap::Round => tiny_skia::LineCap::Round,
            LineCap::Square => tiny_skia::LineCap::Square,
        }
    }
}

impl From<LineJoin> for tiny_skia::LineJoin {
    fn from(join: LineJoin) -> Self {
        match join {
            LineJoin::Miter => tiny_skia::LineJoin::Miter,
            LineJoin::Round => tiny_skia::LineJoin::Round,
            LineJoin::Bevel => tiny_skia::LineJoin::Bevel,
        }
    }
}

/// A kurbo path as tiny-skia's, or `None` when it draws nothing.
fn convert(path: &BezPath) -> Option<tiny_skia::Path> {
    let mut builder = PathBuilder::new();
    let f = |v: f64| v as f32;
    for element in path.elements() {
        match *element {
            PathEl::MoveTo(p) => builder.move_to(f(p.x), f(p.y)),
            PathEl::LineTo(p) => builder.line_to(f(p.x), f(p.y)),
            PathEl::QuadTo(c, p) => builder.quad_to(f(c.x), f(c.y), f(p.x), f(p.y)),
            PathEl::CurveTo(c1, c2, p) => {
                builder.cubic_to(f(c1.x), f(c1.y), f(c2.x), f(c2.y), f(p.x), f(p.y))
            }
            PathEl::ClosePath => builder.close(),
        }
    }
    builder.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disc(radius: f64) -> BezPath {
        kurbo::Shape::to_path(&kurbo::Circle::new((0.0, 0.0), radius), 1e-6)
    }

    #[test]
    fn a_sheet_holds_the_shape_and_its_room() {
        let sheet = Sheet::around(Rect::new(-1.0, -1.0, 1.0, 1.0), 0.5, 100).unwrap();
        assert_eq!((sheet.width, sheet.height), (100, 100));
        assert!((sheet.scale - 100.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn coverage_measures_the_area_drawn() {
        let sheet = Sheet::around(Rect::new(-1.0, -1.0, 1.0, 1.0), 0.1, 512).unwrap();
        let cover = sheet.lay(&disc(1.0), 0.0);
        let painted: f32 = cover.iter().sum();
        let area = f64::from(painted) / (sheet.scale * sheet.scale);
        assert!((area - std::f64::consts::PI).abs() < 0.01, "{area}");
    }

    #[test]
    fn growing_a_shape_widens_it_by_that_much() {
        let sheet = Sheet::around(Rect::new(-1.0, -1.0, 1.0, 1.0), 0.3, 512).unwrap();
        let cover = sheet.lay(&disc(1.0), 0.2);
        let painted: f32 = cover.iter().sum();
        let area = f64::from(painted) / (sheet.scale * sheet.scale);
        let expected = std::f64::consts::PI * 1.2 * 1.2;
        assert!((area - expected).abs() < 0.02, "{area} vs {expected}");
    }

    #[test]
    fn a_shape_drawn_on_a_sheet_traces_back_to_itself() {
        let sheet = Sheet::around(Rect::new(-1.0, -1.0, 1.0, 1.0), 0.1, 512).unwrap();
        let cover = sheet.lay(&disc(1.0), 0.0);
        let traced = sheet.outline(&cover, 0.6).unwrap();
        let area = crate::geom::path::signed_area(&traced).abs();
        assert!((area - std::f64::consts::PI).abs() < 0.02, "{area}");
    }

    #[test]
    fn an_empty_path_draws_nothing() {
        let sheet = Sheet::around(Rect::new(0.0, 0.0, 1.0, 1.0), 0.1, 64).unwrap();
        assert!(sheet.lay(&BezPath::new(), 0.1).iter().all(|&v| v == 0.0));
    }
}
