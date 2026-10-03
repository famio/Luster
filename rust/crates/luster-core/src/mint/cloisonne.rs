//! A badge's metal: enamel recessed into raised metal walls.
//!
//! All of a badge's metal is cut from one sheet, so that the outline, the
//! plate under the enamel, the wall round the edge and the metal standing on
//! the face all agree to the pixel.

use kurbo::BezPath;
use rayon::prelude::*;

use crate::geom::boolean::{self, Rule};
use crate::geom::path::{self, Contours};
use crate::raster::field;
use crate::raster::sheet::level;
use crate::svg::cells::Cell;

use super::consts::*;
use super::plate::plate;
use crate::{CancelToken, Error};

/// The metal of a badge.
pub struct Metal {
    /// The badge's edge.
    pub outline: BezPath,
    /// The outline inset by the wall: the reverse, and the enamel's floor.
    pub plate: BezPath,
    /// A band of one width along the outline: the badge's side and front roll.
    pub wall: BezPath,
    /// What stands on the face inside the wall, opened over the enamel.
    pub metal: Option<BezPath>,
    /// The wall's flat top inside its roll, which a fill runs on under: the
    /// metal there shows only from above.
    pub wall_top: Option<BezPath>,
}

/// How far past the traced edge a cell may be opened. The traced edge is
/// coarser than the cells, so a cell is opened only near it, and never
/// through the badge's side.
const EDGE_SLACK: f64 = 0.003;
/// How far past the edge an opening may reach, so that a fill runs on under
/// the wall rather than stopping short of it.
const EDGE_REACH: f64 = 0.0125;
/// The wall is only its roll and a lip of flat metal inside it: a fill runs on
/// to the edge, and a wider wall would stand inside it as a rim of its own.
const WALL_LIP: f64 = 0.001;
/// All the margin a fill's edge earns: the flat of the wall inside its roll,
/// so that the wall stands round the fill rather than over its edge, where
/// its top would be plated in the fill's colour as a darker strip beside it.
const FILL_EASE: f64 = WALL_CLEAR + WALL_LIP;
/// An island of fill no wider than this is ringed in metal rather than run
/// on under the wall: a dot along the edge, which the wall would bite into
/// wherever the plate round it bulges less.
const SPECK_WIDTH: f64 = 0.05;
/// The flat metal round such a dot, inside the wall's roll.
const SPECK_RIM: f64 = 0.0015;
/// How far the plate under the enamel is inset.
const WALL_INSET: f64 = 0.0015;
/// The roll on the wall's front edge, and how far flat metal stops short of it.
pub const WALL_ROLL: f32 = 0.005;
const WALL_CLEAR: f64 = 0.001;
/// How far, in pixels of the sheet, the metal is let into the enamel's
/// openings before it is cut back to the cells themselves; see `cloisonne`.
const OVERLAP: f64 = 1.5;
/// Half the width of the narrowest metal kept on the face: anything thinner
/// is a hairline the trace and a cell's outline leave between them.
pub const SLIVER: f64 = 0.0003;
/// The narrower margin an edge drawn as a line earns: the roll and the flat
/// metal clear of it, so the wall's flat top starts where the line does. Any
/// less, and the wall runs over the line's edge wherever the badge's outline
/// hugs it but not where the outline swells away towards a neighbour, and
/// the plating on the line steps out there.
pub const CLOISONNE_RIM: f64 = WALL_ROLL as f64 + WALL_CLEAR;
/// How wide a line must be to earn that margin rather than a fill's.
const LINE_BODY: f64 = 0.002;
/// The tracer's tolerance for the metal on the face, in pixels: finer than
/// the rest of the metal's. A line's metal is drawn `OVERLAP` wider than the
/// line and cut back to the enamel beside it, so its trace may wander by less
/// than that. It is traced in straight segments: a curve through the thinned
/// points would round a slight corner of the enamel and drop a strip of the
/// line below the metal.
const FACE_TOLERANCE: f64 = 0.2;

