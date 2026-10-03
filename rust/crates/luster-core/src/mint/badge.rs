//! Striking a badge from read artwork.

use kurbo::{Affine, Rect};
use rayon::prelude::*;

use crate::model::{ENAMEL_COLOR, GOLD, Material, MaterialRole, MintOptions, Rgba};
use crate::design::Artwork;
use crate::geom::boolean::{self, Rule};
use crate::geom::islands;
use crate::geom::path::{self, Contours};
use crate::geom::stroke;
use crate::mesh::{Faces, Slab};
use crate::paint::{self, Face};
use crate::texture::Texture;
use crate::{CancelToken, Error};

use super::cloisonne::{self, WALL_ROLL};
use super::consts::*;
use crate::math;

/// One piece of a badge: a region and the slab it is extruded into.
#[derive(Clone, Debug)]
pub struct Piece {
    pub name: String,
    pub material: u32,
    pub rings: Contours,
    pub slab: Slab,
    /// True for a face that takes its colour from the document once it is
    /// built, rather than the flat colour of its material.
    pub is_painted: bool,
    /// True for plated metal's glaze: a painted face that takes the colour
    /// painted nearest each point of it, and is domed from `slab.back` up to
    /// `slab.front`, `slab.roll` in from its outline, rather than extruded.
    pub glaze: bool,
    /// Which of `rings` carry `slab.roll`; empty for all of them.
    pub rolled: Vec<bool>,
}

/// A struck badge, before it is turned into meshes.
pub struct Struck {
    pub pieces: Vec<Piece>,
    pub materials: Vec<Material>,
    /// Textures the materials refer to: what the document paints on a face
    /// whose colour is not flat.
    pub textures: Vec<Texture>,
}

/// Strikes a badge: the art's own silhouette in metal, its fills set into it
/// as enamel, recessed below the metal its lines stand in; the metal plated in
/// the art's colours unless `options.metal_lines` keeps it bare.
pub fn strike(art: &Artwork, options: &MintOptions, cancel: &CancelToken) -> Result<Struck, Error> {
    // Art and edge share one placement, so they stay registered.
    let (placement, margin) = badge_placement(&art.silhouette)?;
    let place = |rings: &Contours| -> Contours {
        rings.iter().map(|ring| ring.iter().map(|p| placement * *p).collect()).collect()
    };
    let silhouette = place(&art.silhouette);
    // The edge traced from the document's coverage, or the union of the shapes.
    let edge = match &art.outline {
        Some(outline) => {
            let mut outline = outline.clone();
            outline.apply_affine(placement);
            boolean::simplify(&path::contours(&outline, path::TOLERANCE), Rule::EvenOdd)
        }
        None => silhouette.clone(),
    };
    cancel.check()?;

    // The metal is plated in the art's colours where the art has colours to
    // give and they are asked for.
    let plated = art.painting.is_some() && !options.metal_lines;
    // The enamel's floor, sunk `BADGE_RELIEF` below the metal.
    let floor = BADGE_ENAMEL - BADGE_RELIEF;
    // The plate rises to one step below the enamel: coplanar faces z-fight.
    let flush = floor - CELL_STEP;

    let mut materials = vec![
        Material {
            name: "gold".into(),
            role: MaterialRole::Plated,
            // The renderer plates the metal in the colour it is asked for.
            color: GOLD,
            metallic: 1.0,
            roughness: 0.16,
            texture: None,
        },
        Material {
            name: "gold-back".into(),
            role: MaterialRole::GoldBack,
            color: GOLD,
            metallic: 1.0,
            // A badge's reverse is satin, not polished.
            roughness: 0.40,
            texture: None,
        },
    ];
    let mut pieces: Vec<Piece> = Vec::new();

    // The cells, placed once: the metal is cut from them.
    let placed: Vec<(crate::svg::cells::Cell, Contours)> = art
        .cells
        .iter()
        .map(|cell| (cell.clone(), place(&cell.rings)))
        .filter(|(_, rings)| !rings.is_empty())
        .collect();
    // The lines that stand in metal are the visible ones: a stroke that a
    // later fill covers is not metal there.
    let lines: Contours =
        placed.iter().filter(|(cell, _)| cell.is_line).flat_map(|(_, rings)| rings.clone()).collect();

    // The top of the metal, where a wall's face sits.
    let finish = BADGE_ENAMEL + CELL_LADDER as f32 * CELL_STEP;
    let metal = cloisonne::cloisonne(&edge, &lines, &placed, margin, cancel)?;
    cancel.check()?;

    body(&mut pieces, &metal, finish, flush);
    if let Some(face) = &metal.metal {
        face_metal(&mut pieces, &mut materials, face, metal.wall_top.as_ref(), &placed, finish, plated);
    }
    cancel.check()?;

    enamel(&mut pieces, &mut materials, &edge, &placed, art, floor, cancel)?;

    // Art without colours of its own keeps its line work as wire, on top.
    if art.cells.is_empty() {
        cancel.check()?;
        wire(&mut pieces, &silhouette, &place(&art.detail));
    }

    let textures = colour_faces(&mut pieces, &mut materials, art, placement, cancel)?;
    Ok(Struck { pieces, materials, textures })
}

