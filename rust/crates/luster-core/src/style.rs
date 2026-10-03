//! The studio: lighting presets and the light rig every renderer builds.
//!
//! Intensities are in the studio's own units, which each renderer scales.

use crate::model::{MaterialRole, Rgba};
use crate::math;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lighting {
    /// A product photographer's studio: strip lights that sweep across the
    /// enamel as a band of sheen when the badge turns, over a dim tent that
    /// keeps plated lines their colour at any angle, and no lamps at all
    /// (`texture::showcase_environment`). Exposed for the enamel's own
    /// colours, so a badge shows the colours its document paints.
    Showcase,
    /// For looking at a badge's shape rather than at the badge: lamps alone,
    /// no panorama, and every surface a rough dielectric, so that nothing
    /// shines and every facet shows by how it faces the light.
    Off,
}

#[derive(Clone, Copy, Debug)]
pub struct LightingPreset {
    /// Environment intensity; 0 means no environment at all.
    pub environment: f32,
    /// What a RealityKit renderer takes instead: lux per unit of the
    /// intensities below, and the exponent its image-based light is given.
    /// The same in a view and off screen: a still is the view's picture.
    pub lux: f32,
    pub ibl_exponent: f32,
    /// What the lamps and the environment are worth to a renderer that works
    /// in real photometric units and tone maps its own way — Filament, on
    /// Android: lux per unit of the lamps' intensities, and per unit of
    /// `environment`. Its curve is not RealityKit's, so both are set per
    /// lighting rather than scaled from one number: each was matched against
    /// a view on its own, the other turned off. The camera is `FILAMENT`.
    pub filament_lux: f32,
    pub filament_environment_lux: f32,
    /// The roughness Filament is given for the metal and for the reverse.
    /// At the same roughness its image-based light blurs a reflection far
    /// more than RealityKit's does — the reverse's grit melts into a flat
    /// sheen, and the rim's reflection into a wash — so where the panorama
    /// does the lighting they were matched by drawing the reverse and the
    /// rim both ways. Where the lamps do, they are the same.
    ///
    /// The reverse has little room either way: a little smoother, and each
    /// grain of its grit catches the panorama's lights on its own, and the
    /// reverse glitters in patches as RealityKit's and flutter_scene's never
    /// do; a little rougher, and the grit is gone. It was set, turned over
    /// on a phone, where the patches and the glints come to what those two
    /// show.
    pub filament_gold_roughness: f32,
    pub filament_back_roughness: f32,
    /// And what they are worth to flutter_scene, whose own units are its
    /// own: its lamps take `scene_lux`, and its environment `environment`
    /// times `scene_environment`. Set the same way, each on its own against
    /// a view. The exposure they are matched at is `FLUTTER_SCENE`.
    pub scene_lux: f32,
    pub scene_environment: f32,
    pub key: f32,
    pub fill: f32,
    pub back: f32,
    pub headlight: f32,
    pub ambient: f32,
    pub gold_metalness: f32,
    pub gold_roughness: f32,
    pub enamel_roughness: f32,
    pub art_roughness: f32,
    /// The reverse's: satin, rough enough that its grit reads as satin
    /// rather than glitter.
    pub back_roughness: f32,
    /// How much of its glare the enamel keeps, as a part of what glass
    /// reflects (4% head on): below 1, as though through a polarizer.
    pub enamel_specular: f32,
}

