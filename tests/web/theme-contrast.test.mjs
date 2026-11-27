// The Moss theme's colours are tokens in web/app.css. These tests read the stylesheet, so an edit that
// breaks a contrast ratio, brings a violet colour back or hard-codes a colour outside the token blocks fails here.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import { css, scriptTexts } from './page.mjs';

const block = (re) => {
  const m = css.match(re);
  assert.ok(m, 'token block not found: ' + re);
  const out = {};
  // Comments inside the block carry prose, not tokens.
  for (const [, k, v] of m[1].replace(/\/\*[\s\S]*?\*\//g, '').matchAll(/--([a-z0-9-]+):([^;}]+)/g)) out[k] = v.trim();
  return out;
};
const darkRaw = block(/:root\{([^}]*)\}/);
const lightRaw = block(/:root\[data-theme=light\]\{([^}]*)\}/);
const resolve = (raw) => {
  const get = (k, seen = []) => {
    const v = raw[k];
    const ref = v && v.match(/^var\(--([a-z0-9-]+)\)$/);
    if (ref) { assert.ok(!seen.includes(ref[1]), 'token cycle at ' + k); return get(ref[1], [...seen, k]); }
    return v;
  };
  return get;
};
const themes = {
  dark: resolve(darkRaw),
  light: resolve({ ...darkRaw, ...lightRaw }),
};

