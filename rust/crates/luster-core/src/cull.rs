//! Finding the triangles of a badge that no view can see.
//!
//! A badge is a stack of extrusions along z: each piece is a footprint swept
//! from a back height to a front height. A face is hidden when every point of
//! it lies strictly inside the solid of *other* pieces. Footprints are
//! rasterized and shrunk by a pixel, so "inside" means inside the geometry as
//! it is exported, and a face that merely touches a neighbour is kept: pieces
//! that share a boundary leave hairline gaps, through which such a face shows.
//!
//! Front caps are always kept: they are the face of the badge.

use crate::geom::path::{self, Contours};
use crate::mesh::{MeshBuilder, VERTEX_STRIDE};
use crate::mint::Piece;
use crate::raster::Sheet;
use rayon::prelude::*;
use crate::{CancelToken, Error};

/// Raster pixels per badge unit.
const RESOLUTION: usize = 2048;
/// Slack when comparing heights, well under the step between enamel cells.
const LEVEL: f32 = 2e-5;

/// The triangles of each piece that some view could see, as index lists in
/// the order the pieces were given. The search is over the whole badge at full
/// resolution; `cancel` is looked at between its stages and between pieces.
pub fn visible(pieces: &[Piece], meshes: &[MeshBuilder], cancel: &CancelToken) -> Result<Vec<Vec<u32>>, Error> {
    let Some(frame) = Frame::around(pieces) else {
        return Ok(meshes.iter().map(|mesh| mesh.indices.clone()).collect());
    };

    // Pieces with the same outline and roll share one footprint: a plate and
    // its reverse are two pieces of one shape.
    let mut shape_of: Vec<usize> = Vec::with_capacity(pieces.len());
    let mut shapes: Vec<&Piece> = Vec::new();
    for piece in pieces {
        let same = |other: &&Piece| {
            other.slab.roll.to_bits() == piece.slab.roll.to_bits() && other.rings == piece.rings
        };
        match shapes.iter().position(same) {
            Some(shape) => shape_of.push(shape),
            None => {
                shape_of.push(shapes.len());
                shapes.push(piece);
            }
        }
    }
    let footprints: Vec<Footprint> = shapes.par_iter().map(|piece| Footprint::new(piece, &frame)).collect();
    cancel.check()?;
    let solids: Vec<Solid> = pieces
        .iter()
        .zip(&shape_of)
        .map(|(piece, &shape)| Solid::new(piece, &footprints[shape]))
        .collect();
    // The stack, lowest piece first so each column grows upward without gaps.
    let mut order: Vec<usize> = (0..solids.len()).collect();
    order.sort_by(|&a, &b| solids[a].bottom.total_cmp(&solids[b].bottom));
    let stack = Stack::new(&frame, order.iter().map(|&index| &solids[index]));
    cancel.check()?;

    // Each triangle is judged against the other pieces on its own.
    let kept = meshes
        .par_iter()
        .enumerate()
        .map(|(index, mesh)| {
            if cancel.is_cancelled() {
                return Vec::new();
            }
            let solid = &solids[index];
            let others: Vec<&Solid> =
                solids.iter().enumerate().filter(|(i, _)| *i != index).map(|(_, s)| s).collect();
            let back_is_buried = stack.is_back_buried(solid);
            // A piece can be most of the badge, so its triangles are shared out
            // too; they are kept in the order they came.
            mesh.indices
                .par_chunks_exact(3)
                .filter(|triangle| {
                    let points = [
                        position(mesh, triangle[0]),
                        position(mesh, triangle[1]),
                        position(mesh, triangle[2]),
                    ];
                    !is_hidden(points, solid, &others, &stack, back_is_buried)
                })
                .flat_map_iter(|triangle| triangle.iter().copied())
                .collect()
        })
        .collect();
    cancel.check()?;
    Ok(kept)
}

