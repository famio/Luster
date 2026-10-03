//! What the document paints on one face.
//!
//! A cell's own colour is only an average: a cell under a gradient is often
//! many small faces spread across it, each nearly flat and each a different
//! colour. So every face is rendered on a raster of its own, clipped to its
//! region, and takes either the flat colour it reads or a texture of what it
//! shows.

use kurbo::{Affine, Rect};
use rayon::prelude::*;
use tiny_skia::{Mask, Pixmap};

use crate::model::Rgba;
use crate::design::Painting;
use crate::geom::path::{self, Contours};
use crate::svg::composite;
use crate::texture::Texture;
use crate::model::{encoded as to_srgb, linear as to_linear};

/// Texels per unit of the art's space. A cell is cut along every glaze's
/// edge, so colour only ever changes smoothly inside one and a coarse texture
/// holds it; the finer density is only for reading the colour of a face too
/// thin for the first.
const DENSITIES: [f64; 2] = [512.0, 2048.0];
/// Texels per unit for a face read to its nearest colours. Where the document
/// paints nothing, colours meet along texel edges, so it is read finer than a
/// cell; a whole badge still fits one tile of a 2048 sheet.
const NEAREST_DENSITY: f64 = 1024.0;
/// Channel range (of 255) below which a face is one flat colour.
const FLAT: u8 = 8;
/// How far a face's colours are carried outward past its edge, in texels.
const CARRY: usize = 24;

/// What the document paints on a face.
pub enum Face {
    /// The colour changes across it: a texture, and its average.
    Painted { texture: Texture, average: Rgba },
    /// One colour all over.
    Flat(Rgba),
}

impl Face {
    /// The face kept between two brightnesses, as light rather than as it
    /// looks: its brightest channel no higher than `limit` and no lower than
    /// `floor`. A colour outside is scaled whole, so it keeps its hue; black,
    /// which has none, is made up to the floor as grey. Colours between are
    /// left as they are.
    pub fn bounded(self, floor: f32, limit: f32) -> Face {
        let bound = |c: [f32; 3]| -> [f32; 3] {
            let mut linear = c.map(to_linear);
            let top = linear[0].max(linear[1]).max(linear[2]);
            let limit = to_linear(limit);
            if top > limit {
                linear = linear.map(|v| v * limit / top);
            }
            let top = linear[0].max(linear[1]).max(linear[2]);
            if top < floor {
                linear = if top > 1e-4 { linear.map(|v| v * floor / top) } else { [floor; 3] };
            }
            linear.map(to_srgb)
        };
        let rgba = |c: Rgba| {
            let [r, g, b] = bound([c.r, c.g, c.b]);
            Rgba { r, g, b, a: c.a }
        };
        match self {
            Face::Flat(color) => Face::Flat(rgba(color)),
            Face::Painted { mut texture, average } => {
                let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
                let bounded = |texel: [u8; 3]| -> [u8; 3] {
                    bound(texel.map(|v| f32::from(v) / 255.0)).map(byte)
                };
                // Texels come in runs of one colour: those carried past the
                // face's edge are copies, and a face read flat in places is
                // flat there. Each run is bounded once, the rows side by side.
                let row = texture.width as usize * 4;
                texture.rgba.par_chunks_mut(row).for_each(|row| {
                    let mut last: Option<([u8; 3], [u8; 3])> = None;
                    for texel in row.chunks_exact_mut(4) {
                        let colour = [texel[0], texel[1], texel[2]];
                        let out = match last {
                            Some((seen, out)) if seen == colour => out,
                            _ => bounded(colour),
                        };
                        last = Some((colour, out));
                        texel[..3].copy_from_slice(&out);
                    }
                });
                Face::Painted { texture, average: rgba(average) }
            }
        }
    }
}


/// Reads what the document paints inside `region`.
///
/// Both `region` and `rect` are in the art's placed space (the 0...1 y-down
/// square the badge is laid out in), and the texture covers `rect` with its
/// first row at the rect's top.
pub fn face(painting: &Painting, placed: Affine, region: &Contours, rect: Rect) -> Option<Face> {
    if !(rect.width() > 0.0 && rect.height() > 0.0) {
        return None;
    }
    let mut last: Option<Rendering> = None;
    for density in DENSITIES {
        let Some(rendering) = Rendering::new(painting, placed, Some(region), rect, density) else {
            continue;
        };
        if let Some(face) = rendering.read(CARRY) {
            return Some(face);
        }
        last = Some(rendering);
    }
    // Thinner than a texel even at the finer density: what little of it shows
    // is read through its antialiased edge.
    last.and_then(|rendering| rendering.edge_colour()).map(Face::Flat)
}

