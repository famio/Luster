//! A minted badge: submeshes in the shared vertex layout, and their materials.

use rayon::prelude::*;

use crate::mesh::{MeshBuilder, dome, extrude_rolled};
use crate::mint::{self as striking};
use crate::{CancelToken, Error, design};
pub use crate::model::{COPPER, ENAMEL_COLOR, GOLD, Material, MaterialRole, MintOptions, Rgba, SILVER};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct Submesh {
    pub name: String,
    pub material: u32,
    /// `mesh::VERTEX_STRIDE` bytes per vertex.
    pub vertices: Vec<u8>,
    /// u32, little-endian, three per triangle.
    pub indices: Vec<u8>,
    pub vertex_count: u32,
    pub index_count: u32,
    pub min: [f32; 3],
    pub max: [f32; 3],
}

#[derive(Clone, Debug)]
pub struct Badge {
    /// Stable across runs and platforms for the same input and options; equal
    /// keys mean the same badge.
    pub design_key: String,
    pub submeshes: Vec<Submesh>,
    pub materials: Vec<Material>,
    /// What the materials' textures are made of, RGBA8, rows top to bottom.
    pub textures: Vec<crate::texture::Texture>,
}

/// Mints a badge from SVG data: the artwork's silhouette in metal, its fills
/// set into it as enamel, the metal its lines stand in plated in their colours.
pub fn mint(data: &[u8], cancel: &CancelToken) -> Result<Badge, Error> {
    mint_with(data, &MintOptions::default(), cancel)
}

/// The same, with options.
pub fn mint_with(data: &[u8], options: &MintOptions, cancel: &CancelToken) -> Result<Badge, Error> {
    // Leaving the host a core to draw with.
    crate::work::ready();
    struck_badge(data, options, cancel)
}

fn struck_badge(data: &[u8], options: &MintOptions, cancel: &CancelToken) -> Result<Badge, Error> {
    let without_hidden_faces = options.without_hidden_faces;
    let art = design::artwork(data, cancel)?;
    let mut struck = striking::strike(&art, options, cancel)?;
    let tiles = pack_faces(&mut struck);

    // A mesh per piece first: which triangles are hidden is a question about
    // pieces, not about materials. Each is built on its own, side by side;
    // the first piece that fails, in order, says why.
    let built: Vec<Result<MeshBuilder, Error>> = struck
        .pieces
        .par_iter()
        .zip(&tiles)
        .map(|(piece, tile)| {
            cancel.check()?;
            let mut mesh = MeshBuilder::new();
            if piece.glaze && piece.slab.roll > 0.0 {
                dome(&piece.rings, piece.slab, &mut mesh)?;
            } else {
                extrude_rolled(&piece.rings, &piece.rolled, piece.slab, &mut mesh)?;
            }
            if let Some(tile) = *tile {
                lay_on_sheet(&mut mesh, tile);
            }
            if struck.materials[piece.material as usize].role == MaterialRole::GoldBack {
                repeat_grit(&mut mesh);
            }
            Ok(mesh)
        })
        .collect();
    let mut built: Vec<MeshBuilder> = built.into_iter().collect::<Result<_, _>>()?;
    if without_hidden_faces {
        cancel.check()?;
        let visible = crate::cull::visible(&struck.pieces, &built, cancel)?;
        for (mesh, kept) in built.iter_mut().zip(visible) {
            mesh.indices = kept;
        }
    }

    // Then one submesh per material: a renderer draws each in one call.
    let mut meshes: Vec<(u32, MeshBuilder)> = Vec::new();
    for (piece, mesh) in struck.pieces.iter().zip(&built) {
        cancel.check()?;
        let slot = match meshes.iter().position(|(material, _)| *material == piece.material) {
            Some(slot) => slot,
            None => {
                meshes.push((piece.material, MeshBuilder::new()));
                meshes.len() - 1
            }
        };
        meshes[slot].1.append(mesh);
    }

    let submeshes = meshes
        .into_iter()
        .filter(|(_, mesh)| !mesh.is_empty())
        .map(|(material, mesh)| {
            let name = struck.materials[material as usize].name.clone();
            submesh(&name, material, mesh)
        })
        .collect();
    Ok(Badge {
        design_key: design_key(data, options),
        submeshes,
        materials: struck.materials,
        textures: struck.textures,
    })
}

