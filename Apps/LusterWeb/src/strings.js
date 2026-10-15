// The page's words, in English and in Japanese as the Mac app has them.

const en = {
  open: 'Open SVG',
  placeholder: 'Open an SVG to make a 3D badge.',
  drop: 'Drop an SVG',
  metal: 'Metal',
  metalNamed: (name) => `Metal ${name}`,
  gold: 'Gold',
  silver: 'Silver',
  copper: 'Copper',
  lighting: 'Lighting',
  shine: 'Shine and reflections',
  lines: 'Lines',
  metalLines: 'Lines and edge top in metal',
  export: 'Export GLB',
  exporting: 'Exporting…',
  unreadable: 'unreadable',
  ok: 'OK',
  cannotMake: 'Couldn’t Make the Badge',
  cannotOpen: 'Couldn’t Open the File',
  cannotExport: 'Couldn’t Export',
  noSvg: 'There’s no SVG in what was dropped.',
  notSvg: (name) => `${name} isn’t an SVG.`,
  source: 'Source',
};

const ja = {
  open: 'SVG を読み込む',
  placeholder: 'SVG を読み込むと、3D バッジを作ります。',
  drop: 'SVG をドロップ',
  metal: '金属',
  metalNamed: (name) => `金属 ${name}`,
  gold: 'ゴールド',
  silver: 'シルバー',
  copper: 'カッパー',
  lighting: 'ライティング',
  shine: '光沢と映り込み',
  lines: '線',
  metalLines: '線と縁のトップを金属に',
  export: 'GLB を書き出す',
  exporting: '書き出し中…',
  unreadable: '読み込めません',
  ok: 'OK',
  cannotMake: 'バッジを作れませんでした',
  cannotOpen: '読み込めませんでした',
  cannotExport: '書き出せませんでした',
  noSvg: 'ドロップされたものに SVG がありません。',
  notSvg: (name) => `${name} は SVG ではありません。`,
  source: 'ソース',
};

export const lang = (navigator.languages?.[0] ?? navigator.language ?? 'en').toLowerCase().startsWith('ja') ? 'ja' : 'en';
export const t = lang === 'ja' ? ja : en;