/// The metal the badge stands on: the plate, which carries the reverse and
/// the enamel's floor, and the wall, the whole side from reverse to face in
/// one piece, which carries the roll. Two separately traced edges would show
/// a ledge.
fn body(pieces: &mut Vec<Piece>, metal: &cloisonne::Metal, finish: f32, flush: f32) {
    let plate_rings = rings_of(&metal.plate);
    let plate_slab = Slab {
        front: BADGE_DEPTH / 2.0 + flush,
        back: -BADGE_DEPTH / 2.0,
        roll: 0.0,
        faces: Faces { front: true, sides: true, back: false },
    };
    pieces.push(Piece {
        name: "body.0".into(),
        material: 0,
        rings: plate_rings.clone(),
        slab: plate_slab,
        is_painted: false,
        glaze: false,
        rolled: Vec::new(),
    });
    pieces.push(Piece {
        name: "body-back.0".into(),
        material: 1,
        rings: plate_rings,
        slab: Slab { faces: Faces { front: false, sides: false, back: true }, ..plate_slab },
        is_painted: false,
        glaze: false,
        rolled: Vec::new(),
    });
    let first = pieces.len();
    add(
        pieces,
        "edge",
        0,
        &rings_of(&metal.wall),
        Slab {
            front: BADGE_DEPTH / 2.0 + finish,
            back: BADGE_DEPTH / 2.0 + finish - (BADGE_DEPTH + finish - WALL_LIFT),
            roll: WALL_ROLL,
            faces: Faces::ALL,
        },
        false,
    );
    // Only the badge's own edge is rolled. The wall's inner edge meets the
    // enamel or the face metal, and the wall is narrower than two rolls:
    // rolled along both edges, it would fold over itself.
    let outline = rings_of(&metal.outline);
    for piece in &mut pieces[first..] {
        piece.rolled = piece.rings.iter().map(|ring| on_outline(ring, &outline)).collect();
    }
}