pub fn preset(lighting: Lighting) -> LightingPreset {
    match lighting {
        // Lit by its panorama alone: a lamp's highlight is a point, and
        // gold under a point reads as plastic. The panorama is stored at a
        // fraction of its radiance (`texture::SHOWCASE_RANGE`), which the
        // exposure gives back: set in a view so that a badge's enamel shows
        // near the colours its document paints, with white enamel white.
        // Filament and flutter_scene were matched to the view by the mean
        // light over the badge, head on and turned.
        Lighting::Showcase => LightingPreset {
            environment: 6.4, lux: 0.0, ibl_exponent: 3.56,
            filament_lux: 0.0, filament_environment_lux: 893.0, scene_lux: 0.0, scene_environment: 1.105,
            key: 0.0, fill: 0.0, back: 0.0, headlight: 0.0, ambient: 0.0,
            // Polished gold and glassy enamel. The reverse is sandblasted:
            // any smoother and, flat and all metal, it takes a light whole
            // and burns out.
            gold_metalness: 1.0, gold_roughness: 0.10, enamel_roughness: 0.08, art_roughness: 0.08,
            back_roughness: 0.55,
            filament_gold_roughness: 0.10, filament_back_roughness: 0.18,
            // Shot through a polarizer, as enamel pins are: half the
            // enamel's glare goes, and the metal, which a polarizer leaves
            // alone, can be lit bright without the enamel hazing over.
            enamel_specular: 0.5,
        },
        // Nothing to reflect: the light all comes from the lamps, which must
        // carry what the ambient light did, and metal must be a rough
        // dielectric or it renders black.
        Lighting::Off => LightingPreset {
            environment: 0.0, lux: 6.2, ibl_exponent: -8.0,
            filament_lux: 4.39, filament_environment_lux: 0.0, scene_lux: 5.75, scene_environment: 1.0,
            key: 110.0, fill: 60.0, back: 60.0, headlight: 90.0, ambient: 900.0,
            gold_metalness: 0.1, gold_roughness: 0.95, enamel_roughness: 0.95, art_roughness: 0.95,
            back_roughness: 1.0,
            filament_gold_roughness: 0.95, filament_back_roughness: 1.0, enamel_specular: 1.0,
        },
    }
}

/// What the studio is worth to Filament, on Android, whatever the lighting:
/// the camera the lamps and the environment of `filament_lux` and
/// `filament_environment_lux` are exposed for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FilamentCalibration {
    /// The camera's exposure: aperture (f-stops), shutter speed (seconds)
    /// and sensitivity (ISO).
    pub aperture: f32,
    pub shutter: f32,
    pub sensitivity: f32,
}

pub const FILAMENT: FilamentCalibration = FilamentCalibration {
    aperture: 16.0,
    shutter: 1.0 / 125.0,
    sensitivity: 6000.0,
};

/// The same for flutter_scene, set the same way: the rest of what
/// `scene_lux` is to the lamps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneCalibration {
    /// The scene's exposure: what brings the scene into `REALITYKIT_TONE`'s
    /// range, which flutter_scene draws it through.
    pub exposure: f32,
    /// What a lamp's intensity, times `scene_lux`, is worth there.
    pub lamp_unit: f32,
}

pub const FLUTTER_SCENE: SceneCalibration = SceneCalibration { exposure: 0.43, lamp_unit: 0.0009 };

/// RealityKit's tone mapping, measured (`LusterShot --tone-table`), for
/// flutter_scene to show a scene as RealityKit would. For each point of a
/// `REALITYKIT_TONE_SIZE`³ grid over sRGB-encoded [0, 1], red fastest, then
/// green, then blue: what RealityKit shows for that linear colour times
/// `REALITYKIT_TONE_RANGE`, sRGB-encoded, a byte a channel.
///
/// flutter_scene's own curves keep a colour's saturation as it brightens,
/// where RealityKit's lets it fade towards white: the gold that is pale on
/// the Apple side came out orange. Drawn with no curve of its own, clamped,
/// and read through this table, it shows what RealityKit would.
pub static REALITYKIT_TONE: &[u8] = include_bytes!("realitykit_tone.bin");
pub const REALITYKIT_TONE_SIZE: usize = 17;
/// The linear value the top of the table stands for: RealityKit has gone
/// to white well before it.
pub const REALITYKIT_TONE_RANGE: f32 = 4.0;

/// `REALITYKIT_TONE` as the text of a `.cube` file, which is how
/// flutter_scene takes a table.
pub fn realitykit_tone_cube() -> String {
    let mut text = format!("LUT_3D_SIZE {REALITYKIT_TONE_SIZE}\n");
    for rgb in REALITYKIT_TONE.chunks_exact(3) {
        let [r, g, b] = [rgb[0], rgb[1], rgb[2]].map(|v| f32::from(v) / 255.0);
        text.push_str(&format!("{r:.4} {g:.4} {b:.4}\n"));
    }
    text
}

