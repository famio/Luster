//! The badge's proportions, in badge units: a badge is 1 across.

/// A badge's thickness.
pub const BADGE_DEPTH: f32 = 0.025;
/// Depth of a slab laid on the face: thin enough that its back stays in the
/// plate.
pub const FACE_DEPTH: f32 = 0.03;
/// Height of the enamel above the badge's face.
pub const BADGE_ENAMEL: f32 = 0.003;
/// How far a badge's enamel sits below its metal: deep enough that metal
/// plated in the enamel's own colours still stands clear of it.
pub const BADGE_RELIEF: f32 = 0.009;
/// Height step between cells, so that no two faces are coplanar.
pub const CELL_STEP: f32 = 0.00015;
/// Cells climb this many steps before starting again, so a many-coloured badge
/// stays near the face height its metal is cut to.
pub const CELL_LADDER: usize = 8;
/// The badge's metal margin, before placement.
pub const BADGE_RIM: f64 = 1.0 / 72.0;
/// Width of the raised wire on art that has no colours of its own.
pub const WIRE_WIDTH: f64 = 0.012;
/// Area under which a contour is dropped: booleans and traces leave scraps.
pub const SPECKS: f64 = 5e-7;
/// How far apart two pieces' margins may be and still be bridged, in margins.
pub const BADGE_BRIDGE: f32 = 1.5;
/// How far the wall's back stops short of the plate's, so the two
/// backs are not coplanar.
pub const WALL_LIFT: f32 = 0.0004;
/// Raster the face metal is traced at, and the tracer's tolerance in pixels.
pub const METAL_RESOLUTION: usize = 2048;
pub const METAL_TOLERANCE: f64 = 0.6;
/// How far in from its edge plated metal's glaze is domed, and how high the
/// dome rises against that width. A domed glaze catches the light
/// across it where a flat one only mirrors whatever lies straight across from
/// it; a low one stays a glaze rather than a bead.
/// 0 lays the glaze flat.
pub const GLAZE_DOME_WIDTH: f32 = 0.0;
pub const GLAZE_DOME_RISE: f32 = 0.5;
/// The widest seam in plated metal's glaze that is closed before it is
/// domed; see `badge::glaze_parts`.
pub const GLAZE_SEAM: f64 = 0.0004;
/// The brightest plated metal's glaze is painted, as a share of full.
pub const GLAZE_WHITE: f32 = 1.0;
/// The least light the glaze reflects, as its brightest channel in linear
/// light: plated in a dark colour, a metal mirrors next to nothing and reads
/// as black, so a grey line would come out a black one.
pub const GLAZE_DARKEST: f32 = 0.2;