/// All the metal on the face as one piece, and, where the metal is plated in
/// the document's colours, the glaze laid on it.
fn face_metal(
    pieces: &mut Vec<Piece>,
    materials: &mut Vec<Material>,
    face: &kurbo::BezPath,
    wall_top: Option<&kurbo::BezPath>,
    placed: &[(crate::svg::cells::Cell, Contours)],
    finish: f32,
    plated: bool,
) {
    // The art's lines plus the margin, opened over the enamel.
    let rings = rings_of(face);
    add(
        pieces,
        "rim",
        0,
        &rings,
        Slab {
            front: BADGE_DEPTH / 2.0 + finish,
            back: BADGE_DEPTH / 2.0 + finish - FACE_DEPTH.min(BADGE_DEPTH / 2.0),
            roll: 0.0,
            faces: Faces::LAID,
        },
        false,
    );
    // The face metal plated in the document's colours, laid on it
    // and domed when `GLAZE_DOME_WIDTH` asks. Nothing is laid below
    // the metal's face, so its sides still show round every
    // opening.
    if plated {
        // The wall's top too, where the fills run on under it.
        let glazed = match wall_top {
            Some(top) => boolean::union(&rings, &rings_of(top), Rule::NonZero),
            None => rings,
        };
        // Only where the document paints, as its own shapes have it
        // rather than the coarser traced edge: metal it leaves
        // bare, a counter or the metal joining two pieces, stays
        // metal rather than taking the colour next to it.
        let painted: Contours = placed.iter().flat_map(|(_, rings)| rings.iter().cloned()).collect();
        let glazed = boolean::intersection(&glazed, &painted, Rule::NonZero);
        // Without the hairlines left where the wall's traced top and a
        // fill's own edge nearly meet: a fill stops at the wall.
        let opened = boolean::open(&glazed, cloisonne::SLIVER);
        let first = pieces.len();
        for (name, colour, part) in glaze_parts(&opened, placed) {
            materials.push(Material {
                name: name.clone(),
                role: MaterialRole::Glaze,
                color: colour,
                metallic: 1.0,
                roughness: 0.16,
                texture: None,
            });
            add(
                pieces,
                &name,
                materials.len() as u32 - 1,
                &part,
                // The dome's foot is a step above the metal: the two
                // would otherwise be one plane there, and fight.
                Slab {
                    front: BADGE_DEPTH / 2.0 + finish + CELL_STEP,
                    back: BADGE_DEPTH / 2.0 + finish + CELL_STEP,
                    roll: 0.0,
                    faces: Faces { front: true, sides: false, back: false },
                },
                true,
            );
        }
        for piece in &mut pieces[first..] {
            // Where the edge of what is painted falls inside a
            // piece, the colour next to it carries on over it.
            piece.glaze = true;
            // One curve for all of it, so that two islands that
            // meet stand at one height. A part narrower than the
            // curve is only rounded lower. Each colour is domed on
            // its own, so a dome would dip where two colours meet.
            piece.slab.front += GLAZE_DOME_WIDTH * GLAZE_DOME_RISE;
            piece.slab.roll = GLAZE_DOME_WIDTH;
        }
    }
}

/// The enamel: a base cut to the badge's edge, and the cells over it in paint
/// order.
fn enamel(
    pieces: &mut Vec<Piece>,
    materials: &mut Vec<Material>,
    edge: &Contours,
    placed: &[(crate::svg::cells::Cell, Contours)],
    art: &Artwork,
    floor: f32,
    cancel: &CancelToken,
) -> Result<(), Error> {
    // The base enamel, cut to the same edge as the plate: the union of the
    // shapes can reach past the visible art, so the traced edge bounds it.
    materials.push(Material {
        name: "enamel-art".into(),
        role: MaterialRole::Field,
        color: ENAMEL_COLOR,
        metallic: 0.0,
        roughness: 0.08,
        texture: None,
    });
    add(pieces, "art", materials.len() as u32 - 1, edge, laid(floor), false);

    // The cells, in paint order as height: cells should not overlap, but where
    // a sliver does, the later one wins, as in the document.
    for (layer, (cell, rings)) in placed.iter().enumerate() {
        cancel.check()?;
        let rings = rings.clone();
        let name = format!("cell-{}", hex(cell.color));
        materials.push(Material {
            name: name.clone(),
            role: MaterialRole::Field,
            color: cell.color,
            metallic: 0.0,
            roughness: 0.08,
            texture: None,
        });
        let rise = floor + (1 + layer % CELL_LADDER) as f32 * CELL_STEP;
        // Lines under the metal are out of sight and keep the cell's average.
        let is_painted = art.painting.is_some() && !cell.is_line;
        add(pieces, &name, materials.len() as u32 - 1, &rings, laid(rise), is_painted);
    }
    Ok(())
}

/// A layer laid on the face at `rise`: enamel, or wire.
fn laid(rise: f32) -> Slab {
    Slab {
        front: BADGE_DEPTH / 2.0 + rise,
        back: BADGE_DEPTH / 2.0 + rise - FACE_DEPTH.min(BADGE_DEPTH / 2.0),
        roll: 0.0,
        faces: Faces::LAID,
    }
}

/// Line work as wire, on top: for art without colours of its own.
fn wire(pieces: &mut Vec<Piece>, silhouette: &Contours, detail: &Contours) {
    // The art path is stroked, not the traced edge: where the pen is wider
    // than a notch in the outline, the stroke would fill it solid.
    let mut wire = stroke::round(&path::from_contours(silhouette), WIRE_WIDTH);
    wire.extend(path::from_contours(detail).iter());
    let wire = boolean::simplify(&path::contours(&wire, path::TOLERANCE), Rule::NonZero);
    if !wire.is_empty() {
        let rise = BADGE_ENAMEL + (CELL_LADDER + 2) as f32 * CELL_STEP;
        add(pieces, "cloisonne-wire", 0, &wire, laid(rise), false);
    }
}