fn position(mesh: &MeshBuilder, index: u32) -> [f32; 3] {
    let at = index as usize * VERTEX_STRIDE;
    let f = |k: usize| f32::from_le_bytes(mesh.vertices[at + 4 * k..at + 4 * k + 4].try_into().unwrap());
    [f(0), f(1), f(2)]
}

fn is_hidden(
    points: [[f32; 3]; 3],
    solid: &Solid,
    others: &[&Solid],
    stack: &Stack,
    back_is_buried: bool,
) -> bool {
    let [a, b, c] = points;
    let edge = |p: [f32; 3], q: [f32; 3]| [q[0] - p[0], q[1] - p[1], q[2] - p[2]];
    let (u, v) = (edge(a, b), edge(a, c));
    let cross = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let area = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
    if area <= 0.0 {
        return false;
    }
    let normal_z = cross[2] / area;

    // A front cap is the face of the badge.
    if normal_z > 0.999 {
        return false;
    }
    // A back cap is buried with the rest of its piece's back, or not at all.
    if normal_z < -0.999 {
        return back_is_buried && (a[2] - solid.bottom).abs() <= LEVEL;
    }

    let buried = |point: [f32; 2], low: f32, high: f32| {
        stack.is_solid(point, low, high) && is_inside(others, stack.frame.pixel(point), low, high)
    };
    let pixel = 1.0 / stack.frame.scale;

    if normal_z.abs() < 0.02 {
        // A side wall, sampled along its length over its whole height.
        let low = a[2].min(b[2]).min(c[2]);
        let high = a[2].max(b[2]).max(c[2]);
        let span = |p: [f32; 3], q: [f32; 3]| ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2)).sqrt();
        let ends = [(a, b), (b, c), (c, a)]
            .into_iter()
            .max_by(|x, y| span(x.0, x.1).total_cmp(&span(y.0, y.1)))
            .expect("three edges");
        let length = span(ends.0, ends.1);
        let steps = ((length / (1.5 * pixel)).ceil() as usize).max(2);
        for step in 0..=steps {
            let t = step as f32 / steps as f32;
            let point = [
                ends.0[0] + (ends.1[0] - ends.0[0]) * t,
                ends.0[1] + (ends.1[1] - ends.0[1]) * t,
            ];
            if !buried(point, low, high) {
                return false;
            }
        }
        return true;
    }

    // A sloped face (a roll), sampled across its area.
    let length = |p: [f32; 3], q: [f32; 3]| {
        ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2) + (q[2] - p[2]).powi(2)).sqrt()
    };
    let longest = length(a, b).max(length(b, c)).max(length(c, a));
    let count = ((longest / (1.5 * pixel)).ceil() as usize).clamp(2, 16);
    for i in 0..=count {
        for j in 0..=count - i {
            let (u, v) = (i as f32 / count as f32, j as f32 / count as f32);
            let w = 1.0 - u - v;
            let point = [
                a[0] * w + b[0] * u + c[0] * v,
                a[1] * w + b[1] * u + c[1] * v,
            ];
            let z = a[2] * w + b[2] * u + c[2] * v;
            if !buried(point, z, z) {
                return false;
            }
        }
    }
    true
}

/// Whether the pieces, between them, are solid at a pixel from `low` to
/// `high`. Solid is proven upward: a span that starts at or below what is
/// proven so far, and ends above it, extends the proof.
fn is_inside(pieces: &[&Solid], pixel: (i64, i64), low: f32, high: f32) -> bool {
    let mut proven = low;
    let mut reached = false;
    let mut grew = true;
    while grew {
        grew = false;
        for piece in pieces {
            let Some(top) = piece.top_at(pixel) else { continue };
            if piece.bottom > proven + LEVEL || !(top > proven || (!reached && top >= proven)) {
                continue;
            }
            proven = proven.max(top);
            reached = true;
            grew = true;
        }
        if reached && proven >= high - LEVEL {
            return true;
        }
    }
    false
}

