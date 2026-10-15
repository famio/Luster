// LusterSnapshot: a badge drawn as <luster-view> draws it, into a still
// image, without an element. For thumbnails in a list, where a live view of
// each badge would cost far more than the picture is worth.
import { Luster, LusterBadge } from './index.js';
import { LusterError } from './errors.js';
import { Stage, lightingOf, metalColor, srgb, studio as setUpStudio } from './stage.js';

export const LusterSnapshot = {
  /**
   * A square still of a badge, or of the one a source strikes, as an
   * `ImageBitmap`: the picture itself, written into no file, for a canvas to
   * draw. `pixelSize` across, at `tilt` and `spin` (radians; the resting pose
   * unless given), plated in `metal` and lit for `lighting` as a view is.
   * Over `background`, a `#rrggbb` or `{ r, g, b }` colour, or with nothing
   * behind it: its alpha is then the badge's coverage. A source is struck
   * with `metalLines`, `withoutHiddenFaces` and `signal`, as `Luster.mint`
   * strikes it.
   */
  async image(of, options = {}) {
    return (await still(of, options)).transferToImageBitmap();
  },

  /**
   * The same still as a file: a PNG, or `type` with its `quality` where the
   * browser writes it (`image/webp` in Chrome and Firefox; Safari writes
   * PNG). The blob's `type` says which it is.
   */
  async blob(of, { type = 'image/png', quality, ...options } = {}) {
    return (await still(of, options)).convertToBlob({ type, quality });
  },
};

async function still(of, { metal, lighting, tilt, spin, pixelSize = 768, background, metalLines, withoutHiddenFaces, signal } = {}) {
  if (!Number.isInteger(pixelSize) || pixelSize <= 0) {
    throw new RangeError(`a still needs pixels: ${pixelSize}`);
  }
  const badge = of instanceof LusterBadge ? of : await Luster.mint(of, { metalLines, withoutHiddenFaces, signal });
  signal?.throwIfAborted();
  const studio = await setUpStudio();
  const largest = studio.renderer.capabilities.maxTextureSize;
  if (pixelSize > largest) throw new RangeError(`this browser draws at most ${largest} pixels across, not ${pixelSize}`);
  const canvas = new OffscreenCanvas(pixelSize, pixelSize);
  const context = canvas.getContext('2d');
  const stage = new Stage(studio);
  try {
    stage.show(badge, { metal: metalColor(metal, studio.metals), lighting: lightingOf(lighting) });
    stage.pose(tilt ?? studio.handling.restingTilt, spin ?? studio.handling.restingSpin);
    if (!stage.draw(context, pixelSize, pixelSize)) {
      throw new LusterError('engineFailure', 'the page lost its WebGL context while the still was drawn');
    }
  } finally {
    stage.dispose();
    // The views on the page grow the frame back to what they need.
    studio.shrink();
  }
  const behind = srgb(background);
  if (behind) {
    context.globalCompositeOperation = 'destination-over';
    context.fillStyle = `rgb(${behind.r * 255} ${behind.g * 255} ${behind.b * 255})`;
    context.fillRect(0, 0, pixelSize, pixelSize);
  }
  return canvas;
}
