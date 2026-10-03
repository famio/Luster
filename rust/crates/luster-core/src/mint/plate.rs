//! The plate: a badge's outline grown by its metal margin.
//!
//! The margin is grown on a raster. Booleans leave slivers and tangential
//! contact that nothing can extrude, and two islands whose margins fall short
//! of each other should be joined by the metal between them, which is a
//! question about distance rather than about paths.

use rayon::prelude::*;

use crate::geom::islands;
use crate::geom::path::{self, Contours};
use crate::raster::{Sheet, field};

use super::consts::*;
use crate::math;

/// The sheet a plate is drawn on: the art, with room round it for a margin
/// of `margin` and a roll of `roll`.
pub fn sheet(edge: &Contours, margin: f64, roll: f64) -> Option<Sheet> {
    let frame = crate::geom::path::bounds(edge)?;
    Sheet::around(frame, 2.0 * (margin + roll), METAL_RESOLUTION)
}

/// The plate as per-pixel coverage on `sheet`, which `sheet` above makes for
/// the same `edge`, `margin` and `roll`. The wall and the face metal are cut
/// from the same sheet, so all three agree.
///
/// `margin` is the flat metal the art earns, `roll` the rolled edge outside it,
/// and `hair` the narrower margin an edge drawn as a line earns, since a line
/// already stands in metal there; a line narrower than `body` earns a fill's
/// instead. `fill_margin` is what an edge drawn as a
/// fill earns: the margin, or less where the fill is to reach the edge
/// itself. Islands are joined as their full margins would join them either
/// way. `speck` is what an island no wider than `speck.0` earns instead of
/// `fill_margin`: a fill run on under the wall loses a little of its edge,
/// which a large one does not miss but a dot does, and the plate round a dot
/// is seldom round.
pub fn plate(
    sheet: &Sheet,
    edge: &Contours,
    lines: &Contours,
    margin: f64,
    fill_margin: f64,
    speck: (f64, f64),
    (hair, body): (f64, f64),
    roll: f64,
) -> Vec<f32> {
    let (width, height) = (sheet.width, sheet.height);

    // Only a line with some body to it stands in metal. A sliver of stroke
    // poking out from under the fill drawn over it is a line too, and along
    // its length it would take the margin down to the hair and notch the edge.
    let lines: Contours = islands::split(lines, 0.0)
        .into_iter()
        .filter(|island| mean_width(island) >= body)
        .flat_map(|island| island.rings())
        .collect();
    // Lines are read slightly wider than drawn: a fill can poke past its
    // outlining stroke at a sharp corner, and would earn the full margin
    // there, which stands out as a gold spike.
    let (art, on_line) = rayon::join(
        || sheet.lay(&path::from_contours(edge), 0.0),
        || {
            (!lines.is_empty()).then(|| {
                let laid = sheet.lay(&path::from_contours(&lines), BLEED);
                laid.iter().map(|&v| v >= 0.5).collect::<Vec<bool>>()
            })
        },
    );
    // Both margins below are cut from the same distances to the art's edge.
    let edges = match &on_line {
        None => (field::Edge::of(&art, None, width, height), None),
        Some(on_line) => {
            let off_line: Vec<bool> = on_line.par_iter().map(|&v| !v).collect();
            let (fill, line) = rayon::join(
                || field::Edge::of(&art, Some(&off_line), width, height),
                || field::Edge::of(&art, Some(on_line), width, height),
            );
            (fill, Some(line))
        }
    };
    let flat = (margin * sheet.scale) as f32;
    let hairs = (hair * sheet.scale) as f32;

    // The art grown by `fill_margin` at its fills and the hair at its lines.
    // Kept as fractional coverage, not a flag, so the traced outline is a
    // curve rather than a staircase.
    let grow = |fill_margin: f64| -> Vec<f32> {
        let reach = ((fill_margin + roll) * sheet.scale) as f32;
        let mut cover = vec![0.0f32; width * height];
        match &edges {
            (fill, None) => {
                // One margin all round.
                let to_edge = fill.distance(&art, width, height, &[reach - 0.5..reach + 0.5]);
                cover.par_iter_mut().enumerate().for_each(|(index, value)| {
                    *value = art[index].max((0.5 - (to_edge[index] - reach)).clamp(0.0, 1.0));
                });
            }
            (fill, Some(line)) => {
                // The two offsets are blended where they meet, because a crease
                // between them cannot carry a rolled edge.
                let blend = (reach / 2.0).max(1.0);
                // Only the art's boundary is classified, not its areas, so line work
                // behind the edge does not change the margin. Each offset is cut
                // at its own margin, give or take the blend.
                let (fill_cut, line_cut) = ([reach - blend..reach + blend], [hairs - blend..hairs + blend]);
                let (to_fill, to_line) = rayon::join(
                    || fill.distance(&art, width, height, &fill_cut),
                    || line.distance(&art, width, height, &line_cut),
                );
                cover.par_iter_mut().enumerate().for_each(|(index, value)| {
                    let past_fill = to_fill[index] - reach;
                    let past_line = to_line[index] - hairs;
                    let lean = (0.5 + 0.5 * (past_line - past_fill) / blend).clamp(0.0, 1.0);
                    let past = past_line + (past_fill - past_line) * lean - blend * lean * (1.0 - lean);
                    *value = art[index].max((0.5 - past).clamp(0.0, 1.0));
                });
            }
        }
        cover
    };
    let settle = |cover: &mut Vec<f32>| {
        // A badge is one piece: art with islands of its own comes apart wherever
        // the margins of two islands fall short of each other.
        field::bridge(cover, width, height, flat * BADGE_BRIDGE);
        // Where two margins nearly meet they leave a narrow slot; openings
        // narrower than half the margin are filled.
        let mut solid: Vec<bool> = cover.iter().map(|&v| v >= 0.5).collect();
        field::stop_up(&mut solid, width, height, flat / 2.0);
        for (index, filled) in solid.iter().enumerate() {
            if *filled && cover[index] < 0.5 {
                cover[index] = 1.0;
            }
        }
    };
    let mut cover = grow(margin);
    settle(&mut cover);
    if fill_margin < margin {
        // What the full margin joins or fills stays so: the gap between two
        // pieces, a notch, a counter. The full plate is shrunk back by what the
        // fills gave up, and reaches no further than the fills do elsewhere.
        let inset = ((margin - fill_margin) * sheet.scale) as f32;
        let (out, mut bare) = rayon::join(
            || field::distance_to_edge(&cover, None, width, height, &[-inset - 0.5..-inset + 0.5]),
            || grow(fill_margin),
        );
        // The small islands, each grown by its own margin all round.
        let (widest, speck_margin) = speck;
        let small: Contours = islands::split(edge, 0.0)
            .into_iter()
            .filter(|island| {
                crate::geom::path::bounds(&vec![island.outer.clone()]).is_some_and(|b| b.width().max(b.height()) <= widest)
            })
            .flat_map(|island| island.rings())
            .collect();
        if speck_margin > fill_margin && !small.is_empty() {
            let dots = sheet.lay(&path::from_contours(&small), 0.0);
            let reach = ((speck_margin + roll) * sheet.scale) as f32;
            let to_edge = field::distance_to_edge(&dots, None, width, height, &[reach - 0.5..reach + 0.5]);
            bare.par_iter_mut().enumerate().for_each(|(index, value)| {
                let grown = dots[index].max((0.5 - (to_edge[index] - reach)).clamp(0.0, 1.0));
                *value = value.max(grown);
            });
        }
        cover.par_iter_mut().enumerate().for_each(|(index, value)| {
            *value = bare[index].max((0.5 - out[index] - inset).clamp(0.0, 1.0));
        });
        // Two islands whose full margins only just touched were one piece, so
        // nothing bridged them, and shrinking the plate back parts them:
        // they are bridged again as they now stand, as broadly as a bridge
        // cut at the full margin and shrunk back with the rest would be.
        field::bridge(&mut cover, width, height, flat * BADGE_BRIDGE + inset);
    }
    // Where two parts meet, or the outline turns in, a fillet rather than a
    // crease: a roll run into a sharp inside corner folds into a notch.
    let radius = (FILLET * sheet.scale) as f32;
    let largest = (FILLET_LARGEST * FILLET * FILLET * sheet.scale * sheet.scale) as usize;
    field::fillet(&mut cover, width, height, radius, largest);
    cover
}