/// One extruded piece, as the hidden-face test sees it.
struct Solid<'a> {
    bottom: f32,
    top: f32,
    /// Height of the roll carved into the front edge.
    drop: f32,
    shape: &'a Footprint,
}

/// Where a piece stands, as seen from above.
struct Footprint {
    /// Where the piece is solid to its full height (inside the roll), and to
    /// the foot of its roll.
    crown: Mask,
    body: Mask,
    /// Every pixel the piece touches, and those next to them.
    reach: Mask,
}

impl Footprint {
    fn new(piece: &Piece, frame: &Frame) -> Footprint {
        let drop = piece.slab.roll;
        // A rolled piece is at full height only inside its roll.
        let ((body, reach), crown) = rayon::join(
            || {
                let body = frame.cover(&piece.rings);
                rayon::join(|| body.shrunk(), || body.grown())
            },
            || (drop > 0.0).then(|| frame.cover(&crate::geom::offset::inset(&piece.rings, f64::from(drop))).shrunk()),
        );
        let crown = crown.unwrap_or_else(|| body.clone());
        Footprint { crown, body, reach }
    }
}

impl<'a> Solid<'a> {
    fn new(piece: &Piece, shape: &'a Footprint) -> Solid<'a> {
        Solid {
            bottom: piece.slab.back.min(piece.slab.front),
            top: piece.slab.front.max(piece.slab.back),
            drop: piece.slab.roll,
            shape,
        }
    }

    /// How high the piece is solid at a pixel, or nothing where it is not.
    fn top_at(&self, pixel: (i64, i64)) -> Option<f32> {
        if self.shape.crown.contains(pixel) {
            return Some(self.top);
        }
        if self.drop > 0.0 && self.shape.body.contains(pixel) {
            return Some(self.top - self.drop);
        }
        None
    }
}

/// Per pixel, the bottom and top of the solid column the pieces build there.
struct Stack<'a> {
    frame: &'a Frame,
    low: Vec<f32>,
    high: Vec<f32>,
}

impl<'a> Stack<'a> {
    /// Pieces arrive lowest first. One that starts above the column so far
    /// floats over a gap and is left out, so a column is solid throughout.
    /// Every column is built on its own, a row of them at a time.
    fn new<'s>(frame: &'a Frame, solids: impl Iterator<Item = &'s Solid<'s>>) -> Stack<'a> {
        let count = frame.width * frame.height;
        let mut low = vec![f32::INFINITY; count];
        let mut high = vec![f32::NEG_INFINITY; count];
        // Each piece raises the foot of its roll, then its crown.
        let mut layers: Vec<(&Mask, f32, f32)> = Vec::new();
        for solid in solids {
            if solid.drop > 0.0 {
                layers.push((&solid.shape.body, solid.bottom, solid.top - solid.drop));
            }
            layers.push((&solid.shape.crown, solid.bottom, solid.top));
        }
        low.par_chunks_mut(frame.width).zip(high.par_chunks_mut(frame.width)).enumerate().for_each(
            |(y, (low, high))| {
                for &(mask, bottom, top) in &layers {
                    let Some(row) = mask.row(y as i64) else { continue };
                    for (dx, &set) in row.iter().enumerate() {
                        if !set {
                            continue;
                        }
                        let x = mask.x0 as usize + dx;
                        if low[x] == f32::INFINITY {
                            low[x] = bottom;
                            high[x] = top;
                        } else if bottom <= high[x] + LEVEL {
                            high[x] = high[x].max(top);
                        }
                    }
                }
            },
        );
        Stack { frame, low, high }
    }

    fn is_solid(&self, point: [f32; 2], bottom: f32, top: f32) -> bool {
        let (x, y) = self.frame.pixel(point);
        if x < 0 || y < 0 || x >= self.frame.width as i64 || y >= self.frame.height as i64 {
            return false;
        }
        let index = y as usize * self.frame.width + x as usize;
        self.low[index] <= bottom + LEVEL && self.high[index] >= top - LEVEL
    }

    /// Whether solid lies directly under every pixel the piece's back touches.
    fn is_back_buried(&self, solid: &Solid) -> bool {
        let under = solid.bottom - 2.0 * LEVEL;
        let mut any = false;
        for (index, set) in solid.shape.reach.pixels(self.frame) {
            if !set {
                continue;
            }
            any = true;
            if !(self.low[index] < under && self.high[index] >= under) {
                return false;
            }
        }
        any
    }
}