/// Gives every painted face the colour the document paints there: a texture
/// where the colour changes across the face, its own flat colour where it does
/// not.
///
/// Faces that come out the same colour share a material, so a badge carries
/// the colours it has rather than one material per piece.
fn colour_faces(
    pieces: &mut [Piece],
    materials: &mut Vec<Material>,
    art: &Artwork,
    placement: Affine,
    cancel: &CancelToken,
) -> Result<Vec<Texture>, Error> {
    let Some(painting) = &art.painting else { return Ok(Vec::new()) };
    // Faces are read in the art's placed square, which is where the document
    // was laid out; a piece is built centered and y-up.
    let to_art = path::to_badge_units().inverse();
    let placed = to_art * placement;

    let mut textures: Vec<Texture> = Vec::new();
    // Every enamel material already made is a colour a face may read; a face
    // that reads one of them keeps it rather than making another.
    let mut flats: Vec<(Rgba, MaterialRole, u32)> = materials
        .iter()
        .enumerate()
        .filter(|(_, m)| matches!(m.role, MaterialRole::Field | MaterialRole::Glaze))
        .map(|(index, m)| (m.color, m.role, index as u32))
        .collect();
    // Each face is read on its own raster, so they are read side by side;
    // what they read is then taken in order, as one pass would.
    cancel.check()?;
    let read: Vec<Option<Face>> = pieces
        .par_iter()
        .map(|piece| {
            if !piece.is_painted || cancel.is_cancelled() {
                return None;
            }
            let region: Contours =
                piece.rings.iter().map(|ring| ring.iter().map(|p| to_art * *p).collect()).collect();
            let rect = crate::geom::path::bounds(&region)?;
            if piece.glaze {
                paint::nearest(painting, placed, &region, rect).map(|face| face.bounded(GLAZE_DARKEST, GLAZE_WHITE))
            } else {
                paint::face(painting, placed, &region, rect)
            }
        })
        .collect();
    for (piece, face) in pieces.iter_mut().zip(read) {
        if !piece.is_painted {
            continue;
        }
        cancel.check()?;
        let template = materials[piece.material as usize].clone();
        match face {
            Some(Face::Flat(color)) => {
                let same = |a: Rgba, b: Rgba| {
                    let near = 0.5 / 255.0;
                    (a.r - b.r).abs() < near && (a.g - b.g).abs() < near && (a.b - b.b).abs() < near
                };
                // Of the same finish: a glaze never takes a cell's material.
                let found = flats.iter().find(|(known, role, _)| *role == template.role && same(*known, color));
                match found {
                    Some((_, _, material)) => piece.material = *material,
                    None => {
                        materials.push(Material {
                            name: format!("cell-{}", hex(color)),
                            color,
                            ..template
                        });
                        let material = materials.len() as u32 - 1;
                        flats.push((color, template.role, material));
                        piece.material = material;
                    }
                }
            }
            Some(Face::Painted { texture, average }) => {
                textures.push(texture);
                materials.push(Material {
                    name: format!("{}-{}", template.name, piece.name),
                    color: average,
                    // The texture is the front face's alone; the sides take
                    // the average, which is what the flat colour is for.
                    texture: Some(textures.len() as u32 - 1),
                    ..template
                });
                piece.material = materials.len() as u32 - 1;
            }
            // Too thin to read: the cell's own colour stands.
            None => {}
        }
    }
    Ok(textures)
}

