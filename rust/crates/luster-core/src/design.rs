//! What a document becomes before any metal is struck: one silhouette, its
//! line work, and one region per colour, all placed in the 0...1 y-down square
//! the art is normalized to.

use kurbo::{Affine, BezPath, Rect};

use crate::model::Rgba;
use crate::geom::boolean::{self, Rule};
use crate::geom::path::{self, Contours};
use crate::svg::cells::{self, Cell};
use crate::svg::composite;
use crate::svg::read::{self, Element};
use crate::{CancelToken, Error};

/// A document read into art: everything the badge is struck from.
#[derive(Clone, Debug)]
pub struct Artwork {
    /// The union of every painted region: one closed silhouette.
    pub silhouette: Contours,
    /// The stroked regions: the art's line work, which becomes raised wire.
    pub detail: Contours,
    /// One region per colour. Empty when the art has fewer than two colours;
    /// the badge then takes a single enamel.
    pub cells: Vec<Cell>,
    /// The badge's edge, traced from the document's alpha coverage.
    pub outline: Option<BezPath>,
    /// The document's paint, from which each face takes its true colour.
    pub painting: Option<Painting>,
}

/// The document's layers, and the placement that maps them onto the art.
#[derive(Clone, Debug)]
pub struct Painting {
    pub elements: Vec<Element>,
    pub frame: Rect,
    pub placement: Affine,
}

/// Reads an SVG into art normalized to the 0...1 y-down square, fitted by the
/// canvas the document declares.
pub fn artwork(data: &[u8], cancel: &CancelToken) -> Result<Artwork, Error> {
    let document = read::read(data, cancel)?;
    if document.elements.is_empty() {
        return Err(Error::NothingToMint);
    }
    cancel.check()?;

    let frame = if document.frame.width() > 0.0 && document.frame.height() > 0.0 {
        document.frame
    } else {
        composite::bounds(&document.elements)
    };
    // The silhouette needs nothing of the cells, so it is joined up while
    // they are cut.
    let (reading, (silhouette, detail)) = rayon::join(
        || {
            let read = cells::cells(&document.elements, frame);
            cancel.check()?;
            composite::sampling(read, &document.elements, frame, cancel)
        },
        || {
            // A union rather than one merged path, so a hole in one element
            // cannot cut into another.
            let mut silhouette: Contours = Vec::new();
            let mut detail: Contours = Vec::new();
            for element in &document.elements {
                silhouette = if silhouette.is_empty() {
                    element.rings.clone()
                } else {
                    boolean::union(&silhouette, &element.rings, Rule::NonZero)
                };
                if element.is_stroke {
                    detail.extend(element.rings.iter().cloned());
                }
            }
            (silhouette, detail)
        },
    );
    let reading = reading?;
    if silhouette.is_empty() {
        return Err(Error::NothingToMint);
    }

    let placement = placing(frame);
    let place = |rings: &Contours| -> Contours {
        rings.iter().map(|ring| ring.iter().map(|p| placement * *p).collect()).collect()
    };
    // A single colour is not a colour separation (unfilled icons default to
    // black): such art takes the badge's own enamel, and its lines become wire.
    let merged = merging(&reading.cells, placement);
    let cells = if merged.len() > 1 { merged } else { Vec::new() };
    let painting = (!cells.is_empty()).then(|| Painting {
        elements: document.elements.clone(),
        frame,
        placement,
    });

    Ok(Artwork {
        silhouette: place(&silhouette),
        detail: place(&detail),
        cells,
        outline: reading.outline.map(|mut path| {
            path.apply_affine(placement);
            path
        }),
        painting,
    })
}

