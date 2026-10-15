import type { LusterBadge, LusterSource, MintOptions, Rgba } from './index.js';

export interface LusterStillOptions {
  /** `gold`, `silver`, `copper`, a `#rrggbb` colour, or a colour in sRGB
   *  from 0 to 1. The engine's pale gold unless given. */
  metal?: 'gold' | 'silver' | 'copper' | string | Omit<Rgba, 'a'> | null;
  /** `showcase`, the studio the badge is made for, or `off`: lamps alone. */
  lighting?: 'showcase' | 'off';
  /** About x and about y, radians; the resting pose unless given. */
  tilt?: number;
  spin?: number;
  /** Across and down, the still being square. 768 unless given. */
  pixelSize?: number;
  /** A `#rrggbb` colour or one in sRGB from 0 to 1 behind the badge;
   *  nothing behind it unless given, the alpha being the badge's coverage. */
  background?: string | Omit<Rgba, 'a'> | null;
}

/**
 * A badge drawn as `<luster-view>` draws it, into a still image, without an
 * element: for thumbnails in a list, where a live view of each badge would
 * cost far more than the picture is worth.
 */
export declare const LusterSnapshot: {
  /** A square PNG of the badge. */
  image(badge: LusterBadge, options?: LusterStillOptions): Promise<Blob>;
  /** The same, for the badge `source` strikes with the mint's options. */
  fromSource(source: LusterSource, options?: LusterStillOptions & MintOptions): Promise<Blob>;
};
