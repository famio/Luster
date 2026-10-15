// <luster-view>: a badge minted from an SVG, lit in the studio. Drag to turn
// it; it keeps spinning briefly with momentum, unless the system asks for
// reduced motion.
import { Luster } from './index.js';
import { LusterError } from './errors.js';
import { Stage, studio as setUpStudio } from './stage.js';

const TEMPLATE = `
<style>
  :host {
    display: inline-block;
    position: relative;
    width: 300px;
    height: 300px;
    touch-action: none;
    user-select: none;
    -webkit-user-select: none;
    contain: content;
  }
  :host([hidden]) { display: none; }
  canvas { display: block; width: 100%; height: 100%; }
  .placeholder {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    pointer-events: none;
  }
  .placeholder.gone {
    opacity: 0;
    transition: opacity 200ms ease-out;
  }
  ::slotted(*) { pointer-events: auto; }
  .placeholder.gone ::slotted(*) { pointer-events: none; }
</style>
<canvas part="canvas"></canvas>
<div class="placeholder" part="placeholder"><slot></slot></div>
`;

/** The slowest a frame may take while the badge turns before it is drawn
 *  coarser: 45 frames a second. */
const SLOW_FRAME = 1 / 45;

/** The coarsest the badge is drawn while turning. */
const COARSEST = 0.4;

/** How far back a flick's speed is read from, and how long a finger may rest
 *  before letting go for the flick to count, in milliseconds. */
const FLICK_WINDOW = 100;
const FLICK_REST = 40;

/** A flick slower than this is no flick, and none is taken faster, in points
 *  a second: Flutter's bounds, which the other platforms' gestures keep
 *  near. */
const FLICK_SLOWEST = 50;
const FLICK_FASTEST = 8000;

const reducedMotion = globalThis.matchMedia?.('(prefers-reduced-motion: reduce)');

/**
 * Shows the badge minted from `src` (a URL) or `source` (SVG text, its bytes,
 * or a URL), plated in `metal` and lit by `lighting`.
 *
 * Its children are a placeholder, shown in the middle while a source is set
 * and no badge is on screen; the element's `state` attribute says why:
 * `minting`, or `failed`. A badge struck later takes the place of the one on
 * screen without it.
 *
 * Fires `statechange` on every change: minting, ready with a badge, failed,
 * or idle without a source.
 */
export class LusterView extends HTMLElement {
  static observedAttributes = ['src', 'metal', 'lighting', 'metal-lines', 'without-hidden-faces'];

  #shadow;
  #canvas;
  #context;
  #placeholder;
  /** The source given as a property, over `src`. */
  #source = null;
  #sourceSet = false;
  /** @type {Stage | null} */
  #stage = null;
  #studio = null;
  /** Why the studio would not set up, if it would not. */
  #broken = null;
  /** The badge shown, or about to be. */
  #badge = null;
  #state = 'idle';
  #error = null;
  /** Gives up the mint in flight. */
  #minting = null;
  #generation = 0;
  /** What the badge, or the mint in flight, was asked for (`#currentKey`). */
  #key = null;
  /** Whether the element is set up: connected, or moving within the page. */
  #live = false;
  /** Resolves to whether the studio set up. */
  #studioReady = null;
  #settled = false;
  /** Moves on whenever the stage is emptied, giving up a wait for a badge
   *  shown before to be drawn. */
  #emptied = 0;
  #posed = false;
  #width = 0;
  #height = 0;
  #visible = true;
  #dirty = false;
  #resize;
  #intersect;

  #tilt = 0;
  #spin = 0;
  #velocity = 0;
  #pointer = null;
  #samples = [];
  #frame = 0;
  #lastFrame = 0;
  #turningScale = 1;
  #turned = 0;
  #frameTime = 0;

