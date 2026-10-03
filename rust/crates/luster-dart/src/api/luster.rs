//! Dart API. Those not marked `sync` run on flutter_rust_bridge's thread
//! pool, off the UI isolate.

use std::sync::Arc;

use flutter_rust_bridge::frb;
use luster_core::{badge, style, texture, vertex as mesh};

use crate::frb_generated::RustAutoOpaque;

/// The engine's version.
#[frb(sync)]
pub fn version() -> String {
    luster_core::version().to_owned()
}

/// sRGB, straight alpha.
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl From<badge::Rgba> for Rgba {
    fn from(c: badge::Rgba) -> Self {
        Self { r: c.r, g: c.g, b: c.b, a: c.a }
    }
}

impl From<Rgba> for badge::Rgba {
    fn from(c: Rgba) -> Self {
        Self { r: c.r, g: c.g, b: c.b, a: c.a }
    }
}

pub enum MaterialRole {
    Plated,
    GoldBack,
    Field,
    Art,
    /// Metal plated in the document's colours: a metal, in the colour it
    /// carries rather than the metal's.
    Glaze,
}

impl From<badge::MaterialRole> for MaterialRole {
    fn from(r: badge::MaterialRole) -> Self {
        match r {
            badge::MaterialRole::Plated => Self::Plated,
            badge::MaterialRole::GoldBack => Self::GoldBack,
            badge::MaterialRole::Field => Self::Field,
            badge::MaterialRole::Art => Self::Art,
            badge::MaterialRole::Glaze => Self::Glaze,
        }
    }
}

impl From<MaterialRole> for badge::MaterialRole {
    fn from(r: MaterialRole) -> Self {
        match r {
            MaterialRole::Plated => Self::Plated,
            MaterialRole::GoldBack => Self::GoldBack,
            MaterialRole::Field => Self::Field,
            MaterialRole::Art => Self::Art,
            MaterialRole::Glaze => Self::Glaze,
        }
    }
}

pub struct Material {
    pub name: String,
    pub role: MaterialRole,
    pub color: Rgba,
    pub metallic: f32,
    pub roughness: f32,
    /// A texture for the front face, where the document's colour changes
    /// across it; indexes the badge's textures.
    pub texture: Option<u32>,
}

/// One submesh as separate attribute arrays, the shape flutter_scene's
/// `MeshGeometry.fromArrays` takes, in flutter_scene's left-handed space: the
/// engine's z is negated (the badge's face looks down -z) and triangles are
/// wound the other way to match.
pub struct Submesh {
    pub name: String,
    pub material: u32,
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
    pub tex_coords: Vec<f32>,
    pub tangents: Vec<f32>,
    pub indices: Vec<u32>,
}

// Translated, not opaque: flutter_scene reads the arrays in Dart.
#[frb(non_opaque)]
pub struct LusterBadge {
    pub design_key: String,
    pub submeshes: Vec<Submesh>,
    pub materials: Vec<Material>,
    pub textures: Vec<Texture>,
    /// The badge as the engine struck it, which is what a GLB is written from:
    /// the arrays above are already in flutter_scene's space.
    pub struck: RustAutoOpaque<Struck>,
}

/// A struck badge, held in Rust.
#[frb(opaque)]
pub struct Struck(Arc<badge::Badge>);

impl Struck {
    /// The badge as a glTF binary: see `LusterBadge.glb` in Dart.
    pub fn glb(&self, scale: f32, metal: Rgba) -> Vec<u8> {
        luster_core::export::glb(&self.0, scale, metal.into())
    }
}


/// What went wrong, as the other bindings tell it apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LusterErrorKind {
    InvalidSvg,
    InputTooComplex,
    NothingToMint,
    Cancelled,
    /// Dart's `LusterErrorKind.engineFailure`.
    Internal,
}

/// Why a badge could not be minted: thrown to Dart.
#[derive(Debug)]
pub struct LusterError {
    pub kind: LusterErrorKind,
    /// What the engine said, for a person to read.
    pub reason: String,
}

impl From<luster_core::Error> for LusterError {
    fn from(e: luster_core::Error) -> Self {
        use luster_core::Error as E;
        let kind = match &e {
            E::InvalidSvg(_) => LusterErrorKind::InvalidSvg,
            E::InputTooComplex(_) => LusterErrorKind::InputTooComplex,
            E::NothingToMint => LusterErrorKind::NothingToMint,
            E::Cancelled => LusterErrorKind::Cancelled,
            E::Internal(_) => LusterErrorKind::Internal,
        };
        LusterError { kind, reason: e.to_string() }
    }
}

/// What to strike: see `LusterOptions` in Dart.
pub struct MintOptions {
    pub without_hidden_faces: bool,
    pub metal_lines: bool,
}

/// A badge asked for: struck on the engine's own threads, shared with anyone
/// else asking for the same one, and ready at once if it was struck lately.
#[frb(opaque)]
pub struct MintRequest(luster_core::mints::Request);

