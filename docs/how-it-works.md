# How it works

## The badge

Built like a real cloisonné badge. The lines the document *strokes* stand as metal walls
level with the rim, and the enamel sits recessed between them. Fills are never walls,
so lettering and numerals stay enamel, and a fill runs on to the badge's edge with no
margin of its own, so only the metal's sides and its rolled edge show gold round it.

The metal the lines stand in is plated in the art's own colours: each line in its
stroke colour, a grey one lifted to read as gunmetal rather than black. Metal the art
leaves bare — a counter, or the metal joining two pieces — stays gold. `metalLines`
leaves the lines and the edge's top in the metal instead, gold walls round coloured
enamel.

The metal's colour is part of the appearance, not of what is struck: changing it
re-plates the badge and never changes its shape.

## Reading the SVG

The document is walked with its transforms, clip paths and masks (including masks
exported as embedded images, which are traced back into paths), and fills are separated
from strokes. Contents of `<defs>`, `<clipPath>` and `<mask>` are applied, never
painted. Untrusted input is bounded: bytes, elements, nesting and path complexity all
have limits, and anything past them is an error rather than a hang.

## Colour cells

Elements are resolved in paint order into disjoint cells, each keeping only what
nothing above it covers. A translucent paint is a *glaze*: it owns no area and only
cuts the cells beneath it, and every cell takes the colour the document actually
composites to, sampled from a rendering of it — from the pixels inside the cell, never
the edge ones, which are a blend with the neighbour. A stroke's cap ending on a solid
of its own colour is invisible, so it is not treated as a line; the rest of a stroke is
a wire and stays one, however it crosses or overlaps a same-coloured fill. Every boolean
leaves hairline scraps along the edges it cuts, so contours thinner than any art can be
are dropped.

## Exact colours and gradients

A cell's sampled colour is only an average, and a poor one: a cell under a gradient is
often many faces spread across the badge, each a different colour. So once the pieces
are struck, every face is coloured from the document itself, rendered through the
face's own outline. A face of one colour takes exactly that colour. One whose colour
changes — a gradient, or a wash over it — gets a small texture. Cells are cut along
every wash's edge, so colour only changes smoothly inside a face and a coarse texture
holds it, while the boundaries between colours stay geometry and stay sharp. Only
texels well inside the face are trusted and their colours are carried outward, so a
neighbour's colour never bleeds in at the edge.

## One sheet

A drawn badge has a hundred faces, and a texture and a material for each would have a
renderer making them one at a time. They all go on one sheet instead: each painted face
keeps its own pixels at its own density, each flat colour takes a small tile, every
tile's edge is repeated outward so filtering at a seam reads the tile and not its
neighbour, and each piece's texture coordinates are moved onto the tile it was given. A
badge is then one image and a material per kind of face, however many colours it
carries.

## Outline

The badge's edge is traced from the rendered alpha coverage rather than unioned from
the shapes, so overlapping pieces yield one clean contour. The tracer is marching
squares with sub-pixel interpolation, corner detection on the dense trace, and a
centripetal Catmull-Rom fit.

## Geometry

A badge is one metal plate with enamel cells laid on it, each a hair higher than the
last so no two faces are coplanar, sunk below the metal the lines stand in. The metal's
outline is grown round the art on a raster with an exact Euclidean distance field: a
hair where the art ends in a fill, which runs on to the edge, and a little more where it
ends in a visible stroke. A badge is one piece, so where two islands fall short of each
other the gap is closed with metal: the plate is grown and shrunk back by one and a half
margins of a 72-pixel canvas, and of what that adds, the parts touching two different
islands are kept. Gaps up to three margins wide close, ending in fillets, and an
island's own notches are left alone. The openings in the metal are cut to the cells'
own paths, and the side is a single wall from reverse to face whose outer edge carries
the roll: six rings stepped inward, a ring that would fold through itself in a narrow
notch flattened where it does.

The plating is a face of its own laid on the metal, cut to what the document paints
there and read from it, so each line takes its stroke's colours. Shapes cut from one
another meet across hairline seams, which are closed first so none shows.

## Look

Gold is fully metallic, so a generated panorama does the lighting: the showcase, below,
which every appearance reflects. `off` is for looking at a badge's shape instead: key,
fill and back lights and a headlight on the camera, no panorama, and nothing that
shines.

No two renderers agree on what a light is worth: RealityKit tone maps, Filament works
in real photometric units and tone maps its own way, and flutter_scene draws through
RealityKit's tone mapping, measured into a table. The studio (`style.rs`) keeps a column
of numbers for each, all set by drawing the same badge every way and matching what came
out. A still is rendered with the view's own numbers into an sRGB texture, as a view is
shown, so it is the picture the view shows.

## The showcase

`showcase`, the lighting a badge is shown in unless it is asked for `off`, lights a
badge as a product photographer would, with no lamps at all: only a panorama laid out
by the directions a badge's face points in. A flat face mirrors one direction, and the
enamel and the metal round it mirror the same one, so whatever makes the metal bright
hazes dark enamel over. The room is dark where a face usually points, with narrow
strips at the angles it turns through, so glossy enamel catches one as a band of sheen
crossing it rather than a wash; the enamel's colour comes from a large softbox overhead
and two more high to either side, where a face seldom points; a scrim round the camera
grades a face seen head on, and a faint tent keeps metal plated in the art's colours
from going black at any angle. The enamel is seen as through a polarizer, which takes
half its glare and leaves the metal's alone, as enamel badges are photographed. Exposed
for the enamel, it shows the colours the document paints.

## Renderers

- **Apple** — RealityKit. `LusterUIView` and `LusterNSView` are drawn by an `ARView`
  with no camera rather than by SwiftUI.
- **Android** — Filament, straight from the engine's buffers and textures with no glTF
  in between, shaded with gltfio's ubershaders as the same badge's glTF would be. The
  studio is built as a cubemap at runtime, so no baked environment travels with the
  app.
- **Flutter** — flutter_scene, in Dart rather than a platform view: the engine hands
  over the meshes and the sheet of colours, and the widget builds the scene.
  flutter_scene's tone curves keep a colour's saturation as it brightens, where
  RealityKit's let it fade towards white, so the widget draws with no curve of its own
  and reads the result through RealityKit's, measured into a table
  (`LusterShot --tone-table`). Under the showcase the badge is drawn with its own shader
  (`shaders/luster_badge.frag`): flutter_scene's standard image-based lighting and
  nothing else, without the cost of the lamps, shadows and fog the standard material
  carries. The badge is drawn every frame only while it turns, and on a GPU too slow
  for full resolution the widget starts each turn coarser.

## Concurrency

At most two mints run at once across the process, and recent badges are kept, keyed by
the document's bytes and the options; asking for one already being struck waits for
that mint. The engine does this (`mints`), so it is the same on every platform.

The raster work (coverage, distance fields, tracing) runs a band of rows per core, on a
pool that leaves the host a core to draw with; every line of a distance transform is
computed on its own, so the result is the same as a single pass.

## Export

`badge.glb(metal:)` writes glTF 2.0 binary directly: PBR metallic-roughness materials
with the sheet and the reverse's normal map embedded as PNGs. The same badge always
writes the same bytes, on one thread or eight, so two exports can be compared directly.

Optionally the faces no view can see are left out (`withoutHiddenFaces`). The test is
the struck pieces themselves — their footprints, heights and rolls — not a guess from
the triangles: a face goes only when every point of it lies strictly inside other
pieces, one that merely touches a neighbour stays, and a front face is never removed.