/// Reads what the document paints inside `region`, with every point it
/// leaves bare taking the colour painted nearest it: for a face that reaches
/// past the art, as plated metal does.
///
/// Clipped to the region, as a cell is: read across the whole rect, the
/// texels along the face's edge would hold the colour painted beside it, and
/// the face would be tinged with it there.
pub fn nearest(painting: &Painting, placed: Affine, region: &Contours, rect: Rect) -> Option<Face> {
    if !(rect.width() > 0.0 && rect.height() > 0.0) {
        return None;
    }
    Rendering::new(painting, placed, Some(region), rect, NEAREST_DENSITY)?.read(usize::MAX)
}

/// The document painted through one region, on a raster of its own.
struct Rendering {
    pixmap: Pixmap,
    /// How much of each texel the region covers, when there is a region.
    mask: Option<Mask>,
    width: usize,
    height: usize,
}

impl Rendering {
    fn new(
        painting: &Painting,
        placed: Affine,
        region: Option<&Contours>,
        rect: Rect,
        density: f64,
    ) -> Option<Rendering> {
        let width = ((rect.width() * density).ceil() as usize).clamp(4, 2048);
        let height = ((rect.height() * density).ceil() as usize).clamp(4, 2048);
        let mut pixmap = Pixmap::new(width as u32, height as u32)?;

        // The rect's own space, in texels: row 0 is the rect's top.
        let to_texels = Affine::scale_non_uniform(width as f64 / rect.width(), height as f64 / rect.height())
            * Affine::translate((-rect.x0, -rect.y0));
        let mask = match region {
            Some(region) => {
                let mut mask = Mask::new(width as u32, height as u32)?;
                let placed_region: Contours =
                    region.iter().map(|ring| ring.iter().map(|p| to_texels * *p).collect()).collect();
                let clip = path::from_contours(&placed_region);
                mask.fill_path(
                    &composite::skia_path_of(&clip)?,
                    tiny_skia::FillRule::Winding,
                    true,
                    tiny_skia::Transform::identity(),
                );
                Some(mask)
            }
            None => None,
        };

        // The document's own coordinates, through the badge's placement, into
        // this raster.
        composite::paint_through(&painting.elements, &mut pixmap, to_texels * placed * painting.placement, mask.as_ref());
        Some(Rendering { pixmap, mask, width, height })
    }

    /// A texel the region covers whole and the document paints opaque.
    ///
    /// The region is asked directly: every layer is painted through its
    /// antialiased edge, and layer over layer the alpha of a texel it only
    /// partly covers still adds up to opaque.
    fn covered(&self, x: usize, y: usize) -> bool {
        let at = y * self.width + x;
        self.mask.as_ref().is_none_or(|mask| mask.data()[at] == 255) && self.pixmap.pixels()[at].alpha() == 255
    }

