//! JavaScript API, described for TypeScript in `ABOUT` below.

use std::sync::Once;

use luster_core::{CancelToken, badge, style, texture, vertex as mesh};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(typescript_custom_section)]
const ABOUT: &str = r#"
/**
 * A mint runs on the calling thread to the end, so a page calls it from a
 * worker, off the thread that draws; ending the worker is how one is given
 * up. Everything is in the engine's own space, glTF's and three.js's:
 * right-handed, y up, the badge's face turned towards +z.
 *
 * A failure in the engine itself traps the module: the call throws a
 * `WebAssembly.RuntimeError`, the engine's message goes to the console, and
 * the module is not to be called again. Treat it as an `engineFailure` and
 * start a new worker.
 */
"#;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = error)]
    fn console_error(message: &str);
}

/// A trap leaves the worker only its own "unreachable": the engine's message
/// goes to the console first. Set by whatever does the engine's work.
fn report_panics() {
    static SET: Once = Once::new();
    SET.call_once(|| {
        std::panic::set_hook(Box::new(|info| console_error(&format!("luster: {info}"))));
    });
}

/// The engine's version.
#[wasm_bindgen]
pub fn version() -> String {
    luster_core::version().to_owned()
}

/// sRGB, straight alpha.
#[wasm_bindgen]
#[derive(Clone, Copy)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

#[wasm_bindgen]
impl Rgba {
    #[wasm_bindgen(constructor)]
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Rgba {
        Rgba { r, g, b, a }
    }
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

/// The default metal: pale gold.
#[wasm_bindgen(js_name = defaultGold)]
pub fn default_gold() -> Rgba {
    badge::GOLD.into()
}

#[wasm_bindgen]
pub fn silver() -> Rgba {
    badge::SILVER.into()
}

#[wasm_bindgen]
pub fn copper() -> Rgba {
    badge::COPPER.into()
}

#[wasm_bindgen]
#[derive(Clone, Copy)]
pub enum MaterialRole {
    Plated = "plated",
    GoldBack = "goldBack",
    Field = "field",
    Art = "art",
    /// Metal plated in the document's colours: a metal, in the colour it
    /// carries rather than the metal's.
    Glaze = "glaze",
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

impl MaterialRole {
    fn core(self) -> Option<badge::MaterialRole> {
        Some(match self {
            Self::Plated => badge::MaterialRole::Plated,
            Self::GoldBack => badge::MaterialRole::GoldBack,
            Self::Field => badge::MaterialRole::Field,
            Self::Art => badge::MaterialRole::Art,
            Self::Glaze => badge::MaterialRole::Glaze,
            // A string from JavaScript that names no role.
            Self::__Invalid => return None,
        })
    }
}

#[wasm_bindgen(getter_with_clone)]
pub struct Material {
    #[wasm_bindgen(readonly)]
    pub name: String,
    #[wasm_bindgen(readonly)]
    pub role: MaterialRole,
    #[wasm_bindgen(readonly)]
    pub color: Rgba,
    #[wasm_bindgen(readonly)]
    pub metallic: f32,
    #[wasm_bindgen(readonly)]
    pub roughness: f32,
    /// A texture for the front face, where the document's colour changes
    /// across it; indexes the badge's textures.
    #[wasm_bindgen(readonly)]
    pub texture: Option<u32>,
}

/// One submesh: its vertices interleaved as `vertexLayout` says, and three
/// indices a triangle, wound counter-clockwise seen from outside. Each field
/// read is a fresh copy; read it once.
#[wasm_bindgen(getter_with_clone)]
pub struct Submesh {
    #[wasm_bindgen(readonly)]
    pub name: String,
    /// Indexes the badge's materials.
    #[wasm_bindgen(readonly)]
    pub material: u32,
    #[wasm_bindgen(readonly)]
    pub vertices: Vec<f32>,
    #[wasm_bindgen(readonly)]
    pub indices: Vec<u32>,
    #[wasm_bindgen(readonly, js_name = vertexCount)]
    pub vertex_count: u32,
    /// The lowest corner of the submesh's bounds, x, y and z.
    #[wasm_bindgen(readonly)]
    pub min: Vec<f32>,
    /// The highest corner.
    #[wasm_bindgen(readonly)]
    pub max: Vec<f32>,
}

/// Where each attribute sits in a vertex, in floats, as three.js's
/// `InterleavedBuffer` counts: position (3), normal (3), texture coordinates
/// (2, v running down the image, as glTF's) and tangent (4, w the
/// handedness).
#[wasm_bindgen]
pub struct VertexLayout {
    #[wasm_bindgen(readonly)]
    pub stride: u32,
    #[wasm_bindgen(readonly)]
    pub position: u32,
    #[wasm_bindgen(readonly)]
    pub normal: u32,
    #[wasm_bindgen(readonly)]
    pub uv: u32,
    #[wasm_bindgen(readonly)]
    pub tangent: u32,
}

#[wasm_bindgen(js_name = vertexLayout)]
pub fn vertex_layout() -> VertexLayout {
    let floats = |bytes: usize| (bytes / size_of::<f32>()) as u32;
    VertexLayout {
        stride: floats(mesh::VERTEX_STRIDE),
        position: floats(mesh::POSITION_OFFSET),
        normal: floats(mesh::NORMAL_OFFSET),
        uv: floats(mesh::UV_OFFSET),
        tangent: floats(mesh::TANGENT_OFFSET),
    }
}

/// RGBA8, rows top to bottom. Colour textures are sRGB-encoded; a normal map
/// holds its vectors as they are.
#[wasm_bindgen(getter_with_clone)]
pub struct Texture {
    #[wasm_bindgen(readonly)]
    pub width: u32,
    #[wasm_bindgen(readonly)]
    pub height: u32,
    #[wasm_bindgen(readonly)]
    pub rgba: Vec<u8>,
}

impl From<texture::Texture> for Texture {
    fn from(t: texture::Texture) -> Self {
        Self { width: t.width, height: t.height, rgba: t.rgba }
    }
}

/// A badge's texture with its mip chain, made by the engine so that every
/// renderer filters the same levels: the texture's own pixels first, then
/// each level below, each the RGBA8 sRGB of half the one before, to one
/// texel.
#[wasm_bindgen]
pub struct Sheet {
    #[wasm_bindgen(readonly)]
    pub width: u32,
    #[wasm_bindgen(readonly)]
    pub height: u32,
    levels: Vec<Vec<u8>>,
}

#[wasm_bindgen]
impl Sheet {
    #[wasm_bindgen(getter, js_name = levelCount)]
    pub fn level_count(&self) -> u32 {
        self.levels.len() as u32
    }