/// Puts every enamel colour on one sheet — the faces the document paints and
/// the flat cells alike — so that a badge is a handful of materials however
/// many colours it carries.
///
/// The pieces themselves do not change: what moves is where on the sheet each
/// face's colours live, which the mesh then has to point at.
fn pack_faces(struck: &mut striking::Struck) -> Vec<Option<crate::texture::Tile>> {
    let enamel = |m: &Material| matches!(m.role, MaterialRole::Field | MaterialRole::Art | MaterialRole::Glaze);
    let sheeted: Vec<u32> = (0..struck.materials.len() as u32)
        .filter(|&m| enamel(&struck.materials[m as usize]))
        .collect();
    let painted = sheeted.iter().any(|&m| struck.materials[m as usize].texture.is_some());
    // A badge of a few colours is better off with those colours than with a
    // sheet to read them from.
    if !painted && sheeted.len() <= 4 {
        return vec![None; struck.pieces.len()];
    }

    // A painted material belongs to one piece, so a tile per material covers
    // both it and the flat colours, which any number of pieces may share.
    let faces: Vec<crate::texture::Texture> = sheeted
        .iter()
        .map(|&m| match struck.materials[m as usize].texture {
            Some(texture) => struck.textures[texture as usize].clone(),
            None => flat_tile(struck.materials[m as usize].color),
        })
        .collect();
    let (sheet, places) = crate::texture::atlas(&faces);

    // A face keeps the finish of the piece it belongs to, so there is one
    // material per finish that reads the sheet, not one for the whole badge.
    let mut merged: Vec<(MaterialRole, u32, u32)> = Vec::new();
    let mut materials: Vec<Material> = Vec::new();
    let mut moved: HashMap<u32, u32> = HashMap::new();
    let mut places_of: HashMap<u32, crate::texture::Tile> = HashMap::new();
    for (&old, place) in sheeted.iter().zip(&places) {
        let m = &struck.materials[old as usize];
        let key = (m.role, m.metallic.to_bits(), m.roughness.to_bits());
        let slot = match merged.iter().position(|k| *k == key) {
            Some(slot) => slot,
            None => {
                merged.push(key);
                materials.push(Material {
                    name: match m.role {
                        MaterialRole::Art => "faces-art".into(),
                        MaterialRole::Glaze => "faces-glaze".into(),
                        _ => format!("faces-{}", materials.len()),
                    },
                    color: Rgba::rgb(1.0, 1.0, 1.0),
                    texture: Some(0),
                    ..m.clone()
                });
                materials.len() - 1
            }
        };
        moved.insert(old, slot as u32);
        places_of.insert(old, *place);
    }

    // The materials that stay off the sheet keep their order; the merged ones
    // follow.
    let mut kept: Vec<Material> = Vec::new();
    let mut at: HashMap<u32, u32> = HashMap::new();
    for (index, m) in struck.materials.iter().enumerate() {
        if enamel(m) {
            continue;
        }
        at.insert(index as u32, kept.len() as u32);
        kept.push(m.clone());
    }
    let first = kept.len() as u32;
    kept.extend(materials);

    // A tile belongs to a piece, not to the material it now shares.
    let mut tiles = Vec::with_capacity(struck.pieces.len());
    for piece in &mut struck.pieces {
        tiles.push(places_of.get(&piece.material).copied());
        piece.material = match moved.get(&piece.material) {
            Some(slot) => first + slot,
            None => at[&piece.material],
        };
    }
    struck.materials = kept;
    struck.textures = vec![sheet];
    tiles
}

/// A tile of one colour, for a face the document paints evenly.
fn flat_tile(color: Rgba) -> crate::texture::Texture {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let pixel = [byte(color.r), byte(color.g), byte(color.b), byte(color.a)];
    crate::texture::Texture { width: 4, height: 4, rgba: pixel.repeat(16) }
}