/// The raster every footprint is drawn on.
struct Frame {
    origin: [f32; 2],
    scale: f32,
    width: usize,
    height: usize,
}

impl Frame {
    fn around(pieces: &[Piece]) -> Option<Frame> {
        let rings: Contours = pieces.iter().flat_map(|p| p.rings.iter().cloned()).collect();
        let box_ = crate::geom::path::bounds(&rings)?;
        let scale = RESOLUTION as f32;
        let margin = 8.0 / f64::from(scale);
        let box_ = box_.inflate(margin, margin);
        let width = (box_.width() * f64::from(scale)).ceil() as usize;
        let height = (box_.height() * f64::from(scale)).ceil() as usize;
        // A badge's own raster; anything larger is not a badge.
        (width > 0 && height > 0 && width * height <= 1 << 25).then_some(Frame {
            origin: [box_.x0 as f32, box_.y0 as f32],
            scale,
            width,
            height,
        })
    }

    fn pixel(&self, point: [f32; 2]) -> (i64, i64) {
        (
            ((point[0] - self.origin[0]) * self.scale).floor() as i64,
            ((point[1] - self.origin[1]) * self.scale).floor() as i64,
        )
    }

    /// How much of each pixel a footprint covers, cropped to its bounds.
    fn cover(&self, rings: &Contours) -> Coverage {
        let Some(box_) = crate::geom::path::bounds(rings) else { return Coverage::empty() };
        // Room round the edges for the pixel the masks grow by.
        let pad = 3i64;
        let first = self.pixel([box_.x0 as f32, box_.y0 as f32]);
        let last = self.pixel([box_.x1 as f32, box_.y1 as f32]);
        let x0 = (first.0 - pad).max(0);
        let y0 = (first.1 - pad).max(0);
        let width = ((last.0 + pad + 1).min(self.width as i64) - x0).max(0) as usize;
        let height = ((last.1 + pad + 1).min(self.height as i64) - y0).max(0) as usize;
        if width == 0 || height == 0 {
            return Coverage::empty();
        }
        // Drawn on a sheet of its own, in badge units.
        let frame = kurbo::Rect::new(
            f64::from(self.origin[0]) + x0 as f64 / f64::from(self.scale),
            f64::from(self.origin[1]) + y0 as f64 / f64::from(self.scale),
            f64::from(self.origin[0]) + (x0 + width as i64) as f64 / f64::from(self.scale),
            f64::from(self.origin[1]) + (y0 + height as i64) as f64 / f64::from(self.scale),
        );
        let Some(sheet) = Sheet::exactly(frame, width, height) else { return Coverage::empty() };
        let values = sheet.lay(&path::from_contours(rings), 0.0);
        Coverage { x0, y0, width, height, values }
    }
}

/// How much of each pixel a footprint covers, on a crop of the frame.
struct Coverage {
    x0: i64,
    y0: i64,
    width: usize,
    height: usize,
    values: Vec<f32>,
}

impl Coverage {
    fn empty() -> Coverage {
        Coverage { x0: 0, y0: 0, width: 0, height: 0, values: Vec::new() }
    }

    /// The pixels covered all the way, less a pixel all round: strictly inside.
    fn shrunk(&self) -> Mask {
        self.spread(0.98, true)
    }

    /// The pixels covered at all, plus a pixel all round.
    fn grown(&self) -> Mask {
        self.spread(1.0 / 255.0, false)
    }

