// LusterSnapshot: a badge drawn as <luster-view> draws it, into a still
// image, without an element. For thumbnails in a list, where a live view of
// each badge would cost far more than the picture is worth.
import { Luster } from './index.js';
import { LusterError } from './errors.js';
import { Stage, metalColor, srgb, studio as setUpStudio } from './stage.js';

export const LusterSnapshot = {
  /**
   * A square PNG of `badge`, `pixelSize` across, at `tilt` and `spin`
   * (radians; the resting pose unless given), plated in `metal` and lit for
   * `lighting` as a view is. Over `background`, a `#rrggbb` or `{ r, g, b }`
   * colour, or with nothing behind it: the image's alpha is then the badge's
   * coverage.
   */
  async image(badge, { metal, lighting, tilt, spin, pixelSize = 768, background } = {}) {
    if (!Number.isInteger(pixelSize) || pixelSize <= 0) {
      throw new RangeError(`a still needs pixels: ${pixelSize}`);
    }
    const studio = await setUpStudio();
    const canvas = document.createElement('canvas');
    canvas.width = canvas.height = pixelSize;
    const context = canvas.getContext('2d');
    const stage = new Stage(studio);
    try {
      stage.show(badge, {
        metal: metalColor(metal, studio.metals),
        lighting: lighting === 'off' ? 'off' : 'showcase',
      });
      stage.pose(tilt ?? studio.handling.restingTilt, spin ?? studio.handling.restingSpin);
      if (!stage.draw(context, pixelSize, pixelSize)) {
        throw new LusterError('engineFailure', 'the page lost its WebGL context while the still was drawn');
      }
    } finally {
      stage.dispose();
    }
    const behind = srgb(background);
    if (behind) {
      context.globalCompositeOperation = 'destination-over';
      context.fillStyle = `rgb(${behind.r * 255} ${behind.g * 255} ${behind.b * 255})`;
      context.fillRect(0, 0, pixelSize, pixelSize);
    }
    return new Promise((resolve, reject) =>
      canvas.toBlob((png) => (png ? resolve(png) : reject(new Error('the still could not be written as a PNG'))), 'image/png'),
    );
  },

  /** The same, for the badge `source` strikes with `options`. */
  async fromSource(source, { metalLines, withoutHiddenFaces, signal, ...still } = {}) {
    const badge = await Luster.mint(source, { metalLines, withoutHiddenFaces, signal });
    return LusterSnapshot.image(badge, still);
  },
};
