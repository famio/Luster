//! Cutting a document into disjoint colour cells.
//!
//! Walking top to bottom, each element keeps only the area nothing above it
//! covers. A glaze (a fill that is not quite opaque) owns no area of its own:
//! it only cuts the cells beneath it, so that each side can be sampled for the
//! colour it ends up showing. A line lying directly on a fill of its own
//! colour is invisible there and gives that area back, but only where the line
//! ends on the fill: a wire that runs across a band of its own colour stays.

use std::sync::Arc;

use kurbo::Rect;

use crate::model::Rgba;
use crate::geom::boolean::{self, Rule};
use crate::geom::islands;
use crate::geom::path::Contours;
use crate::svg::read::{Cap, Element};
use crate::math;

/// Alpha at or above this is a solid paint; anything below is a glaze.
const OPAQUE: f32 = 0.99;

/// One colour cell: a region and the colour the document paints there.
#[derive(Clone, Debug)]
pub struct Cell {
    pub rings: Contours,
    pub color: Rgba,
    /// True for a cell that came from a stroke; a badge raises these as
    /// metal.
    pub is_line: bool,
}

/// The disjoint cells of a document, bottom to top.
pub fn cells(elements: &[Element], canvas: Rect) -> Vec<Cell> {
    // Hairline: far thinner than any art, far thicker than boolean residue.
    // Measured on badges drawn on a 72-unit canvas, the residue ran to 0.024
    // units and the thinnest real piece was 0.04.
    let hairline = canvas.width().max(canvas.height()) * 4e-4;

    /// A line already emitted, painted above the element being cut.
    struct Drawn {
        /// Where its cells sit in `out`.
        cells: std::ops::Range<usize>,
        color: Rgba,
        /// Glaze count when it was emitted; later glazes lie beneath it.
        glazed: usize,
        /// Solid count when it was emitted; later solids lie beneath it.
        solid: usize,
        ends: Vec<Cap>,
    }

    let mut shown = shown(elements).into_iter();
    let mut out: Vec<Cell> = Vec::new();
    let mut glazes: Vec<Contours> = Vec::new();
    let mut solids: Vec<(Contours, Rect)> = Vec::new();
    let mut lines: Vec<Drawn> = Vec::new();

    for element in elements.iter().rev() {
        let Some(color) = element.color else { continue };
        if color.a < OPAQUE {
            if !element.rings.is_empty() {
                glazes.push(element.rings.clone());
            }
            continue;
        }
        let mut visible = shown.next().unwrap_or_default();

        if !element.is_stroke {
            for line in &lines {
                if !matches(line.color, color) {
                    continue;
                }
                // What this fill takes back from the line above it.
                let mut lost: Contours = Vec::new();
                for index in line.cells.clone() {
                    let shared =
                        boolean::intersection(&out[index].rings, &element.rings, Rule::NonZero);
                    lost = if lost.is_empty() {
                        shared
                    } else {
                        boolean::union(&lost, &shared, Rule::NonZero)
                    };
                }
                // Nothing to lose: the cuts below can only shrink it, and only
                // what reaches a cap is kept, so a piece clear of every cap is
                // out before anything is cut from it.
                if lost.is_empty() || !nears(&lost, &line.ends) {
                    continue;
                }
                // Where a solid lies between the line and the fill, the line
                // does not meet the fill. Solids are cut away one by one,
                // skipping those clear of what is left.
                for (solid, box_) in &solids[line.solid..] {
                    if !box_.overlaps(bounds(&lost)) {
                        continue;
                    }
                    lost = boolean::difference(&lost, solid, Rule::NonZero);
                    if lost.is_empty() {
                        break;
                    }
                }
                if lost.is_empty() {
                    continue;
                }
                for glaze in &glazes[line.glazed..] {
                    lost = boolean::difference(&lost, glaze, Rule::NonZero);
                }
                // A backing and the glaze over it often share a path;
                // subtracting one from the other leaves zero-width contours
                // that would cut slits into the line.
                lost = substantial(&lost, hairline);
                if lost.is_empty() {
                    continue;
                }
                // Only where the line ends on the fill. A line is a wire, and a
                // wire runs on across a same-coloured band: cut there it would
                // stand in metal either side and drop to enamel between.
                lost = endings(&lost, &line.ends);
                if lost.is_empty() {
                    continue;
                }
                for index in line.cells.clone() {
                    out[index].rings = boolean::difference(&out[index].rings, &lost, Rule::NonZero);
                }
                visible = if visible.is_empty() {
                    lost
                } else {
                    boolean::union(&visible, &lost, Rule::NonZero)
                };
            }
        }

        solids.push((element.rings.clone(), bounds(&element.rings)));
        // Every boolean leaves hairline scraps along the edges it cut, and two
        // elements trimmed by one mask share an edge exactly. Scraps are not
        // art, and treated as line work they notch the margin.
        let kept = substantial(&visible, hairline);
        if !kept.is_empty() {
            let first = out.len();
            let pieces = cut(Cell { rings: kept, color, is_line: element.is_stroke }, &glazes);
            out.extend(pieces.into_iter().map(|piece| Cell {
                rings: substantial(&piece.rings, hairline),
                ..piece
            }));
            if element.is_stroke {
                lines.push(Drawn {
                    cells: first..out.len(),
                    color,
                    glazed: glazes.len(),
                    solid: solids.len(),
                    ends: element.ends.clone(),
                });
            }
        }
    }

    out.retain(|cell| !cell.rings.is_empty());
    out.reverse();
    out
}

