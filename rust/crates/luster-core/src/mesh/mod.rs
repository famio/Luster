//! Extruding flat regions into slabs, written straight into the vertex layout
//! every renderer takes.
//!
//! A piece is one island: an outer ring and the holes inside it. Its front
//! face may be rolled, a quarter-round that eats `roll` of both the face's
//! width and its depth, which is how a badge's edge catches the light.



use crate::geom::offset::inset_ring;
use crate::geom::path::Contours;
use crate::Error;
use crate::math;

/// Bytes per vertex: position f32×3, normal f32×3, uv f32×2, tangent f32×4
/// (xyz + handedness). Little-endian, interleaved.
pub const VERTEX_STRIDE: usize = 48;
pub const POSITION_OFFSET: usize = 0;
pub const NORMAL_OFFSET: usize = 12;
pub const UV_OFFSET: usize = 24;
pub const TANGENT_OFFSET: usize = 32;

/// Side normals are shared across a vertex when the outline turns by less than
/// this; sharper corners keep a crease.
const SMOOTH_TURN: f32 = 35.0 * std::f32::consts::PI / 180.0;

/// Edges shorter than this, in badge units, are passed over when a side's
/// normals are blended: a fifth of a pixel of the metal's raster.
const SHORT_EDGE: f64 = 1e-4;

/// Steps in a rolled edge: at a badge's size no one counts six flats.
const ROLL_STEPS: usize = 6;

#[derive(Default)]
pub struct MeshBuilder {
    pub vertices: Vec<u8>,
    pub indices: Vec<u32>,
    pub min: [f32; 3],
    pub max: [f32; 3],
    count: u32,
}

impl MeshBuilder {
    pub fn new() -> Self {
        Self { min: [f32::MAX; 3], max: [f32::MIN; 3], ..Default::default() }
    }

    pub fn vertex_count(&self) -> u32 {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    fn push(&mut self, p: [f32; 3], n: [f32; 3], uv: [f32; 2], t: [f32; 4]) -> u32 {
        for v in p.iter().chain(&n).chain(&uv).chain(&t) {
            self.vertices.extend_from_slice(&v.to_le_bytes());
        }
        for i in 0..3 {
            self.min[i] = self.min[i].min(p[i]);
            self.max[i] = self.max[i].max(p[i]);
        }
        self.count += 1;
        self.count - 1
    }

    /// Adds another mesh's triangles to this one.
    pub fn append(&mut self, other: &MeshBuilder) {
        let base = self.count;
        self.vertices.extend_from_slice(&other.vertices);
        self.indices.extend(other.indices.iter().map(|i| i + base));
        self.count += other.count;
        for i in 0..3 {
            self.min[i] = self.min[i].min(other.min[i]);
            self.max[i] = self.max[i].max(other.max[i]);
        }
    }

    pub fn index_bytes(&self) -> Vec<u8> {
        self.indices.iter().flat_map(|i| i.to_le_bytes()).collect()
    }
}

/// Which parts of a slab to build.
#[derive(Clone, Copy, Debug)]
pub struct Faces {
    pub front: bool,
    pub sides: bool,
    pub back: bool,
}

impl Faces {
    pub const ALL: Faces = Faces { front: true, sides: true, back: true };
    /// A piece laid on a plate: its back is inside the plate and never seen.
    pub const LAID: Faces = Faces { front: true, sides: true, back: false };
}

/// A slab to build from a region.
#[derive(Clone, Copy, Debug)]
pub struct Slab {
    /// Height of the front face, in badge units.
    pub front: f32,
    pub back: f32,
    /// Quarter-round at the front edge; 0 leaves it square.
    pub roll: f32,
    pub faces: Faces,
}

impl Slab {
    pub fn new(front: f32, back: f32, faces: Faces) -> Slab {
        Slab { front, back, roll: 0.0, faces }
    }