  constructor() {
    super();
    this.#shadow = this.attachShadow({ mode: 'open' });
    this.#shadow.innerHTML = TEMPLATE;
    this.#canvas = this.#shadow.querySelector('canvas');
    this.#placeholder = this.#shadow.querySelector('.placeholder');
    this.addEventListener('pointerdown', (e) => this.#down(e));
    this.addEventListener('pointermove', (e) => this.#move(e));
    this.addEventListener('pointerup', (e) => this.#up(e));
    this.addEventListener('pointercancel', (e) => this.#up(e, true));
    this.addEventListener('lostpointercapture', (e) => this.#up(e, true));
  }

  // MARK: Properties

  /** SVG text, its bytes, or a URL; over `src` once set. Null shows nothing. */
  get source() {
    return this.#sourceSet ? this.#source : this.#srcURL();
  }

  set source(value) {
    this.#source = value ?? null;
    this.#sourceSet = true;
    this.#load();
  }

  get src() {
    return this.getAttribute('src') ?? '';
  }

  set src(value) {
    this.setAttribute('src', value);
  }

  /** `gold`, `silver`, `copper`, a `#rrggbb` colour, or an `{ r, g, b }`
   *  in sRGB from 0 to 1. The engine's pale gold unless given. */
  get metal() {
    return this.#metal ?? this.getAttribute('metal') ?? 'gold';
  }

  set metal(value) {
    if (typeof value === 'string') {
      this.#metal = null;
      this.setAttribute('metal', value);
      // The same string as before changes no attribute, but may replace a
      // colour given as an object.
      this.#appear();
    } else {
      this.#metal = value;
      this.#appear();
    }
  }

  #metal = null;

  /** `showcase`, the studio the badge is made for, or `off`: lamps alone,
   *  for looking at its shape. */
  get lighting() {
    return this.getAttribute('lighting') === 'off' ? 'off' : 'showcase';
  }

  set lighting(value) {
    this.setAttribute('lighting', value);
  }

  get metalLines() {
    return this.hasAttribute('metal-lines');
  }

  set metalLines(value) {
    this.toggleAttribute('metal-lines', Boolean(value));
  }

  get withoutHiddenFaces() {
    return this.hasAttribute('without-hidden-faces');
  }

  set withoutHiddenFaces(value) {
    this.toggleAttribute('without-hidden-faces', Boolean(value));
  }

  /** Whether a flick keeps the badge turning; never while the system asks
   *  for reduced motion. */
  get momentum() {
    return this.#momentum;
  }

  set momentum(value) {
    this.#momentum = Boolean(value);
  }

  #momentum = true;

  /** `idle`, `minting`, `ready` or `failed`. */
  get state() {
    return this.#state;
  }

  /** The badge on screen once `ready`. */
  get badge() {
    return this.#state === 'ready' ? this.#badge : null;
  }

  /** Why it `failed`. */
  get error() {
    return this.#state === 'failed' ? this.#error : null;
  }

  attributeChangedCallback(name, old, value) {
    if (old === value) return;
    if (name === 'src') {
      if (!this.#sourceSet) this.#load();
    } else if (name === 'metal-lines' || name === 'without-hidden-faces') {
      this.#load();
    } else {
      this.#appear();
    }
  }

  // MARK: Life

  connectedCallback() {
    // Set on the element before it was defined, they are its own and hide
    // the class's: set them again through it.
    for (const name of ['source', 'src', 'metal', 'lighting', 'metalLines', 'withoutHiddenFaces', 'momentum']) {
      if (Object.hasOwn(this, name)) {
        const value = this[name];
        delete this[name];
        this[name] = value;
      }
    }
    // Moved within the page: it is still set up.
    if (this.#live) return;
    this.#live = true;
    // A studio that would not set up is tried again.
    this.#broken = null;
    this.#resize = new ResizeObserver((entries) => this.#resized(entries[0]));
    try {
      this.#resize.observe(this, { box: 'device-pixel-content-box' });
    } catch {
      this.#resize.observe(this);
    }
    this.#intersect = new IntersectionObserver((entries) => {
      this.#visible = entries[entries.length - 1].isIntersecting;
      if (this.#visible && this.#dirty) this.#draw();
    });
    this.#intersect.observe(this);
    this.#placeholder.classList.toggle('gone', this.#placeholderGone());
    this.#studioReady = this.#setUp();
    // The badge it had when it went is shown again if it is still the one
    // asked for; anything else is minted.
    if (!(this.#state === 'ready' && this.#badge && sameKey(this.#key, this.#currentKey()))) this.#load();
  }

  disconnectedCallback() {
    // Moved within the page, it is connected again before this runs.
    queueMicrotask(() => {
      if (!this.isConnected && this.#live) this.#tearDown();
    });
  }

  #tearDown() {
    this.#live = false;
    this.#resize.disconnect();
    this.#intersect.disconnect();
    if (this.#minting) {
      // Given up, not failed: minted again if the element comes back.
      this.#generation++;
      this.#giveUp();
      this.#key = null;
    }
    cancelAnimationFrame(this.#frame);
    this.#frame = 0;
    this.#pointer = null;
    this.#velocity = 0;
    // The GPU's copy goes; the badge stays, to show again if the element
    // comes back.
    this.#stage?.dispose();
    this.#stage = null;
  }

  /** Sets up the studio and the element's stage in it, and shows the badge
   *  it has. False if the studio would not set up. */
  async #setUp() {
    try {
      const studio = (this.#studio = await setUpStudio());
      if (!this.#live) return false;
      if (this.#stage) return true;
      this.#context ??= this.#canvas.getContext('2d');
      const stage = (this.#stage = new Stage(studio));
      stage.onLost = () => this.#draw();
      if (!this.#posed) {
        this.#posed = true;
        this.#tilt = studio.handling.restingTilt;
        this.#spin = studio.handling.restingSpin;
      }
      stage.pose(this.#tilt, this.#spin);
      if (this.#badge) {
        stage.show(this.#badge, this.#appearance());
        this.#draw();
        if (!this.#settled) this.#settle();
      }
      return true;
    } catch (e) {
      if (!this.#live) return false;
      this.#generation++;
      this.#giveUp();
      this.#broken =
        e instanceof LusterError ? e : new LusterError('engineFailure', `the view could not start: ${e?.message ?? e}`);
      this.#report('failed', null, this.#broken);
      return false;
    }
  }

  // MARK: The badge

  /** What the element is asked to show: the badge it has is for this. */
  #currentKey() {
    return [
      this.#sourceSet ? this.#source : this.getAttribute('src'),
      this.metalLines,
      this.withoutHiddenFaces,
    ];
  }

  #srcURL() {
    const src = this.getAttribute('src');
    if (!src) return null;
    try {
      return new URL(src, this.ownerDocument.baseURI);
    } catch {
      return null;
    }
  }

  /** Whether there is no source to show. */
  #empty() {
    const source = this.#sourceSet ? this.#source : this.getAttribute('src');
    return source === null || source === undefined || source === '';
  }

  #options() {
    return { metalLines: this.metalLines, withoutHiddenFaces: this.withoutHiddenFaces };
  }

  #giveUp() {
    this.#minting?.abort();
    this.#minting = null;
  }

  async #load() {
    if (!this.#live) return;
    // A studio that would not set up is tried again for what is asked next,
    // and fails it if it still will not.
    if (this.#broken) {
      this.#broken = null;
      this.#studioReady = this.#setUp();
    }
    this.#giveUp();
    const mine = ++this.#generation;
    this.#key = this.#currentKey();
    if (this.#empty()) {
      this.#badge = null;
      this.#stage?.show(null, this.#appearance());
      this.#settled = false;
      this.#emptied++;
      this.#draw();
      this.#report('idle', null, null);
      return;
    }
    const source = this.source;
    if (source === null) {
      const src = this.getAttribute('src');
      this.#report('failed', null, new LusterError('unreadableSource', `${src} is not a URL`));
      return;
    }
    this.#report('minting', null, null);
    const minting = (this.#minting = new AbortController());
    try {
      // Struck while the studio sets up; shown once both are done.
      const badge = await Luster.mint(source, { ...this.#options(), signal: minting.signal });
      if (mine !== this.#generation) return;
      this.#minting = null;
      // A studio that would not set up is the element's failure, told once.
      if (!(await this.#studioReady) || mine !== this.#generation) return;
      this.#badge = badge;
      this.#stage?.show(badge, this.#appearance());
      this.#draw();
      this.#report('ready', badge, null);
      if (!this.#settled) this.#settle();
    } catch (e) {
      if (mine !== this.#generation) return;
      this.#minting = null;
      if (!(await this.#studioReady) || mine !== this.#generation) return;
      this.#report('failed', null, e);
    }
  }

  #report(state, badge, error) {
    this.#state = state;
    this.#error = error;
    this.setAttribute('state', state);
    this.#placeholder.classList.toggle('gone', this.#placeholderGone());
    this.dispatchEvent(new CustomEvent('statechange', { detail: { state, badge, error } }));
  }

  /** Gone while there is nothing to wait for, and once a badge is on
   *  screen: a later source's mint or failure leaves it there. */
  #placeholderGone() {
    return this.#empty() || (this.#settled && this.#badge !== null);
  }

  /** Lets the placeholder go once the badge has reached the screen: two
   *  frames on, as the first frame that draws it can be slow to show. A
   *  newer source does not stop it: the badge stays on screen while the next
   *  is struck. */
  #settle() {
    const emptied = this.#emptied;
    requestAnimationFrame(() =>
      requestAnimationFrame(() => {
        if (emptied !== this.#emptied || !this.#badge || !this.#stage) return;
        this.#settled = true;
        this.#placeholder.classList.toggle('gone', this.#placeholderGone());
      }),
    );
  }

  #appearance() {
    const metals = this.#studio?.metals;
    return { metal: metalColor(this.#metal ?? this.getAttribute('metal'), metals), lighting: this.lighting };
  }

  #appear() {
    if (!this.#stage) return;
    this.#stage.plate(this.#appearance());
    this.#draw();
  }

  // MARK: Drawing

  #resized(entry) {
    const device = entry.devicePixelContentBoxSize?.[0];
    const ratio = globalThis.devicePixelRatio || 1;
    const width = device ? device.inlineSize : Math.round(entry.contentRect.width * ratio);
    const height = device ? device.blockSize : Math.round(entry.contentRect.height * ratio);
    if (width === this.#width && height === this.#height) return;
    this.#width = width;
    this.#height = height;
    // Drawn now, before the page is painted, so that it never shows
    // stretched.
    this.#draw();
  }

  #draw() {
    const stage = this.#stage;
    if (!stage || !this.#context || this.#width === 0 || this.#height === 0) return;
    if (!this.#visible) {
      this.#dirty = true;
      return;
    }
    this.#dirty = false;
    const scale = this.#moving ? this.#turningScale : 1;
    const width = Math.max(1, Math.round(this.#width * scale));
    const height = Math.max(1, Math.round(this.#height * scale));
    if (this.#canvas.width !== width || this.#canvas.height !== height) {
      this.#canvas.width = width;
      this.#canvas.height = height;
    }
    stage.draw(this.#context, width, height);
  }

  // MARK: Turning it

  get #moving() {
    return this.#pointer !== null || this.#velocity !== 0;
  }

  #down(e) {
    if (this.#pointer !== null || !e.isPrimary || (e.pointerType === 'mouse' && e.button !== 0)) return;
    if (!this.#badge) return;
    this.#pointer = { id: e.pointerId, x: e.clientX, y: e.clientY };
    try {
      this.setPointerCapture(e.pointerId);
    } catch {
      // A pointer no longer down; its drag ends with its up.
    }
    this.#velocity = 0;
    this.#samples = [{ t: e.timeStamp, x: e.clientX }];
    this.#turnOn();
  }

  #move(e) {
    const pointer = this.#pointer;
    if (!pointer || e.pointerId !== pointer.id) return;
    const handling = this.#studio.handling;
    const dx = e.clientX - pointer.x;
    const dy = e.clientY - pointer.y;
    pointer.x = e.clientX;
    pointer.y = e.clientY;
    this.#spin += dx * handling.spinPerPoint;
    this.#tilt = Math.min(handling.tiltLimit, Math.max(-handling.tiltLimit, this.#tilt + dy * handling.tiltPerPoint));
    this.#stage?.pose(this.#tilt, this.#spin);
    this.#samples.push({ t: e.timeStamp, x: e.clientX });
    while (this.#samples.length > 2 && e.timeStamp - this.#samples[0].t > FLICK_WINDOW) this.#samples.shift();
  }

  #up(e, cancelled = false) {
    const pointer = this.#pointer;
    if (!pointer || e.pointerId !== pointer.id) return;
    this.#pointer = null;
    const still = reducedMotion?.matches || !this.momentum;
    this.#velocity = cancelled || still ? 0 : this.#flick(e.timeStamp) * this.#studio.handling.spinPerPoint;
    this.#turnOn();
  }

  /** The finger's speed across as it let go, points a second: none if it
   *  had come to rest. */
  #flick(now) {
    const samples = this.#samples.filter((s) => now - s.t <= FLICK_WINDOW);
    if (samples.length < 2) return 0;
    const first = samples[0];
    const last = samples[samples.length - 1];
    if (now - last.t > FLICK_REST || last.t <= first.t) return 0;
    const speed = ((last.x - first.x) / (last.t - first.t)) * 1000;
    if (Math.abs(speed) < FLICK_SLOWEST) return 0;
    return Math.max(-FLICK_FASTEST, Math.min(FLICK_FASTEST, speed));
  }

  /** Draws every frame while the badge turns. */
  #turnOn() {
    if (this.#frame) return;
    this.#lastFrame = 0;
    this.#frame = requestAnimationFrame((t) => this.#tick(t));
  }

  #tick(now) {
    const dt = this.#lastFrame ? (now - this.#lastFrame) / 1000 : 0;
    this.#lastFrame = now;
    this.#frame = 0;
    if (!this.#stage) return;
    this.#pace(dt);
    if (this.#pointer === null && this.#velocity !== 0) {
      const coast = this.#studio.engine.coast(this.#velocity, dt);
      this.#spin += coast.turn;
      this.#velocity = coast.velocity;
      coast.free();
      this.#stage.pose(this.#tilt, this.#spin);
    }
    if (this.#moving) {
      this.#draw();
      this.#frame = requestAnimationFrame((t) => this.#tick(t));
    } else {
      // At rest: drawn once more, at full resolution.
      this.#turned = 0;
      this.#draw();
    }
  }

  /** Draws the badge coarser while it turns if frames come too slowly, and
   *  keeps the resolution it settles on for the next turn; at rest it is
   *  drawn in full. */
  #pace(dt) {
    if (++this.#turned <= 3 || dt === 0) return;
    this.#frameTime = this.#frameTime === 0 ? dt : this.#frameTime * 0.7 + dt * 0.3;
    if (this.#turned % 6 === 0 && this.#frameTime > SLOW_FRAME && this.#turningScale > COARSEST) {
      // A frame's cost goes with its pixels, the square of the scale.
      const keepsUp = this.#turningScale * Math.sqrt(SLOW_FRAME / this.#frameTime) * 0.95;
      this.#turningScale = Math.max(COARSEST, Math.min(keepsUp, this.#turningScale * 0.9));
      this.#turned = 0;
      this.#frameTime = 0;
    }
  }
}

/** A metal as sRGB from 0 to 1: one of the engine's by name, or a colour. */
function sameKey(a, b) {
  return a !== null && a.length === b.length && a.every((x, i) => x === b[i]);
}

function metalColor(metal, metals) {
  if (metal && typeof metal === 'object') {
    return { r: +metal.r, g: +metal.g, b: +metal.b, a: 1 };
  }
  if (typeof metal === 'string') {
    if (Object.hasOwn(metals, metal)) return metals[metal];
    const hex = /^#([0-9a-f]{6})$/i.exec(metal.trim());
    if (hex) {
      const n = parseInt(hex[1], 16);
      return { r: ((n >> 16) & 255) / 255, g: ((n >> 8) & 255) / 255, b: (n & 255) / 255, a: 1 };
    }
  }
  return metals.gold;
}

if (!customElements.get('luster-view')) customElements.define('luster-view', LusterView);
