//! The uniffi surface: the only place that knows about FFI types. Minting is
//! a request the engine runs on threads of its own; everything else is
//! synchronous, and the Swift and Kotlin wrappers run the slow parts off the
//! main thread.
//!
//! Badges cross as the engine makes them: interleaved vertices, right-handed,
//! v running down the image. Each renderer turns them into what it takes on
//! its own side (RealityKit flips v; Filament reads them as they are). The
//! Dart surface, luster-dart, converts in Rust instead, since a per-vertex
//! loop in Dart would cost more than the mint.

use std::sync::Arc;

use luster_core::{badge, style, texture};

uniffi::setup_scaffolding!();

/// The engine's version.
#[uniffi::export]
pub fn version() -> String {
    luster_core::version().to_owned()
}

// MARK: - Errors and cancellation

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum LusterError {
    // Not `message`: Kotlin's Throwable has one of those, and a field by that
    // name comes out of the bindings as a clash.
    #[error("the SVG could not be read: {reason}")]
    InvalidSvg { reason: String },
    #[error("the SVG is too complex: {reason}")]
    InputTooComplex { reason: String },
    #[error("the SVG has nothing to mint")]
    NothingToMint,
    #[error("cancelled")]
    Cancelled,
    #[error("the engine failed: {reason}")]
    Internal { reason: String },
}

impl From<luster_core::Error> for LusterError {
    fn from(e: luster_core::Error) -> Self {
        use luster_core::Error as E;
        match e {
            E::InvalidSvg(reason) => Self::InvalidSvg { reason },
            E::InputTooComplex(reason) => Self::InputTooComplex { reason },
            E::NothingToMint => Self::NothingToMint,
            E::Cancelled => Self::Cancelled,
            E::Internal(reason) => Self::Internal { reason },
        }
    }
}

// MARK: - Badges

#[derive(uniffi::Record, Clone, Copy)]
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

#[derive(uniffi::Enum, Clone, Copy)]
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

#[derive(uniffi::Record)]
pub struct Material {
    pub name: String,
    pub role: MaterialRole,
    /// sRGB, straight alpha.
    pub color: Rgba,
    pub metallic: f32,
    pub roughness: f32,
    /// A texture for the front face, where the document's colour changes
    /// across it; indexes the badge's textures.
    pub texture: Option<u32>,
}

/// Vertices are laid out as `vertex_layout()` says: position f32×3, normal
/// f32×3, uv f32×2, tangent f32×4, little-endian. Indices are u32, three per
/// triangle.
#[derive(uniffi::Record)]
pub struct Submesh {
    pub name: String,
    pub material: u32,
    pub vertices: Vec<u8>,
    pub indices: Vec<u8>,
    pub vertex_count: u32,
    pub index_count: u32,
}

/// Where each attribute sits in a vertex, in bytes.
#[derive(uniffi::Record)]
pub struct VertexLayout {
    pub stride: u32,
    pub position: u32,
    pub normal: u32,
    pub uv: u32,
    pub tangent: u32,
}

#[uniffi::export]
pub fn vertex_layout() -> VertexLayout {
    use luster_core::vertex as mesh;
    VertexLayout {
        stride: mesh::VERTEX_STRIDE as u32,
        position: mesh::POSITION_OFFSET as u32,
        normal: mesh::NORMAL_OFFSET as u32,
        uv: mesh::UV_OFFSET as u32,
        tangent: mesh::TANGENT_OFFSET as u32,
    }
}

/// A minted badge. Immutable; buffers are copied out on request.
#[derive(uniffi::Object)]
pub struct LusterBadge(Arc<badge::Badge>);

#[uniffi::export]
impl LusterBadge {
    pub fn design_key(&self) -> String {
        self.0.design_key.clone()
    }

    pub fn submeshes(&self) -> Vec<Submesh> {
        self.0
            .submeshes
            .iter()
            .map(|s| Submesh {
                name: s.name.clone(),
                material: s.material,
                vertices: s.vertices.clone(),
                indices: s.indices.clone(),
                vertex_count: s.vertex_count,
                index_count: s.index_count,
            })
            .collect()
    }

    pub fn materials(&self) -> Vec<Material> {
        self.0
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
            .collect()
    }

    /// The badge as a glTF binary, textures and all, its metal plated in
    /// `metal`, `scale` badge-widths across. Blocks: call it off the main
    /// thread.
    pub fn glb(&self, scale: f32, metal: Rgba) -> Vec<u8> {
        luster_core::export::glb(&self.0, scale, metal.into())
    }

    /// What the materials' textures are made of, in the order they index.
    pub fn textures(&self) -> Vec<Texture> {
        self.0
            .textures
            .iter()
            .map(|t| Texture { width: t.width, height: t.height, rgba: t.rgba.clone() })
            .collect()
    }
}


