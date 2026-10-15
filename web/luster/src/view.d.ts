import type { LusterBadge, LusterError, LusterSource, Rgba } from './index.js';

export type LusterViewState = 'idle' | 'minting' | 'ready' | 'failed';

export interface LusterStateChange {
  readonly state: LusterViewState;
  /** The badge on screen, once `ready`. */
  readonly badge: LusterBadge | null;
  /** Why it `failed`. */
  readonly error: LusterError | Error | null;
}

/**
 * `<luster-view>`: a badge minted from an SVG, lit in the studio, drawn by
 * three.js. Drag to turn it; it keeps spinning briefly with momentum, unless
 * the system asks for reduced motion. Pinch to bring it closer and slide two
 * fingers to look about it; a double tap puts it back.
 *
 * Its children are a placeholder, shown in the middle while a source is set
 * and no badge is on screen; the element's `state` attribute says why. A
 * badge struck later takes the place of the one on screen without it.
 *
 * Attributes: `src`, `metal`, `lighting`, `metal-lines`,
 * `without-hidden-faces`. Fires `statechange` on every change.
 */
export class LusterView extends HTMLElement {
  /** SVG text, its bytes, or a URL; over `src` once set. Null shows nothing. */
  source: LusterSource | null;
  /** A URL, resolved against the document. */
  src: string;
  /** `gold`, `silver`, `copper`, a `#rrggbb` colour, or a colour in sRGB
   *  from 0 to 1. The engine's pale gold unless given. */
  metal: 'gold' | 'silver' | 'copper' | string | Omit<Rgba, 'a'> | null;
  /** `showcase`, the studio the badge is made for, or `off`: lamps alone,
   *  for looking at its shape. */
  lighting: 'showcase' | 'off';
  /** Leave the lines and the edge's top in the metal. */
  metalLines: boolean;
  /** Leave out the triangles no view can see. */
  withoutHiddenFaces: boolean;
  /** Whether a flick keeps the badge turning; never while the system asks
   *  for reduced motion. */
  momentum: boolean;
  /** Whether a pinch brings the badge closer. Off, the badge is put back at
   *  its size and a pinch is the page's. */
  zoomable: boolean;
  readonly state: LusterViewState;
  /** The badge on screen once `ready`. */
  readonly badge: LusterBadge | null;
  /** Why it `failed`. */
  readonly error: LusterError | Error | null;

  addEventListener<K extends keyof LusterViewEventMap>(
    type: K,
    listener: (this: LusterView, event: LusterViewEventMap[K]) => unknown,
    options?: boolean | AddEventListenerOptions,
  ): void;
  addEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | AddEventListenerOptions,
  ): void;
}

export interface LusterViewEventMap extends HTMLElementEventMap {
  statechange: CustomEvent<LusterStateChange>;
}

declare global {
  interface HTMLElementTagNameMap {
    'luster-view': LusterView;
  }
}