const channel = (c) => { c /= 255; return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4; };
const rgb = (hex) => { assert.match(hex, /^#[0-9a-f]{6}$/i, 'not a hex colour: ' + hex); return [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16)); };
const lum = (hex) => { const [r, g, b] = rgb(hex).map(channel); return 0.2126 * r + 0.7152 * g + 0.0722 * b; };
const ratio = (a, b) => { const x = lum(a), y = lum(b); return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05); };
const blend = (fg, bg, a) => '#' + rgb(fg).map((v, i) => Math.round(v * a + rgb(bg)[i] * (1 - a)).toString(16).padStart(2, '0')).join('');
const hue = (hex) => {
  const [r, g, b] = rgb(hex).map((v) => v / 255), mx = Math.max(r, g, b), d = mx - Math.min(r, g, b);
  const h = d === 0 ? 0 : mx === r ? ((g - b) / d) % 6 : mx === g ? (b - r) / d + 2 : (r - g) / d + 4;
  return (h * 60 + 360) % 360;
};

const SURFACES = ['bg', 'panel', 'panel2'];
/** The colour tokens each chart in web/page-performance.js draws together (memory also draws --line for Cached, which is a track colour). */
const CHARTS = {
  'CPU user and system': ['acc', 's2'],
  'GPU device, renderer and tiler': ['fg', 'acc', 's2'],
  'Disk throughput, operations, network': ['acc', 's2'],
  'Memory layers': ['acc', 's2', 's3'],
  'Capacity bar': ['acc', 's3'],
};
/** [foreground token, background token, minimum ratio, what the pair is] */
const PAIRS = [
  ...['bg', 'panel', 'panel2', 'sel'].map((s) => ['fg', s, 4.5, 'body text']),
  ['fg2', 'panel', 4.5, 'secondary text'],
  ['fg2', 'bg', 4.5, 'secondary text'],
  ...['bg', 'panel', 'panel2', 'sel'].map((s) => ['mut', s, 4.5, 'muted text']),
  ...['bg', 'panel', 'panel2', 'sel'].map((s) => ['acc', s, 4.5, 'accent text (links, selected option)']),
  ['fg', 'panel2', 4.5, 'chart tooltip value and label'],
  ['fg2', 'panel2', 4.5, 'chart tooltip time'],
  ['edge', 'panel', 3, 'chart tooltip border'],
  ...['bg', 'panel'].map((s) => ['fg2', s, 3, 'chart guide line']),
  ['accfg', 'acc', 4.5, 'primary button label'],
  ['accfg', 'acchov', 4.5, 'primary button label on hover'],
  ['link', 'bg', 4.5, 'link text'],
  ['link', 'panel', 4.5, 'link text'],
  ['link', 'panel2', 4.5, 'link text'],
  ...['bg', 'panel', 'panel2'].map((s) => ['ok', s, 4.5, 'healthy status text']),
  ...['bg', 'panel', 'panel2'].map((s) => ['warn', s, 4.5, 'warning text']),
  ...['bg', 'panel', 'panel2'].map((s) => ['bad', s, 4.5, 'quiet danger label and error text']),
  ['badfg', 'bad', 4.5, 'danger button label'],
  ['fg', 'input-bg', 4.5, 'typed text in a field'],
  ['mut', 'input-bg', 4.5, 'placeholder text'],
  ['input-bd', 'input-bg', 3, 'field border'],
  ['input-bd', 'panel', 3, 'field border against the page'],
  ...SURFACES.map((s) => ['edge', s, 3, 'button border']),
  ...[...SURFACES, 'sel'].map((s) => ['edge2', s, 3, 'chip, menu and select border (a chip can sit on a selected row)']),
  ...SURFACES.map((s) => ['fg', s, 3, 'focus ring']),
  ...SURFACES.map((s) => ['acc', s, 3, 'chart series, selected-row bar']),
  ['s2', 'panel', 3, 'second chart series (charts sit in cards)'],
  ['s2', 'panel2', 2.4, 'second chart series on a track'],
  ...['bg', 'panel'].map((s) => ['s3', s, 3, 'third chart series and the purgeable stripes']),
  ...SURFACES.map((s) => ['ok', s, 3, 'chart or tile in the OK colour']),
  ...SURFACES.map((s) => ['warn', s, 3, 'warning meter or segment']),
  ...SURFACES.map((s) => ['bad', s, 3, 'danger meter']),
  ...['bg', 'panel'].map((s) => ['logo-accent', s, 3, 'the sidebar mark\'s accent end (it sits on the sidebar panel)']),
  ['chip-ok-fg', 'chip-ok-bg', 4.5, 'OK chip'],
  ['chip-warn-fg', 'chip-warn-bg', 4.5, 'warning chip'],
  ['chip-bad-fg', 'chip-bad-bg', 4.5, 'danger chip'],
  ['chip-n-fg', 'chip-n-bg', 4.5, 'neutral chip'],
];

for (const [name, get] of Object.entries(themes)) {
  test(name + ' theme: every token pair the stylesheet uses meets its contrast ratio', () => {
    const failures = [];
    for (const [f, b, min, what] of PAIRS) {
      const r = ratio(get(f), get(b));
      if (r < min) failures.push(what + ': --' + f + ' ' + get(f) + ' on --' + b + ' ' + get(b) + ' is ' + r.toFixed(2) + ', needs ' + min);
    }
    assert.deepEqual(failures, []);
  });

  test(name + ' theme: a disabled button stays readable (label and dashed border at 75% opacity)', () => {
    for (const s of SURFACES) {
      const eff = blend(get('mut'), get(s), 0.75);
      assert.ok(ratio(eff, get(s)) >= 3, 'disabled label on --' + s + ': ' + ratio(eff, get(s)).toFixed(2));
    }
  });

  test(name + ' theme: green means brand and cyan means OK, so a healthy chip never looks selected', () => {
    const gap = Math.abs(hue(get('acc')) - hue(get('ok')));
    assert.ok(gap >= 25 && gap <= 335, 'accent and OK hues are ' + gap.toFixed(0) + ' degrees apart');
    assert.ok(Math.abs(hue(get('chip-ok-fg')) - hue(get('acc'))) >= 25, 'the OK chip text must not be the accent green');
    assert.ok(Math.abs(hue(get('s2')) - hue(get('acc'))) >= 25, 'the second chart series must not be the accent green');
  });

  test(name + ' theme: the second chart series differs from the accent in lightness, not only hue', () => {
    assert.ok(ratio(get('acc'), get('s2')) >= 1.5, 'acc vs s2 is ' + ratio(get('acc'), get('s2')).toFixed(2));
  });

  test(name + ' theme: series drawn on the same chart differ pairwise in lightness (colour-blind readers)', () => {
    const bad = [];
    for (const [chart, series] of Object.entries(CHARTS)) {
      for (let i = 0; i < series.length; i++) for (let j = i + 1; j < series.length; j++) {
        const r = ratio(get(series[i]), get(series[j]));
        if (r < 1.3) bad.push(chart + ': --' + series[i] + ' vs --' + series[j] + ' is ' + r.toFixed(2));
      }
    }
    assert.deepEqual(bad, []);
  });

  test(name + ' theme: --ok is defined and has a consumer, so a rename fails here', () => {
    assert.match(get('ok'), /^#[0-9a-f]{6}$/i);
    // Nothing in the stylesheet reads --ok. The Overview tile reads it through var(--' + col + ') and passes 'ok' for a healthy count.
    const overview = scriptTexts.find((t) => /const tile = \(l, v, sub, to, col\)/.test(t));
    assert.ok(overview, 'the Overview tile builder moved');
    assert.match(overview, /'color:var\(--' \+ col \+ '\)'/);
    assert.match(overview, /review \? 'warn' : 'ok'/);
  });
}

test('the dark and light themes define the same tokens', () => {
  for (const k of Object.keys(lightRaw)) assert.ok(k in darkRaw, '--' + k + ' is only in the light block');
  const same = ['font-sans', 'font-mono', 'hh'];
  for (const k of Object.keys(darkRaw)) if (!same.includes(k)) assert.ok(k in lightRaw, '--' + k + ' has no light value');
});

test('every var(--token) in the stylesheet and scripts is defined', () => {
  const local = new Set([...css.matchAll(/(?:^|[;{\s])--([a-z0-9-]+):/g)].map((m) => m[1]));
  const used = new Set([css, ...scriptTexts].flatMap((t) => [...t.matchAll(/var\(--([a-z0-9-]+)\)/g)].map((m) => m[1])));
  // Set on one element by a script or a rule and read below it (a table's column widths, a drawer's width).
  const perElement = ['tracks-min', 'cols', 'gap', 'pl', 'pr', 'dw', 'focus-edge', 'edge-rest'];
  for (const u of used) assert.ok(local.has(u) || perElement.includes(u), '--' + u + ' is used but never defined');
});

test('no violet or old-theme colour is left in web/', () => {
  const old = ['b388ff', '6d3fd1', '120d1d', '170f26', '1d1530', '221a38', '35294f', 'a99bc9', 'ece4ff', 'f7f5fb', 'efeaf8', 'd8cfe8',
    '5f5478', 'ede6fa', '2b2442', 'd3c9f0', 'e6dff4', '3d3160', '7ee2b8', '0f7a55', '14382b', '0b5a3e', 'd4f1e4', 'c0254f', '40150e'];
  const dir = new URL('../../web/', import.meta.url);
  for (const f of readdirSync(dir).filter((n) => /\.(css|js|html)$/.test(n))) {
    const text = readFileSync(new URL(f, dir), 'utf8').toLowerCase();
    for (const hex of old) assert.ok(!text.includes(hex), f + ' still has #' + hex);
  }
});

const NAMED = ['black', 'white', 'red', 'green', 'blue', 'yellow', 'orange', 'purple', 'violet', 'pink', 'gray', 'grey', 'cyan', 'magenta', 'teal', 'navy',
  'maroon', 'olive', 'lime', 'aqua', 'silver', 'gold', 'brown', 'indigo', 'tan', 'coral', 'salmon', 'crimson', 'lavender', 'plum', 'orchid', 'turquoise', 'khaki'];
// The only literal colours outside the token blocks: black drop shadows, which are the same in both themes.
const SHADOWS = ['0 12px 30px rgba(0,0,0,.35)', '0 24px 60px rgba(0,0,0,.6)', '2px 0 16px rgba(0,0,0,.3)'];

test('colours live in the token blocks: no hex, rgb, hsl or named colour elsewhere in app.css', () => {
  let rest = css.replace(/:root(\[data-theme=light\])?\{[^}]*\}/g, '').replace(/\/\*[\s\S]*?\*\//g, '');
  for (const sh of SHADOWS) { assert.ok(rest.includes(sh), 'allowed shadow gone, trim the list: ' + sh); rest = rest.split(sh).join(''); }
  assert.deepEqual(rest.match(/#[0-9a-f]{3,8}\b/gi) || [], []);
  assert.deepEqual(rest.match(/\b(rgba?|hsla?|hwb|lab|lch|oklab|oklch|color)\(/gi) || [], []);
  // Only declaration values: a selector or a font name is not a colour.
  const values = [...rest.matchAll(/([a-z-]+)\s*:\s*([^;{}]+)/g)].filter(([, prop]) => !/^(font|font-family|content|src)$/.test(prop)).map((m) => m[2]);
  const named = values.flatMap((v) => v.replace(/var\([^)]*\)|url\([^)]*\)|'[^']*'/g, '').toLowerCase().split(/[^a-z-]+/)).filter((w) => NAMED.includes(w));
  assert.deepEqual(named, []);
});

test('colours live in the token blocks: no literal colour in the page scripts (the report page excepted)', () => {
  // The exported report is a standalone print page with its own inline style, so it is the one exception.
  for (const t of scriptTexts) {
    const code = t.replace(/const REPORT_CSS[\s\S]*?;\n/, '');
    assert.deepEqual(code.match(/['"`]#[0-9a-f]{3,8}\b/gi) || [], []);
    assert.deepEqual(code.match(/\b(rgba?|hsla?)\(/g) || [], []);
    assert.deepEqual(code.match(new RegExp('(?:background|color|fill|stroke)(?:-color)?\\s*[:=]\\s*[\'"]?(?:' + NAMED.join('|') + ')\\b', 'gi')) || [], []);
  }
});

test('buttons are outlined and square: no chamfer, no clip-path, a 2px border, a dashed disabled border', () => {
  const btn = css.split('\n').filter((l) => /^\.btn[\s.{:\[]/.test(l)).join('\n');
  assert.ok(!/clip-path|--cut|::before/.test(btn), 'the chamfer is gone');
  assert.match(btn, /\.btn\{[^}]*border:2px solid var\(--bd\)/);
  assert.match(btn, /\.btn\{[^}]*--bd:var\(--edge\)/);
  assert.match(btn, /\.btn:focus-visible\{outline:3px solid var\(--fg\);outline-offset:2px\}/);
  assert.match(btn, /\[aria-disabled=true\]\{[^}]*border-style:dashed;opacity:\.75/);
  assert.match(btn, /\.btn\.btn-acc\{[^}]*--fill:var\(--acc\)[^}]*--ink:var\(--accfg\)/);
  assert.match(btn, /\.btn\.btn-danger\{[^}]*--fill:var\(--bad\)[^}]*--ink:var\(--badfg\)/);
});

test('forced colors keep the capacity bar segments and legend squares apart', () => {
  const fc = css.match(/@media\(forced-colors:active\)\{\.capbar\{.*\n/)[0];
  assert.match(fc, /\.capbar i\{border-right:2px solid ButtonText;forced-color-adjust:none\}/);
  assert.match(fc, /\.legend-dot\{border:1px solid ButtonText;forced-color-adjust:none\}/);
});

test('forced colors name a border for every button variant and GrayText for a disabled one', () => {
  const fc = css.match(/@media\(forced-colors:active\)\{\.btn,[\s\S]*?\n/)[0];
  for (const v of ['.btn.btn-acc', '.btn.btn-danger', '.btn.btn-danger.quiet']) assert.ok(fc.includes(v + '{') || fc.includes(v + ','), v + ' is missing');
  assert.match(fc, /--bd:ButtonText/);
  assert.match(fc, /--bd:Highlight/);
  assert.match(fc, /--bd:GrayText;--fill:ButtonFace;--ink:GrayText/);
});

test('the chart tooltip reads only tokens the contrast pairs cover, and forced colors use system colors', () => {
  const tip = css.match(/\.chart-tip\{([^}]*)\}/)[1];
  assert.match(tip, /background:var\(--panel2\)/);
  assert.match(tip, /color:var\(--fg\)/);
  assert.match(tip, /border:1px solid var\(--edge\)/);
  assert.match(css.match(/\.chart-tip-t\{([^}]*)\}/)[1], /color:var\(--fg2\)/);
  assert.match(css.match(/\.chart-guide\{([^}]*)\}/)[1], /var\(--fg2\)/);
  const fc = css.match(/@media\(forced-colors:active\)\{\.chart-tip\{[^\n]*/)[0];
  for (const w of ['Canvas', 'CanvasText', 'Highlight']) assert.ok(fc.includes(w), w + ' is missing from the forced-colors chart rules');
});