/// How one of the badge's materials is finished under a lighting: what every
/// renderer sets on its physically based material.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Finish {
    pub metallic: f32,
    pub roughness: f32,
    /// How much a dielectric reflects, as a part of glass's 4% head on:
    /// glTF's `KHR_materials_specular` factor. Metal takes no notice of it.
    pub specular: f32,
    /// Coloured by the metal the badge is plated in, rather than by the
    /// material's own colour.
    pub plated: bool,
    /// What Filament is given in place of `roughness`: see
    /// `LightingPreset::filament_gold_roughness`.
    pub filament_roughness: f32,
}

/// The roughness Filament gives the reverse as it turns: `facing` is how
/// squarely the reverse faces the camera, the cosine of the angle between
/// them, 1 head on and 0 edge on.
///
/// Filament takes one lobe for a reflection whatever the angle it is seen
/// at, where RealityKit's spreads as a face turns edge on. Head on, Filament
/// is given a roughness of its own (`filament_back_roughness`), as its
/// reflection blurs more than RealityKit's there; seen nearly edge on, that
/// lobe is too narrow, and the reverse reflects the panorama's dark gaps
/// between its lights and goes black where RealityKit's catches the lights
/// round them. So the roughness goes over to RealityKit's own as the
/// reverse turns away.
///
/// A lighting whose two roughnesses agree is left as it is. One lit by lamps
/// should keep them so: a lamp's highlight in Filament is already broader
/// than in RealityKit, and a rougher reverse would spread it further.
pub fn filament_back_roughness(preset: &LightingPreset, facing: f32) -> f32 {
    let p = preset;
    let away = 1.0 - facing.clamp(0.0, 1.0);
    p.filament_back_roughness + (p.back_roughness - p.filament_back_roughness) * away
}

pub fn finish(role: MaterialRole, preset: &LightingPreset) -> Finish {
    let p = preset;
    let metal = |roughness: f32, filament_roughness: f32, plated: bool| Finish {
        metallic: p.gold_metalness,
        roughness,
        specular: 1.0,
        plated,
        filament_roughness,
    };
    let enamel = |roughness: f32| Finish {
        metallic: 0.0,
        roughness,
        specular: p.enamel_specular,
        plated: false,
        filament_roughness: roughness,
    };
    match role {
        MaterialRole::Plated => metal(p.gold_roughness, p.filament_gold_roughness, true),
        MaterialRole::GoldBack => metal(p.back_roughness, p.filament_back_roughness, true),
        MaterialRole::Field => enamel(p.enamel_roughness),
        MaterialRole::Art => enamel(p.art_roughness),
        // Metal in the document's colours: plated as the gold is, its
        // reflection tinted by the colour rather than laid over it.
        MaterialRole::Glaze => metal(p.gold_roughness, p.filament_gold_roughness, false),
    }
}

/// What a lamp is worth, in the preset's own units: each renderer multiplies
/// it by what those are worth to it (`lux`, `filament_lux`, `scene_lux` and
/// `FLUTTER_SCENE.lamp_unit`).
/// None of the renderers has an ambient light, so the headlight carries half
/// of it.
pub fn lamp(role: LightRole, preset: &LightingPreset) -> f32 {
    match role {
        LightRole::Key => preset.key,
        LightRole::Fill => preset.fill,
        LightRole::Back => preset.back,
        LightRole::Headlight => preset.headlight + preset.ambient * 0.5,
    }
}

