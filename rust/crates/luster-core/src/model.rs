//! What a badge is made of: colours and materials. A leaf: every stage of the
//! engine uses these, and they use nothing of it but its maths.

use crate::math;

/// An sRGB-encoded channel, 0...1, as light: what a renderer mixes.
pub fn linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { math::powf((c + 0.055) / 1.055, 2.4) }
}

/// Light, 0...1, encoded as sRGB.
pub fn encoded(c: f32) -> f32 {
    if c <= 0.0031308 { c * 12.92 } else { 1.055 * math::powf(c, 1.0 / 2.4) - 0.055 }
}

/// sRGB color with straight alpha, 0...1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Rgba {
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }
}

/// What a material is for, so renderers can re-light and re-plate a badge
/// without rebuilding it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaterialRole {
    /// Face and edge metal: plated in the metal the renderer is asked for.
    Plated,
    /// The reverse: metal with a sandblasted normal map.
    GoldBack,
    /// Enamel cells.
    Field,
    /// Painted art faces.
    Art,
    /// Metal plated in the document's colours: a metal, tinted by the colour
    /// it carries rather than by the metal's.
    Glaze,
}

#[derive(Clone, Debug)]
pub struct Material {
    pub name: String,
    pub role: MaterialRole,
    pub color: Rgba,
    pub metallic: f32,
    pub roughness: f32,
    /// A texture for the front face, where the document's colour changes
    /// across it. Indexes the badge's textures.
    pub texture: Option<u32>,
}

/// The default metal: pale gold.
pub const GOLD: Rgba = Rgba::rgb(1.0, 0.8, 0.52);
/// The other metals the apps offer beside gold.
pub const SILVER: Rgba = Rgba::rgb(0.93, 0.93, 0.95);
pub const COPPER: Rgba = Rgba::rgb(0.95, 0.64, 0.54);
/// The enamel a single-coloured badge takes, when the art is not a colour
/// separation of its own: bone.
pub const ENAMEL_COLOR: Rgba = Rgba::rgb(0.94, 0.91, 0.84);

/// What to strike, beyond the artwork itself. Everything here changes the
/// badge's shape or what it is made of; the metal's colour does not, and is
/// the renderer's to choose (or `export::glb`'s, for a file).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MintOptions {
    /// Leave the lines and the edge's top in the metal, rather than plate them
    /// in the colours the document paints on them.
    pub metal_lines: bool,
    /// Leave out the triangles no view can see: a smaller badge to send, at the
    /// cost of the work to find them.
    pub without_hidden_faces: bool,
}