/// What of each solid element, top to bottom, no solid element above it
/// covers.
///
/// What lies above an element is a union built up one element at a time.
/// Each element is cut by its part of the union while the next part is
/// built, rather than after it.
fn shown(elements: &[Element]) -> Vec<Contours> {
    let solid: Vec<&Element> =
        elements.iter().rev().filter(|element| element.color.is_some_and(|color| color.a >= OPAQUE)).collect();
    let mut shown: Vec<Contours> = vec![Vec::new(); solid.len()];
    rayon::scope(|scope| {
        let mut covered: Arc<Contours> = Arc::new(Vec::new());
        for (index, (element, shows)) in solid.iter().zip(shown.iter_mut()).enumerate() {
            let above = Arc::clone(&covered);
            scope.spawn(move |_| {
                *shows = if above.is_empty() {
                    element.rings.clone()
                } else {
                    boolean::difference(&element.rings, &above, Rule::NonZero)
                };
            });
            // Nothing lies below the last.
            if index + 1 < solid.len() {
                covered = Arc::new(if covered.is_empty() {
                    element.rings.clone()
                } else {
                    boolean::union(&covered, &element.rings, Rule::NonZero)
                });
            }
        }
    });
    shown
}

/// The rings without their slivers: those whose mean width (area over half the
/// perimeter) is under `hairline`.
fn substantial(rings: &Contours, hairline: f64) -> Contours {
    rings
        .iter()
        .filter(|ring| {
            let (mut area, mut length) = (0.0, 0.0);
            for (i, a) in ring.iter().enumerate() {
                let b = ring[(i + 1) % ring.len()];
                area += (a.x * b.y - b.x * a.y) / 2.0;
                length += math::distance(*a, b);
            }
            length > 0.0 && 2.0 * area.abs() / length > hairline
        })
        .cloned()
        .collect()
}

/// The pieces of `lost` that reach one of the line's caps: within the cap's
/// radius of where the stroke ends.
fn endings(lost: &Contours, ends: &[Cap]) -> Contours {
    let mut kept: Contours = Vec::new();
    for island in islands::split(lost, 0.0) {
        let rings = island.rings();
        let reaches = ends.iter().any(|end| {
            !boolean::intersection(&rings, &disc(end), Rule::NonZero).is_empty()
        });
        if reaches {
            kept.extend(rings);
        }
    }
    kept
}

/// Whether the rings come within any cap's box at all: a cheap bound on
/// `endings`, which can keep nothing when this is false.
fn nears(rings: &Contours, ends: &[Cap]) -> bool {
    let box_ = bounds(rings);
    ends.iter().any(|end| {
        Rect::new(
            end.point.x - end.radius,
            end.point.y - end.radius,
            end.point.x + end.radius,
            end.point.y + end.radius,
        )
        .overlaps(box_)
    })
}

/// A cap as a region: the disc the stroke's round end covers.
fn disc(end: &Cap) -> Contours {
    let steps = 32;
    vec![(0..steps)
        .map(|i| {
            let angle = std::f64::consts::TAU * i as f64 / steps as f64;
            kurbo::Point::new(
                end.point.x + end.radius * math::cos(angle),
                end.point.y + end.radius * math::sin(angle),
            )
        })
        .collect()]
}

fn bounds(rings: &Contours) -> Rect {
    let mut box_: Option<Rect> = None;
    for point in rings.iter().flatten() {
        let next = Rect::new(point.x, point.y, point.x, point.y);
        box_ = Some(box_.map_or(next, |b| b.union(next)));
    }
    box_.unwrap_or(Rect::ZERO)
}

/// Splits one cell along the edge of every glaze lying over it.
fn cut(cell: Cell, glazes: &[Contours]) -> Vec<Cell> {
    let mut pieces = vec![cell];
    for glaze in glazes {
        let mut next: Vec<Cell> = Vec::with_capacity(pieces.len());
        for piece in pieces {
            let under = boolean::intersection(&piece.rings, glaze, Rule::NonZero);
            let clear = if under.is_empty() {
                piece.rings.clone()
            } else {
                boolean::difference(&piece.rings, glaze, Rule::NonZero)
            };
            if under.is_empty() || clear.is_empty() {
                next.push(piece);
                continue;
            }
            next.push(Cell { rings: clear, ..piece.clone() });
            next.push(Cell { rings: under, ..piece });
        }
        pieces = next;
    }
    pieces
}