/// The radius of the fillet in the plate's inside corners, and the most it
/// fills, in squares of that radius: more is a gap between parts. Just wider
/// than the roll, which is what folds in a sharp corner; a wider fillet webs
/// a gap that tapers shut, as between a ring and the edge it runs along, and
/// leaves a pocket beyond the web.
const FILLET: f64 = 0.007;
const FILLET_LARGEST: f64 = 2.0;

/// How much wider than drawn a line is read; see `plate`.
const BLEED: f64 = 0.01;

/// An island's mean width: its area over half its perimeter, holes and all.
/// A stroke's comes out at the stroke's width.
fn mean_width(island: &islands::Island) -> f64 {
    let ring_area =
        |ring: &Vec<kurbo::Point>| path::signed_area(&path::from_contours(std::slice::from_ref(ring))).abs();
    let length = |ring: &Vec<kurbo::Point>| -> f64 {
        ring.iter().zip(ring.iter().cycle().skip(1)).map(|(a, b)| math::distance(*a, *b)).sum()
    };
    let area = ring_area(&island.outer) - island.holes.iter().map(ring_area).sum::<f64>();
    let perimeter = length(&island.outer) + island.holes.iter().map(length).sum::<f64>();
    if perimeter > 0.0 { 2.0 * area / perimeter } else { 0.0 }
}