/// Moves a piece's texture coordinates onto its tile of the shared sheet.
fn lay_on_sheet(mesh: &mut MeshBuilder, tile: crate::texture::Tile) {
    let stride = crate::mesh::VERTEX_STRIDE;
    for vertex in mesh.vertices.chunks_exact_mut(stride) {
        for axis in 0..2 {
            let at = crate::mesh::UV_OFFSET + axis * 4;
            let uv = f32::from_le_bytes(vertex[at..at + 4].try_into().unwrap());
            let moved = tile.offset[axis] + uv * tile.scale[axis];
            vertex[at..at + 4].copy_from_slice(&moved.to_le_bytes());
        }
    }
}

/// Maps the reverse onto its grit by where each point is, not by the piece's
/// bounds: the grit repeats every [`crate::texture::GRIT_TILE`] and keeps its
/// size and shape whatever the badge's.
///
/// The directions match the cap's own mapping, mirrored as seen from behind.
fn repeat_grit(mesh: &mut MeshBuilder) {
    let tile = crate::texture::GRIT_TILE;
    let stride = crate::mesh::VERTEX_STRIDE;
    for vertex in mesh.vertices.chunks_exact_mut(stride) {
        for axis in 0..2 {
            let from = crate::mesh::POSITION_OFFSET + axis * 4;
            let position = f32::from_le_bytes(vertex[from..from + 4].try_into().unwrap());
            let at = crate::mesh::UV_OFFSET + axis * 4;
            vertex[at..at + 4].copy_from_slice(&(-position / tile).to_le_bytes());
        }
    }
}

fn submesh(name: &str, material: u32, mesh: MeshBuilder) -> Submesh {
    Submesh {
        name: name.into(),
        material,
        index_count: mesh.indices.len() as u32,
        vertex_count: mesh.vertex_count(),
        indices: mesh.index_bytes(),
        min: mesh.min,
        max: mesh.max,
        vertices: mesh.vertices,
    }
}