/// Merges the disjoint cells into one region per colour: one material and one
/// extrusion each. Colours are matched on a coarse key so near-duplicates join;
/// the merged cell takes the average of its members' colours.
fn merging(cells: &[Cell], placement: Affine) -> Vec<Cell> {
    /// Colour key quantized to 16 levels per channel.
    fn key(color: Rgba) -> u32 {
        let step = |v: f32| (v.clamp(0.0, 1.0) * 15.0).round() as u32;
        step(color.r) << 10 | step(color.g) << 5 | step(color.b)
    }

    let mut order: Vec<u32> = Vec::new();
    let mut shapes: Vec<(u32, Contours, Vec<Rgba>)> = Vec::new();
    for cell in cells {
        // Lines get their own buckets: they are raised as metal, so they must
        // not merge into the enamel beside them.
        let bucket = key(cell.color) << 1 | u32::from(cell.is_line);
        match shapes.iter_mut().find(|(b, _, _)| *b == bucket) {
            Some((_, rings, colors)) => {
                rings.extend(cell.rings.iter().cloned());
                colors.push(cell.color);
            }
            None => {
                order.push(bucket);
                shapes.push((bucket, cell.rings.clone(), vec![cell.color]));
            }
        }
    }

    shapes
        .into_iter()
        .filter(|(_, rings, colors)| !rings.is_empty() && !colors.is_empty())
        .map(|(bucket, rings, colors)| {
            let count = colors.len() as f32;
            Cell {
                rings: rings
                    .iter()
                    .map(|ring| ring.iter().map(|p| placement * *p).collect())
                    .collect(),
                color: Rgba {
                    r: colors.iter().map(|c| c.r).sum::<f32>() / count,
                    g: colors.iter().map(|c| c.g).sum::<f32>() / count,
                    b: colors.iter().map(|c| c.b).sum::<f32>() / count,
                    a: 1.0,
                },
                is_line: bucket & 1 == 1,
            }
        })
        .collect()
}

/// The transform that maps `frame` onto the unit square: longest side 1,
/// centered, still y-down.
pub fn placing(frame: Rect) -> Affine {
    let scale = 1.0 / frame.width().max(frame.height());
    Affine::translate((0.5, 0.5)) * Affine::scale(scale) * Affine::translate((-frame.center().x, -frame.center().y))
}

/// The signed area a region encloses.
pub fn area(rings: &Contours) -> f64 {
    path::signed_area(&path::from_contours(rings))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn art(svg: &str) -> Artwork {
        artwork(svg.as_bytes(), &CancelToken::new()).expect("readable")
    }

    #[test]
    fn the_art_is_placed_in_the_unit_square() {
        let art = art(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
                <rect x="10" y="10" width="80" height="80" fill="#7040a0"/>
            </svg>"##,
        );
        let box_ = kurbo::Shape::bounding_box(&path::from_contours(&art.silhouette));
        assert!((box_.x0 - 0.1).abs() < 1e-6 && (box_.width() - 0.8).abs() < 1e-6, "{box_:?}");
    }

    #[test]
    fn one_colour_takes_the_badges_own_enamel() {
        let art = art(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
                <circle cx="50" cy="50" r="40" fill="#7040a0"/>
            </svg>"##,
        );
        assert!(art.cells.is_empty(), "a single colour is not a colour separation");
        assert!(art.painting.is_none());
    }

    #[test]
    fn two_colours_become_two_cells() {
        let art = art(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
                <circle cx="50" cy="50" r="40" fill="#7040a0"/>
                <circle cx="50" cy="50" r="20" fill="#60dc00"/>
            </svg>"##,
        );
        assert_eq!(art.cells.len(), 2);
        assert!(art.painting.is_some());
        // The ring keeps the area the inner disc does not cover.
        let areas: Vec<f64> = art.cells.iter().map(|c| area(&c.rings).abs()).collect();
        let outer = std::f64::consts::PI * 0.16 - std::f64::consts::PI * 0.04;
        assert!(areas.iter().any(|a| (a - outer).abs() < 0.01), "{areas:?}");
    }

    #[test]
    fn a_stroke_is_kept_as_line_work() {
        let art = art(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
                <circle cx="50" cy="50" r="40" fill="#7040a0"/>
                <path fill="none" stroke="#4d4d4d" stroke-width="3" d="M18,58 C 38,34 62,72 82,46"/>
            </svg>"##,
        );
        assert!(!art.detail.is_empty(), "the stroke is line work");
        assert!(art.cells.iter().any(|c| c.is_line));
    }

    #[test]
    fn nothing_painted_is_nothing_to_mint() {
        let empty = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"/>"##;
        assert_eq!(artwork(empty.as_bytes(), &CancelToken::new()).unwrap_err(), Error::NothingToMint);
    }
}