/// Plated metal's glaze cut into one part per cell, each named and coloured
/// for it, so that where two colours meet they meet along an edge. Read as
/// one face, the two would meet along a step of one texture's texels, blurred.
///
/// The glaze is taken as one region with its seams closed: cells cut from
/// one another, as strokes over strokes are, meet across hairline gaps, and
/// each side of one would be domed down to its foot there, a groove where two
/// strokes run into each other. The cells are disjoint, so each takes what of
/// `opened`, the glaze before its seams are closed, it covers; the seams
/// belong to neither, and each goes whole to the cell nearest it.
fn glaze_parts(opened: &Contours, placed: &[(crate::svg::cells::Cell, Contours)]) -> Vec<(String, Rgba, Contours)> {
    // A cell's colour is only an average of what it paints; it stands where
    // its part is too thin to be read.
    let bounded = |colour: Rgba| match Face::Flat(colour).bounded(GLAZE_DARKEST, GLAZE_WHITE) {
        Face::Flat(colour) => colour,
        Face::Painted { average, .. } => average,
    };
    let bounds: Vec<Option<Rect>> = placed.iter().map(|(_, rings)| crate::geom::path::bounds(rings)).collect();
    // The seams are closed alongside the cutting: neither waits on the other.
    let (mut parts, seams) = rayon::join(
        || -> Vec<Contours> {
            placed
                .par_iter()
                .zip(&bounds)
                .map(|((_, rings), rect)| match (rect, crate::geom::path::bounds(opened)) {
                    (Some(rect), Some(glaze)) if rect.overlaps(glaze) => {
                        boolean::intersection(opened, rings, Rule::NonZero)
                    }
                    _ => Vec::new(),
                })
                .collect()
        },
        || boolean::difference(&boolean::close(opened, GLAZE_SEAM), opened, Rule::NonZero),
    );
    for island in islands::split(&seams, SPECKS) {
        let count = island.outer.len() as f64;
        let (x, y) = island.outer.iter().fold((0.0, 0.0), |(x, y), p| (x + p.x / count, y + p.y / count));
        let at = kurbo::Point::new(x, y);
        let mut best: Option<(f64, usize)> = None;
        for (index, (_, rings)) in placed.iter().enumerate() {
            let Some(rect) = bounds[index] else { continue };
            let outside = math::hypot((rect.x0 - at.x).max(at.x - rect.x1).max(0.0), (rect.y0 - at.y).max(at.y - rect.y1).max(0.0));
            if best.is_some_and(|(nearest, _)| outside >= nearest) {
                continue;
            }
            let distance = distance_to(at, rings);
            if best.is_none_or(|(nearest, _)| distance < nearest) {
                best = Some((distance, index));
            }
        }
        if let Some((_, index)) = best {
            parts[index].extend(island.rings());
        }
    }
    parts
        .into_iter()
        .zip(placed)
        .enumerate()
        .filter(|(_, (part, _))| !part.is_empty())
        .map(|(index, (part, (cell, _)))| (format!("plated-metal-{index}"), bounded(cell.color), part))
        .collect()
}

/// How far `p` lies from the nearest edge of `rings`.
fn distance_to(p: kurbo::Point, rings: &Contours) -> f64 {
    rings
        .iter()
        .flat_map(|ring| ring.iter().zip(ring.iter().cycle().skip(1)))
        .map(|(a, b)| {
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let length = dx * dx + dy * dy;
            let t = if length > 0.0 { (((p.x - a.x) * dx + (p.y - a.y) * dy) / length).clamp(0.0, 1.0) } else { 0.0 };
            math::hypot(a.x + t * dx - p.x, a.y + t * dy - p.y)
        })
        .fold(f64::INFINITY, f64::min)
}

/// Whether `ring` runs along `outline`: whether its points lie on it.
fn on_outline(ring: &[kurbo::Point], outline: &Contours) -> bool {
    let near = |p: kurbo::Point| distance_to(p, outline) < 1e-4;
    // A few of its points, not all: a ring either is the outline or is well
    // inside it.
    let step = (ring.len() / 8).max(1);
    ring.iter().step_by(step).filter(|p| near(**p)).count() * 2 > ring.len().div_ceil(step)
}

/// A traced path as disjoint rings: nesting means holes, not winding.
fn rings_of(path: &kurbo::BezPath) -> Contours {
    boolean::simplify(&path::contours(path, path::TOLERANCE), Rule::EvenOdd)
}

/// Adds one piece per island, so that each is extruded and textured on its
/// own bounds.
fn add(pieces: &mut Vec<Piece>, name: &str, material: u32, rings: &Contours, slab: Slab, is_painted: bool) {
    for (index, island) in islands::split(rings, SPECKS).into_iter().enumerate() {
        pieces.push(Piece {
            name: format!("{name}.{index}"),
            material,
            rings: island.rings(),
            slab,
            is_painted,
            glaze: false,
            rolled: Vec::new(),
        });
    }
}

