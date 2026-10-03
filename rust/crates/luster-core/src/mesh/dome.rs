//! A glaze domed over its region.

use kurbo::Point;
use lyon_tessellation::path::FillRule;
use crate::geom::path::Contours;
use crate::Error;
use crate::math;

use super::*;

/// Builds a region as a glaze domed over it: a quarter ellipse from
/// `slab.back` at the outline up to `slab.front`, which it reaches `slab.roll`
/// in from it, and level beyond. It has no sides: its foot sits on whatever is
/// below.
///
/// The steps of the curve are cut as offsets of the region, and each band
/// between two is tessellated as a region of its own. An outline moved in point by point
/// folds over itself in a sharp notch and leaves the band inside out there;
/// an offset cannot. Every vertex takes its height and its normal from its own
/// distance to the outline, so the bands meet without a seam.
pub fn dome(rings: &Contours, slab: Slab, out: &mut MeshBuilder) -> Result<(), Error> {
    if rings.is_empty() {
        return Ok(());
    }
    let (min, max) = bounds(rings);
    let size = [(max[0] - min[0]).max(1e-9), (max[1] - min[1]).max(1e-9)];
    let reach = f64::from(slab.roll);
    let rise = slab.front - slab.back;

    // The region shrunk by each step of the curve, the last of them the level
    // top. Each is cut from the region itself, not from the step before.
    // Offsets are cut from the outline thinned to a fraction of the first
    // step: an outline in steps far finer than the offset makes each offset
    // edge cross dozens of its neighbours, and the cut crawls.
    let thinned: Contours = rings
        .iter()
        .map(|ring| crate::geom::simplify::closed(ring, DOME_THINNING))
        .filter(|ring| ring.len() >= 3)
        .collect();
    let steps: Vec<Contours> = (1..=ROLL_STEPS)
        .map(|step| {
            let angle = step as f64 / ROLL_STEPS as f64 * std::f64::consts::FRAC_PI_2;
            crate::geom::boolean::inset(&thinned, reach * (1.0 - math::cos(angle)))
        })
        .collect();

    let edges = Edges::new(rings, reach);
    // Height and normal from the distance to the outline: steep at the foot,
    // level at the reach.
    let lift = |p: Point| -> ([f32; 3], [f32; 3]) {
        let (distance, outward) = edges.nearest(p);
        let t = (distance / reach).min(1.0);
        let s = 1.0 - t;
        let z = slab.back + rise * (1.0 - s * s).max(0.0).sqrt() as f32;
        // The profile's slope, as a normal leaning out across the outline.
        let (across, up) = if t >= 1.0 {
            (0.0, 1.0)
        } else {
            let run = reach * (1.0 - s * s).max(0.0).sqrt();
            let climb = f64::from(rise) * s;
            let length = (run * run + climb * climb).sqrt().max(1e-12);
            (climb / length, run / length)
        };
        let n = [(outward.0 * across) as f32, (outward.1 * across) as f32, up as f32];
        ([p.x as f32, p.y as f32, z], n)
    };

    // The bands from the outline in, then the top.
    let mut regions: Vec<Contours> = Vec::with_capacity(ROLL_STEPS + 1);
    let mut from = rings.clone();
    for step in &steps {
        let mut band = from.clone();
        band.extend(step.iter().cloned());
        regions.push(band);
        from = step.clone();
    }
    regions.push(from);
    for region in &regions {
        let (points, triangles) = tessellate(region, FillRule::EvenOdd)?;
        let first = out.vertex_count();
        for p in points {
            let (position, normal) = lift(p);
            let uv = [(position[0] - min[0]) / size[0], (min[1] + size[1] - position[1]) / size[1]];
            out.push(position, normal, uv, [1.0, 0.0, 0.0, 1.0]);
        }
        out.indices.extend(triangles.into_iter().flatten().map(|i| first + i));
    }
    Ok(())
}

/// How far a dome's outline may be moved in thinning it before its offsets
/// are cut: well under its first step, and under anything seen.
const DOME_THINNING: f64 = 0.00002;
