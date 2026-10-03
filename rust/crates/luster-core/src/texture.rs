//! Generated textures: the showcase's panorama the metal reflects, the
//! reverse's grit, and the sheet a badge's painted faces are packed on.

use crate::math;

/// RGBA8, rows top to bottom. Colour textures are sRGB-encoded; a normal map
/// holds its vectors as they are.
#[derive(Clone, Debug)]
pub struct Texture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Texture {
    /// The texture as a PNG, for writing into a glTF file.
    ///
    /// The settings are fixed, so that the same texture always makes the same
    /// bytes and an export can be compared with another byte for byte.
    pub fn png(&self) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut out, self.width, self.height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_compression(png::Compression::Default);
            if let Ok(mut writer) = encoder.write_header() {
                let _ = writer.write_image_data(&self.rgba);
            }
        }
        out
    }
}

/// How bright the showcase's panorama is at full white. Its lights are
/// painted in radiance, the brightest at about twelve where two overlap,
/// and stored divided by this so that they fit an sRGB byte; `style::preset`
/// gives the exposure back. A byte of sRGB holds the lights and the all but
/// black surround alike: rendered from floats instead, the badge comes out
/// within two levels of it.
pub const SHOWCASE_RANGE: f32 = 14.0;

/// The showcase: a product photographer's studio for a glossy badge.
///
/// A flat face reflects one direction, so what a badge looks like is mostly
/// which part of the room its face points at, and the room is laid out by
/// those directions. At rest the face looks down and to the left of the
/// camera, about 40° round and 12° down, and a drag swings that across the
/// whole front.
///
/// Enamel and the metal round it face the same way and see the same light,
/// and metal reflects twenty times what enamel does: what makes the metal
/// bright hazes dark enamel over. So the front, where a face points, is kept
/// dim, and the enamel's colour comes from where a face seldom points:
/// a large softbox overhead and two more high up either side. What the
/// front does hold is shaped. Narrow strips stand at the angles a face
/// turns through, so glossy enamel catches one as a band of sheen crossing
/// it rather than a wash, with a thin one leaning across where the face
/// points at rest; a scrim round the camera, bright at the top and dark at
/// the bottom, grades a face seen head on; a card low down catches one
/// tipped toward the viewer, and a faint tent keeps metal plated in the
/// art's colours from going black wherever it points. Strips far round
/// either side line the rim. The surround is all but black, which is what
/// gives the metal its edges.
pub fn showcase_environment() -> Texture {
    let panels = [
        // Overhead, and high up either side: most of what the enamel's
        // colour is lit by.
        Panel { azimuth: 0.0, elevation: 60.0, width: 120.0, height: 50.0, roll: 0.0,
                corners: Corners::Square, soft: 0.25, rgb: [1.0, 0.98, 0.96], radiance: 7.5, top_bottom: [0.6, 1.0] },
        Panel { azimuth: -105.0, elevation: 35.0, width: 50.0, height: 50.0, roll: 0.0,
                corners: Corners::Soft, soft: 0.5, rgb: [1.0, 0.97, 0.93], radiance: 2.0, top_bottom: [1.0, 1.0] },
        Panel { azimuth: 105.0, elevation: 35.0, width: 50.0, height: 50.0, roll: 0.0,
                corners: Corners::Soft, soft: 0.5, rgb: [0.97, 0.98, 1.0], radiance: 2.0, top_bottom: [1.0, 1.0] },
        // The tent: a faint fill over the whole front.
        Panel { azimuth: 0.0, elevation: 0.0, width: 150.0, height: 120.0, roll: 0.0,
                corners: Corners::Round, soft: 0.6, rgb: [1.0, 0.98, 0.96], radiance: 0.1, top_bottom: [1.0, 1.0] },
        // The scrim round the camera, bright at the top and dark at the bottom.
        Panel { azimuth: 0.0, elevation: 10.0, width: 38.0, height: 36.0, roll: 0.0,
                corners: Corners::Soft, soft: 0.45, rgb: [1.0, 0.98, 0.95], radiance: 1.0, top_bottom: [1.4, 0.15] },
        // The stripe the face crosses at rest.
        Panel { azimuth: -44.0, elevation: -8.0, width: 4.5, height: 70.0, roll: 22.0,
                corners: Corners::Square, soft: 0.3, rgb: [1.0, 0.98, 0.95], radiance: 3.5, top_bottom: [1.0, 1.0] },
        // Strips to the left and the right, the right ones a shade cooler.
        Panel { azimuth: -78.0, elevation: 0.0, width: 13.0, height: 75.0, roll: 0.0,
                corners: Corners::Square, soft: 0.25, rgb: [1.0, 0.97, 0.93], radiance: 6.0, top_bottom: [1.0, 0.55] },
        Panel { azimuth: 38.0, elevation: 2.0, width: 12.0, height: 70.0, roll: -10.0,
                corners: Corners::Square, soft: 0.3, rgb: [0.97, 0.98, 1.0], radiance: 5.0, top_bottom: [1.0, 0.5] },
        Panel { azimuth: 82.0, elevation: 4.0, width: 10.0, height: 80.0, roll: 0.0,
                corners: Corners::Square, soft: 0.25, rgb: [0.96, 0.98, 1.0], radiance: 6.0, top_bottom: [1.0, 1.0] },
        // Far round either side, for the rim.
        Panel { azimuth: 125.0, elevation: 6.0, width: 7.0, height: 80.0, roll: 0.0,
                corners: Corners::Square, soft: 0.2, rgb: [0.95, 0.97, 1.0], radiance: 8.0, top_bottom: [1.0, 1.0] },
        Panel { azimuth: -128.0, elevation: 10.0, width: 7.0, height: 70.0, roll: 0.0,
                corners: Corners::Square, soft: 0.2, rgb: [1.0, 0.96, 0.9], radiance: 6.0, top_bottom: [1.0, 1.0] },
        // The card below, and a little warmth off the floor.
        Panel { azimuth: 0.0, elevation: -38.0, width: 110.0, height: 26.0, roll: 0.0,
                corners: Corners::Square, soft: 0.5, rgb: [1.0, 0.95, 0.88], radiance: 0.4, top_bottom: [1.0, 0.4] },
        Panel { azimuth: 0.0, elevation: -70.0, width: 120.0, height: 40.0, roll: 0.0,
                corners: Corners::Round, soft: 0.6, rgb: [0.6, 0.45, 0.3], radiance: 0.1, top_bottom: [1.0, 1.0] },
    ];
    // The surround by elevation, floor to ceiling: all but black.
    let surround: [(f32, [f32; 3]); 5] = [
        (-90.0, [0.012, 0.010, 0.008]),
        (-25.0, [0.014, 0.013, 0.012]),
        (0.0, [0.008, 0.008, 0.009]),
        (35.0, [0.014, 0.014, 0.016]),
        (90.0, [0.035, 0.035, 0.038]),
    ];

    let (w, h) = (1024usize, 512usize);
    let placed: Vec<Placed> = panels.iter().map(Placed::new).collect();
    let columns: Vec<(f32, f32)> = (0..w)
        .map(|x| {
            let longitude = ((x as f32 + 0.5) / w as f32 - 0.5) * std::f32::consts::TAU;
            (math::sin(longitude), math::cos(longitude))
        })
        .collect();
    let mut rgba = vec![0u8; w * h * 4];
    for y in 0..h {
        let theta = (y as f32 + 0.5) / h as f32 * std::f32::consts::PI;
        let (st, ct) = (math::sin(theta), math::cos(theta));
        let elevation = 90.0 - theta.to_degrees();
        let base = graded(&surround, elevation);
        for (x, &(sl, cl)) in columns.iter().enumerate() {
            let d = [st * sl, ct, -st * cl];
            let mut c = base;
            for p in &placed {
                let a = p.cover(d);
                if a > 0.0 {
                    for (channel, light) in c.iter_mut().zip(p.light) {
                        *channel += light * a;
                    }
                }
            }
            let o = (y * w + x) * 4;
            for k in 0..3 {
                let encoded = crate::model::encoded((c[k] / SHOWCASE_RANGE).clamp(0.0, 1.0));
                rgba[o + k] = (encoded * 255.0).round() as u8;
            }
            rgba[o + 3] = 255;
        }
    }
    Texture { width: w as u32, height: h as u32, rgba }
}