/// Which way a lamp shines, in the engine's right-handed, y-up space: down -z,
/// turned by `pitch` and then `yaw` as `Light` describes.
pub fn light_direction(pitch: f32, yaw: f32) -> [f32; 3] {
    let (sp, cp) = (math::sin(pitch), math::cos(pitch));
    let (sy, cy) = (math::sin(yaw), math::cos(yaw));
    [-sy * cp, sp, -cy * cp]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightRole {
    Key,
    Fill,
    Back,
    /// Rides on the camera, pointing where it looks.
    Headlight,
}

/// A directional light that shines down -z, turned by `pitch` and `yaw`
/// (radians): orientation = Rx(pitch) · Ry(yaw).
#[derive(Clone, Copy, Debug)]
pub struct Light {
    pub role: LightRole,
    pub color: Rgba,
    pub pitch: f32,
    pub yaw: f32,
}

pub fn lights() -> [Light; 4] {
    [
        // Warm key from above.
        Light { role: LightRole::Key, color: Rgba::rgb(1.0, 0.96, 0.9), pitch: -0.5, yaw: 0.4 },
        // Cool fill from the far side.
        Light { role: LightRole::Fill, color: Rgba::rgb(0.88, 0.92, 1.0), pitch: -0.15, yaw: -0.9 },
        // Warm light on the reverse.
        Light { role: LightRole::Back, color: Rgba::rgb(1.0, 0.93, 0.85), pitch: -0.3, yaw: std::f32::consts::PI - 0.4 },
        Light { role: LightRole::Headlight, color: Rgba::rgb(1.0, 0.97, 0.92), pitch: 0.0, yaw: 0.0 },
    ]
}

/// Camera: vertical field of view (degrees) and distance from the badge.
pub const CAMERA_FOV: f32 = 26.0;
pub const CAMERA_DISTANCE: f32 = 2.75;

/// How much room the badge is given beyond its own size.
pub const FRAME_MARGIN: f32 = 1.06;

/// The camera's vertical field of view, in degrees, for a badge `width` by
/// `height` across in a view `aspect` wide over tall. The studio's field of
/// view is the vertical one; a view too narrow to hold the badge at that
/// framing is widened until the badge fits, so a portrait phone shows it all.
pub fn field_of_view(width: f32, height: f32, aspect: f32) -> f32 {
    let plain = 2.0 * CAMERA_DISTANCE * math::tan(CAMERA_FOV.to_radians() / 2.0);
    let tall = plain
        .max(height * FRAME_MARGIN)
        .max(width * FRAME_MARGIN / aspect.max(0.01));
    (2.0 * math::atan(tall / (2.0 * CAMERA_DISTANCE))).to_degrees()
}

/// How the badge answers a drag, the same on every platform: radians per
/// point dragged, how far it may tip, and how a flick's spin fades.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Handling {
    pub spin_per_point: f32,
    pub tilt_per_point: f32,
    pub tilt_limit: f32,
    /// What is left of the spin after one 60 Hz frame.
    pub damping: f32,
    /// Below this spin speed (radians per second) the badge comes to rest.
    pub resting_speed: f32,
}

pub const HANDLING: Handling = Handling {
    spin_per_point: 0.010,
    tilt_per_point: 0.006,
    tilt_limit: 0.55,
    damping: 0.94,
    resting_speed: 0.012,
};

/// Where the badge sits before it is touched, tilt (about x) and spin (about
/// y) in radians: slightly off-axis, so the edge shows depth.
pub const RESTING_POSE: [f32; 2] = [0.12, -0.35];