impl MintRequest {
    #[frb(sync)]
    pub fn new(svg: Vec<u8>, options: MintOptions) -> MintRequest {
        let options = luster_core::MintOptions {
            metal_lines: options.metal_lines,
            without_hidden_faces: options.without_hidden_faces,
        };
        MintRequest(luster_core::mints::request(svg, options))
    }

    /// The badge, once it is struck; a `cancelled` error once given up.
    pub async fn badge(&self) -> Result<LusterBadge, LusterError> {
        Ok(translate(self.0.badge().await?))
    }

    /// Gives the badge up: `badge` ends `cancelled`, and the engine stops if
    /// nobody else wants it. Nothing once it has come.
    #[frb(sync)]
    pub fn cancel(&self) {
        self.0.cancel()
    }
}

/// The badge as Dart reads it.
fn translate(badge: Arc<badge::Badge>) -> LusterBadge {
    LusterBadge {
        design_key: badge.design_key.clone(),
        submeshes: badge.submeshes.iter().map(split).collect(),
        materials: badge
            .materials
            .iter()
            .map(|m| Material {
                name: m.name.clone(),
                role: m.role.into(),
                color: m.color.into(),
                metallic: m.metallic,
                roughness: m.roughness,
                texture: m.texture,
            })
            .collect(),
        textures: badge
            .textures
            .iter()
            .map(|t| Texture { width: t.width, height: t.height, rgba: t.rgba.clone() })
            .collect(),
        struck: RustAutoOpaque::new(Struck(badge)),
    }
}

/// Splits the engine's interleaved vertices into attribute arrays.
fn split(s: &badge::Submesh) -> Submesh {
    let n = s.vertex_count as usize;
    let f = |o: usize| f32::from_le_bytes(s.vertices[o..o + 4].try_into().unwrap());
    let mut out = Submesh {
        name: s.name.clone(),
        material: s.material,
        positions: Vec::with_capacity(n * 3),
        normals: Vec::with_capacity(n * 3),
        tex_coords: Vec::with_capacity(n * 2),
        tangents: Vec::with_capacity(n * 4),
        indices: s
            .indices
            .chunks_exact(12)
            .flat_map(|t| {
                let i = |k: usize| u32::from_le_bytes(t[4 * k..4 * k + 4].try_into().unwrap());
                [i(0), i(2), i(1)]
            })
            .collect(),
    };
    // Right-handed to left-handed: negate z on every vector.
    let flip = |mut v: Vec<f32>, z: usize, stride: usize| {
        v.iter_mut().skip(z).step_by(stride).for_each(|c| *c = -*c);
        v
    };
    for i in 0..n {
        let o = i * mesh::VERTEX_STRIDE;
        out.positions.extend((0..3).map(|k| f(o + mesh::POSITION_OFFSET + 4 * k)));
        out.normals.extend((0..3).map(|k| f(o + mesh::NORMAL_OFFSET + 4 * k)));
        out.tex_coords.extend((0..2).map(|k| f(o + mesh::UV_OFFSET + 4 * k)));
        out.tangents.extend((0..4).map(|k| f(o + mesh::TANGENT_OFFSET + 4 * k)));
    }
    out.positions = flip(out.positions, 2, 3);
    out.normals = flip(out.normals, 2, 3);
    out.tangents = flip(out.tangents, 2, 4);
    out
}

/// RGBA8, sRGB, rows top to bottom.
pub struct Texture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// The panorama the badge reflects (equirectangular): the showcase's.
pub fn showcase_environment() -> Texture {
    let t = texture::showcase_environment();
    Texture { width: t.width, height: t.height, rgba: t.rgba }
}

/// The badge's reverse: a tangent-space normal map of sandblasted metal, the
/// same for every badge.
pub fn sandblast() -> Texture {
    let t = texture::sandblast();
    Texture { width: t.width, height: t.height, rgba: t.rgba }
}

#[frb(sync)]
pub fn default_gold() -> Rgba {
    badge::GOLD.into()
}

#[frb(sync)]
pub fn silver() -> Rgba {
    badge::SILVER.into()
}

#[frb(sync)]
pub fn copper() -> Rgba {
    badge::COPPER.into()
}

/// See `LusterLighting` in Dart.
pub enum Lighting {
    Showcase,
    Off,
}

fn core_lighting(l: Lighting) -> style::Lighting {
    match l {
        Lighting::Showcase => style::Lighting::Showcase,
        Lighting::Off => style::Lighting::Off,
    }
}

/// What flutter_scene needs of a lighting: how much of the environment to
/// take, and what the lamps are worth here. What each lamp and material is
/// given comes from `lamp` and `finish`.
pub struct LightingPreset {
    /// What the environment is worth here: flutter_scene's
    /// `environmentIntensity`.
    pub environment: f32,
    pub scene_lux: f32,
}