/// A light in the showcase as the badge sees it: a flat panel facing the
/// badge, so it keeps its shape wherever it hangs, rather than a shape drawn
/// on the panorama and stretched toward the poles.
struct Panel {
    /// Where its centre is: degrees round from the camera's side (+z) toward
    /// +x, and up from the horizon.
    azimuth: f32,
    elevation: f32,
    /// How wide and tall it looks, in degrees, and how far it is turned
    /// about its centre.
    width: f32,
    height: f32,
    roll: f32,
    corners: Corners,
    /// How far either side of its edge it fades, as a part of its half-size.
    soft: f32,
    rgb: [f32; 3],
    radiance: f32,
    /// What its radiance is multiplied by at its top and at its bottom.
    top_bottom: [f32; 2],
}

/// The shape of a panel: an ellipse, or a rectangle with its corners
/// rounded a lot or a little (a superellipse of degree 2, 4 or 8).
#[derive(Clone, Copy)]
enum Corners {
    Round,
    Soft,
    Square,
}

/// A panel worked out once: where it faces, its own axes, and its size as
/// the tangents it spans.
struct Placed<'a> {
    panel: &'a Panel,
    centre: [f32; 3],
    across: [f32; 3],
    up: [f32; 3],
    half: [f32; 2],
    light: [f32; 3],
}