    /// The face as read from the texels that are the region's own colour, or
    /// nothing when there are none. Colours are carried `carry` texels past
    /// them, and the rest takes their average.
    ///
    /// Only texels well inside the region qualify. On the edge, the clip's
    /// antialiasing and that of whatever is painted next door are applied
    /// independently, and a cell's outline is a flattened copy of the
    /// document's curves, so a neighbour's paint reaches a texel or so in.
    fn read(&self, carry: usize) -> Option<Face> {
        let count = self.width * self.height;
        let mut known = vec![false; count];
        for y in 0..self.height {
            for x in 0..self.width {
                if !self.covered(x, y) {
                    continue;
                }
                // The rect is the region's bounds, so its border is the
                // region's edge too: a texel there is as suspect as any other
                // on the edge. Read as covered, the flat top of a letter would
                // take the colour painted beneath it.
                known[y * self.width + x] = (-1i64..=1).all(|dy| {
                    (-1i64..=1).all(|dx| {
                        let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                        nx >= 0
                            && ny >= 0
                            && nx < self.width as i64
                            && ny < self.height as i64
                            && self.covered(nx as usize, ny as usize)
                    })
                });
            }
        }
        // A face too thin to have an interior settles for the texels it covers
        // whole.
        if !known.iter().any(|&k| k) {
            for y in 0..self.height {
                for x in 0..self.width {
                    known[y * self.width + x] = self.covered(x, y);
                }
            }
        }
        let mut frontier: Vec<usize> = (0..count).filter(|&i| known[i]).collect();
        if frontier.is_empty() {
            return None;
        }

        // The pixmap is premultiplied; these texels are opaque, so the
        // components are already the colour.
        let mut rgba: Vec<[u8; 4]> =
            self.pixmap.pixels().iter().map(|p| [p.red(), p.green(), p.blue(), p.alpha()]).collect();
        let (mut low, mut high, mut sum) = ([255u8; 3], [0u8; 3], [0u64; 3]);
        for &index in &frontier {
            for channel in 0..3 {
                let value = rgba[index][channel];
                low[channel] = low[channel].min(value);
                high[channel] = high[channel].max(value);
                sum[channel] += u64::from(value);
            }
        }
        let total = frontier.len() as u64;
        let mean = [sum[0] / total, sum[1] / total, sum[2] / total];
        let average = Rgba {
            r: mean[0] as f32 / 255.0,
            g: mean[1] as f32 / 255.0,
            b: mean[2] as f32 / 255.0,
            a: 1.0,
        };
        if (0..3).all(|c| high[c] - low[c] < FLAT) {
            return Some(Face::Flat(average));
        }

        // Carry the colours outward a ring at a time, then flood what is left:
        // sampling at the face's edge must never pick up a neighbour's colour
        // or the background.
        for _ in 0..carry {
            if frontier.is_empty() {
                break;
            }
            let mut next: Vec<usize> = Vec::new();
            for &index in &frontier {
                let (x, y) = (index % self.width, index / self.width);
                let neighbours = [
                    (x.wrapping_sub(1), y),
                    (x + 1, y),
                    (x, y.wrapping_sub(1)),
                    (x, y + 1),
                ];
                for (nx, ny) in neighbours {
                    if nx >= self.width || ny >= self.height {
                        continue;
                    }
                    let neighbour = ny * self.width + nx;
                    if known[neighbour] {
                        continue;
                    }
                    known[neighbour] = true;
                    let colour = rgba[index];
                    rgba[neighbour][..3].copy_from_slice(&colour[..3]);
                    next.push(neighbour);
                }
            }
            frontier = next;
        }
        let mut bytes = Vec::with_capacity(count * 4);
        for (index, texel) in rgba.iter().enumerate() {
            if known[index] {
                bytes.extend_from_slice(&[texel[0], texel[1], texel[2], 255]);
            } else {
                bytes.extend_from_slice(&[mean[0] as u8, mean[1] as u8, mean[2] as u8, 255]);
            }
        }
        Some(Face::Painted {
            texture: Texture { width: self.width as u32, height: self.height as u32, rgba: bytes },
            average,
        })
    }

    /// The colour of the partly covered texels, weighted by how much of each
    /// the region covers.
    fn edge_colour(&self) -> Option<Rgba> {
        let (mut sum, mut weight) = ([0u64; 3], 0u64);
        for texel in self.pixmap.pixels() {
            let alpha = u64::from(texel.alpha());
            if alpha < 32 {
                continue;
            }
            // Premultiplied: the components are already weighted by alpha.
            sum[0] += u64::from(texel.red());
            sum[1] += u64::from(texel.green());
            sum[2] += u64::from(texel.blue());
            weight += alpha;
        }
        (weight > 0).then(|| Rgba {
            r: (sum[0] as f32 / weight as f32).min(1.0),
            g: (sum[1] as f32 / weight as f32).min(1.0),
            b: (sum[2] as f32 / weight as f32).min(1.0),
            a: 1.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CancelToken;
    use crate::design;

    /// A white letter over a band laid on several layers reads as white: not
    /// the band along its edge, where each layer's antialiased alpha adds up to
    /// opaque, nor along the border of its rect, which is its edge too.
    #[test]
    fn a_letter_over_layered_paint_reads_flat() {
        let art = design::artwork(
            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 72 72">
                <rect x="0" y="0" width="72" height="72" fill="#FFFFFF"/>
                <rect x="0" y="0" width="72" height="72" fill="#7040A0"/>
                <rect x="0" y="0" width="72" height="72" fill="#A060C0"/>
                <rect x="0" y="0" width="72" height="72" fill="#FFFFFF"/>
                <rect x="0" y="0" width="72" height="72" fill="#D06385"/>
                <path d="M30.9988 51.6984L30.578 46.1613L33.9921 45.9031L34.0877 47.2037L32.5098 47.328L32.5672 48.1313L33.9825 48.0261L34.0782 49.2789L32.6628 49.3841L32.7298 50.2256L34.3077 50.1013L34.4129 51.4306L30.9988 51.6888V51.6984Z" fill="#FFFFFF"/>
            </svg>"##,
            &CancelToken::new(),
        )
        .expect("readable");
        let painting = art.painting.as_ref().expect("painted");
        let white = art.cells.iter().find(|c| c.color.g > 0.99).expect("a white cell");
        let rect = crate::geom::path::bounds(&white.rings).expect("bounds");
        match face(painting, Affine::IDENTITY, &white.rings, rect) {
            Some(Face::Flat(color)) => assert!(color.g > 0.99, "{color:?}"),
            Some(Face::Painted { .. }) => panic!("read the band beneath as part of the face"),
            None => panic!("no face"),
        }
    }
}