/// FNV-1a over everything a badge is made from: the input, the options, and
/// the engine version.
fn design_key(data: &[u8], options: &MintOptions) -> String {
    let flags = [options.metal_lines as u8, options.without_hidden_faces as u8];
    let mut h: u64 = 0xcbf29ce484222325;
    for b in data.iter().copied().chain(flags).chain(crate::version().bytes()) {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::VERTEX_STRIDE;

    const SQUARE_WITH_HOLE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
        <path fill="#c33" fill-rule="evenodd" d="M0 0H100V100H0Z M30 30H70V70H30Z"/>
        <circle cx="50" cy="50" r="10" fill="#36c"/>
    </svg>"##;

    #[test]
    fn a_badge_is_one_submesh_per_material() {
        let badge = mint(SQUARE_WITH_HOLE.as_bytes(), &CancelToken::new()).unwrap();
        let mut names: Vec<&str> = badge.submeshes.iter().map(|s| s.name.as_str()).collect();
        assert!(names.starts_with(&["gold", "gold-back"]), "{names:?}");
        let count = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), count, "a material drawn in two submeshes: {names:?}");
        for s in &badge.submeshes {
            assert_eq!(s.vertices.len(), s.vertex_count as usize * VERTEX_STRIDE);
            assert_eq!(s.indices.len(), s.index_count as usize * 4);
        }
    }

    #[test]
    fn the_enamel_sits_sunk_inside_the_metal() {
        let badge = mint(SQUARE_WITH_HOLE.as_bytes(), &CancelToken::new()).unwrap();
        let metal = badge.submeshes.iter().find(|s| s.name == "gold").expect("gold");
        let enamel = badge
            .submeshes
            .iter()
            .find(|s| badge.materials[s.material as usize].role == MaterialRole::Field)
            .expect("enamel");
        assert!(metal.max[0] >= enamel.max[0], "the enamel runs on to the edge, not past it");
        assert!(enamel.max[2] < metal.max[2], "the enamel is sunk below the metal");
        // The whole badge is a badge across.
        let across = metal.max[0] - metal.min[0];
        assert!((across - 1.0).abs() < 0.01, "{across}");
    }

    /// A triangle wound against its own normals is invisible wherever back
    /// faces are dropped, which is everywhere: it leaves a hole the stage
    /// shows through.
    #[test]
    fn every_triangle_turns_the_way_its_normals_point() {
        for metal_lines in [false, true] {
            let badge = mint_with(
                SQUARE_WITH_HOLE.as_bytes(),
                &MintOptions { metal_lines, ..MintOptions::default() },
                &CancelToken::new(),
            )
            .unwrap();
            for submesh in &badge.submeshes {
                let at = |i: u32, offset: usize| -> [f32; 3] {
                    let o = i as usize * VERTEX_STRIDE + offset;
                    let f = |k: usize| {
                        f32::from_le_bytes(
                            submesh.vertices[o + k * 4..o + k * 4 + 4].try_into().unwrap(),
                        )
                    };
                    [f(0), f(1), f(2)]
                };
                let index = |t: usize| {
                    u32::from_le_bytes(submesh.indices[t * 4..t * 4 + 4].try_into().unwrap())
                };
                let mut away = 0;
                for triangle in 0..submesh.index_count as usize / 3 {
                    let (i, j, k) =
                        (index(triangle * 3), index(triangle * 3 + 1), index(triangle * 3 + 2));
                    let (a, b, c) = (at(i, 0), at(j, 0), at(k, 0));
                    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                    let turn = [
                        u[1] * v[2] - u[2] * v[1],
                        u[2] * v[0] - u[0] * v[2],
                        u[0] * v[1] - u[1] * v[0],
                    ];
                    // A triangle with no area at all points nowhere; the
                    // inset leaves a few where a notch pinches shut.
                    if turn.iter().map(|c| c * c).sum::<f32>() < 1e-24 {
                        continue;
                    }
                    let mut normal = [0.0f32; 3];
                    for corner in [i, j, k] {
                        let n = at(corner, 12);
                        for axis in 0..3 {
                            normal[axis] += n[axis];
                        }
                    }
                    if (0..3).map(|axis| turn[axis] * normal[axis]).sum::<f32>() < 0.0 {
                        away += 1;
                    }
                }
                assert_eq!(away, 0, "{} has {away} triangles facing away", submesh.name);
            }
        }
    }

    #[test]
    fn a_cancelled_mint_stops() {
        let cancel = CancelToken::new();
        cancel.cancel();
        assert_eq!(mint(SQUARE_WITH_HOLE.as_bytes(), &cancel).unwrap_err(), Error::Cancelled);
    }

    #[test]
    fn empty_and_broken_input_are_errors() {
        let empty = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"/>"#;
        assert_eq!(mint(empty.as_bytes(), &CancelToken::new()).unwrap_err(), Error::NothingToMint);
        assert!(matches!(mint(b"not svg", &CancelToken::new()), Err(Error::InvalidSvg(_))));
    }

    #[test]
    fn what_changes_the_badge_changes_its_key() {
        let key = |options: MintOptions| {
            mint_with(SQUARE_WITH_HOLE.as_bytes(), &options, &CancelToken::new()).unwrap().design_key
        };
        let plain = key(MintOptions::default());
        assert_eq!(plain, key(MintOptions::default()));
        assert_ne!(plain, key(MintOptions { metal_lines: true, ..MintOptions::default() }));
        assert_ne!(plain, key(MintOptions { without_hidden_faces: true, ..MintOptions::default() }));
    }

    #[test]
    fn the_same_input_mints_the_same_bytes() {
        let a = mint(SQUARE_WITH_HOLE.as_bytes(), &CancelToken::new()).unwrap();
        let b = mint(SQUARE_WITH_HOLE.as_bytes(), &CancelToken::new()).unwrap();
        assert_eq!(a.design_key, b.design_key);
        for (x, y) in a.submeshes.iter().zip(&b.submeshes) {
            assert_eq!((&x.vertices, &x.indices), (&y.vertices, &y.indices));
        }
    }
}
