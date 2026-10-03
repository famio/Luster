//! A badge as a GLB: one binary glTF file, meshes, materials and textures.
//!
//! The writer assembles bytes directly. The engine's vertex layout is already
//! interleaved the way glTF wants it, so the buffer is the submeshes' own
//! bytes, one after another.

use crate::badge::{Badge, Submesh};
use crate::model::{MaterialRole, Rgba};
use crate::mesh::{NORMAL_OFFSET, POSITION_OFFSET, TANGENT_OFFSET, UV_OFFSET, VERTEX_STRIDE};
// glTF colours are linear; the engine's are sRGB.
use crate::model::linear;

/// Writes `badge` as a GLB, its metal plated in `metal` (`badge::GOLD` is the
/// engine's own). `scale` sizes it: at 1 the badge is one unit (a metre, in
/// glTF) across.
///
/// The textures travel with it: what the document paints on each face, and
/// the sandblast on the reverse, as PNGs in the same binary chunk.
pub fn glb(badge: &Badge, scale: f32, metal: Rgba) -> Vec<u8> {
    let mut buffer: Vec<u8> = Vec::new();
    let mut views = String::new();
    let mut accessors = String::new();
    let mut meshes = String::new();
    let mut nodes = String::new();
    let mut view_count = 0usize;
    let mut accessor_count = 0usize;

    for (index, submesh) in badge.submeshes.iter().enumerate() {
        let (vertices, min, max) = scaled(submesh, scale);
        // Vertices, interleaved, then indices.
        let vertex_view = view_count;
        views += &view(&mut buffer, &vertices, Some(VERTEX_STRIDE), 34962);
        let index_view = vertex_view + 1;
        views += &view(&mut buffer, &submesh.indices, None, 34963);
        view_count += 2;

        let position = accessor_count;
        accessors += &format!(
            r#"{{"bufferView":{vertex_view},"byteOffset":{POSITION_OFFSET},"componentType":5126,"count":{},"type":"VEC3","min":[{},{},{}],"max":[{},{},{}]}},"#,
            submesh.vertex_count, min[0], min[1], min[2], max[0], max[1], max[2]
        );
        accessors += &format!(
            r#"{{"bufferView":{vertex_view},"byteOffset":{NORMAL_OFFSET},"componentType":5126,"count":{},"type":"VEC3"}},"#,
            submesh.vertex_count
        );
        accessors += &format!(
            r#"{{"bufferView":{vertex_view},"byteOffset":{UV_OFFSET},"componentType":5126,"count":{},"type":"VEC2"}},"#,
            submesh.vertex_count
        );
        accessors += &format!(
            r#"{{"bufferView":{vertex_view},"byteOffset":{TANGENT_OFFSET},"componentType":5126,"count":{},"type":"VEC4"}},"#,
            submesh.vertex_count
        );
        accessors += &format!(
            r#"{{"bufferView":{index_view},"componentType":5125,"count":{},"type":"SCALAR"}},"#,
            submesh.index_count
        );
        accessor_count += 5;

        meshes += &format!(
            r#"{{"name":{},"primitives":[{{"attributes":{{"POSITION":{},"NORMAL":{},"TEXCOORD_0":{},"TANGENT":{}}},"indices":{},"material":{}}}]}},"#,
            quoted(&submesh.name),
            position,
            position + 1,
            position + 2,
            position + 3,
            position + 4,
            submesh.material
        );
        nodes += &format!(r#"{{"mesh":{index},"name":{}}},"#, quoted(&submesh.name));
    }

    let wanted = badge.materials.iter().any(|m| m.role == MaterialRole::GoldBack);
    let mut images = String::new();
    let pngs = badge.textures.iter().map(|texture| texture.png());
    for (index, png) in pngs.chain(wanted.then(|| grit_png().to_vec())).enumerate() {
        let slot = view_count + index;
        views += &view(&mut buffer, &png, None, 0);
        images += &format!(r#"{{"bufferView":{slot},"mimeType":"image/png"}},"#);
    }
    let grit_image = wanted.then_some(badge.textures.len());
    let textures: String = (0..badge.textures.len() + usize::from(wanted))
        .map(|index| format!(r#"{{"source":{index}}},"#))
        .collect();

    let materials: String = badge
        .materials
        .iter()
        .map(|m| {
            let base = m.texture.map_or(String::new(), |index| {
                format!(r#","baseColorTexture":{{"index":{index}}}"#)
            });
            // A textured face carries the document's own colours; its factor
            // must not tint them.
            let color = match m.role {
                MaterialRole::Plated | MaterialRole::GoldBack => metal,
                _ => m.color,
            };
            let tint = if m.texture.is_some() { [1.0, 1.0, 1.0] } else {
                [linear(color.r), linear(color.g), linear(color.b)]
            };
            let normal = match (m.role, grit_image) {
                // At full strength, as RealityKit and Filament draw it: the grit
                // is shallow in the map itself.
                (MaterialRole::GoldBack, Some(index)) => {
                    format!(r#","normalTexture":{{"index":{index}}}"#)
                }
                _ => String::new(),
            };
            format!(
                r#"{{"name":{},"pbrMetallicRoughness":{{"baseColorFactor":[{},{},{},1],"metallicFactor":{},"roughnessFactor":{}{base}}}{normal},"doubleSided":false}},"#,
                quoted(&m.name),
                tint[0],
                tint[1],
                tint[2],
                m.metallic,
                m.roughness
            )
        })
        .collect();

    let children: String =
        (0..badge.submeshes.len()).map(|i| i.to_string()).collect::<Vec<_>>().join(",");
    let json = format!(
        concat!(
            r#"{{"asset":{{"version":"2.0","generator":"Luster {version}"}},"#,
            r#""scene":0,"scenes":[{{"nodes":[{root}]}}],"#,
            r#""nodes":[{nodes}{{"name":"badge","children":[{children}]}}],"#,
            r#""meshes":[{meshes}],"materials":[{materials}],"#,
            r#"{images}{textures}"#,
            r#""accessors":[{accessors}],"bufferViews":[{views}],"#,
            r#""buffers":[{{"byteLength":{length}}}]}}"#
        ),
        version = crate::version(),
        root = badge.submeshes.len(),
        nodes = nodes,
        children = children,
        meshes = trim(&meshes),
        materials = trim(&materials),
        images = if images.is_empty() { String::new() } else { format!(r#""images":[{}],"#, trim(&images)) },
        textures = if textures.is_empty() { String::new() } else { format!(r#""textures":[{}],"#, trim(&textures)) },
        accessors = trim(&accessors),
        views = trim(&views),
        length = buffer.len()
    );

    assemble(&json, &buffer)
}

/// The reverse's grit as a PNG. It is the same on every badge and most of an
/// export's time, so it is compressed once.
fn grit_png() -> &'static [u8] {
    static PNG: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    PNG.get_or_init(|| crate::texture::sandblast().png())
}

/// The submesh's vertices with their positions scaled, and the bounds glTF
/// asks for on the position accessor.
fn scaled(submesh: &Submesh, scale: f32) -> (Vec<u8>, [f32; 3], [f32; 3]) {
    let mut bytes = submesh.vertices.clone();
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for vertex in bytes.chunks_exact_mut(VERTEX_STRIDE) {
        for axis in 0..3 {
            let at = POSITION_OFFSET + axis * 4;
            let value = f32::from_le_bytes(vertex[at..at + 4].try_into().unwrap()) * scale;
            vertex[at..at + 4].copy_from_slice(&value.to_le_bytes());
            min[axis] = min[axis].min(value);
            max[axis] = max[axis].max(value);
        }
    }
    (bytes, min, max)
}

/// Appends `data` to the buffer, padded, and describes it as a view.
fn view(buffer: &mut Vec<u8>, data: &[u8], stride: Option<usize>, target: u32) -> String {
    // An image's view has no target: it is not vertex or index data.
    while buffer.len() % 4 != 0 {
        buffer.push(0);
    }
    let offset = buffer.len();
    buffer.extend_from_slice(data);
    let stride = stride.map_or(String::new(), |s| format!(r#","byteStride":{s}"#));
    let target = if target == 0 { String::new() } else { format!(r#","target":{target}"#) };
    format!(
        r#"{{"buffer":0,"byteOffset":{offset},"byteLength":{}{stride}{target}}},"#,
        data.len()
    )
}

/// The GLB container: a header, the JSON chunk, then the binary chunk.
fn assemble(json: &str, buffer: &[u8]) -> Vec<u8> {
    let mut json = json.as_bytes().to_vec();
    while json.len() % 4 != 0 {
        json.push(b' ');
    }
    let mut binary = buffer.to_vec();
    while binary.len() % 4 != 0 {
        binary.push(0);
    }
    let length = 12 + 8 + json.len() + 8 + binary.len();

    let mut out = Vec::with_capacity(length);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(length as u32).to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json);
    out.extend_from_slice(&(binary.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&binary);
    out
}



fn quoted(text: &str) -> String {
    let escaped: String = text
        .chars()
        .flat_map(|c| match c {
            '"' | '\\' => vec!['\\', c],
            c if (c as u32) < 0x20 => vec![' '],
            c => vec![c],
        })
        .collect();
    format!("\"{escaped}\"")
}

fn trim(list: &str) -> &str {
    list.strip_suffix(',').unwrap_or(list)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CancelToken;
    use crate::model::GOLD;

    const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
        <circle cx="50" cy="50" r="40" fill="#7040a0"/>
        <circle cx="50" cy="50" r="20" fill="#60dc00"/>
    </svg>"##;

    fn badge() -> Badge {
        crate::mint(SVG.as_bytes(), &CancelToken::new()).expect("a badge")
    }

    /// Parses the GLB back into its header, JSON and binary chunk.
    fn parts(glb: &[u8]) -> (serde_json::Value, usize) {
        assert_eq!(&glb[0..4], b"glTF");
        assert_eq!(u32::from_le_bytes(glb[8..12].try_into().unwrap()) as usize, glb.len());
        let json_length = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        assert_eq!(&glb[16..20], b"JSON");
        let json: serde_json::Value =
            serde_json::from_slice(&glb[20..20 + json_length]).expect("valid JSON");
        let binary_length =
            u32::from_le_bytes(glb[20 + json_length..24 + json_length].try_into().unwrap()) as usize;
        assert_eq!(&glb[24 + json_length..28 + json_length], b"BIN\0");
        (json, binary_length)
    }

    #[test]
    fn a_badge_writes_a_readable_glb() {
        let badge = badge();
        let glb = glb(&badge, 1.0, GOLD);
        let (json, binary) = parts(&glb);
        assert_eq!(json["asset"]["version"], "2.0");
        assert_eq!(json["meshes"].as_array().unwrap().len(), badge.submeshes.len());
        assert_eq!(json["materials"].as_array().unwrap().len(), badge.materials.len());
        // The binary chunk is the buffer padded to four bytes, and no more.
        let length = json["buffers"][0]["byteLength"].as_u64().unwrap() as usize;
        assert!(binary % 4 == 0 && (length..length + 4).contains(&binary), "{length} in {binary}");
        // One node per mesh, and one more holding them together.
        assert_eq!(json["nodes"].as_array().unwrap().len(), badge.submeshes.len() + 1);
    }

    #[test]
    fn every_accessor_stays_inside_its_view() {
        let glb = glb(&badge(), 1.0, GOLD);
        let (json, _) = parts(&glb);
        let views = json["bufferViews"].as_array().unwrap();
        for accessor in json["accessors"].as_array().unwrap() {
            let view = &views[accessor["bufferView"].as_u64().unwrap() as usize];
            let size: usize = match accessor["type"].as_str().unwrap() {
                "VEC4" => 16,
                "VEC3" => 12,
                "VEC2" => 8,
                _ => 4,
            };
            let stride = view["byteStride"].as_u64().map_or(size, |s| s as usize);
            let offset = accessor["byteOffset"].as_u64().unwrap_or(0) as usize;
            let count = accessor["count"].as_u64().unwrap() as usize;
            let needed = offset + (count - 1) * stride + size;
            assert!(needed <= view["byteLength"].as_u64().unwrap() as usize, "{accessor}");
        }
    }

    /// The same badge must write the same bytes, however many threads the
    /// engine had: an export is how a change in the engine is checked.
    #[test]
    fn an_export_is_the_same_bytes_every_time() {
        let badge = badge();
        assert_eq!(glb(&badge, 1.0, GOLD), glb(&badge, 1.0, GOLD));
        let again = crate::mint(SVG.as_bytes(), &crate::CancelToken::new()).expect("a badge");
        assert_eq!(glb(&badge, 1.0, GOLD), glb(&again, 1.0, GOLD), "two mints of one file differ");
    }

    #[test]
    fn a_textured_face_carries_the_texture_and_not_a_tint() {
        // A gradient: the faces under it are painted, so the GLB holds images.
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
            <defs><linearGradient id="g" x1="0" y1="0" x2="100" y2="0" gradientUnits="userSpaceOnUse">
                <stop offset="0" stop-color="#ff0000"/><stop offset="1" stop-color="#0000ff"/>
            </linearGradient></defs>
            <rect x="10" y="10" width="80" height="80" fill="url(#g)"/>
            <circle cx="50" cy="50" r="12" fill="#ffffff"/>
        </svg>"##;
        let badge = crate::mint(svg.as_bytes(), &crate::CancelToken::new()).expect("a badge");
        assert!(!badge.textures.is_empty(), "a gradient paints its faces");
        let (json, _) = parts(&glb(&badge, 1.0, GOLD));
        let images = json["images"].as_array().expect("images");
        assert!(images.len() >= badge.textures.len());
        for image in images {
            assert_eq!(image["mimeType"], "image/png");
        }
        for material in json["materials"].as_array().unwrap() {
            let pbr = &material["pbrMetallicRoughness"];
            if pbr["baseColorTexture"].is_null() {
                continue;
            }
            // A textured face carries the document's own colours; a tint
            // would multiply them.
            let factor = pbr["baseColorFactor"].as_array().unwrap();
            for channel in &factor[..3] {
                assert_eq!(channel.as_f64().unwrap(), 1.0, "a textured material is tinted");
            }
        }
    }

    #[test]
    fn scaling_sizes_the_badge() {
        let badge = badge();
        let (json, _) = parts(&glb(&badge, 2.0, GOLD));
        let width = |json: &serde_json::Value| {
            let a = &json["accessors"][0];
            a["max"][0].as_f64().unwrap() - a["min"][0].as_f64().unwrap()
        };
        let wide = width(&json);
        let (plain, _) = parts(&glb(&badge, 1.0, GOLD));
        assert!((wide - 2.0 * width(&plain)).abs() < 1e-6, "{wide}");
    }

    /// The metal is the file's to choose: the badge is struck without one.
    #[test]
    fn the_metal_plates_the_metal_and_nothing_else() {
        let badge = badge();
        let silver = Rgba::rgb(0.8, 0.8, 0.8);
        let (gold, _) = parts(&glb(&badge, 1.0, GOLD));
        let (plated, _) = parts(&glb(&badge, 1.0, silver));
        for (m, (a, b)) in badge.materials.iter().zip(
            gold["materials"].as_array().unwrap().iter().zip(plated["materials"].as_array().unwrap()),
        ) {
            let factor = |j: &serde_json::Value| j["pbrMetallicRoughness"]["baseColorFactor"].clone();
            match m.role {
                MaterialRole::Plated | MaterialRole::GoldBack => {
                    assert_ne!(factor(a), factor(b), "{} kept its colour", m.name);
                    let red = factor(b)[0].as_f64().unwrap() as f32;
                    assert!((red - linear(0.8)).abs() < 1e-6, "{} is not silver", m.name);
                }
                _ => assert_eq!(factor(a), factor(b), "{} took the metal", m.name),
            }
        }
    }
}