impl<'a> Placed<'a> {
    fn new(panel: &'a Panel) -> Self {
        let (az, el) = (panel.azimuth.to_radians(), panel.elevation.to_radians());
        let centre = [math::cos(el) * math::sin(az), math::sin(el), math::cos(el) * math::cos(az)];
        // Level: its horizontal axis lies in the horizon's plane.
        let across = normalized([centre[2], 0.0, -centre[0]]);
        let up = cross(centre, across);
        let (s, c) = libm::sincosf(panel.roll.to_radians());
        let turned = |a: [f32; 3], b: [f32; 3], sa: f32, sb: f32| {
            [a[0] * sa + b[0] * sb, a[1] * sa + b[1] * sb, a[2] * sa + b[2] * sb]
        };
        let (across, up) = (turned(across, up, c, s), turned(across, up, -s, c));
        let half = [math::tan((panel.width / 2.0).to_radians()), math::tan((panel.height / 2.0).to_radians())];
        let light = panel.rgb.map(|v| v * panel.radiance);
        Placed { panel, centre, across, up, half, light }
    }

    /// How much of the panel's light reaches along `d`: 1 inside it, fading
    /// to nothing across its edge, and graded top to bottom.
    fn cover(&self, d: [f32; 3]) -> f32 {
        let facing = dot(d, self.centre);
        if facing <= 1e-3 {
            return 0.0;
        }
        // Where `d` meets the panel's plane, in its own half-sizes.
        let x = dot(d, self.across) / facing / self.half[0];
        let y = dot(d, self.up) / facing / self.half[1];
        let p = self.panel;
        let reach = 1.0 + p.soft;
        if x.abs() >= reach || y.abs() >= reach {
            return 0.0;
        }
        // |(x, y)| in the panel's own norm, by square roots rather than
        // powers: exact, and cheap enough for every pixel.
        let (x2, y2) = (x * x, y * y);
        let r = match p.corners {
            Corners::Round => (x2 + y2).sqrt(),
            Corners::Soft => (x2 * x2 + y2 * y2).sqrt().sqrt(),
            Corners::Square => {
                let (x4, y4) = (x2 * x2, y2 * y2);
                (x4 * x4 + y4 * y4).sqrt().sqrt().sqrt()
            }
        };
        let edge = 1.0 - smoothstep(1.0 - p.soft, reach, r);
        let t = ((y + 1.0) / 2.0).clamp(0.0, 1.0);
        edge * (p.top_bottom[1] + (p.top_bottom[0] - p.top_bottom[1]) * t)
    }
}

/// The colour at `elevation` of a surround graded between stops.
fn graded(stops: &[(f32, [f32; 3])], elevation: f32) -> [f32; 3] {
    let at = stops.iter().position(|s| s.0 >= elevation).unwrap_or(stops.len() - 1).max(1);
    let (low, high) = (stops[at - 1], stops[at]);
    let t = ((elevation - low.0) / (high.0 - low.0)).clamp(0.0, 1.0);
    lerp3(low.1, high.1, t)
}