#[frb(sync)]
pub fn lighting_preset(lighting: Lighting) -> LightingPreset {
    let p = style::preset(core_lighting(lighting));
    LightingPreset { environment: p.environment * p.scene_environment, scene_lux: p.scene_lux }
}

/// What the studio is worth to flutter_scene whatever the lighting.
pub struct SceneCalibration {
    pub exposure: f32,
    /// What a lamp's intensity, times `scene_lux`, is worth here.
    pub lamp_unit: f32,
}

#[frb(sync)]
pub fn scene_calibration() -> SceneCalibration {
    let s = style::FLUTTER_SCENE;
    SceneCalibration { exposure: s.exposure, lamp_unit: s.lamp_unit }
}

/// RealityKit's tone mapping as a `.cube` table, for flutter_scene to draw
/// through with no tone curve of its own: a scene exposed by
/// `SceneCalibration.exposure`, clamped and read through it, shows what
/// RealityKit would.
pub fn realitykit_tone_cube() -> String {
    style::realitykit_tone_cube()
}

/// How one of the badge's materials is finished under a lighting.
pub struct Finish {
    pub metallic: f32,
    pub roughness: f32,
    /// How much a dielectric reflects, as a part of glass's 4% head on:
    /// glTF's `KHR_materials_specular` factor. Metal takes no notice of it.
    pub specular: f32,
    /// Coloured by the metal the badge is plated in, rather than by the
    /// material's own colour.
    pub plated: bool,
}

#[frb(sync)]
pub fn finish(role: MaterialRole, lighting: Lighting) -> Finish {
    let preset = style::preset(core_lighting(lighting));
    let f = style::finish(role.into(), &preset);
    Finish { metallic: f.metallic, roughness: f.roughness, specular: f.specular, plated: f.plated }
}

pub enum LightRole {
    Key,
    Fill,
    Back,
    /// Rides on the camera, pointing where it looks.
    Headlight,
}

impl From<style::LightRole> for LightRole {
    fn from(r: style::LightRole) -> Self {
        match r {
            style::LightRole::Key => Self::Key,
            style::LightRole::Fill => Self::Fill,
            style::LightRole::Back => Self::Back,
            style::LightRole::Headlight => Self::Headlight,
        }
    }
}

impl From<LightRole> for style::LightRole {
    fn from(r: LightRole) -> Self {
        match r {
            LightRole::Key => Self::Key,
            LightRole::Fill => Self::Fill,
            LightRole::Back => Self::Back,
            LightRole::Headlight => Self::Headlight,
        }
    }
}

/// What a lamp is worth in the preset's units; multiply by `scene_lux`.
#[frb(sync)]
pub fn lamp(role: LightRole, lighting: Lighting) -> f32 {
    style::lamp(role.into(), &style::preset(core_lighting(lighting)))
}

/// A lamp in the studio: which way it points, and what colour it is.
pub struct Light {
    pub role: LightRole,
    pub color: Rgba,
    /// Which way it shines, already in flutter_scene's left-handed space.
    pub direction: Vec<f32>,
}

#[frb(sync)]
pub fn studio_lights() -> Vec<Light> {
    style::lights()
        .into_iter()
        .map(|l| {
            let [x, y, z] = style::light_direction(l.pitch, l.yaw);
            Light { role: l.role.into(), color: l.color.into(), direction: vec![x, y, -z] }
        })
        .collect()
}

/// Where the camera stands.
pub struct Camera {
    pub fov: f32,
    pub distance: f32,
}

#[frb(sync)]
pub fn studio_camera() -> Camera {
    Camera { fov: style::CAMERA_FOV, distance: style::CAMERA_DISTANCE }
}

/// The camera's vertical field of view, degrees, for a badge `width` by
/// `height` across in a view `aspect` wide over tall.
#[frb(sync)]
pub fn field_of_view(width: f32, height: f32, aspect: f32) -> f32 {
    style::field_of_view(width, height, aspect)
}

/// How the badge answers a drag, the same on every platform.
pub struct Handling {
    /// Radians per logical pixel dragged.
    pub spin_per_point: f32,
    pub tilt_per_point: f32,
    pub tilt_limit: f32,
    /// Where the badge sits before it is touched, radians.
    pub resting_tilt: f32,
    pub resting_spin: f32,
}

#[frb(sync)]
pub fn handling() -> Handling {
    let h = style::HANDLING;
    Handling {
        spin_per_point: h.spin_per_point,
        tilt_per_point: h.tilt_per_point,
        tilt_limit: h.tilt_limit,
        resting_tilt: style::RESTING_POSE[0],
        resting_spin: style::RESTING_POSE[1],
    }
}

/// How far a flick carries the badge over a frame, and the spin it is left with.
pub struct Coast {
    pub turn: f32,
    pub velocity: f32,
}

/// Carries a flick's spin (radians per second) over `dt` seconds.
#[frb(sync)]
pub fn coast(velocity: f32, dt: f32) -> Coast {
    let (turn, velocity) = style::coast(velocity, dt);
    Coast { turn, velocity }
}