    pub fn rolled(mut self, roll: f32) -> Slab {
        self.roll = roll;
        self
    }
}

/// Extrudes one region, given as disjoint rings in badge units (y up).
///
/// The caps are UV-mapped over the region's bounds; the back is mirrored so
/// its texture reads correctly from behind.
#[cfg(test)]
pub fn extrude(rings: &Contours, slab: Slab, out: &mut MeshBuilder) -> Result<(), Error> {
    extrude_rolled(rings, &vec![true; rings.len()], slab, out)
}

/// The same, with only the rings `rolled` marks rolled: the rest stand square
/// to the face. A band narrower than two rolls, rolled along both its edges,
/// would fold over itself where the two meet.
pub fn extrude_rolled(rings: &Contours, rolled: &[bool], slab: Slab, out: &mut MeshBuilder) -> Result<(), Error> {
    if rings.is_empty() {
        return Ok(());
    }
    let (min, max) = bounds(rings);
    let size = [(max[0] - min[0]).max(1e-9), (max[1] - min[1]).max(1e-9)];
    let is_rolled = |index: usize| slab.roll > 0.0 && rolled.get(index).copied().unwrap_or(true);
    let (edge, square): (Contours, Contours) = {
        let (edge, square): (Vec<_>, Vec<_>) = rings.iter().enumerate().partition(|(index, _)| is_rolled(*index));
        (edge.into_iter().map(|(_, ring)| ring.clone()).collect(), square.into_iter().map(|(_, ring)| ring.clone()).collect())
    };

    // The rolled edge: rings stepping in and down from the outline, the last of
    // them the face itself.
    let steps = if edge.is_empty() { Vec::new() } else { roll_rings(&edge, slab) };
    let mut face: Contours = steps.last().map_or_else(|| edge.clone(), |(rings, _)| rings.clone());
    face.extend(square.iter().cloned());
    let face_z = slab.front;
    // Where the side wall stops: at the roll's start, or at the face itself.
    let shoulder = slab.front - slab.roll;

    if slab.faces.front {
        cap(&face, face_z, true, min, size, out)?;
    }
    if slab.faces.back {
        cap(rings, slab.back, false, min, size, out)?;
    }
    if slab.faces.sides {
        for (index, ring) in rings.iter().enumerate() {
            let top = if is_rolled(index) { shoulder } else { slab.front };
            side(ring, rings, top, slab.back, out);
        }
    }
    // The roll's bands, from the outline inwards.
    let mut from: (&Contours, f32) = (&edge, shoulder);
    for (to, z) in &steps {
        band(from.0, from.1, to, *z, out);
        from = (to, *z);
    }
    Ok(())
}

/// The rings of a rolled edge, each with the height it sits at: a quarter
/// circle of `ROLL_STEPS` flats, eating `roll` of the face's width and depth.
fn roll_rings(rings: &Contours, slab: Slab) -> Vec<(Contours, f32)> {
    let roll = f64::from(slab.roll);
    (1..=ROLL_STEPS)
        .map(|step| {
            let angle = step as f64 / ROLL_STEPS as f64 * std::f64::consts::FRAC_PI_2;
            let inset = roll * (1.0 - math::cos(angle));
            let height = slab.front - (roll * (1.0 - math::sin(angle))) as f32;
            (rings.iter().map(|ring| inset_ring(ring, inset)).collect(), height)
        })
        .collect()
}

mod dome;
mod surface;

pub use dome::dome;
use surface::*;

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Point;
    use crate::geom::path::{contours, TOLERANCE};

    fn disc(radius: f64) -> Contours {
        let path = kurbo::Shape::to_path(&kurbo::Circle::new((0.0, 0.0), radius), 1e-6);
        contours(&path, TOLERANCE)
    }