fn smoothstep(from: f32, to: f32, x: f32) -> f32 {
    let t = ((x - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn normalized(v: [f32; 3]) -> [f32; 3] {
    let length = dot(v, v).sqrt();
    if length == 0.0 { [1.0, 0.0, 0.0] } else { [v[0] / length, v[1] / length, v[2] / length] }
}

/// The showcase's panorama as a cubemap: six `size` square faces stacked in
/// one image, in the order +X, -X, +Y, -Y, +Z, -Z.
///
/// A renderer that reflects a cubemap wants this and not the panorama, and
/// building it here means no baked file has to travel with the app. The
/// pixels stay sRGB-encoded, as the panorama's are.
pub fn showcase_cubemap(size: u32) -> Texture {
    cubemap(&showcase_environment(), size)
}

/// What a diffuse surface receives from the showcase, in the same layout.
pub fn showcase_irradiance(size: u32) -> Texture {
    irradiance(&showcase_environment(), size)
}

fn cubemap(panorama: &Texture, size: u32) -> Texture {
    let size = size.max(1) as usize;
    let mut rgba = vec![0u8; size * size * 6 * 4];
    for face in 0..6 {
        for y in 0..size {
            for x in 0..size {
                let u = (x as f32 + 0.5) / size as f32 * 2.0 - 1.0;
                let v = (y as f32 + 0.5) / size as f32 * 2.0 - 1.0;
                let colour = look_up(panorama, direction(face, u, v));
                let at = ((face * size + y) * size + x) * 4;
                rgba[at..at + 4].copy_from_slice(&colour);
            }
        }
    }
    Texture { width: size as u32, height: (size * 6) as u32, rgba }
}

fn irradiance(panorama: &Texture, size: u32) -> Texture {
    // Coarse enough to integrate quickly, fine enough that a softbox still
    // reads as a direction rather than a wash.
    let (lw, lh) = (128usize, 64usize);
    let mut light = Vec::with_capacity(lw * lh);
    for y in 0..lh {
        let theta = (y as f32 + 0.5) / lh as f32 * std::f32::consts::PI;
        // The area a texel covers on the sphere.
        let weight = math::sin(theta);
        for x in 0..lw {
            let phi = (x as f32 + 0.5) / lw as f32 * std::f32::consts::TAU;
            let dir = [
                math::sin(theta) * math::sin(phi - std::f32::consts::PI),
                math::cos(theta),
                -math::sin(theta) * math::cos(phi - std::f32::consts::PI),
            ];
            let rgba = look_up(panorama, dir);
            let linear = |c: u8| crate::model::linear(f32::from(c) / 255.0);
            light.push((dir, [linear(rgba[0]), linear(rgba[1]), linear(rgba[2])], weight));
        }
    }
    let scale = std::f32::consts::TAU * std::f32::consts::PI / (lw * lh) as f32;

    let size = size.max(1) as usize;
    let mut rgba = vec![0u8; size * size * 6 * 4];
    for face in 0..6 {
        for y in 0..size {
            for x in 0..size {
                let u = (x as f32 + 0.5) / size as f32 * 2.0 - 1.0;
                let v = (y as f32 + 0.5) / size as f32 * 2.0 - 1.0;
                let n = direction(face, u, v);
                let mut sum = [0.0f32; 3];
                for (dir, colour, weight) in &light {
                    let cosine = n[0] * dir[0] + n[1] * dir[1] + n[2] * dir[2];
                    if cosine <= 0.0 {
                        continue;
                    }
                    let w = cosine * weight * scale;
                    for c in 0..3 {
                        sum[c] += colour[c] * w;
                    }
                }
                let encode = |c: f32| {
                    // The average radiance, not the integral: a surface of
                    // albedo 1 under an even sky reads as that sky.
                    let c = (c / std::f32::consts::PI).clamp(0.0, 1.0);
                    let s = crate::model::encoded(c);
                    (s * 255.0).round() as u8
                };
                let at = ((face * size + y) * size + x) * 4;
                rgba[at..at + 4].copy_from_slice(&[encode(sum[0]), encode(sum[1]), encode(sum[2]), 255]);
            }
        }
    }
    Texture { width: size as u32, height: (size * 6) as u32, rgba }
}

/// Which way a point on a cubemap face looks, in the usual order and
/// orientation: +X, -X, +Y, -Y, +Z, -Z, with y up.
fn direction(face: usize, u: f32, v: f32) -> [f32; 3] {
    let d = match face {
        0 => [1.0, -v, -u],
        1 => [-1.0, -v, u],
        2 => [u, 1.0, v],
        3 => [u, -1.0, -v],
        4 => [u, -v, 1.0],
        _ => [-u, -v, -1.0],
    };
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    [d[0] / len, d[1] / len, d[2] / len]
}

/// The panorama's colour in a direction, sampled bilinearly. The seam wraps.
fn look_up(panorama: &Texture, dir: [f32; 3]) -> [u8; 4] {
    let (w, h) = (panorama.width as usize, panorama.height as usize);
    let longitude = math::atan2(dir[0], -dir[2]);
    let latitude = math::acos(dir[1].clamp(-1.0, 1.0));
    let x = (longitude / std::f32::consts::TAU + 0.5) * w as f32 - 0.5;
    let y = (latitude / std::f32::consts::PI) * h as f32 - 0.5;
    let x0 = x.floor();
    let y0 = y.floor();
    let (fx, fy) = (x - x0, y - y0);
    let at = |x: i64, y: i64| -> [f32; 4] {
        let x = x.rem_euclid(w as i64) as usize;
        let y = y.clamp(0, h as i64 - 1) as usize;
        let i = (y * w + x) * 4;
        [
            f32::from(panorama.rgba[i]),
            f32::from(panorama.rgba[i + 1]),
            f32::from(panorama.rgba[i + 2]),
            f32::from(panorama.rgba[i + 3]),
        ]
    };
    let (x0, y0) = (x0 as i64, y0 as i64);
    let (a, b, c, d) = (at(x0, y0), at(x0 + 1, y0), at(x0, y0 + 1), at(x0 + 1, y0 + 1));
    let mut out = [0u8; 4];
    for i in 0..4 {
        let top = a[i] + (b[i] - a[i]) * fx;
        let bottom = c[i] + (d[i] - c[i]) * fx;
        out[i] = (top + (bottom - top) * fy).round().clamp(0.0, 255.0) as u8;
    }
    out
}

/// How much of the badge, in badge units, one tile of [`sandblast`] covers.
pub const GRIT_TILE: f32 = 0.125;

/// The badge's reverse: a tangent-space normal map of sandblasted metal, a
/// fine even grit with nothing in it for the eye to pick out.
///
/// The map tiles, and the reverse repeats it every [`GRIT_TILE`]: one map
/// stretched over the whole badge would be too coarse to be grit.
///
/// glTF and RealityKit want the normals themselves, not a height map.
pub fn sandblast() -> Texture {
    // Small, and repeated often: the size of the grit on screen is the tile
    // over the side, and a smaller map is a smaller file and less memory.
    let side = 256usize;
    // White noise from a hash of the texel, so the grit is the same on every
    // platform, then blurred with wrapping so that the map tiles. Blurred
    // noise has no lattice to show, as value noise does.
    let noise = |salt: u64| -> Vec<f32> {
        (0..side * side)
            .map(|i| {
                let mut h = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt;
                h ^= h >> 33;
                h = h.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
                h ^= h >> 33;
                (h >> 40) as f32 / (1u64 << 24) as f32
            })
            .collect()
    };
    let blur = |field: Vec<f32>, sigma: f32| -> Vec<f32> {
        let reach = (sigma * 3.0).ceil() as i64;
        let weights: Vec<f32> = (-reach..=reach).map(|d| math::exp(-(d * d) as f32 / (2.0 * sigma * sigma))).collect();
        let total: f32 = weights.iter().sum();
        let pass = |from: &[f32], across: bool| -> Vec<f32> {
            let mut to = vec![0f32; side * side];
            for y in 0..side {
                for x in 0..side {
                    let mut sum = 0.0;
                    for (k, w) in weights.iter().enumerate() {
                        let d = k as i64 - reach;
                        let (sx, sy) = if across {
                            ((x as i64 + d).rem_euclid(side as i64) as usize, y)
                        } else {
                            (x, (y as i64 + d).rem_euclid(side as i64) as usize)
                        };
                        sum += from[sy * side + sx] * w;
                    }
                    to[y * side + x] = sum / total;
                }
            }
            to
        };
        pass(&pass(&field, true), false)
    };
    // Two sizes of grit, a pixel or two on a badge filling a screen. Nothing
    // coarser: a broad swell would show the tiles repeating.
    let fine = blur(noise(0xB1A5), 1.2);
    let coarse = blur(noise(0x5A7D), 2.5);
    let height: Vec<f32> = fine.iter().zip(&coarse).map(|(f, c)| f * 0.6 + c * 0.4).collect();

    // Slopes into normals. The strength is what the grit reads as at a
    // badge's size: enough to break up the reflection, never enough to show.
    const STRENGTH: f32 = 3.0;
    let at = |x: usize, y: usize| height[(y % side) * side + (x % side)];
    let mut rgba = vec![0u8; side * side * 4];
    for y in 0..side {
        for x in 0..side {
            let dx = at(x + 1, y) - at((x + side - 1) % side, y);
            let dy = at(x, y + 1) - at(x, (y + side - 1) % side);
            let n = [-dx * STRENGTH, -dy * STRENGTH, 1.0];
            let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            let byte = |v: f32| ((v / length * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0).round() as u8;
            let at = (y * side + x) * 4;
            rgba[at] = byte(n[0]);
            rgba[at + 1] = byte(n[1]);
            rgba[at + 2] = byte(n[2]);
            rgba[at + 3] = 255;
        }
    }
    Texture { width: side as u32, height: side as u32, rgba }
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_showcase_is_a_dark_room_lit_by_its_panels() {
        let panorama = showcase_environment();
        assert_eq!((panorama.width, panorama.height), (1024, 512));
        let at = |azimuth: f32, elevation: f32| {
            // The panorama's convention: u = 0.5 looks down -z, u = 0.75
            // down +x, and azimuth is measured from +z toward +x.
            let u = 1.0 - azimuth / 360.0;
            let v = (90.0 - elevation) / 180.0;
            let (x, y) = ((u.rem_euclid(1.0) * 1024.0) as usize, (v * 512.0) as usize);
            panorama.rgba[(y * 1024 + x) * 4]
        };
        // The stripe a face at rest crosses stands out of what is either side.
        assert!(at(-44.0, -8.0) > at(-52.0, -8.0) + 60, "the stripe: {} {}", at(-44.0, -8.0), at(-52.0, -8.0));
        // Behind the badge is dark; the enamel is lit from overhead, where a
        // face seldom points, more than from the camera's side, where it does.
        assert!(at(180.0, 0.0) < 12, "behind the badge: {}", at(180.0, 0.0));
        assert!(at(0.0, 60.0) > at(0.0, 0.0) + 60);
        // The scrim round the camera is bright at the top, dark at the bottom.
        assert!(at(0.0, 22.0) > at(0.0, -4.0) + 30);
        // Nothing is clipped: the brightest light still fits a byte.
        assert!(panorama.rgba.chunks_exact(4).all(|p| p[0] < 255 && p[1] < 255 && p[2] < 255));
    }

    #[test]
    fn the_showcase_is_the_same_every_time() {
        assert_eq!(showcase_environment().rgba, showcase_environment().rgba);
    }

    #[test]
    fn the_reverse_is_a_normal_map_of_shallow_grit() {
        let grit = sandblast();
        let normals: Vec<[f32; 3]> = grit
            .rgba
            .chunks_exact(4)
            .map(|p| {
                let f = |v: u8| f32::from(v) / 255.0 * 2.0 - 1.0;
                [f(p[0]), f(p[1]), f(p[2])]
            })
            .collect();
        assert!(normals.iter().all(|n| n[2] > 0.0), "a normal map never points into the surface");
        let mean = normals.iter().map(|n| n[2]).sum::<f32>() / normals.len() as f32;
        assert!(mean > 0.95, "the grit is shallow: {mean}");
        // It is not flat: some texels lean, but none far over.
        let lean = |n: &[f32; 3]| (n[0] * n[0] + n[1] * n[1]).sqrt();
        assert!(normals.iter().any(|n| lean(n) > 0.2));
        assert!(normals.iter().all(|n| lean(n) < 0.7));
    }

    #[test]
    fn the_reverse_is_the_same_every_time() {
        assert_eq!(sandblast().rgba, sandblast().rgba);
    }
}

/// Where a tile ended up in an atlas: the multiplier and offset that turn the
/// tile's own 0…1 coordinates into the atlas's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tile {
    pub scale: [f32; 2],
    pub offset: [f32; 2],
}

/// Lays tiles out on one sheet, tallest first, row by row.
///
/// A badge paints a face at a time, which on a detailed design is a hundred
/// small textures; a renderer would rather have one. Each tile keeps its own
/// pixels, with its edge repeated into a border so that filtering at the seam
/// reads the tile and not its neighbour.
pub fn atlas(tiles: &[Texture]) -> (Texture, Vec<Tile>) {
    const PAD: u32 = 4;
    let mut order: Vec<usize> = (0..tiles.len()).collect();
    // Tallest first, and by size then index so the same input always packs the
    // same way.
    order.sort_by_key(|&i| (u32::MAX - tiles[i].height, u32::MAX - tiles[i].width, i));

    let area: u64 = tiles.iter().map(|t| u64::from(t.width + 2 * PAD) * u64::from(t.height + 2 * PAD)).sum();
    let widest = tiles.iter().map(|t| t.width + 2 * PAD).max().unwrap_or(1);
    // Room for the rows that will not fill exactly.
    let mut width = ((area as f64) * 1.1).sqrt().ceil() as u32;
    width = width.max(widest).next_power_of_two();

    let mut places = vec![Tile { scale: [1.0, 1.0], offset: [0.0, 0.0] }; tiles.len()];
    let mut spots: Vec<(usize, u32, u32)> = Vec::with_capacity(tiles.len());
    let (mut x, mut y, mut row) = (0, 0, 0);
    for &i in &order {
        let (w, h) = (tiles[i].width + 2 * PAD, tiles[i].height + 2 * PAD);
        if x + w > width {
            x = 0;
            y += row;
            row = 0;
        }
        spots.push((i, x + PAD, y + PAD));
        x += w;
        row = row.max(h);
    }
    let height = (y + row).max(1).next_power_of_two();

    let mut sheet = Texture { width, height, rgba: vec![0; (width as usize * height as usize) * 4] };
    for (i, x, y) in spots {
        blit(&mut sheet, &tiles[i], x, y, PAD);
        places[i] = Tile {
            scale: [tiles[i].width as f32 / width as f32, tiles[i].height as f32 / height as f32],
            offset: [x as f32 / width as f32, y as f32 / height as f32],
        };
    }
    (sheet, places)
}

/// Copies a tile onto the sheet at (x, y), with its edge pixels repeated
/// `pad` further out on every side.
fn blit(sheet: &mut Texture, tile: &Texture, x: u32, y: u32, pad: u32) {
    let (tw, th) = (tile.width as i64, tile.height as i64);
    if tw == 0 || th == 0 {
        return;
    }
    let pad = pad as i64;
    for row in -pad..th + pad {
        let sy = row.clamp(0, th - 1);
        let dy = y as i64 + row;
        if dy < 0 || dy >= sheet.height as i64 {
            continue;
        }
        for column in -pad..tw + pad {
            let sx = column.clamp(0, tw - 1);
            let dx = x as i64 + column;
            if dx < 0 || dx >= sheet.width as i64 {
                continue;
            }
            let from = ((sy * tw + sx) * 4) as usize;
            let to = ((dy * sheet.width as i64 + dx) * 4) as usize;
            sheet.rgba[to..to + 4].copy_from_slice(&tile.rgba[from..from + 4]);
        }
    }
}

#[cfg(test)]
mod packing {
    use super::*;

    fn block(width: u32, height: u32, shade: u8) -> Texture {
        Texture { width, height, rgba: vec![shade; (width * height * 4) as usize] }
    }

    #[test]
    fn every_tile_is_found_again_where_its_place_says() {
        let tiles = [block(40, 10, 10), block(7, 30, 20), block(4, 4, 30), block(60, 25, 40)];
        let (sheet, places) = atlas(&tiles);
        for (tile, place) in tiles.iter().zip(&places) {
            // The middle of the tile, read through the place it was given.
            let x = (place.offset[0] + 0.5 * place.scale[0]) * sheet.width as f32;
            let y = (place.offset[1] + 0.5 * place.scale[1]) * sheet.height as f32;
            let at = ((y as u32 * sheet.width + x as u32) * 4) as usize;
            assert_eq!(sheet.rgba[at], tile.rgba[0], "tile {}x{}", tile.width, tile.height);
        }
    }

    #[test]
    fn tiles_do_not_run_into_one_another() {
        let tiles = [block(30, 30, 100), block(30, 30, 200)];
        let (sheet, places) = atlas(&tiles);
        // A pixel just outside a tile is its own edge repeated, never its
        // neighbour's colour.
        for (tile, place) in tiles.iter().zip(&places) {
            let x = (place.offset[0] * sheet.width as f32) as i64 - 1;
            let y = (place.offset[1] * sheet.height as f32) as i64;
            if x < 0 {
                continue;
            }
            let at = ((y * sheet.width as i64 + x) * 4) as usize;
            assert_eq!(sheet.rgba[at], tile.rgba[0]);
        }
    }

    #[test]
    fn the_same_tiles_pack_the_same_way() {
        let tiles = [block(12, 9, 1), block(9, 12, 2), block(12, 9, 3)];
        let (a, first) = atlas(&tiles);
        let (b, second) = atlas(&tiles);
        assert_eq!(first, second);
        assert_eq!((a.width, a.height, a.rgba), (b.width, b.height, b.rgba));
    }
}

#[cfg(test)]
mod cubemaps {
    use super::*;

    #[test]
    fn a_cubemap_is_six_square_faces() {
        let sheet = showcase_cubemap(16);
        assert_eq!((sheet.width, sheet.height), (16, 96));
        assert_eq!(sheet.rgba.len(), 16 * 96 * 4);
    }

    #[test]
    fn the_faces_meet_at_their_edges() {
        // Each edge of +Z is an edge of the face beside it: the same
        // directions along it, so the faces are turned as they are sampled.
        // The panorama is not asked: a panel's edge can fall on a seam.
        type Edge = fn(f32) -> (f32, f32);
        let beside: [(Edge, usize, Edge); 4] = [
            (|t: f32| (1.0, t), 0, |t: f32| (-1.0, t)),  // +X
            (|t: f32| (-1.0, t), 1, |t: f32| (1.0, t)),  // -X
            (|t: f32| (t, -1.0), 2, |t: f32| (t, 1.0)),  // +Y
            (|t: f32| (t, 1.0), 3, |t: f32| (t, -1.0)),  // -Y
        ];
        for (front_edge, face, edge) in beside {
            for k in 0..=8 {
                let t = k as f32 / 4.0 - 1.0;
                let ((fu, fv), (u, v)) = (front_edge(t), edge(t));
                let (a, b) = (direction(4, fu, fv), direction(face, u, v));
                assert!(dot(a, b) > 0.9999, "+Z against face {face} at {t}: {a:?} vs {b:?}");
            }
        }
    }

    #[test]
    fn what_a_surface_receives_is_softer_than_what_it_reflects() {
        let size = 8;
        let sharp = showcase_cubemap(size);
        let soft = showcase_irradiance(size);
        let spread = |t: &Texture| {
            let lumas: Vec<f32> = t
                .rgba
                .chunks_exact(4)
                .map(|p| 0.2126 * f32::from(p[0]) + 0.7152 * f32::from(p[1]) + 0.0722 * f32::from(p[2]))
                .collect();
            let mean = lumas.iter().sum::<f32>() / lumas.len() as f32;
            (lumas.iter().map(|l| (l - mean) * (l - mean)).sum::<f32>() / lumas.len() as f32).sqrt()
        };
        assert!(spread(&soft) < spread(&sharp), "a cosine lobe evens the light out");
    }
}