    /// Thresholds the coverage, then takes each pixel's 3x3 minimum
    /// (shrinking) or maximum. Beyond the crop counts as empty.
    fn spread(&self, threshold: f32, shrinking: bool) -> Mask {
        let (w, h) = (self.width, self.height);
        let mut rows = vec![false; w * h];
        rows.par_chunks_mut(w.max(1)).enumerate().for_each(|(y, row)| {
            let values = &self.values[y * w..(y + 1) * w];
            for (x, set) in row.iter_mut().enumerate() {
                let left = x > 0 && values[x - 1] >= threshold;
                let here = values[x] >= threshold;
                let right = x + 1 < w && values[x + 1] >= threshold;
                *set = if shrinking { left && here && right } else { left || here || right };
            }
        });
        let mut out = vec![false; w * h];
        out.par_chunks_mut(w.max(1)).enumerate().for_each(|(y, row)| {
            let here = &rows[y * w..(y + 1) * w];
            let below = (y > 0).then(|| &rows[(y - 1) * w..y * w]);
            let above = (y + 1 < h).then(|| &rows[(y + 1) * w..(y + 2) * w]);
            for (x, set) in row.iter_mut().enumerate() {
                let below = below.is_some_and(|r| r[x]);
                let above = above.is_some_and(|r| r[x]);
                *set = if shrinking { below && here[x] && above } else { below || here[x] || above };
            }
        });
        Mask { x0: self.x0, y0: self.y0, width: w, height: h, values: out }
    }
}

/// A set of pixels on a crop of the frame.
#[derive(Clone)]
struct Mask {
    x0: i64,
    y0: i64,
    width: usize,
    height: usize,
    values: Vec<bool>,
}

impl Mask {
    fn contains(&self, pixel: (i64, i64)) -> bool {
        let (x, y) = (pixel.0 - self.x0, pixel.1 - self.y0);
        x >= 0
            && y >= 0
            && (x as usize) < self.width
            && (y as usize) < self.height
            && self.values[y as usize * self.width + x as usize]
    }

    /// The mask's pixels on one row of the frame, if it reaches that row.
    fn row(&self, y: i64) -> Option<&[bool]> {
        let y = y - self.y0;
        (y >= 0 && (y as usize) < self.height)
            .then(|| &self.values[y as usize * self.width..(y as usize + 1) * self.width])
    }