/// What to strike, beyond the artwork itself. The metal's colour is not
/// here: it changes no part of the badge's shape, and is the renderer's.
#[derive(uniffi::Record)]
pub struct MintOptions {
    /// Leave out the triangles no view can see: a smaller badge to send on,
    /// at the cost of the work to find them.
    pub without_hidden_faces: bool,
    /// Leave the lines and the edge's top in the metal, rather than plate them
    /// in the colours the document paints on them.
    pub metal_lines: bool,
}

/// A badge asked for: struck on the engine's own threads, shared with anyone
/// else asking for the same one, and ready at once if it was struck lately.
/// Dropping it, or `cancel`, gives it up.
#[derive(uniffi::Object)]
pub struct MintRequest(luster_core::mints::Request);

#[uniffi::export]
impl MintRequest {
    #[uniffi::constructor]
    pub fn new(svg: Vec<u8>, options: MintOptions) -> Arc<Self> {
        let options = luster_core::MintOptions {
            metal_lines: options.metal_lines,
            without_hidden_faces: options.without_hidden_faces,
        };
        Arc::new(Self(luster_core::mints::request(svg, options)))
    }

    /// The badge, once it is struck; `Cancelled` once given up.
    pub async fn badge(&self) -> Result<Arc<LusterBadge>, LusterError> {
        Ok(Arc::new(LusterBadge(self.0.badge().await?)))
    }

    /// Gives the badge up: `badge` ends with `Cancelled`, and the engine stops
    /// if nobody else wants it. Nothing once it has come.
    pub fn cancel(&self) {
        self.0.cancel()
    }
}

/// The default metal color.
#[uniffi::export]
pub fn default_gold() -> Rgba {
    badge::GOLD.into()
}

#[uniffi::export]
pub fn silver() -> Rgba {
    badge::SILVER.into()
}

#[uniffi::export]
pub fn copper() -> Rgba {
    badge::COPPER.into()
}

// MARK: - Studio

#[derive(uniffi::Enum, Clone, Copy)]
pub enum Lighting {
    /// A product photographer's studio, lit by its panorama alone: see
    /// `showcase_environment`.
    Showcase,
    /// Lamps alone and nothing that shines, for looking at a badge's shape.
    Off,
}

/// What a renderer needs of a lighting beyond each lamp and material, which
/// come from `lamp` and `finish`.
#[derive(uniffi::Record)]
pub struct LightingPreset {
    /// How much of the environment to take.
    pub environment: f32,
    /// What a RealityKit renderer takes, in a view and off screen alike:
    /// lux per unit of a lamp's worth, and its image-based light's exponent.
    pub lux: f32,
    pub ibl_exponent: f32,
    /// What the lamps and the environment are worth to a renderer in real
    /// photometric units (Filament): lux per unit of a lamp's worth, and per
    /// unit of `environment`. The camera is `filament_calibration`.
    pub filament_lux: f32,
    pub filament_environment_lux: f32,
}

fn core_lighting(l: Lighting) -> style::Lighting {
    match l {
        Lighting::Showcase => style::Lighting::Showcase,
        Lighting::Off => style::Lighting::Off,
    }
}

#[uniffi::export]
pub fn lighting_preset(lighting: Lighting) -> LightingPreset {
    let p = style::preset(core_lighting(lighting));
    LightingPreset {
        environment: p.environment,
        lux: p.lux,
        ibl_exponent: p.ibl_exponent,
        filament_lux: p.filament_lux,
        filament_environment_lux: p.filament_environment_lux,
    }
}

/// The camera Filament exposes the studio with, whatever the lighting.
#[derive(uniffi::Record)]
pub struct FilamentCalibration {
    /// The camera's exposure: aperture (f-stops), shutter speed (seconds)
    /// and sensitivity (ISO).
    pub aperture: f32,
    pub shutter: f32,
    pub sensitivity: f32,
}

#[uniffi::export]
pub fn filament_calibration() -> FilamentCalibration {
    let f = style::FILAMENT;
    FilamentCalibration {
        aperture: f.aperture,
        shutter: f.shutter,
        sensitivity: f.sensitivity,
    }
}

