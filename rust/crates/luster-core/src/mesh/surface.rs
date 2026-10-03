//! The parts every piece is built from: capped faces, walls, and the bands
//! between two outlines.

use std::collections::HashMap;
use kurbo::Point;
use lyon_tessellation::path::{FillRule, Path};
use lyon_tessellation::{BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers};
use crate::geom::path::Contours;
use crate::Error;
use crate::math;

use super::*;

/// A region's triangles, each turning counter-clockwise seen from the front.
pub(super) fn tessellate(rings: &Contours, rule: FillRule) -> Result<(Vec<Point>, Vec<[u32; 3]>), Error> {
    let path = tessellator_path(rings);
    let mut buffers: VertexBuffers<lyon_tessellation::math::Point, u32> = VertexBuffers::new();
    FillTessellator::new()
        .tessellate_path(
            &path,
            &FillOptions::tolerance(0.0005).with_fill_rule(rule),
            &mut BuffersBuilder::new(&mut buffers, |v: FillVertex| v.position()),
        )
        .map_err(|e| Error::InvalidSvg(format!("tessellation failed: {e:?}")))?;
    let points = buffers.vertices.iter().map(|p| Point::new(f64::from(p.x), f64::from(p.y))).collect();
    let triangles = buffers
        .indices
        .chunks_exact(3)
        .map(|t| {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| buffers.vertices[i as usize]);
            if (b - a).cross(c - a) > 0.0 { [t[0], t[1], t[2]] } else { [t[0], t[2], t[1]] }
        })
        .collect();
    Ok((points, triangles))
}

/// A region's outline, bucketed so that only the edges near a point are
/// asked how far it is from them.
pub(super) struct Edges {
    cell: f64,
    buckets: HashMap<(i64, i64), Vec<(Point, Point)>>,
    reach: f64,
}

impl Edges {
    pub(super) fn new(rings: &Contours, reach: f64) -> Edges {
        let cell = reach.max(1e-6);
        let mut buckets: HashMap<(i64, i64), Vec<(Point, Point)>> = HashMap::new();
        for ring in rings {
            for (a, b) in ring.iter().zip(ring.iter().cycle().skip(1)) {
                let (x0, x1) = ((a.x.min(b.x) / cell).floor() as i64, (a.x.max(b.x) / cell).floor() as i64);
                let (y0, y1) = ((a.y.min(b.y) / cell).floor() as i64, (a.y.max(b.y) / cell).floor() as i64);
                for x in x0..=x1 {
                    for y in y0..=y1 {
                        buckets.entry((x, y)).or_default().push((*a, *b));
                    }
                }
            }
        }
        Edges { cell, buckets, reach }
    }

    /// How far `p` is from the outline, up to the reach, and the way out of
    /// the material towards it there.
    pub(super) fn nearest(&self, p: Point) -> (f64, (f64, f64)) {
        let (cx, cy) = ((p.x / self.cell).floor() as i64, (p.y / self.cell).floor() as i64);
        let mut best = (self.reach, (0.0, 0.0));
        for x in cx - 1..=cx + 1 {
            for y in cy - 1..=cy + 1 {
                for (a, b) in self.buckets.get(&(x, y)).into_iter().flatten() {
                    let (dx, dy) = (b.x - a.x, b.y - a.y);
                    let length = dx * dx + dy * dy;
                    let t = if length > 0.0 { (((p.x - a.x) * dx + (p.y - a.y) * dy) / length).clamp(0.0, 1.0) } else { 0.0 };
                    let (qx, qy) = (a.x + t * dx - p.x, a.y + t * dy - p.y);
                    let distance = math::hypot(qx, qy);
                    if distance < best.0 {
                        // On the outline itself the way out is the edge's own:
                        // the material is on the left of the way a ring runs.
                        let way = if distance > 1e-9 {
                            (qx / distance, qy / distance)
                        } else {
                            let len = length.sqrt().max(1e-12);
                            (dy / len, -dx / len)
                        };
                        best = (distance, way);
                    }
                }
            }
        }
        best
    }
}

/// Fills a region as one cap, facing front or back.
pub(super) fn cap(
    rings: &Contours,
    z: f32,
    front: bool,
    min: [f32; 2],
    size: [f32; 2],
    out: &mut MeshBuilder,
) -> Result<(), Error> {
    let path = tessellator_path(rings);
    let mut buffers: VertexBuffers<lyon_tessellation::math::Point, u32> = VertexBuffers::new();
    FillTessellator::new()
        .tessellate_path(
            &path,
            &FillOptions::tolerance(0.0005).with_fill_rule(FillRule::NonZero),
            &mut BuffersBuilder::new(&mut buffers, |v: FillVertex| v.position()),
        )
        .map_err(|e| Error::InvalidSvg(format!("tessellation failed: {e:?}")))?;

    let base = out.vertex_count();
    for p in &buffers.vertices {
        let u = (p.x - min[0]) / size[0];
        let v = (min[1] + size[1] - p.y) / size[1];
        if front {
            out.push([p.x, p.y, z], [0.0, 0.0, 1.0], [u, v], [1.0, 0.0, 0.0, 1.0]);
        } else {
            out.push([p.x, p.y, z], [0.0, 0.0, -1.0], [1.0 - u, v], [-1.0, 0.0, 0.0, 1.0]);
        }
    }
    for tri in buffers.indices.chunks_exact(3) {
        let [a, b, c] = [tri[0], tri[1], tri[2]].map(|i| buffers.vertices[i as usize]);
        // Counter-clockwise seen from the side the face looks at.
        let ccw = (b - a).cross(c - a) > 0.0;
        let (i, j) = if ccw == front { (tri[1], tri[2]) } else { (tri[2], tri[1]) };
        out.indices.extend([base + tri[0], base + i, base + j]);
    }
    Ok(())
}