/// The plate's outline: the art grown by its margin, traced back to a path.
#[cfg(test)]
pub fn grown(edge: &Contours, lines: &Contours, margin: f64, hair: f64, roll: f64) -> Option<kurbo::BezPath> {
    let sheet = sheet(edge, margin, roll)?;
    let cover = plate(&sheet, edge, lines, margin, margin, (0.0, margin), (hair, hair / 2.0), roll);
    sheet.outline(&cover, METAL_TOLERANCE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::boolean::{self, Rule};

    fn disc(radius: f64) -> Contours {
        let path = kurbo::Shape::to_path(&kurbo::Circle::new((0.0, 0.0), radius), 1e-6);
        path::contours(&path, path::TOLERANCE)
    }

    fn area(rings: &Contours) -> f64 {
        path::signed_area(&path::from_contours(rings)).abs()
    }

    #[test]
    fn the_plate_is_the_art_grown_by_its_margin() {
        let grown = grown(&disc(0.4), &Vec::new(), 0.02, 0.02, 0.0).expect("a plate");
        let rings = boolean::simplify(&path::contours(&grown, 1e-4), Rule::EvenOdd);
        let expected = std::f64::consts::PI * 0.42 * 0.42;
        assert!((area(&rings) - expected).abs() < expected * 0.02, "{} vs {expected}", area(&rings));
    }

    #[test]
    fn two_islands_close_together_are_bridged_into_one() {
        let mut art = disc(0.2);
        let mut other: Contours = disc(0.2)
            .iter()
            .map(|ring| ring.iter().map(|p| kurbo::Point::new(p.x + 0.46, p.y)).collect())
            .collect();
        art.append(&mut other);
        let grown = grown(&art, &Vec::new(), 0.03, 0.03, 0.0).expect("a plate");
        let rings = boolean::simplify(&path::contours(&grown, 1e-4), Rule::EvenOdd);
        assert_eq!(rings.len(), 1, "the gap between the discs was bridged");
    }

    #[test]
    fn far_apart_islands_stay_apart() {
        let mut art = disc(0.2);
        let mut other: Contours = disc(0.2)
            .iter()
            .map(|ring| ring.iter().map(|p| kurbo::Point::new(p.x + 0.9, p.y)).collect())
            .collect();
        art.append(&mut other);
        let grown = grown(&art, &Vec::new(), 0.02, 0.02, 0.0).expect("a plate");
        let rings = boolean::simplify(&path::contours(&grown, 1e-4), Rule::EvenOdd);
        assert_eq!(rings.len(), 2);
    }

    #[test]
    fn an_edge_drawn_as_a_line_earns_the_narrower_margin() {
        // A disc whose rim is a drawn line: the margin there is the hair.
        let art = disc(0.4);
        let ring = crate::geom::stroke::round(&path::from_contours(&disc(0.4)), 0.02);
        let lines = boolean::simplify(&path::contours(&ring, path::TOLERANCE), Rule::NonZero);
        let wide = grown(&art, &Vec::new(), 0.04, 0.004, 0.0).expect("a plate");
        let narrow = grown(&art, &lines, 0.04, 0.004, 0.0).expect("a plate");
        let wide = area(&boolean::simplify(&path::contours(&wide, 1e-4), Rule::EvenOdd));
        let narrow = area(&boolean::simplify(&path::contours(&narrow, 1e-4), Rule::EvenOdd));
        assert!(narrow < wide, "{narrow} vs {wide}");
    }

    #[test]
    fn a_sliver_of_line_on_the_edge_earns_the_full_margin() {
        // A stroke hidden under the fill above it can leave a sliver along the
        // edge, a fraction of a pixel wide: it stands in no metal.
        let art = disc(0.4);
        let mut arc = kurbo::BezPath::new();
        arc.move_to((0.4, 0.0));
        let sweep = kurbo::Arc::new((0.0, 0.0), (0.4, 0.4), 0.0, 0.6, 0.0);
        sweep.to_cubic_beziers(1e-6, |a, b, c| arc.curve_to(a, b, c));
        let sliver = crate::geom::stroke::round(&arc, 0.0005);
        let lines = boolean::simplify(&path::contours(&sliver, path::TOLERANCE), Rule::NonZero);
        let plain = grown(&art, &Vec::new(), 0.04, 0.004, 0.0).expect("a plate");
        let with = grown(&art, &lines, 0.04, 0.004, 0.0).expect("a plate");
        let plain = area(&boolean::simplify(&path::contours(&plain, 1e-4), Rule::EvenOdd));
        let with = area(&boolean::simplify(&path::contours(&with, 1e-4), Rule::EvenOdd));
        assert!((with - plain).abs() < plain * 1e-3, "{with} vs {plain}");
    }
}