/// Carries a flick's spin over `dt` seconds: how far the badge turns, and the
/// speed it is left with. The turn is how far the fading spin carries it over
/// the whole time, not the speed it started with times the time, so 120 Hz
/// lands where 60 Hz does.
pub fn coast(velocity: f32, dt: f32) -> (f32, f32) {
    if velocity == 0.0 {
        return (0.0, 0.0);
    }
    let dt = dt.clamp(0.0, 0.1);
    let fade = -math::log2(HANDLING.damping) * std::f32::consts::LN_2 * 60.0;
    let left = math::powf(HANDLING.damping, dt * 60.0);
    let turn = velocity * (1.0 - left) / fade;
    let velocity = velocity * left;
    (turn, if velocity.abs() < HANDLING.resting_speed { 0.0 } else { velocity })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_square_badge_fits_the_plain_framing_and_a_narrow_view_widens_it() {
        let plain = field_of_view(1.0, 1.0, 1.0);
        assert!((plain - CAMERA_FOV).abs() < 1e-4, "{plain}");
        assert!(field_of_view(1.0, 1.0, 0.5) > plain);
        // A badge bigger than the framing is fitted whatever the view.
        assert!(field_of_view(1.0, 1.4, 1.0) > plain);
    }

    #[test]
    fn the_tone_table_is_a_whole_cube_from_black_to_white() {
        let n = REALITYKIT_TONE_SIZE;
        assert_eq!(REALITYKIT_TONE.len(), n * n * n * 3);
        let at = |r: usize, g: usize, b: usize| {
            let i = ((b * n + g) * n + r) * 3;
            [REALITYKIT_TONE[i], REALITYKIT_TONE[i + 1], REALITYKIT_TONE[i + 2]]
        };
        assert_eq!(at(0, 0, 0), [0, 0, 0]);
        assert_eq!(at(n - 1, n - 1, n - 1), [255, 255, 255]);
        // Greys stay grey, and brighten until they are white, well before
        // the top of the range.
        let greys: Vec<[u8; 3]> = (0..n).map(|i| at(i, i, i)).collect();
        assert!(greys.iter().all(|[r, g, b]| r == g && g == b));
        let white = greys.iter().position(|g| g[0] == 255).unwrap();
        assert!(white < n - 1);
        assert!(greys[..=white].windows(2).all(|w| w[1][0] > w[0][0]));
        // The table's first line is its size, then one line a point.
        let cube = realitykit_tone_cube();
        assert_eq!(cube.lines().count(), n * n * n + 1);
        assert_eq!(cube.lines().nth(1), Some("0.0000 0.0000 0.0000"));
    }

    #[test]
    fn the_reverse_roughens_in_filament_as_it_turns_edge_on() {
        let showcase = preset(Lighting::Showcase);
        let head_on = filament_back_roughness(&showcase, 1.0);
        assert_eq!(head_on, finish(MaterialRole::GoldBack, &showcase).filament_roughness);
        assert!((filament_back_roughness(&showcase, 0.0) - showcase.back_roughness).abs() < 1e-6);
        assert!(filament_back_roughness(&showcase, 0.5) > head_on);
        // The lamps' lighting gives both renderers one roughness, so it
        // stays put.
        let off = preset(Lighting::Off);
        assert_eq!(filament_back_roughness(&off, 0.0), off.filament_back_roughness);
    }

    #[test]
    fn a_flick_fades_at_the_same_rate_at_any_refresh_rate() {
        let run = |frames: usize, dt: f32| {
            let (mut turned, mut v) = (0.0, 9.0);
            for _ in 0..frames {
                let (t, next) = coast(v, dt);
                turned += t;
                v = next;
            }
            turned
        };
        assert!((run(60, 1.0 / 60.0) - run(120, 1.0 / 120.0)).abs() < 1e-3);
    }

    #[test]
    fn a_flick_comes_to_rest() {
        let mut v = 9.0;
        for _ in 0..600 {
            v = coast(v, 1.0 / 60.0).1;
        }
        assert_eq!(v, 0.0);
    }

    #[test]
    fn the_headlight_carries_half_the_ambient() {
        let p = preset(Lighting::Off);
        assert_eq!(lamp(LightRole::Headlight, &p), p.headlight + p.ambient * 0.5);
        let back = finish(MaterialRole::GoldBack, &p);
        assert!(back.plated && back.roughness >= p.gold_roughness);
    }

    #[test]
    fn the_showcase_is_lit_by_its_panorama_alone() {
        let showcase = preset(Lighting::Showcase);
        for role in [LightRole::Key, LightRole::Fill, LightRole::Back, LightRole::Headlight] {
            assert_eq!(lamp(role, &showcase), 0.0);
        }
        assert!(showcase.environment > 0.0);
        // Polished metal, and a satin reverse rougher than it.
        let face = finish(MaterialRole::Plated, &showcase);
        let back = finish(MaterialRole::GoldBack, &showcase);
        assert!(face.roughness < 0.2 && back.roughness > face.roughness + 0.3);
        // Through a polarizer: the enamel's glare is cut, the metal's kept.
        assert!(finish(MaterialRole::Field, &showcase).specular < 1.0);
        assert_eq!(face.specular, 1.0);
    }

    #[test]
    fn the_lamps_lighting_has_no_panorama_and_nothing_that_shines() {
        let off = preset(Lighting::Off);
        assert_eq!(off.environment, 0.0);
        assert!(off.key > 0.0 && off.headlight > 0.0);
        for role in [MaterialRole::Plated, MaterialRole::GoldBack, MaterialRole::Field, MaterialRole::Art] {
            assert!(finish(role, &off).roughness > 0.9, "{role:?}");
        }
        assert_eq!(finish(MaterialRole::Field, &off).specular, 1.0);
    }
}