/// The rings as a path the tessellator takes. Rings are disjoint and already
/// wound (outers one way, holes the other), so the non-zero rule fills them.
pub fn tessellator_path(rings: &Contours) -> Path {
    let mut builder = Path::builder();
    for ring in rings {
        let Some((first, rest)) = ring.split_first() else { continue };
        let point = |p: &Point| lyon_tessellation::math::point(p.x as f32, p.y as f32);
        builder.begin(point(first));
        for p in rest {
            builder.line_to(point(p));
        }
        builder.end(true);
    }
    builder.build()
}

pub(super) fn bounds(rings: &Contours) -> ([f32; 2], [f32; 2]) {
    let mut min = [f32::MAX; 2];
    let mut max = [f32::MIN; 2];
    for p in rings.iter().flatten() {
        min = [min[0].min(p.x as f32), min[1].min(p.y as f32)];
        max = [max[0].max(p.x as f32), max[1].max(p.y as f32)];
    }
    (min, max)
}

/// Whether `p` is inside the rings, by winding.
pub(super) fn inside(p: Point, rings: &Contours) -> bool {
    crate::geom::path::inside(p, rings, false)
}

/// The wall of one ring, from `front` down to `back`.
pub(super) fn side(contour: &[Point], all: &Contours, front: f32, back: f32, out: &mut MeshBuilder) {
    let n = contour.len();
    if n < 3 {
        return;
    }
    // Right-hand normal of each edge; flipped when it points into the solid.
    let mut normals: Vec<[f32; 2]> = (0..n)
        .map(|i| {
            let d = contour[(i + 1) % n] - contour[i];
            let len = math::length(d).max(1e-12);
            [(d.y / len) as f32, (-d.x / len) as f32]
        })
        .collect();
    let longest = (0..n)
        .max_by(|&a, &b| {
            let la = (contour[(a + 1) % n] - contour[a]).hypot2();
            let lb = (contour[(b + 1) % n] - contour[b]).hypot2();
            la.total_cmp(&lb)
        })
        .unwrap_or(0);
    let mid = contour[longest].lerp(contour[(longest + 1) % n], 0.5);
    let probe = Point::new(
        mid.x + f64::from(normals[longest][0]) * 1e-4,
        mid.y + f64::from(normals[longest][1]) * 1e-4,
    );
    if inside(probe, all) {
        for v in &mut normals {
            *v = [-v[0], -v[1]];
        }
    }

    let turn = |a: [f32; 2], b: [f32; 2]| math::acos((a[0] * b[0] + a[1] * b[1]).clamp(-1.0, 1.0));
    let blend = |a: [f32; 2], b: [f32; 2]| {
        let s = [a[0] + b[0], a[1] + b[1]];
        let len = (s[0] * s[0] + s[1] * s[1]).sqrt().max(1e-12);
        [s[0] / len, s[1] / len]
    };
    let perimeter: f32 =
        (0..n).map(|i| math::length(contour[(i + 1) % n] - contour[i]) as f32).sum();
    let mut along = 0.0f32;

    // A vertex is shaded from the edges either side of it that have length
    // enough to show, passing over any scrap between them: a boolean leaves a
    // few hundred-thousandths of an edge at a vertex, and blending with those
    // would put the whole turn there, leaving each real edge lit flat.
    let length = |i: usize| math::length(contour[(i + 1) % n] - contour[i]);
    let shown = |i: usize| length(i) >= SHORT_EDGE;
    let before = |vertex: usize| {
        (1..=n).map(|k| (vertex + n - k) % n).find(|&i| shown(i)).unwrap_or((vertex + n - 1) % n)
    };
    let after = |vertex: usize| (0..n).map(|k| (vertex + k) % n).find(|&i| shown(i)).unwrap_or(vertex);
    // The normal at `vertex`, on the face of `edge`.
    let at = |vertex: usize, edge: usize| {
        let (prev, next) = (normals[before(vertex)], normals[after(vertex)]);
        if turn(prev, next) < SMOOTH_TURN { blend(prev, next) } else { normals[edge] }
    };

    for i in 0..n {
        let (a, b) = (contour[i], contour[(i + 1) % n]);
        let here = normals[i];
        let (na, nb) = (at(i, i), at((i + 1) % n, i));
        let d = b - a;
        let len = math::length(d).max(1e-12) as f32;
        let t = [(d.x as f32) / len, (d.y as f32) / len, 0.0, 1.0];
        let (u0, u1) = (along / perimeter, (along + len) / perimeter);
        along += len;

        let base = out.vertex_count();
        out.push([a.x as f32, a.y as f32, front], [na[0], na[1], 0.0], [u0, 0.0], t);
        out.push([b.x as f32, b.y as f32, front], [nb[0], nb[1], 0.0], [u1, 0.0], t);
        out.push([b.x as f32, b.y as f32, back], [nb[0], nb[1], 0.0], [u1, 1.0], t);
        out.push([a.x as f32, a.y as f32, back], [na[0], na[1], 0.0], [u0, 1.0], t);
        // Wind the quad to face along its normal.
        let face = [d.y as f32, -(d.x as f32)];
        if face[0] * here[0] + face[1] * here[1] > 0.0 {
            out.indices.extend([base, base + 2, base + 1, base, base + 3, base + 2]);
        } else {
            out.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
}

/// Whether the triangle (a, b, c) turns the way `normal` points.
pub(super) fn facing(a: [f32; 3], b: [f32; 3], c: [f32; 3], normal: [f32; 3]) -> bool {
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let turn = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    turn[0] * normal[0] + turn[1] * normal[1] + turn[2] * normal[2] > 0.0
}

/// One step of a roll: the band between two rings of the same shape, the
/// second inset and lower than the first.
pub(super) fn band(from: &Contours, from_z: f32, to: &Contours, to_z: f32, out: &mut MeshBuilder) {
    let turn = |a: [f32; 2], b: [f32; 2]| math::acos((a[0] * b[0] + a[1] * b[1]).clamp(-1.0, 1.0));
    let blend = |a: [f32; 2], b: [f32; 2]| {
        let s = [a[0] + b[0], a[1] + b[1]];
        let len = (s[0] * s[0] + s[1] * s[1]).sqrt().max(1e-12);
        [s[0] / len, s[1] / len]
    };
    let rise = to_z - from_z;
    for (outer, inner) in from.iter().zip(to) {
        let n = outer.len();
        if n < 3 || inner.len() != n {
            continue;
        }
        // Each edge's way out of the material, across the edge.
        let outward: Vec<[f32; 2]> = (0..n)
            .map(|i| {
                let j = (i + 1) % n;
                let edge = outer[j] - outer[i];
                let len = math::length(edge).max(1e-12);
                let across = [(edge.y / len) as f32, (-edge.x / len) as f32];
                let towards = outer[i] - inner[j];
                if f64::from(across[0]) * towards.x + f64::from(across[1]) * towards.y < 0.0 {
                    [-across[0], -across[1]]
                } else {
                    across
                }
            })
            .collect();
        // Shared across a vertex where the outline turns gently, as a side
        // wall's are: flat per edge, a curve shows every edge it was cut into.
        // A sharper corner keeps its crease, each face its own normal.
        let smooth = |vertex: usize| -> Option<[f32; 2]> {
            let (before, after) = (outward[(vertex + n - 1) % n], outward[vertex]);
            (turn(before, after) < SMOOTH_TURN).then(|| blend(before, after))
        };
        // The band's own slope gives the normal, out across the step and up;
        // the step is measured at `vertex`.
        let normal = |vertex: usize, way: [f32; 2]| -> [f32; 3] {
            let step = math::length(inner[vertex] - outer[vertex]) as f32;
            let slope = (step * step + rise * rise).sqrt().max(1e-9);
            let lean = rise.abs() / slope;
            [way[0] * lean, way[1] * lean, step / slope]
        };
        for i in 0..n {
            let j = (i + 1) % n;
            let (a, b) = (outer[i], outer[j]);
            let (c, d) = (inner[j], inner[i]);
            let edge = b - a;
            let len = math::length(edge).max(1e-12);
            let t = [(edge.x / len) as f32, (edge.y / len) as f32, 0.0, 1.0];
            let flat = normal(i, outward[i]);
            let ni = smooth(i).map_or(flat, |way| normal(i, way));
            let nj = smooth(j).map_or(flat, |way| normal(j, way));
            let base = out.vertex_count();
            let corners = [
                [a.x as f32, a.y as f32, from_z],
                [b.x as f32, b.y as f32, from_z],
                [c.x as f32, c.y as f32, to_z],
                [d.x as f32, d.y as f32, to_z],
            ];
            out.push(corners[0], ni, [0.0, 0.0], t);
            out.push(corners[1], nj, [1.0, 0.0], t);
            out.push(corners[2], nj, [1.0, 1.0], t);
            out.push(corners[3], ni, [0.0, 1.0], t);
            // Wind the quad so that a viewer on the normal's side sees its
            // front. A band climbs as it steps in, where a side wall
            // descends, and that turns the sense of the winding round; it is
            // read from the corners rather than assumed.
            if facing(corners[0], corners[2], corners[1], flat) {
                out.indices.extend([base, base + 2, base + 1, base, base + 3, base + 2]);
            } else {
                out.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
            }
        }
    }
}