    /// Each pixel of the mask with its index in the frame.
    fn pixels<'a>(&'a self, frame: &'a Frame) -> impl Iterator<Item = (usize, bool)> + 'a {
        (0..self.height).flat_map(move |y| {
            (0..self.width).map(move |x| {
                let index = (y + self.y0 as usize) * frame.width + x + self.x0 as usize;
                (index, self.values[y * self.width + x])
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::{Faces, Slab, extrude};
    use crate::mint::Piece;
    use kurbo::Point;

    fn slab(name: &str, x0: f64, y0: f64, x1: f64, y1: f64, back: f32, front: f32) -> Piece {
        Piece {
            name: name.into(),
            material: 0,
            rings: vec![vec![
                Point::new(x0, y0),
                Point::new(x1, y0),
                Point::new(x1, y1),
                Point::new(x0, y1),
            ]],
            slab: Slab::new(front, back, Faces::ALL),
            is_painted: false,
            glaze: false,
            rolled: Vec::new(),
        }
    }

    /// The faces kept of a piece, by which way they look.
    fn kinds(mesh: &MeshBuilder, kept: &[u32]) -> (usize, usize, usize) {
        let (mut front, mut back, mut side) = (0, 0, 0);
        for triangle in kept.chunks_exact(3) {
            let p = [
                position(mesh, triangle[0]),
                position(mesh, triangle[1]),
                position(mesh, triangle[2]),
            ];
            let u = [p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]];
            let v = [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]];
            let z = u[0] * v[1] - u[1] * v[0];
            let area = {
                let c = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], z];
                (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt()
            };
            let normal = z / area;
            if normal > 0.999 {
                front += 1;
            } else if normal < -0.999 {
                back += 1;
            } else {
                side += 1;
            }
        }
        (front, back, side)
    }

    #[test]
    fn only_buried_faces_are_hidden() {
        let pieces = vec![
            slab("plate", 0.0, 0.0, 1.0, 1.0, 0.0, 0.1),
            // Two slabs sunk into the plate, sharing the wall at x = 0.5.
            slab("left", 0.1, 0.1, 0.5, 0.9, 0.05, 0.12),
            slab("right", 0.5, 0.1, 0.9, 0.9, 0.05, 0.12),
            // And one wholly inside the right-hand slab.
            slab("inner", 0.6, 0.3, 0.8, 0.7, 0.06, 0.11),
        ];
        let meshes: Vec<MeshBuilder> = pieces
            .iter()
            .map(|piece| {
                let mut mesh = MeshBuilder::new();
                extrude(&piece.rings, piece.slab, &mut mesh).expect("built");
                mesh
            })
            .collect();
        let cancel = CancelToken::new();
        cancel.cancel();
        assert_eq!(visible(&pieces, &meshes, &cancel).err(), Some(crate::Error::Cancelled));
        let kept = visible(&pieces, &meshes, &CancelToken::new()).unwrap();

        // The plate shows its reverse and its sides; its face is kept, as
        // every front is.
        let base = kinds(&meshes[0], &kept[0]);
        assert_eq!(base, (2, 2, 8), "the plate lost something");

        // Sunk slabs lose their backs. Their walls stay, the shared one
        // included: the neighbour only touches it.
        for index in [1, 2] {
            let (front, back, side) = kinds(&meshes[index], &kept[index]);
            assert_eq!(front, 2, "{}: the face was removed", pieces[index].name);
            assert_eq!(back, 0, "{}: a buried back was kept", pieces[index].name);
            assert_eq!(side, 8, "{}: a wall that only touches a neighbour was removed", pieces[index].name);
        }

        // The slab inside another keeps nothing but its front.
        assert_eq!(kinds(&meshes[3], &kept[3]), (2, 0, 0), "the buried slab kept more than its face");
    }

    /// Removing hidden faces only ever removes: nothing is moved or added,
    /// and every front cap survives.
    #[test]
    fn removing_hidden_faces_only_removes() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
            <circle cx="50" cy="50" r="40" fill="#7040a0" stroke="#302010" stroke-width="3"/>
            <circle cx="50" cy="50" r="20" fill="#60dc00"/>
        </svg>"##;
        let full = crate::mint_with(
            svg.as_bytes(),
            &crate::MintOptions::default(),
            &crate::CancelToken::new(),
        )
        .expect("a badge");
        let lean = crate::mint_with(
            svg.as_bytes(),
            &crate::MintOptions { without_hidden_faces: true, ..crate::MintOptions::default() },
            &crate::CancelToken::new(),
        )
        .expect("a badge");

        // The same submeshes, in the same order, with the same vertices.
        assert_eq!(full.submeshes.len(), lean.submeshes.len());
        let mut removed = 0usize;
        for (all, kept) in full.submeshes.iter().zip(&lean.submeshes) {
            assert_eq!(all.name, kept.name);
            assert_eq!(all.vertices, kept.vertices, "{}: the vertices moved", all.name);
            let triangles = |s: &crate::Submesh| -> Vec<[u32; 3]> {
                s.indices
                    .chunks_exact(12)
                    .map(|t| {
                        let i = |k: usize| u32::from_le_bytes(t[4 * k..4 * k + 4].try_into().unwrap());
                        [i(0), i(1), i(2)]
                    })
                    .collect()
            };
            let (all, kept) = (triangles(all), triangles(kept));
            for triangle in &kept {
                assert!(all.contains(triangle), "the lean badge has a triangle the full one lacks");
            }
            removed += all.len() - kept.len();
        }
        assert!(removed > 0, "nothing was removed: the edge's line runs on under the wall");
    }
}