/// Places the art in the badge's square, leaving room for the rolled edge round
/// it, and returns in badge units the margin that joins the art's islands.
fn badge_placement(art: &Contours) -> Result<(Affine, f64), Error> {
    let box_ = crate::geom::path::bounds(art).ok_or(Error::NothingToMint)?;
    let extent = box_.width().max(box_.height());
    if extent <= 0.0 {
        return Err(Error::NothingToMint);
    }
    let fit = 1.0 / (extent + 2.0 * f64::from(WALL_ROLL));
    // The art is scaled to leave the roll inside the unit square, then moved
    // into badge units: y up, centered.
    let placement = path::to_badge_units()
        * Affine::translate((0.5, 0.5))
        * Affine::scale(fit)
        * Affine::translate((-box_.center().x, -box_.center().y));
    Ok((placement, fit * BADGE_RIM))
}

fn hex(c: Rgba) -> String {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("{:02x}{:02x}{:02x}", byte(c.r), byte(c.g), byte(c.b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design;

    fn strike_svg(svg: &str) -> Struck {
        let art = design::artwork(svg.as_bytes(), &CancelToken::new()).expect("art");
        strike(&art, &MintOptions::default(), &CancelToken::new()).expect("a badge")
    }

    const DISC: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
        <circle cx="50" cy="50" r="40" fill="#7040a0"/>
    </svg>"##;

    const TWO_COLOURS: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
        <circle cx="50" cy="50" r="40" fill="#7040a0"/>
        <circle cx="50" cy="50" r="20" fill="#60dc00"/>
    </svg>"##;

    /// Where a grey line runs into a white one, each colour is plated on its
    /// own part and reads flat: read as one face, they would meet along a step
    /// of one texture's texels.
    #[test]
    fn lines_of_two_colours_plate_apart() {
        let struck = strike_svg(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
                <circle cx="50" cy="50" r="45" fill="#7040a0"/>
                <path d="M20 50H80" stroke="#4d4d4d" stroke-width="6"/>
                <circle cx="50" cy="50" r="30" fill="none" stroke="#ffffff" stroke-width="6"/>
            </svg>"##,
        );
        let glazes: Vec<&Material> = struck
            .pieces
            .iter()
            .filter(|p| p.glaze)
            .map(|p| &struck.materials[p.material as usize])
            .collect();
        assert!(glazes.iter().all(|m| m.texture.is_none()), "a glaze read as a texture");
        let lightest = |m: &&Material| m.color.r.max(m.color.g).max(m.color.b);
        assert!(glazes.iter().any(|m| lightest(m) > 0.99), "no white part");
        assert!(glazes.iter().any(|m| lightest(m) < 0.6), "no grey part");
    }

    #[test]
    fn a_badge_is_a_plate_with_enamel_on_it() {
        let struck = strike_svg(DISC);
        let names: Vec<&str> = struck.pieces.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"body.0"), "{names:?}");
        assert!(names.contains(&"art.0"));
        assert!(struck.materials.iter().any(|m| m.role == MaterialRole::Plated));
    }

    /// A fill runs on to the badge's edge: the metal round it is only the
    /// wall's roll, not a margin of its own.
    #[test]
    fn a_fill_runs_on_to_the_edge() {
        let struck = strike_svg(DISC);
        let wall = struck.pieces.iter().find(|p| p.name.starts_with("edge")).expect("a wall");
        let enamel = struck.pieces.iter().find(|p| p.name == "art.0").expect("the enamel");
        let edge = crate::geom::path::bounds(&wall.rings).unwrap();
        let fill = crate::geom::path::bounds(&enamel.rings).unwrap();
        let roll = f64::from(WALL_ROLL);
        assert!(edge.width() > fill.width(), "{edge:?} vs {fill:?}");
        assert!(edge.width() < fill.width() + 4.0 * roll, "a margin past the fill: {edge:?} vs {fill:?}");
    }

    #[test]
    fn each_colour_becomes_its_own_piece_and_material() {
        let struck = strike_svg(TWO_COLOURS);
        let cells: Vec<&Piece> = struck.pieces.iter().filter(|p| p.name.starts_with("cell-")).collect();
        assert_eq!(cells.len(), 2);
        assert!(cells.iter().all(|p| p.is_painted), "faces are coloured from the document");
        // The base enamel and one per colour; the faces read the colours
        // their cells already carry.
        assert_eq!(struck.materials.iter().filter(|m| m.role == MaterialRole::Field).count(), 3);
    }

    #[test]
    fn the_pieces_are_stacked_without_two_faces_at_one_height() {
        let struck = strike_svg(TWO_COLOURS);
        let mut heights: Vec<f32> =
            struck.pieces.iter().filter(|p| p.name.starts_with("cell-")).map(|p| p.slab.front).collect();
        heights.sort_by(f32::total_cmp);
        for pair in heights.windows(2) {
            assert!(pair[1] - pair[0] >= CELL_STEP * 0.9, "{heights:?}");
        }
        let body = struck.pieces.iter().find(|p| p.name == "body.0").unwrap();
        assert!(heights[0] > body.slab.front, "the enamel sits above the plate");
    }

    /// Every piece is built with the face it was asked for: what the
    /// extruder makes must cover the region it was given. A rolled piece
    /// loses a band of its own roll, and no more.
    #[test]
    fn every_piece_keeps_its_face() {
        use crate::mesh::{MeshBuilder, VERTEX_STRIDE, extrude};
        for svg in [DISC, TWO_COLOURS] {
            let struck = strike_svg(svg);
            for piece in &struck.pieces {
                if !piece.slab.faces.front {
                    continue;
                }
                let mut mesh = MeshBuilder::new();
                extrude(&piece.rings, piece.slab, &mut mesh).expect("built");
                let vertex = |i: u32| {
                    let o = i as usize * VERTEX_STRIDE;
                    let f = |k: usize| {
                        f32::from_le_bytes(mesh.vertices[o + 4 * k..o + 4 * k + 4].try_into().unwrap())
                    };
                    [f(0), f(1), f(2)]
                };
                let built: f32 = mesh
                    .indices
                    .chunks_exact(3)
                    .map(|t| [vertex(t[0]), vertex(t[1]), vertex(t[2])])
                    .filter(|t| t.iter().all(|p| (p[2] - piece.slab.front).abs() < 1e-6))
                    .map(|[a, b, c]| {
                        ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])) / 2.0
                    })
                    .sum();
                let wanted = path::signed_area(&path::from_contours(&piece.rings)).abs() as f32;
                assert!(built > 0.0, "{} was built inside out", piece.name);
                if piece.slab.roll > 0.0 {
                    // The roll eats a band of its own width off the face,
                    // all along the outline.
                    let outline: f64 = piece
                        .rings
                        .iter()
                        .map(|ring| ring.iter().zip(ring.iter().cycle().skip(1)).map(|(a, b)| math::distance(*a, *b)).sum::<f64>())
                        .sum();
                    let band = (outline * f64::from(piece.slab.roll)) as f32;
                    assert!(
                        built < wanted && wanted - built < band * 1.05,
                        "{}: {built} of {wanted}, a band of {band}",
                        piece.name
                    );
                } else {
                    assert!(
                        (built - wanted).abs() < wanted * 0.02,
                        "{}: {built} of {wanted}",
                        piece.name
                    );
                }
            }
        }
    }

    /// A badge is walls and recessed enamel: a band round the edge,
    /// metal standing on the face, and the colours sunk below both.
    #[test]
    fn a_badge_has_walls_with_the_enamel_sunk_between_them() {
        let struck = strike_svg(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
                <circle cx="50" cy="50" r="40" fill="#7040a0"/>
                <path d="M20,50 H80" stroke="#20c040" stroke-width="4" fill="none"/>
            </svg>"##,
        );
        let names: Vec<&str> = struck.pieces.iter().map(|p| p.name.as_str()).collect();
        assert!(names.iter().any(|n| n.starts_with("edge")), "{names:?}");
        assert!(names.iter().any(|n| n.starts_with("rim")), "metal stands on the face: {names:?}");

        let wall = struck.pieces.iter().find(|p| p.name.starts_with("edge")).unwrap();
        assert!(wall.slab.roll > 0.0, "the wall carries the roll");
        let cell = struck.pieces.iter().find(|p| p.name.starts_with("cell-")).unwrap();
        assert!(
            wall.slab.front - cell.slab.front > BADGE_RELIEF * 0.9,
            "the enamel is sunk below the metal: {} under {}",
            cell.slab.front,
            wall.slab.front
        );
    }

    /// Plated metal carries the document's colours on the metal's face: a
    /// stroke's own colour on the line. The metal round a fill is not
    /// plated: the fill stops at the wall rather than running on under it.
    #[test]
    fn plated_metal_takes_the_colours_painted_on_and_near_it() {
        let art = design::artwork(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
                <circle cx="50" cy="50" r="40" fill="#7040a0"/>
                <path d="M20,50 H80" stroke="#20c040" stroke-width="4" fill="none"/>
            </svg>"##
                .as_bytes(),
            &CancelToken::new(),
        )
        .expect("art");
        let struck = strike(&art, &MintOptions::default(), &CancelToken::new()).expect("a badge");
        let glaze: Vec<&Piece> = struck.pieces.iter().filter(|p| p.name.starts_with("plated-metal")).collect();
        assert!(!glaze.is_empty(), "the metal is glazed");
        let rim = struck.pieces.iter().find(|p| p.name.starts_with("rim")).unwrap();
        assert!(glaze.iter().all(|p| p.slab.front > rim.slab.front && !p.slab.faces.sides), "laid on the metal's face");

        // The line in its own green, whether a face reads one colour or
        // several; the metal round the disc bare.
        let near = |a: u8, b: f32| a.abs_diff((b * 255.0).round() as u8) < 4;
        let has = |r: u8, g: u8, b: u8| {
            glaze.iter().any(|piece| {
                let material = &struck.materials[piece.material as usize];
                match material.texture {
                    Some(texture) => struck.textures[texture as usize].rgba.chunks_exact(4).any(|t| {
                        t[0].abs_diff(r) < 4 && t[1].abs_diff(g) < 4 && t[2].abs_diff(b) < 4
                    }),
                    None => {
                        let c = material.color;
                        near(r, c.r) && near(g, c.g) && near(b, c.b)
                    }
                }
            })
        };
        assert!(has(0x20, 0xc0, 0x40), "the line's own colour");
        assert!(!has(0x70, 0x40, 0xa0), "the metal round the disc is plated in its colour");

        // With its lines left in the metal, the same badge's metal is bare.
        let options = MintOptions { metal_lines: true, ..MintOptions::default() };
        let bare = strike(&art, &options, &CancelToken::new()).unwrap();
        assert!(!bare.pieces.iter().any(|p| p.name.starts_with("plated-metal")));
    }

    /// Plated metal is domed to one curve throughout, so that where two
    /// of its islands meet they stand at one height.
    #[test]
    fn plated_metal_stands_at_one_height() {
        let art = design::artwork(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
                <circle cx="50" cy="50" r="40" fill="#7040a0"/>
                <path d="M20,50 C40,30 60,70 80,50" stroke="#20c040" stroke-width="4" fill="none"/>
                <path d="M50,20 C30,40 70,60 50,80" stroke="#e0e0e0" stroke-width="9" fill="none"/>
                <path d="M62,28 H72" stroke="#e0e0e0" stroke-width="4" fill="none"/>
            </svg>"##
                .as_bytes(),
            &CancelToken::new(),
        )
        .expect("art");
        let struck = strike(&art, &MintOptions::default(), &CancelToken::new()).expect("a badge");
        let glaze: Vec<&Piece> = struck.pieces.iter().filter(|p| p.glaze).collect();
        assert!(glaze.len() > 1, "{} islands", glaze.len());
        assert!(glaze.windows(2).all(|w| w[0].slab.front == w[1].slab.front && w[0].slab.roll == w[1].slab.roll));
    }

    #[test]
    fn art_of_one_colour_gets_a_wire() {
        let struck = strike_svg(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
                <path d="M20,20 H80 V80 H20 Z" fill="#000000"/>
            </svg>"##,
        );
        assert!(struck.pieces.iter().any(|p| p.name.starts_with("cloisonne-wire")), "line work is raised as wire");
    }
}