/// Whether two colours are equal to within 1.5/255 per sRGB channel.
fn matches(a: Rgba, b: Rgba) -> bool {
    let near = 1.5 / 255.0;
    (a.r - b.r).abs() < near && (a.g - b.g).abs() < near && (a.b - b.b).abs() < near
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CancelToken;
    use crate::geom::path::{from_contours, signed_area};
    use crate::svg::read;

    fn cells_of(svg: &str) -> Vec<Cell> {
        let document = read::read(svg.as_bytes(), &CancelToken::new()).expect("readable");
        cells(&document.elements, document.frame)
    }

    fn area(cell: &Cell) -> f64 {
        signed_area(&from_contours(&cell.rings)).abs()
    }

    #[test]
    fn what_is_covered_belongs_to_the_shape_on_top() {
        let cells = cells_of(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
                <rect width="100" height="100" fill="#ff0000"/>
                <rect x="0" y="0" width="100" height="50" fill="#0000ff"/>
            </svg>"##,
        );
        assert_eq!(cells.len(), 2);
        // Bottom to top: the red keeps only the half the blue does not cover.
        assert!((area(&cells[0]) - 5000.0).abs() < 1.0, "{}", area(&cells[0]));
        assert!((area(&cells[1]) - 5000.0).abs() < 1.0);
    }

    #[test]
    fn a_glaze_owns_no_area_but_cuts_what_is_under_it() {
        let cells = cells_of(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
                <rect width="100" height="100" fill="#ffffff"/>
                <rect width="100" height="50" fill="#0000ff" fill-opacity="0.5"/>
            </svg>"##,
        );
        // One fill, split along the glaze's edge: each half is sampled on its own.
        assert_eq!(cells.len(), 2);
        for cell in &cells {
            assert!((area(cell) - 5000.0).abs() < 1.0, "{}", area(cell));
            assert!(!cell.is_line);
        }
    }

    #[test]
    fn a_stroke_becomes_a_line_cell() {
        let cells = cells_of(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
                <circle cx="50" cy="50" r="40" fill="#7040a0"/>
                <path fill="none" stroke="#4d4d4d" stroke-width="3" d="M18,58 C 38,34 62,72 82,46"/>
            </svg>"##,
        );
        assert_eq!(cells.len(), 2);
        assert!(cells.iter().any(|c| c.is_line), "the stroke is line work");
        assert!(cells.iter().any(|c| !c.is_line), "the disc is a fill");
    }

    /// Whether any cell of that kind covers the point.
    fn covers(cells: &[Cell], x: f64, y: f64, line: bool) -> bool {
        cells
            .iter()
            .filter(|cell| cell.is_line == line)
            .any(|cell| crate::geom::path::inside(kurbo::Point::new(x, y), &cell.rings, false))
    }

    /// A stroke lying on a fill of its own colour is invisible where it ends on
    /// it, and the fill takes that area back.
    #[test]
    fn a_line_on_its_own_colour_is_not_a_line() {
        let cells = cells_of(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
                <rect x="20" y="20" width="60" height="30" fill="#FFFFFF"/>
                <path fill="none" stroke="#FFFFFF" stroke-width="6" stroke-linecap="round" d="M10,70 L50,45"/>
                <rect x="20" y="20" width="60" height="30" fill="#FF0000" fill-opacity="0.6"/>
            </svg>"##,
        );
        assert!(cells.iter().any(|c| c.is_line), "the stroke is gone altogether");
        assert!(covers(&cells, 30.0, 57.5, true), "the stroke is not a line where it crosses bare canvas");
        assert!(!covers(&cells, 49.0, 46.0, true), "the stroke's end is a line where it lies on its own colour");
        assert!(covers(&cells, 49.0, 46.0, false), "nothing is painted where the stroke's end was");
    }

    /// Only a cap gives itself up: a wire that runs across a band of its own
    /// colour keeps its full width.
    #[test]
    fn a_line_crossing_its_own_colour_stays_a_line() {
        let cells = cells_of(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
                <rect x="40" y="0" width="20" height="100" fill="#C58A2B"/>
                <rect x="0" y="62" width="100" height="20" fill="#C58A2B"/>
                <path fill="none" stroke="#C58A2B" stroke-width="6" stroke-linecap="round" d="M10,50 H90"/>
                <path fill="none" stroke="#C58A2B" stroke-width="6" d="M20,80 H80 V95 H20 Z"/>
            </svg>"##,
        );
        for x in [42.0, 50.0, 58.0] {
            assert!(covers(&cells, x, 47.5, true), "the stroke lost its top edge crossing the band at x = {x}");
            assert!(covers(&cells, x, 52.5, true), "the stroke lost its bottom edge crossing the band at x = {x}");
        }
        // The closed stroke's top runs along the horizontal band's lower edge,
        // half over it: it keeps both halves.
        assert!(covers(&cells, 50.0, 78.5, true), "the closed stroke lost the half that overlaps the band");
        assert!(covers(&cells, 50.0, 81.5, true));
    }

    #[test]
    fn hairline_scraps_are_dropped() {
        // Two fills sharing an edge exactly: the lower one keeps nothing but a
        // sliver, which is not art.
        let cells = cells_of(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
                <rect x="10" y="10" width="80" height="80" fill="#336699"/>
                <rect x="10" y="10" width="80" height="80" fill="#993366"/>
            </svg>"##,
        );
        assert_eq!(cells.len(), 1, "only the shape on top is left");
    }
}