    /// Front-face area of whatever was built at `z`.
    fn area_at(mesh: &MeshBuilder, z: f32) -> f32 {
        let vertex = |i: u32| {
            let o = i as usize * VERTEX_STRIDE;
            let f = |k: usize| {
                f32::from_le_bytes(mesh.vertices[o + 4 * k..o + 4 * k + 4].try_into().unwrap())
            };
            [f(0), f(1), f(2)]
        };
        mesh.indices
            .chunks_exact(3)
            .map(|t| [vertex(t[0]), vertex(t[1]), vertex(t[2])])
            .filter(|t| t.iter().all(|p| (p[2] - z).abs() < 1e-6))
            .map(|[a, b, c]| ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])) / 2.0)
            .sum()
    }

    #[test]
    fn a_square_slab_keeps_its_area_on_both_faces() {
        let square = vec![vec![
            Point::new(-1.0, -1.0),
            Point::new(1.0, -1.0),
            Point::new(1.0, 1.0),
            Point::new(-1.0, 1.0),
        ]];
        let mut mesh = MeshBuilder::new();
        extrude(&square, Slab::new(0.1, -0.1, Faces::ALL), &mut mesh).unwrap();
        assert!((area_at(&mesh, 0.1) - 4.0).abs() < 1e-5, "{}", area_at(&mesh, 0.1));
        assert!((area_at(&mesh, -0.1) + 4.0).abs() < 1e-5, "the back faces the other way");
    }

    #[test]
    fn a_roll_narrows_the_face_by_its_radius() {
        let mut mesh = MeshBuilder::new();
        extrude(&disc(1.0), Slab::new(0.1, -0.1, Faces::ALL).rolled(0.2), &mut mesh).unwrap();
        // The face is inset by the roll; the outline is untouched.
        let face = area_at(&mesh, 0.1);
        let expected = std::f32::consts::PI * 0.8 * 0.8;
        assert!((face - expected).abs() < 0.02, "{face} vs {expected}");
        assert!((mesh.max[0] - 1.0).abs() < 1e-3, "{}", mesh.max[0]);
        // The wall stops where the roll starts.
        assert!((mesh.min[2] + 0.1).abs() < 1e-6);
    }

    /// A rolled round edge is shaded as a curve along the outline, not as the
    /// flats the outline was cut into: where two faces of one step of the roll
    /// meet, they agree on the normal. (Across the steps the profile keeps its
    /// six flats.)
    #[test]
    fn a_round_roll_is_shaded_smooth_along_the_edge() {
        let mut mesh = MeshBuilder::new();
        extrude(&disc(1.0), Slab::new(0.1, -0.1, Faces::ALL).rolled(0.2), &mut mesh).unwrap();
        let read = |i: usize, k: usize| {
            let o = i * VERTEX_STRIDE + 4 * k;
            f32::from_le_bytes(mesh.vertices[o..o + 4].try_into().unwrap())
        };
        let mut seen: Vec<([f32; 3], [f32; 3])> = Vec::new();
        let mut shared = 0;
        for i in 0..mesh.vertex_count() as usize {
            let position = [read(i, 0), read(i, 1), read(i, 2)];
            let normal = [read(i, 3), read(i, 4), read(i, 5)];
            // Only the roll: tilted normals, neither a face nor the wall.
            if normal[2].abs() < 1e-3 || normal[2].abs() > 1.0 - 1e-3 {
                continue;
            }
            for (other, other_normal) in &seen {
                // One step of the roll: on a round edge its tilt is the same all
                // the way round.
                let same_step = (other_normal[2] - normal[2]).abs() < 1e-4;
                if same_step && (0..3).all(|k| (other[k] - position[k]).abs() < 1e-6) {
                    shared += 1;
                    let gap = (0..3).map(|k| (other_normal[k] - normal[k]).abs()).fold(0.0, f32::max);
                    assert!(gap < 1e-5, "a seam in the roll's shading at {position:?}: {gap}");
                }
            }
            seen.push((position, normal));
        }
        assert!(shared > 0, "the roll's faces meet at shared corners");
    }

    #[test]
    fn a_hole_is_kept_through_the_slab() {
        let mut rings = disc(1.0);
        let mut hole = disc(0.5);
        hole[0].reverse();
        rings.append(&mut hole);
        let mut mesh = MeshBuilder::new();
        extrude(&rings, Slab::new(0.05, -0.05, Faces::ALL), &mut mesh).unwrap();
        let expected = std::f32::consts::PI * (1.0 - 0.25);
        let face = area_at(&mesh, 0.05);
        assert!((face - expected).abs() < 0.02, "{face} vs {expected}");
    }

    #[test]
    fn a_notch_narrower_than_the_roll_does_not_turn_the_piece_inside_out() {
        // A square with a slot cut into one side, narrower than twice the roll.
        let shape = vec![vec![
            Point::new(-1.0, -1.0),
            Point::new(1.0, -1.0),
            Point::new(1.0, 1.0),
            Point::new(0.05, 1.0),
            Point::new(0.05, 0.2),
            Point::new(-0.05, 0.2),
            Point::new(-0.05, 1.0),
            Point::new(-1.0, 1.0),
        ]];
        let mut mesh = MeshBuilder::new();
        extrude(&shape, Slab::new(0.1, -0.1, Faces::ALL).rolled(0.15), &mut mesh).unwrap();
        let face = area_at(&mesh, 0.1);
        // Smaller than the shape, and still the right way round.
        assert!(face > 0.0 && face < 4.0, "{face}");
    }

    /// A dome covers its region whole, every triangle facing up, even where a
    /// sharp notch is narrower than its curve: no gap for what is below to
    /// show through, and nothing inside out.
    #[test]
    fn a_dome_covers_a_notched_region_whole_and_face_up() {
        let shape = vec![vec![
            Point::new(-1.0, -1.0),
            Point::new(1.0, -1.0),
            Point::new(1.0, 1.0),
            Point::new(0.05, 1.0),
            Point::new(0.02, 0.2),
            Point::new(-0.02, 0.2),
            Point::new(-0.05, 1.0),
            Point::new(-1.0, 1.0),
        ]];
        let mut mesh = MeshBuilder::new();
        dome(&shape, Slab::new(0.05, 0.0, Faces::LAID).rolled(0.15), &mut mesh).unwrap();
        let vertex = |i: u32| {
            let o = i as usize * VERTEX_STRIDE;
            let f = |k: usize| f32::from_le_bytes(mesh.vertices[o + 4 * k..o + 4 * k + 4].try_into().unwrap());
            [f(0), f(1), f(2)]
        };
        let mut covered = 0.0f32;
        for t in mesh.indices.chunks_exact(3) {
            let [a, b, c] = [vertex(t[0]), vertex(t[1]), vertex(t[2])];
            let turn = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
            assert!(turn >= -1e-9, "a triangle faces down: {a:?} {b:?} {c:?}");
            covered += turn / 2.0;
            for p in [a, b, c] {
                assert!(p[2] >= -1e-6 && p[2] <= 0.05 + 1e-6, "off the curve: {p:?}");
            }
        }
        // The square less the slot, a trapezoid 0.04 wide at its foot and 0.1
        // at its mouth.
        let wanted = 4.0 - 0.8 * (0.04 + 0.1) / 2.0;
        assert!((covered - wanted).abs() < wanted * 0.01, "{covered} of {wanted}");
    }

    #[test]
    fn nothing_is_built_from_nothing() {
        let mut mesh = MeshBuilder::new();
        extrude(&Vec::new(), Slab::new(0.1, -0.1, Faces::ALL), &mut mesh).unwrap();
        assert!(mesh.is_empty());
    }
}