#[derive(uniffi::Enum, Clone, Copy)]
pub enum LightRole {
    Key,
    Fill,
    Back,
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

/// A directional light that shines down -z, turned by `pitch` and `yaw`
/// (radians): orientation = Rx(pitch) · Ry(yaw).
#[derive(uniffi::Record)]
pub struct Light {
    pub role: LightRole,
    pub color: Rgba,
    pub pitch: f32,
    pub yaw: f32,
}

#[uniffi::export]
pub fn studio_lights() -> Vec<Light> {
    style::lights()
        .iter()
        .map(|l| Light {
            role: l.role.into(),
            color: l.color.into(),
            pitch: l.pitch,
            yaw: l.yaw,
        })
        .collect()
}

#[derive(uniffi::Record)]
pub struct Camera {
    /// Vertical field of view, degrees.
    pub fov: f32,
    pub distance: f32,
}

#[uniffi::export]
pub fn studio_camera() -> Camera {
    Camera { fov: style::CAMERA_FOV, distance: style::CAMERA_DISTANCE }
}

/// The camera's vertical field of view, degrees, for a badge `width` by
/// `height` across in a view `aspect` wide over tall.
#[uniffi::export]
pub fn field_of_view(width: f32, height: f32, aspect: f32) -> f32 {
    style::field_of_view(width, height, aspect)
}

/// How one of the badge's materials is finished under a lighting.
#[derive(uniffi::Record)]
pub struct Finish {
    pub metallic: f32,
    pub roughness: f32,
    /// How much a dielectric reflects, as a part of glass's 4% head on:
    /// glTF's `KHR_materials_specular` factor. Metal takes no notice of it.
    pub specular: f32,
    /// Coloured by the metal the badge is plated in, rather than by the
    /// material's own colour.
    pub plated: bool,
    /// What Filament is given in place of `roughness`: its image-based
    /// light blurs a reflection more than RealityKit's at the same value.
    pub filament_roughness: f32,
}

#[uniffi::export]
pub fn finish(role: MaterialRole, lighting: Lighting) -> Finish {
    let preset = style::preset(core_lighting(lighting));
    let f = style::finish(role.into(), &preset);
    Finish {
        metallic: f.metallic,
        roughness: f.roughness,
        specular: f.specular,
        plated: f.plated,
        filament_roughness: f.filament_roughness,
    }
}

/// The roughness Filament gives the reverse in place of `Finish`'s, by how
/// squarely it faces the camera: 1 head on, 0 edge on.
#[uniffi::export]
pub fn filament_back_roughness(lighting: Lighting, facing: f32) -> f32 {
    style::filament_back_roughness(&style::preset(core_lighting(lighting)), facing)
}

/// What a lamp is worth in the preset's units; multiply by the renderer's
/// own lux.
#[uniffi::export]
pub fn lamp(role: LightRole, lighting: Lighting) -> f32 {
    style::lamp(role.into(), &style::preset(core_lighting(lighting)))
}

#[derive(uniffi::Record)]
pub struct Direction {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Which way a lamp shines, right-handed and y up.
#[uniffi::export]
pub fn light_direction(pitch: f32, yaw: f32) -> Direction {
    let [x, y, z] = style::light_direction(pitch, yaw);
    Direction { x, y, z }
}

/// How the badge answers a drag, the same on every platform.
#[derive(uniffi::Record)]
pub struct Handling {
    /// Radians per point dragged.
    pub spin_per_point: f32,
    pub tilt_per_point: f32,
    pub tilt_limit: f32,
    /// Where the badge sits before it is touched, radians.
    pub resting_tilt: f32,
    pub resting_spin: f32,
}

#[uniffi::export]
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
#[derive(uniffi::Record)]
pub struct Coast {
    pub turn: f32,
    pub velocity: f32,
}

/// Carries a flick's spin (radians per second) over `dt` seconds.
#[uniffi::export]
pub fn coast(velocity: f32, dt: f32) -> Coast {
    let (turn, velocity) = style::coast(velocity, dt);
    Coast { turn, velocity }
}

/// RGBA8, sRGB-encoded, rows top to bottom.
#[derive(uniffi::Record)]
pub struct Texture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// The panorama the badge reflects (equirectangular): the showcase's. Takes a
/// few milliseconds: call it off the main thread and keep the result.
#[uniffi::export]
pub fn showcase_environment() -> Texture {
    let t = texture::showcase_environment();
    Texture { width: t.width, height: t.height, rgba: t.rgba }
}

/// The showcase's panorama as a cubemap: six square faces stacked in one
/// image, in the order +X, -X, +Y, -Y, +Z, -Z. For renderers that reflect a
/// cubemap rather than a panorama.
#[uniffi::export]
pub fn showcase_cubemap(size: u32) -> Texture {
    let t = luster_core::texture::showcase_cubemap(size);
    Texture { width: t.width, height: t.height, rgba: t.rgba }
}

/// What a diffuse surface receives from the showcase, in the same layout.
#[uniffi::export]
pub fn showcase_irradiance(size: u32) -> Texture {
    let t = luster_core::texture::showcase_irradiance(size);
    Texture { width: t.width, height: t.height, rgba: t.rgba }
}

/// The badge's reverse: a tangent-space normal map of sandblasted metal.
/// The same for every badge, so a renderer makes it once and keeps it.
#[uniffi::export]
pub fn sandblast() -> Texture {
    let t = texture::sandblast();
    Texture { width: t.width, height: t.height, rgba: t.rgba }
}
