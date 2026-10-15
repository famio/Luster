// Luster on the web: drop an SVG, pick the metal, the lighting and the lines,
// export a GLB. The badge is minted and drawn here, in the browser.
import '@famio/luster/view';
import { Luster } from '@famio/luster';
import { lang, t } from './strings.js';

const $ = (id) => document.getElementById(id);
const view = $('view');
const stage = $('stage');
const exportButton = $('export');

document.documentElement.lang = lang;
for (const element of document.querySelectorAll('[data-text]')) element.textContent = t[element.dataset.text];

/** The file on stage, as the caption shows it. */
let fileName = '';
/** The last badge struck: what export writes, even after a later document
 *  fails, as the stage keeps showing it; named for the file it came from. */
let badge = null;
let badgeName = '';
let metal = 'gold';
let exporting = false;
/** Counts the files asked for, so one read late gives way to a later one. */
let opened = 0;
const metals = Luster.metals();

// Metal

for (const choice of document.querySelectorAll('input[name="metal"]')) {
  choice.onchange = () => (view.metal = metal = choice.value);
}
view.metal = metal;

// Lighting and lines

$('shine').onchange = (e) => (view.lighting = e.target.checked ? 'showcase' : 'off');
$('metal-lines').onchange = (e) => (view.metalLines = e.target.checked);

// Opening

const pick = $('pick');
$('open').onclick = () => pick.click();
pick.onchange = () => {
  if (pick.files[0]) open(pick.files[0]);
  // The same file again is a change too.
  pick.value = '';
};

async function open(file) {
  const mine = ++opened;
  if (!(file.type === 'image/svg+xml' || /\.svg$/i.test(file.name))) {
    return alert(t.cannotOpen, t.notSvg(file.name));
  }
  let bytes;
  try {
    bytes = new Uint8Array(await file.arrayBuffer());
  } catch (e) {
    if (mine !== opened) return;
    $('file').textContent = caption(file.name, true);
    return alert(t.cannotOpen, e.message);
  }
  if (mine === opened) show(bytes, file.name);
}

function show(bytes, name) {
  fileName = name;
  $('file').textContent = name;
  $('placeholder').hidden = true;
  view.source = bytes;
}

view.addEventListener('statechange', ({ detail }) => {
  if (detail.state === 'ready') {
    badge = detail.badge;
    badgeName = fileName;
    $('file').textContent = fileName;
  } else if (detail.state === 'failed') {
    if (fileName) $('file').textContent = caption(fileName, true);
    alert(t.cannotMake, detail.error?.message ?? '');
  }
  settleExport();
});

/** Export writes the badge on stage, and none while another is being struck
 *  in its place. */
function settleExport() {
  exportButton.disabled = !badge || exporting || view.state === 'minting';
}

function caption(name, failed) {
  return failed ? `${name}  (${t.unreadable})` : name;
}

// Dropping

const carriesSvg = (e) => [...e.dataTransfer.types].some((type) => type === 'Files' || type === 'image/svg+xml' || type === 'text/plain');
/** A link or text dragged within the page is not dropped on the stage. */
let draggingHere = false;
document.addEventListener('dragstart', () => (draggingHere = true));
document.addEventListener('dragend', () => (draggingHere = false));

window.addEventListener('dragover', (e) => {
  if (draggingHere || !carriesSvg(e)) return;
  e.preventDefault();
  e.dataTransfer.dropEffect = 'copy';
  stage.classList.add('dropping');
});
window.addEventListener('dragleave', (e) => {
  // Leaving the window, not one element for another; Safari names no
  // element either way, so the pointer has to be outside too.
  const outside = e.clientX <= 0 || e.clientY <= 0 || e.clientX >= innerWidth || e.clientY >= innerHeight;
  if (!e.relatedTarget && outside) stage.classList.remove('dropping');
});
// A drag given up with Escape ends with neither; the pointer moves again.
window.addEventListener('mousemove', () => stage.classList.remove('dropping'));
window.addEventListener('drop', async (e) => {
  if (draggingHere) return;
  e.preventDefault();
  stage.classList.remove('dropping');
  const file = e.dataTransfer.files[0];
  if (file) return open(file);
  // An SVG dragged from another page or a design tool, as its text.
  const text = e.dataTransfer.getData('image/svg+xml') || e.dataTransfer.getData('text/plain');
  opened++;
  if (/<svg[\s>]/i.test(text)) return show(new TextEncoder().encode(text), 'badge.svg');
  alert(t.cannotOpen, t.noSvg);
});

// Export

exportButton.onclick = exportGlb;

async function exportGlb() {
  if (exportButton.disabled) return;
  // What is on stage now, whatever is opened while it is written.
  const writing = badge;
  const name = badgeName;
  exporting = true;
  settleExport();
  exportButton.textContent = t.exporting;
  try {
    const glb = await writing.glb({ metal: (await metals)[metal] });
    const link = document.createElement('a');
    link.href = URL.createObjectURL(new Blob([glb], { type: 'model/gltf-binary' }));
    link.download = `${exportBase(name)}.glb`;
    link.click();
    setTimeout(() => URL.revokeObjectURL(link.href), 60_000);
  } catch (e) {
    alert(t.cannotExport, e.message ?? String(e));
  } finally {
    exporting = false;
    settleExport();
    exportButton.textContent = t.export;
  }
}

/** The download's name: the file's, without its extension, never hidden. */
function exportBase(name) {
  const stem = name.trim().replace(/\.[^.]*$/, '').replace(/^\.+/, '');
  return stem || 'badge';
}

// Keys, as the Mac app's menu has them.

window.addEventListener('keydown', (e) => {
  if (!(e.metaKey || e.ctrlKey) || e.altKey || $('alert').open) return;
  // By the key's place, whatever the layout writes there.
  if (e.code === 'KeyO' && !e.shiftKey) {
    e.preventDefault();
    pick.click();
  } else if (e.code === 'KeyE' && e.shiftKey) {
    e.preventDefault();
    exportGlb();
  }
});

// Alerts

function alert(title, message) {
  const dialog = $('alert');
  $('alert-title').textContent = title;
  $('alert-message').textContent = message;
  if (!dialog.open) dialog.showModal();
}