    /// Level `index`, `max(1, width >> index)` by `max(1, height >> index)`.
    pub fn level(&self, index: u32) -> Option<Vec<u8>> {
        self.levels.get(index as usize).cloned()
    }
}

/// A minted badge, held in the module's memory until `free` is called or it
/// is collected.
#[wasm_bindgen]
pub struct LusterBadge(badge::Badge);

#[wasm_bindgen]
impl LusterBadge {
    /// Stable across runs and platforms for the same document and options;
    /// equal keys mean the same badge.
    #[wasm_bindgen(getter, js_name = designKey)]
    pub fn design_key(&self) -> String {
        self.0.design_key.clone()
    }

    pub fn submeshes(&self) -> Vec<Submesh> {
        report_panics();
        self.0
            .submeshes
            .iter()
            .map(|s| Submesh {
                name: s.name.clone(),
                material: s.material,
                vertices: s.vertices.as_chunks().0.iter().map(|v| f32::from_le_bytes(*v)).collect(),
                indices: s.indices.as_chunks().0.iter().map(|i| u32::from_le_bytes(*i)).collect(),
                vertex_count: s.vertex_count,
                min: s.min.to_vec(),
                max: s.max.to_vec(),
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

    /// In the order `Material.texture` indexes them. Each call builds the mip
    /// chains again; keep the result.
    pub fn textures(&self) -> Vec<Sheet> {
        report_panics();
        self.0
            .textures
            .iter()
            .map(|t| {
                let below = texture::mipmaps(t).into_iter().map(|level| level.rgba);
                Sheet {
                    width: t.width,
                    height: t.height,
                    levels: std::iter::once(t.rgba.clone()).chain(below).collect(),
                }
            })
            .collect()
    }

    /// The badge as a glTF binary, `scale` units (metres, in glTF) across and
    /// plated in `metal`.
    pub fn glb(&self, scale: f32, metal: &Rgba) -> Vec<u8> {
        report_panics();
        luster_core::export::glb(&self.0, scale, (*metal).into())
    }
}

#[wasm_bindgen(typescript_custom_section)]
const MINT_OPTIONS: &str = r#"
/** What to strike, beyond the artwork itself. Both are off unless given. */
export interface MintOptions {
  /** Leave out the triangles no view can see: a smaller badge to send on,
   *  at the cost of the work to find them. */
  withoutHiddenFaces?: boolean;
  /** Leave the lines and the edge's top in the metal, rather than plate them
   *  in the colours the document paints on them. */
  metalLines?: boolean;
}

/** Why a badge could not be minted: what `mint` throws. A mint is given up
 *  by ending its worker, so none is ever cancelled. */
export interface LusterError extends Error {
  name: "LusterError";
  kind: "invalidSvg" | "inputTooComplex" | "nothingToMint" | "engineFailure";
}
"#;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "MintOptions")]
    pub type MintOptionsArg;
}

/// Only `true` sets one: a string such as "false" is not taken for it.
fn flag(options: &JsValue, name: &str) -> bool {
    js_sys::Reflect::get(options, &JsValue::from_str(name)).is_ok_and(|v| v.as_bool() == Some(true))
}

/// Mints a badge from SVG data: the artwork's silhouette in metal, its fills
/// set into it as enamel, the metal its lines stand in plated in their
/// colours. Throws a `LusterError`.
#[wasm_bindgen]
pub fn mint(svg: &[u8], options: Option<MintOptionsArg>) -> Result<LusterBadge, JsValue> {
    report_panics();
    let options = options.map(JsValue::from).unwrap_or(JsValue::UNDEFINED);
    let options = if options.is_object() {
        badge::MintOptions {
            without_hidden_faces: flag(&options, "withoutHiddenFaces"),
            metal_lines: flag(&options, "metalLines"),
        }
    } else {
        badge::MintOptions::default()
    };
    badge::mint_with(svg, &options, &CancelToken::new()).map(LusterBadge).map_err(error)
}

fn error(e: luster_core::Error) -> JsValue {
    use luster_core::Error as E;
    let kind = match &e {
        E::InvalidSvg(_) => "invalidSvg",
        E::InputTooComplex(_) => "inputTooComplex",
        E::NothingToMint => "nothingToMint",
        // No token here is ever cancelled.
        E::Cancelled => "engineFailure",
        E::Internal(_) => "engineFailure",
    };
    let thrown = js_sys::Error::new(&e.to_string());
    thrown.set_name("LusterError");
    let _ = js_sys::Reflect::set(&thrown, &"kind".into(), &kind.into());
    thrown.into()
}

/// The panorama the badge reflects (equirectangular): the showcase's. Takes a
/// few milliseconds; keep the result.
#[wasm_bindgen(js_name = showcaseEnvironment)]
pub fn showcase_environment() -> Texture {
    report_panics();
    texture::showcase_environment().into()
}

/// The largest cubemap face: six of them, four bytes a texel, are 100 MB,
/// and a size much past it overflows the module's 32-bit lengths.
const MAX_FACE: u32 = 2048;

fn face(size: u32) -> Result<u32, JsError> {
    if (1..=MAX_FACE).contains(&size) {
        Ok(size)
    } else {
        Err(JsError::new(&format!("a cubemap face is 1 to {MAX_FACE} texels across, not {size}")))
    }
}

/// The showcase's panorama as a cubemap: six `size` square faces stacked in
/// one image, in the order +X, -X, +Y, -Y, +Z, -Z.
#[wasm_bindgen(js_name = showcaseCubemap)]
pub fn showcase_cubemap(size: u32) -> Result<Texture, JsError> {
    report_panics();
    Ok(texture::showcase_cubemap(face(size)?).into())
}

/// What a diffuse surface receives from the showcase, in the same layout.
#[wasm_bindgen(js_name = showcaseIrradiance)]
pub fn showcase_irradiance(size: u32) -> Result<Texture, JsError> {
    report_panics();
    Ok(texture::showcase_irradiance(face(size)?).into())
}

/// The badge's reverse: a tangent-space normal map of sandblasted metal, the
/// same for every badge.
#[wasm_bindgen]
pub fn sandblast() -> Texture {
    report_panics();
    texture::sandblast().into()
}

#[wasm_bindgen]
#[derive(Clone, Copy)]
pub enum Lighting {
    /// The studio the panorama is painted for.
    Showcase = "showcase",
    /// For looking at a badge's shape rather than at the badge: lamps alone,
    /// no panorama, and every surface a rough dielectric.
    Off = "off",
}

fn preset(lighting: Lighting) -> Result<style::LightingPreset, JsError> {
    Ok(style::preset(match lighting {
        Lighting::Showcase => style::Lighting::Showcase,
        Lighting::Off => style::Lighting::Off,
        Lighting::__Invalid => return Err(JsError::new("no such lighting")),
    }))
}

/// What a scene takes of a lighting, in flutter_scene's calibration, which a
/// renderer here is measured against: how much of the environment, and what
/// the lamps are worth. What each lamp and material is given comes from
/// `lamp` and `finish`.
#[wasm_bindgen]
pub struct LightingPreset {
    #[wasm_bindgen(readonly)]
    pub environment: f32,
    #[wasm_bindgen(readonly, js_name = sceneLux)]
    pub scene_lux: f32,
}

#[wasm_bindgen(js_name = lightingPreset)]
pub fn lighting_preset(lighting: Lighting) -> Result<LightingPreset, JsError> {
    let p = preset(lighting)?;
    Ok(LightingPreset { environment: p.environment * p.scene_environment, scene_lux: p.scene_lux })
}

/// What flutter_scene's studio is worth whatever the lighting.
#[wasm_bindgen]
pub struct SceneCalibration {
    /// What brings the scene into the tone table's range.
    #[wasm_bindgen(readonly)]
    pub exposure: f32,
    /// What a lamp's intensity, times `sceneLux`, is worth.
    #[wasm_bindgen(readonly, js_name = lampUnit)]
    pub lamp_unit: f32,
}

#[wasm_bindgen(js_name = sceneCalibration)]
pub fn scene_calibration() -> SceneCalibration {
    let s = style::FLUTTER_SCENE;
    SceneCalibration { exposure: s.exposure, lamp_unit: s.lamp_unit }
}

/// RealityKit's tone mapping, measured: for each point of a `size`³ grid
/// over sRGB-encoded [0, 1], red fastest, then green, then blue, what
/// RealityKit shows for that linear colour times `range`, sRGB-encoded, a
/// byte a channel. A scene exposed by `SceneCalibration.exposure`, drawn
/// with no tone curve of its own, clamped and read through it, shows what
/// RealityKit would.
#[wasm_bindgen(getter_with_clone)]
pub struct ToneTable {
    #[wasm_bindgen(readonly)]
    pub size: u32,
    #[wasm_bindgen(readonly)]
    pub range: f32,
    #[wasm_bindgen(readonly)]
    pub rgb: Vec<u8>,
}

#[wasm_bindgen(js_name = realitykitTone)]
pub fn realitykit_tone() -> ToneTable {
    ToneTable {
        size: style::REALITYKIT_TONE_SIZE as u32,
        range: style::REALITYKIT_TONE_RANGE,
        rgb: style::REALITYKIT_TONE.to_vec(),
    }
}

/// How one of the badge's materials is finished under a lighting.
#[wasm_bindgen]
pub struct Finish {
    #[wasm_bindgen(readonly)]
    pub metallic: f32,
    #[wasm_bindgen(readonly)]
    pub roughness: f32,
    /// How much a dielectric reflects, as a part of glass's 4% head on:
    /// glTF's `KHR_materials_specular` factor. Metal takes no notice of it.
    #[wasm_bindgen(readonly)]
    pub specular: f32,
    /// Coloured by the metal the badge is plated in, rather than by the
    /// material's own colour.
    #[wasm_bindgen(readonly)]
    pub plated: bool,
}

#[wasm_bindgen]
pub fn finish(role: MaterialRole, lighting: Lighting) -> Result<Finish, JsError> {
    let role = role.core().ok_or_else(|| JsError::new("no such material role"))?;
    let f = style::finish(role, &preset(lighting)?);
    Ok(Finish {
        metallic: f.metallic,
        roughness: f.roughness,
        specular: f.specular,
        plated: f.plated,
    })
}

#[wasm_bindgen]
#[derive(Clone, Copy)]
pub enum LightRole {
    Key = "key",
    Fill = "fill",
    Back = "back",
    /// Rides on the camera, pointing where it looks.
    Headlight = "headlight",
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

/// What a lamp is worth in the preset's units; multiply by `sceneLux`.
#[wasm_bindgen]
pub fn lamp(role: LightRole, lighting: Lighting) -> Result<f32, JsError> {
    let role = match role {
        LightRole::Key => style::LightRole::Key,
        LightRole::Fill => style::LightRole::Fill,
        LightRole::Back => style::LightRole::Back,
        LightRole::Headlight => style::LightRole::Headlight,
        LightRole::__Invalid => return Err(JsError::new("no such light role")),
    };
    Ok(style::lamp(role, &preset(lighting)?))
}

/// A lamp in the studio: which way it shines, and what colour it is.
#[wasm_bindgen(getter_with_clone)]
pub struct Light {
    #[wasm_bindgen(readonly)]
    pub role: LightRole,
    #[wasm_bindgen(readonly)]
    pub color: Rgba,
    /// x, y and z, a unit vector.
    #[wasm_bindgen(readonly)]
    pub direction: Vec<f32>,
}

#[wasm_bindgen(js_name = studioLights)]
pub fn studio_lights() -> Vec<Light> {
    style::lights()
        .into_iter()
        .map(|l| Light {
            role: l.role.into(),
            color: l.color.into(),
            direction: style::light_direction(l.pitch, l.yaw).to_vec(),
        })
        .collect()
}

/// Where the camera stands: its vertical field of view in degrees, and how
/// far it is from the badge.
#[wasm_bindgen]
pub struct Camera {
    #[wasm_bindgen(readonly)]
    pub fov: f32,
    #[wasm_bindgen(readonly)]
    pub distance: f32,
}

#[wasm_bindgen(js_name = studioCamera)]
pub fn studio_camera() -> Camera {
    Camera { fov: style::CAMERA_FOV, distance: style::CAMERA_DISTANCE }
}

/// The camera's vertical field of view, degrees, for a badge `width` by
/// `height` across in a view `aspect` wide over tall.
#[wasm_bindgen(js_name = fieldOfView)]
pub fn field_of_view(width: f32, height: f32, aspect: f32) -> f32 {
    style::field_of_view(width, height, aspect)
}

/// How the badge answers a drag, the same on every platform.
#[wasm_bindgen]
pub struct Handling {
    /// Radians per CSS pixel dragged.
    #[wasm_bindgen(readonly, js_name = spinPerPoint)]
    pub spin_per_point: f32,
    #[wasm_bindgen(readonly, js_name = tiltPerPoint)]
    pub tilt_per_point: f32,
    #[wasm_bindgen(readonly, js_name = tiltLimit)]
    pub tilt_limit: f32,
    /// Where the badge sits before it is touched, radians.
    #[wasm_bindgen(readonly, js_name = restingTilt)]
    pub resting_tilt: f32,
    #[wasm_bindgen(readonly, js_name = restingSpin)]
    pub resting_spin: f32,
}

#[wasm_bindgen]
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
#[wasm_bindgen]
pub struct Coast {
    #[wasm_bindgen(readonly)]
    pub turn: f32,
    #[wasm_bindgen(readonly)]
    pub velocity: f32,
}

/// Carries a flick's spin (radians per second) over `dt` seconds.
#[wasm_bindgen]
pub fn coast(velocity: f32, dt: f32) -> Coast {
    let (turn, velocity) = style::coast(velocity, dt);
    Coast { turn, velocity }
}
