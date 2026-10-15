/** sRGB, straight alpha, each 0 to 1. */
export interface Rgba {
  r: number;
  g: number;
  b: number;
  a: number;
}

export type LusterErrorKind =
  /** The data is not an SVG document. */
  | 'invalidSvg'
  /** The SVG exceeds the engine's limits (shapes, path segments, …). */
  | 'inputTooComplex'
  /** The SVG draws nothing a badge can be made of. */
  | 'nothingToMint'
  /** The source is larger than `Luster.maximumBytes`. */
  | 'tooLarge'
  /** The URL could not be fetched. */
  | 'unreadableSource'
  /** The engine itself went wrong: a bug, not the document's doing. */
  | 'engineFailure';

/** Why a badge could not be made: the document, its source, or the engine. */
export class LusterError extends Error {
  constructor(kind: LusterErrorKind, message: string);
  readonly name: 'LusterError';
  readonly kind: LusterErrorKind;
}

export interface MintOptions {
  /** Leave out the triangles no view can see: a smaller badge to send on, at
   *  the cost of the work to find them. */
  withoutHiddenFaces?: boolean;
  /** Leave the lines and the edge's top in the metal, rather than plate them
   *  in the colours the document paints on them. */
  metalLines?: boolean;
  /** Gives the badge up: the promise rejects with the signal's reason, and the
   *  mint stops once no one else is waiting for it. */
  signal?: AbortSignal;
}

/** One submesh, in glTF's and three.js's space: right-handed, y up, the
 *  badge's face turned towards +z. */
export interface Submesh {
  readonly name: string;
  /** Indexes the badge's materials. */
  readonly material: number;
  /** Interleaved, twelve floats a vertex: position (3), normal (3), texture
   *  coordinates (2, v running down the image, as glTF's) and tangent (4, w
   *  the handedness). */
  readonly vertices: Float32Array;
  /** Three a triangle, wound counter-clockwise seen from outside. */
  readonly indices: Uint32Array;
  readonly vertexCount: number;
  /** The lowest corner of the submesh's bounds, x, y and z. */
  readonly min: Float32Array;
  readonly max: Float32Array;
}

export type MaterialRole = 'plated' | 'goldBack' | 'field' | 'art' | 'glaze';

export interface Material {
  readonly name: string;
  readonly role: MaterialRole;
  readonly color: Readonly<Rgba>;
  readonly metallic: number;
  readonly roughness: number;
  /** A texture for the front face, where the document's colour changes across
   *  it; indexes the badge's textures. */
  readonly texture: number | undefined;
}

/** A texture with its mip chain, made by the engine so that every renderer
 *  filters the same levels: RGBA8, sRGB-encoded, rows top to bottom; each level
 *  `max(1, width >> i)` by `max(1, height >> i)`, to one texel. */
export interface Sheet {
  readonly width: number;
  readonly height: number;
  readonly levels: readonly Uint8Array[];
}

/** A struck badge: what a view shows, and the bytes to send one somewhere
 *  else. Its meshes are shared with everyone given the same badge: read them,
 *  change none. */
export class LusterBadge {
  private constructor();
  /** Identifies the badge: the document and the options it was struck with.
   *  Equal keys mean the same badge. */
  readonly designKey: string;
  readonly submeshes: readonly Submesh[];
  readonly materials: readonly Material[];
  /** In the order `Material.texture` indexes them. */
  readonly textures: readonly Sheet[];
  /** The badge as a glTF binary: meshes, materials and textures in one file,
   *  its metal plated in `metal` (the engine's pale gold unless given),
   *  `scale` units (metres, in glTF) across. The same badge always writes the
   *  same bytes. An aborted `signal` rejects with its reason. */
  glb(options?: { scale?: number; metal?: Rgba; signal?: AbortSignal }): Promise<Uint8Array>;
}

/** An SVG: its text, its bytes, or an http(s) URL fetched with
 *  `Luster.loader`. */
export type LusterSource = string | URL | Uint8Array | ArrayBuffer;

export declare const Luster: {
  /** Larger documents are refused: before they are fetched in full when the
   *  server says how large they are. */
  readonly maximumBytes: number;
  /** What a URL source is fetched with: `fetch` unless the page sets another. */
  loader: (url: URL, signal?: AbortSignal) => Promise<Uint8Array | ArrayBuffer>;
  /** Fetches and compiles the engine. A mint does it itself; calling it early
   *  gets it done before the first badge is wanted. It is done once, and
   *  tried again after a failure. */
  init(): Promise<void>;
  /** The engine's version. */
  version(): Promise<string>;
  /** The engine's metals. */
  metals(): Promise<{ gold: Rgba; silver: Rgba; copper: Rgba }>;
  /** Mints a badge in a worker, off the thread that draws. At most two mints
   *  run at once across the page; the rest wait their turn. Recent badges are
   *  kept by their bytes and options, so asking again is immediate, and asking
   *  for one already being minted waits for that mint rather than starting
   *  another. Fails with a `LusterError`; an aborted `signal` rejects with
   *  its reason, and `Luster.close()` with an `AbortError`. */
  mint(source: LusterSource, options?: MintOptions): Promise<LusterBadge>;
  /** Ends the workers, letting go of their memory, which a module never gives
   *  back while it lives. Mints under way reject; badges already made keep
   *  their meshes, and their GLB mints them again. */
  close(): void;
};