/// Cuts all of a badge's metal from one sheet. An edge drawn as a fill earns
/// no margin: the fill runs out to the rolled edge. `margin` is what joins
/// the art's islands into one piece.
///
/// The sheet is the badge at full resolution, the slowest thing a mint does;
/// `cancel` is looked at between its stages.
pub fn cloisonne(
    edge: &Contours,
    lines: &Contours,
    cells: &[(Cell, Contours)],
    margin: f64,
    cancel: &CancelToken,
) -> Result<Metal, Error> {
    let wall_width = f64::from(WALL_ROLL) + WALL_CLEAR + WALL_LIP;
    // A dot's margin reaches past the wall, so the wall stands round it.
    let speck = (SPECK_WIDTH, WALL_CLEAR + WALL_LIP + SPECK_RIM);
    let sheet = super::plate::sheet(edge, margin, f64::from(WALL_ROLL)).ok_or(Error::NothingToMint)?;
    let count = sheet.pixels();

    let edge_path = path::from_contours(edge);
    // The openings are drawn a little small, so the traced metal reaches
    // into each cell; it is then cut back to the cells' own outlines, and the
    // edge between metal and enamel is the document's rather than a trace of
    // it, which wanders by a pixel, rounds the corners and lets the floor
    // show through where it falls short.
    let fills: Vec<BezPath> =
        cells.iter().filter(|(cell, _)| !cell.is_line).map(|(_, rings)| path::from_contours(rings)).collect();
    let colour = || {
        sheet.coverage_bytes(|canvas| {
            canvas.add(fills.iter().cloned());
            for fill in &fills {
                canvas.erase(fill, 2.0 * OVERLAP / sheet.scale);
            }
        })
    };
    // Lines are drawn a little wide for the same reason.
    let walls = || {
        sheet.coverage_bytes(|canvas| {
            for (_, rings) in cells.iter().filter(|(cell, _)| cell.is_line) {
                canvas.lay(&path::from_contours(rings), OVERLAP / sheet.scale);
            }
        })
    };
    let near = || sheet.coverage_bytes(|canvas| canvas.lay(&edge_path, EDGE_SLACK));
    let within = || {
        sheet.coverage_bytes(|canvas| {
            canvas.lay(&edge_path, 0.0);
            canvas.erase(&edge_path, 2.0 * (EDGE_SLACK + 0.001));
        })
    };
    // Those coverages need nothing of the plate but its sheet, nor do the
    // openings the traced metal is cut back to, which are booleans on the
    // cells alone: each is drawn on one thread while the plate is grown on
    // the rest. The coverages are held as bytes until they are read.
    let ((cover, enamel), ((colour, walls), (near, within))) = rayon::join(
        || {
            rayon::join(
                || {
                    let rim = (CLOISONNE_RIM, LINE_BODY);
                    plate(&sheet, edge, lines, margin, FILL_EASE, speck, rim, f64::from(WALL_ROLL))
                },
                || openings(edge, cells),
            )
        },
        || rayon::join(|| rayon::join(colour, walls), || rayon::join(near, within)),
    );
    cancel.check()?;

    // Signed distance from the outline, from which every inset below is cut.
    let cuts: Vec<std::ops::Range<f32>> = [WALL_INSET, wall_width, f64::from(WALL_ROLL) + WALL_CLEAR]
        .iter()
        .map(|inset| {
            let pixels = (inset * sheet.scale) as f32;
            -pixels - 0.5..-pixels + 0.5
        })
        .collect();
    let (outline, out) = rayon::join(
        || sheet.outline(&cover, METAL_TOLERANCE),
        || field::distance_to_edge(&cover, None, sheet.width, sheet.height, &cuts),
    );
    let outline = outline.ok_or(Error::NothingToMint)?;
    cancel.check()?;
    let inside = |distance: f64| -> Vec<f32> {
        let pixels = (distance * sheet.scale) as f32;
        (0..count).into_par_iter().map(|i| (0.5 - out[i] - pixels).clamp(0.0, 1.0)).collect()
    };
    // Flat metal stops short of the roll and runs on under the wall's inner
    // edge, which is rolled too and would otherwise show as a groove.
    let clear = || inside(f64::from(WALL_ROLL) + WALL_CLEAR);
    let ((plate_path, hollow), clear) = rayon::join(
        || {
            rayon::join(
                || sheet.outline(&inside(WALL_INSET), METAL_TOLERANCE),
                || sheet.outline(&inside(wall_width), METAL_TOLERANCE),
            )
        },
        clear,
    );
    let plate_path = plate_path.ok_or(Error::NothingToMint)?;
    cancel.check()?;
    // The wall is the outline minus its inset. The two never touch, so an
    // even-odd fill stands in for the boolean.
    let mut wall = outline.clone();
    if let Some(hollow) = &hollow {
        wall.extend(hollow.iter());
    }

    let metal: Vec<f32> = (0..count)
        .into_par_iter()
        .map(|i| {
            let open = level(colour[i]).min(level(near[i])).max(level(within[i]));
            cover[i].min(1.0 - open).max(level(walls[i])).min(clear[i])
        })
        .collect();
    cancel.check()?;
    let (metal, wall_top) = rayon::join(
        || {
            sheet.outline_straight(&metal, FACE_TOLERANCE).map(|traced| {
                let traced = boolean::simplify(&path::contours(&traced, path::TOLERANCE), Rule::EvenOdd);
                // Where the metal's trace and a cell's own outline nearly agree, the
                // difference leaves hairlines of metal no wider than a pixel.
                path::from_contours(&boolean::open(&boolean::difference(&traced, &enamel, Rule::NonZero), SLIVER))
            })
        },
        || {
            // The wall's top is a band between the roll and the wall's inner edge.
            let wide = inside(wall_width);
            let band: Vec<f32> = (0..count).into_par_iter().map(|i| clear[i].min(1.0 - wide[i])).collect();
            sheet.outline(&band, METAL_TOLERANCE)
        },
    );
    cancel.check()?;
    Ok(Metal { outline, plate: plate_path, wall, metal, wall_top })
}

/// What is cut away for the enamel, as the cells themselves have it.
fn openings(edge: &Contours, cells: &[(Cell, Contours)]) -> Contours {
    // Only enamel is cut away: where a line crosses a fill, the line is metal.
    let normalized = |line: bool| -> Contours {
        cells
            .iter()
            .filter(|(cell, _)| cell.is_line == line)
            .flat_map(|(_, rings)| boolean::simplify(rings, Rule::EvenOdd))
            .collect()
    };
    // And only where the openings were allowed: near the badge's edge, as
    // `near` has it, never out through its side.
    let reach = boolean::union(
        edge,
        &path::contours(
            &crate::geom::stroke::round(&path::from_contours(edge), 2.0 * EDGE_REACH),
            path::TOLERANCE,
        ),
        Rule::NonZero,
    );
    boolean::intersection(
        &boolean::difference(&normalized(false), &normalized(true), Rule::NonZero),
        &reach,
        Rule::NonZero,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Rgba;
    use crate::geom::boolean::{self, Rule};

    fn disc(radius: f64, at: (f64, f64)) -> Contours {
        let path = kurbo::Shape::to_path(&kurbo::Circle::new(at, radius), 1e-6);
        path::contours(&path, path::TOLERANCE)
    }

    fn area(path: &BezPath) -> f64 {
        path::signed_area(path).abs()
    }

    fn metal() -> Metal {
        let edge = disc(0.4, (0.0, 0.0));
        let cell = Cell { rings: disc(0.25, (0.0, 0.0)), color: Rgba::rgb(0.2, 0.6, 0.9), is_line: false };
        cloisonne(&edge, &Vec::new(), &[(cell.clone(), cell.rings.clone())], 0.02, &CancelToken::new()).expect("metal")
    }

    #[test]
    fn a_cancel_stops_it() {
        let edge = disc(0.4, (0.0, 0.0));
        let cancel = CancelToken::new();
        cancel.cancel();
        assert_eq!(cloisonne(&edge, &Vec::new(), &[], 0.02, &cancel).err(), Some(Error::Cancelled));
    }

    #[test]
    fn the_plate_sits_inside_the_outline() {
        let metal = metal();
        assert!(area(&metal.plate) < area(&metal.outline));
        // Only the wall's own width apart.
        let outline = kurbo::Shape::bounding_box(&metal.outline);
        let plate = kurbo::Shape::bounding_box(&metal.plate);
        let wall = f64::from(WALL_ROLL) + WALL_CLEAR + WALL_LIP;
        assert!((outline.width() - plate.width()) / 2.0 < wall * 1.5);
    }

    #[test]
    fn the_wall_is_a_band_along_the_edge() {
        let metal = metal();
        let rings = boolean::simplify(&path::contours(&metal.wall, 1e-4), Rule::EvenOdd);
        assert_eq!(rings.len(), 2, "an outer ring and the hole inside it");
        let band = path::signed_area(&path::from_contours(&rings)).abs();
        assert!(band < area(&metal.outline) * 0.25, "the wall is a band, not a disc: {band}");
    }

    #[test]
    fn the_enamel_is_opened_where_a_cell_covers_the_face() {
        let metal = metal().metal.expect("face metal");
        // The face metal is what is left round the cell: less than the plate.
        let rings = boolean::simplify(&path::contours(&metal, 1e-4), Rule::EvenOdd);
        let face = path::signed_area(&path::from_contours(&rings)).abs();
        let disc = std::f64::consts::PI * 0.42 * 0.42;
        assert!(face < disc * 0.6, "the cell was opened out of the metal: {face}");
        assert!(face > 0.0);
    }

    /// A dot on its own, beside the badge, is ringed by the wall rather than
    /// run on under it: the wall would bite into it unevenly, as the plate
    /// round a dot bulges towards whatever it is joined to.
    #[test]
    fn a_dot_is_not_run_under_the_wall() {
        let body = disc(0.3, (0.0, 0.0));
        let dot = disc(0.016, (0.0, -0.34));
        let edge = boolean::union(&body, &dot, Rule::NonZero);
        let colour = Rgba::rgb(0.7, 0.5, 0.1);
        let cells = [
            (Cell { rings: body.clone(), color: Rgba::rgb(0.2, 0.6, 0.9), is_line: false }, body.clone()),
            (Cell { rings: dot.clone(), color: colour, is_line: false }, dot.clone()),
        ];
        let metal = cloisonne(&edge, &Vec::new(), &cells, 0.02, &CancelToken::new()).expect("metal");
        let wall = boolean::simplify(&path::contours(&metal.wall, 1e-4), Rule::EvenOdd);
        let under = boolean::intersection(&wall, &dot, Rule::NonZero);
        let covered = path::signed_area(&path::from_contours(&under)).abs();
        let whole = std::f64::consts::PI * 0.016 * 0.016;
        assert!(covered < whole * 0.01, "the wall covers {:.1}% of the dot", covered / whole * 100.0);
    }

    #[test]
    fn a_line_stands_in_metal() {
        let edge = disc(0.4, (0.0, 0.0));
        let fill = Cell { rings: disc(0.3, (0.0, 0.0)), color: Rgba::rgb(0.2, 0.6, 0.9), is_line: false };
        let mut bar = BezPath::new();
        bar.move_to((-0.3, 0.0));
        bar.line_to((0.3, 0.0));
        let stroked = crate::geom::stroke::round(&bar, 0.02);
        let line = Cell {
            rings: boolean::simplify(&path::contours(&stroked, path::TOLERANCE), Rule::NonZero),
            color: Rgba::rgb(0.3, 0.3, 0.3),
            is_line: true,
        };
        let cells = [(fill.clone(), fill.rings.clone()), (line.clone(), line.rings.clone())];
        let with_line = cloisonne(&edge, &line.rings, &cells, 0.02, &CancelToken::new()).expect("metal");
        let face = with_line.metal.expect("face metal");
        // The bar's middle stands in metal, where the fill alone would have
        // opened the enamel.
        let rings = boolean::simplify(&path::contours(&face, 1e-4), Rule::EvenOdd);
        assert!(path::inside(kurbo::Point::new(0.0, 0.0), &rings, false), "the line stands in metal");
    }
}
