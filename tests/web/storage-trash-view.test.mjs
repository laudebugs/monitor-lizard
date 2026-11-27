// The Move to Trash dialog layer: rendering with hostile names, request handling, focus, the inert page and the
// guards around a dialog that cannot be drawn. It lifts the `storage-trash-view` block with the logic it calls and runs
// it against a small stand-in DOM, so no browser is needed.
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { html } from './page.mjs';
const block = (name) => {
  const m = html.match(new RegExp('/\\* ' + name + ':begin \\*/([\\s\\S]*?)/\\* ' + name + ':end \\*/'));
  assert.ok(m, name + ' block not found in the dashboard scripts (web/*.js)');
  return m[1];
};
const code = ['url-state', 'requester', 'disk-net', 'page-shell', 'storage-page', 'storage-trash', 'storage-trash-view'].map(block).join('\n');
const exported = 'trashOpen trashApply trashDispatch trashRequest trashSync trashClose closeTopLayer trashKeys trashDialog trashLayer trashFocusBack trashIdle trashSelection trashSummary openTrash'.split(' ');
const make = new Function('env', 'const { S, h, $, render, announce, api, loadStorage, labelTag, rec, document, setTimeout } = env;\nconst sigOpen = () => false, sigClose = () => {};\n' + code + '\nreturn { ' + exported.join(', ') + ' };');

// ---- the stand-in DOM
class El {
  constructor(tag, doc) { this.tag = tag; this.attrs = {}; this.kids = []; this.on = {}; this.doc = doc; }
  setAttribute(k, v) { this.attrs[k] = String(v); }
  getAttribute(k) { return k in this.attrs ? this.attrs[k] : null; }
  addEventListener(t, f) { this.on[t] = f; }
  focus() { this.doc.activeElement = this; }
  contains(n) { return n === this || this.kids.some((k) => k instanceof El && k.contains(n)); }
  all() { return this.kids.flatMap((k) => (k instanceof El ? [k, ...k.all()] : [])); }
  querySelectorAll(sel) {
    if (sel === 'button:not([disabled]), a[href]') return this.all().filter((e) => (e.tag === 'button' && !('disabled' in e.attrs)) || (e.tag === 'a' && 'href' in e.attrs));
    throw new Error('selector ' + sel);
  }
  querySelector(sel) { const m = /^\[([\w-]+)\]$/.exec(sel); assert.ok(m, sel); return this.all().find((e) => m[1] in e.attrs) || null; }
  get text() { return this.kids.map((k) => (typeof k === 'string' ? k : k.text)).join(' '); }
  allText() { return [this.text, ...this.all().map((e) => Object.values(e.attrs).join(' '))].join(' '); }
}
function world(extra = {}) {
  const doc = { activeElement: null }, ids = {}, timers = [], said = [], calls = { render: 0 };
  const shell = new El('div', doc);
  const nodesOf = (kids) => kids.flat(Infinity).filter((k) => k != null && k !== false);
  const h = (tag, attrs = {}, ...kids) => {
    if (extra.h) extra.h();
    const e = new El(tag, doc);
    for (const [k, v] of Object.entries(attrs)) {
      if (v == null || v === false) continue;
      if (k.startsWith('on')) e.addEventListener(k.slice(2).toLowerCase(), v); else e.setAttribute(k, v === true ? '' : v);
    }
    e.kids = nodesOf(kids).map(String).length ? nodesOf(kids).map((k) => (k instanceof El ? k : String(k))) : [];
    if (attrs.id) ids[attrs.id] = e;
    return e;
  };
  doc.querySelector = (sel) => (sel === '.shell' ? shell : null);
  const S = { view: 'storage', trash: undefined, modal: null };
  const env = { S, h, document: doc, $: (id) => ids[id] || null, render: () => { calls.render++; }, announce: (t) => said.push(t),
    loadStorage: () => { calls.rescan = (calls.rescan || 0) + 1; }, labelTag: (l) => h('span', { class: 'tag' }, l),
    rec: (m) => { Object.assign(S, m); }, setTimeout: (f) => timers.push(f), api: extra.api || (async () => { throw new Error('no api'); }) };
  return { P: make(env), S, said, timers, calls, shell, doc, ids, h, env };
}
const hex = (n) => n.toString(16).padStart(16, '0');
const row = (n, extra = {}) => ({ kind: 'item', id: hex(n), name: 'cache' + n, path: '/Users/me/Library/Caches/c' + n, label: 'safe', bytes: 1e9, ...extra });
const item = (n, status, extra = {}) => ({ id: hex(n), path: '/Users/me/Library/Caches/c' + n, status, code: null, reason: null, bytes: 1e9, ...extra });
const HOSTILE = 'evil\u202egnp\nnext\u200bline <b>&';
const BAD = /[\u202e\u200b\n]/;

test('the yours confirmation names the folder with escapes, never raw bidi, newline or zero-width characters', () => {
  const w = world(), y = row(3, { label: 'yours', name: HOSTILE, path: '/Users/me/' + HOSTILE });
  w.S.trash = { phase: 'confirming-yours', seq: 2, ids: [y.id], yours: [{ id: y.id, name: y.name, path: y.path, label: 'yours', bytes: 1 }], known: {}, confirmYours: false, summary: null };
  const dlg = w.P.trashDialog(), all = dlg.allText();
  assert.ok(!BAD.test(all), 'no raw character anywhere in text or attributes');
  assert.ok(all.includes('\\u{202e}') && all.includes('\\u{200b}') && all.includes('\\u{a}'));
  assert.ok(all.includes('<b>&'), 'markup characters are plain text, written through text nodes');
  const buttons = dlg.querySelectorAll('button:not([disabled]), a[href]').map((b) => b.text);
  assert.ok(buttons.some((t) => t.startsWith('Include') && t.includes('\\u{202e}')), 'the button that confirms names the escaped folder');
});

test('refusal reasons, codes, notes and audit errors from the server are escaped in the result', () => {
  const w = world();
  const sel = w.P.trashSelection([row(1)], [hex(1)]);
  w.S.trash = { phase: 'done', seq: 3, ids: sel.ids, yours: [], known: {}, confirmYours: false, summary: null };
  const body = { ok: false, dry_run: false, history_id: 4, note: 'n\u202e', audit_error: 'a\u200b', items: [item(1, 'failed', { reason: 'r\u202e\nz', code: 'c\u200b' })] };
  w.S.trash.summary = w.P.trashSummary(body, {});
  const all = w.P.trashDialog().allText();
  assert.ok(!BAD.test(all) && !/[\u202e\u200b]/.test(all));
  assert.ok(all.includes('\\u{202e}'));
  assert.ok(all.includes('Recorded as request 4 in History.'));
});

test('a closed dialog draws nothing, and a drawn one is a modal dialog with a labelled heading to focus', () => {
  const w = world();
  assert.equal(w.P.trashDialog(), null);
  w.S.trash = { phase: 'error', seq: 1, ids: [hex(1)], yours: [], known: {}, message: { text: 'x', retry: true } };
  const dlg = w.P.trashDialog(), box = dlg.all().find((e) => e.attrs.role === 'dialog');
  assert.equal(box.attrs['aria-modal'], 'true');
  assert.equal(box.attrs['aria-labelledby'], 'trash-title');
  assert.ok(dlg.querySelector('[data-autofocus]'), 'a heading takes focus when the dialog opens');
});

test('the page behind the dialog is inert while it is open and free again once it closes', () => {
  const w = world();
  w.S.trash = { phase: 'error', seq: 1, ids: [hex(1)], yours: [], known: {}, message: { text: 'x', retry: false } };
  assert.ok(w.P.trashLayer());
  assert.equal(w.shell.inert, true);
  w.S.trash = w.P.trashIdle();
  assert.equal(w.P.trashLayer(), null);
  assert.equal(w.shell.inert, false);
});

test('background polls are held only while a dialog is open (the quiet() guard)', () => {
  const w = world();
  assert.equal(w.P.trashOpen(), false);
  w.S.trash = { phase: 'previewed' };
  assert.equal(w.P.trashOpen(), true);
  w.S.trash = w.P.trashIdle();
  assert.equal(w.P.trashOpen(), false);
  assert.match(html, /const quiet = \(m\) => \{ Object\.assign\(S, m\); if \(!S\.menu && !S\.modal && !trashOpen\(\) && !sigOpen\(\)\) render\(\); \};/);
});

test('a dialog that cannot be drawn is closed with a message instead of freezing the page', () => {
  let boom = false;
  const w = world({ h: () => { if (boom) throw new Error('draw failed'); } });
  w.S.trash = { phase: 'error', seq: 1, ids: [hex(1)], yours: [], known: {}, message: { text: 'x', retry: false } };
  boom = true;
  assert.equal(w.P.trashLayer(), null);
  assert.equal(w.S.trash.phase, 'idle');
  assert.equal(w.P.trashOpen(), false, 'quiet() renders again');
  assert.match(w.said.at(-1), /could not be drawn and was closed\. Nothing was moved\./);
  assert.equal(w.shell.inert, false);
  w.S.trash = { phase: 'moving', seq: 2, ids: [hex(1)], yours: [], known: {}, summary: null };
  w.P.trashLayer();
  assert.match(w.said.at(-1), /open History to check/, 'a move in flight says to check History');
});

test('leaving the Storage page closes a preview with a message and focus, but not a running move', () => {
  const w = world();
  w.S.view = 'guide';
  w.S.trash = { phase: 'previewed', seq: 2, ids: [hex(1)], key: 'k', yours: [], known: {}, summary: { counts: {}, items: [] } };
  w.P.trashLayer();
  assert.equal(w.S.trash.phase, 'idle');
  assert.match(w.said.at(-1), /left the Storage page/);
  assert.equal(w.timers.length, 1, 'focus is moved after the render');
  w.S.trash = { phase: 'moving', seq: 3, ids: [hex(1)], known: {}, yours: [] };
  w.P.trashLayer();
  assert.equal(w.S.trash.phase, 'moving');
});

test('a changed selection or label closes the dialog with an announcement and a focus return, with no drawing', () => {
  const w = world();
  const sel = w.P.trashSelection([row(1)], [hex(1)]);
  w.S.trash = { phase: 'previewed', seq: 2, ids: sel.ids, key: sel.key, yours: [], known: {}, token: 'T', summary: { counts: {}, items: [] } };
  w.P.trashSync(sel);
  assert.equal(w.S.trash.phase, 'previewed', 'the same selection changes nothing');
  w.P.trashSync(w.P.trashSelection([row(1, { label: 'yours' })], [hex(1)]));
  assert.equal(w.S.trash.phase, 'idle');
  assert.match(w.said.at(-1), /selection or a label changed/);
  assert.equal(w.timers.length, 1);
  assert.equal(w.calls.render, 0, 'called from inside render, so it never renders itself');
  w.timers[0]();
  assert.equal(w.doc.activeElement, null, 'with neither the trigger nor main on the page nothing can take focus');
  const main = w.h('main', { id: 'main' }); w.timers[0]();
  assert.equal(w.doc.activeElement, main, 'the fallback is the main region');
});

test('closing the dialog returns focus to the Move to Trash button', () => {
  const w = world();
  const btn = w.h('button', { id: 'st-trash-btn' });
  w.S.trash = { phase: 'error', seq: 1, ids: [hex(1)], yours: [], known: {}, message: { text: 'x', retry: false } };
  w.P.trashClose();
  assert.equal(w.S.trash.phase, 'idle');
  assert.equal(w.doc.activeElement, btn);
  assert.equal(w.calls.render, 1);
});

test('Tab stays inside the dialog: it wraps at both ends', () => {
  const w = world();
  w.S.trash = { phase: 'error', seq: 1, ids: [hex(1)], yours: [], known: {}, message: { text: 'x', retry: true } };
  const dlg = w.P.trashDialog(), box = dlg.all().find((e) => e.attrs.role === 'dialog'), btns = box.querySelectorAll('button:not([disabled]), a[href]');
  const press = (shiftKey) => { let stopped = false; box.on.keydown({ key: 'Tab', shiftKey, currentTarget: box, preventDefault: () => { stopped = true; } }); return stopped; };
  btns.at(-1).focus();
  assert.equal(press(false), true);
  assert.equal(w.doc.activeElement, btns[0]);
  btns[0].focus();
  assert.equal(press(true), true);
  assert.equal(w.doc.activeElement, btns.at(-1));
  btns[0].focus();
  assert.equal(press(false), false, 'a Tab in the middle is left to the browser');
  const other = w.h('button', {}); other.focus();
  assert.equal(press(false), true, 'focus outside the dialog is pulled back in');
  assert.equal(w.doc.activeElement, btns[0]);
});

test('one Escape closes one layer: the Trash dialog first, then the Stop dialog', () => {
  const w = world();
  w.S.modal = { label: 'x' };
  w.S.trash = { phase: 'previewed', seq: 1, ids: [hex(1)], yours: [], known: {}, token: 'T', summary: { counts: { would_move: 1 }, items: [] } };
  assert.equal(w.P.closeTopLayer(), 'trash');
  assert.deepEqual([w.S.trash.phase, w.S.modal !== null], ['idle', true]);
  assert.equal(w.P.closeTopLayer(), 'stop');
  assert.equal(w.S.modal, null);
  assert.equal(w.P.closeTopLayer(), null);
  assert.equal((html.match(/e\.key === 'Escape'/g) || []).filter(Boolean).length >= 1, true);
  assert.match(html, /if \(e\.key === 'Escape' && closeTopLayer\(\)\) e\.preventDefault\(\);/);
  assert.doesNotMatch(html, /Escape' && S\.modal\) rec/, 'the Stop dialog is closed by closeTopLayer only');
});

test('the shell looks for autofocus inside the dialog first, never in the inert page', () => {
  assert.match(html, /const af = \(trash \|\| sig \|\| document\)\.querySelector\('\[data-autofocus\]'\)/);
});

function reqWorld(result) {
  const w = world({ api: async () => { if (result instanceof Error || result.status >= 400 || result === 'net') throw result.status ? Object.assign(new Error(result.text), result) : new Error('Failed to fetch'); return { status: 200, json: async () => result.body }; } });
  const sel = w.P.trashSelection([row(1)], [hex(1)]);
  w.S.trash = { phase: 'previewing', seq: 5, ids: sel.ids, key: sel.key, yours: [], known: Object.fromEntries(sel.items.map((i) => [i.id, i])), confirmYours: false };
  return { w, fx: { do: 'preview', seq: 5, ids: sel.ids, confirmYours: false } };
}
test('trashRequest turns an answer into a reply event: ok, a refusal with its body, a bad body, a dead network', async () => {
  let { w, fx } = reqWorld({ body: { ok: true, dry_run: true, items: [item(1, 'would_move')], confirm_token: 'T' } });
  await w.P.trashRequest(fx);
  assert.equal(w.S.trash.phase, 'previewed');
  ({ w, fx } = reqWorld({ status: 409, text: '{"ok":false,"code":"busy","error":"x"}' }));
  await w.P.trashRequest(fx);
  assert.deepEqual([w.S.trash.phase, /still running/.test(w.S.trash.message.text)], ['error', true]);
  ({ w, fx } = reqWorld({ status: 500, text: 'not json' }));
  await w.P.trashRequest(fx);
  assert.equal(w.S.trash.phase, 'error');
  ({ w, fx } = reqWorld('net'));
  await w.P.trashRequest(fx);
  assert.match(w.S.trash.message.text, /could not reach the server/);
  ({ w, fx } = reqWorld({ status: 500, text: JSON.stringify({ ok: false, items: [item(1, 'moved')] }) }));
  w.S.trash = { ...w.S.trash, phase: 'moving' };
  await w.P.trashRequest({ ...fx, do: 'commit', token: 'T' });
  assert.equal(w.S.trash.phase, 'done', 'a 500 with a body shows what moved');
});

test('a reply that arrives after the dialog was closed changes nothing', async () => {
  const { w, fx } = reqWorld({ body: { ok: true, dry_run: true, items: [item(1, 'would_move')], confirm_token: 'T' } });
  w.S.trash = w.P.trashIdle();
  await w.P.trashRequest(fx);
  assert.equal(w.S.trash.phase, 'idle');
});

test('the yours step reads as a question, not a final yes, and repeats the Put Back caveat', () => {
  const w = world();
  w.S.trash = { phase: 'confirming-yours', seq: 2, ids: [hex(3)], yours: [{ id: hex(3), name: 'Taxes', path: '/Users/me/Taxes', label: 'yours', bytes: 1 }], known: {}, confirmYours: false, summary: null };
  const dlg = w.P.trashDialog(), all = dlg.allText();
  assert.match(dlg.all().find((e) => e.attrs.id === 'trash-title').text, /^Include your own folder in the preview\?$/);
  assert.ok(all.includes('Nothing moves yet. You will see one more preview before the final Move.'));
  assert.ok(all.includes('until you move it back from the Trash by hand'));
  assert.ok(!/put it back/i.test(all), 'no promise that Put Back works');
  assert.ok(all.includes('Put Back may not be offered'));
  assert.ok(dlg.querySelectorAll('button:not([disabled]), a[href]').some((b) => b.text === 'Include \u201cTaxes\u201d and preview again'));
  assert.ok(!/removal|removed/i.test(all));
});

test('the result: the rescan advice sits above the History link, failed folders say to check the Trash', () => {
  const w = world();
  const sum = w.P.trashSummary({ ok: false, dry_run: false, history_id: 9, items: [item(1, 'moved'), item(2, 'failed', { reason: 'busy', bytes: null }), item(4, 'not_attempted', { bytes: null })] }, {});
  w.S.trash = { phase: 'done', seq: 3, ids: [hex(1)], yours: [], known: {}, confirmYours: false, summary: sum };
  const dlg = w.P.trashDialog(), all = dlg.allText();
  assert.ok(all.indexOf('Moved folders may stay listed for up to a minute. Wait a minute, then press Rescan.') > -1);
  assert.ok(all.indexOf('Moved folders may stay listed') < all.indexOf('Open History'));
  assert.ok(!all.includes('The page asked for a fresh scan'));
  assert.ok(all.includes('Check your Trash for folders that failed or were not attempted.'));
  assert.ok(all.includes('Moved 1 of 3 folders to the Trash (1.0 GB).'));
  assert.ok(!/removal|removed|\bitems?\b/i.test(dlg.text), 'folder and Trash are the only terms');
});

// ---- the dialog's Move button is aria-disabled, not disabled, while it cannot act
const draw = (w) => { const dlg = w.P.trashDialog(), box = dlg.all().find((e) => e.attrs.role === 'dialog'); return { dlg, box, move: dlg.all().find((e) => e.tag === 'button' && /^(Move|Nothing can be moved|Continue)/.test(e.text)) }; };
const previewed = (extra = {}) => ({ phase: 'previewed', seq: 2, ids: [hex(1)], key: 'k', yours: [], known: {}, token: null, confirmYours: false,
  summary: { counts: { would_move: 0, moved: 0, refused: 1, failed: 0, not_attempted: 0 }, items: [], would: { bytes: 0, unknown: 0 } }, ...extra });

test('Move with nothing movable: aria-disabled, no disabled attribute, in the focus trap, and a click, Enter or Space moves nothing', () => {
  const reqs = [], w = world({ api: async (u) => { reqs.push(u); throw new Error('must not call'); } });
  w.S.trash = previewed();
  const { box, move } = draw(w);
  assert.ok(move, 'the button is drawn');
  assert.equal(move.attrs['aria-disabled'], 'true');
  assert.ok(!('disabled' in move.attrs), 'the disabled attribute would drop it from the Tab order');
  assert.equal(move.attrs['aria-describedby'], 'trash-desc');
  const stops = box.querySelectorAll('button:not([disabled]), a[href]');
  assert.ok(stops.includes(move), 'trashKeys reaches it');
  // Tab from the last button wraps to the first; Tab from the one before the Move button lands on it.
  stops[0].focus(); box.on.keydown({ key: 'Tab', shiftKey: false, currentTarget: box, preventDefault() {} });
  stops.at(-1).focus(); let stopped = false; box.on.keydown({ key: 'Tab', shiftKey: false, currentTarget: box, preventDefault: () => { stopped = true; } });
  assert.equal(stopped, true); assert.equal(w.doc.activeElement, stops[0]);
  // A keyboard activation is a click event on a button; each path must do nothing but say why.
  let prevented = 0;
  for (let n = 0; n < 3; n++) move.on.click({ preventDefault: () => { prevented++; } });
  assert.equal(prevented, 3);
  assert.equal(w.S.trash.phase, 'previewed', 'the phase did not change');
  assert.equal(reqs.length, 0, 'no request');
  assert.equal(w.calls.render, 0);
  assert.equal(w.said.length, 3);
  assert.match(w.said[0], /Nothing can be moved/);
});

test('Move with a preview token is a live button, and one click sends one request', () => {
  const reqs = [], w = world({ api: async (u) => { reqs.push(u); return { status: 200, json: async () => ({}) }; } });
  w.S.trash = previewed({ token: 'T', summary: { counts: { would_move: 1, moved: 0, refused: 0, failed: 0, not_attempted: 0 }, items: [], would: { bytes: 1e9, unknown: 0 } } });
  const { move } = draw(w);
  assert.equal(move.attrs['aria-disabled'], undefined);
  move.on.click({ preventDefault() {} });
  assert.equal(w.S.trash.phase, 'moving');
  assert.equal(reqs.length, 1);
  const again = draw(w).dlg.all().find((e) => e.tag === 'button' && e.text === 'Moving...');
  assert.equal(again.attrs['aria-disabled'], 'true');
  again.on.click({ preventDefault() {} }); again.on.click({ preventDefault() {} });
  assert.equal(reqs.length, 1, 'the Moving button cannot start a second move');
  assert.equal(w.S.trash.phase, 'moving');
});

test('a folder only partly measured makes the dialog say "at least", never a plain total', () => {
  const w = world();
  const body = { dry_run: true, items: [{ id: hex(1), status: 'would_move', bytes: 5e9, path: '/Users/me/Library/Caches/c1' }] };
  const known = { [hex(1)]: { id: hex(1), name: 'c1', path: '/Users/me/Library/Caches/c1', label: 'safe', bytes: 5e9, lowerBound: true } };
  const sum = w.P.trashSummary(body, known);
  w.S.trash = previewed({ token: 'T', known, summary: sum });
  assert.match(draw(w).dlg.allText(), /would move to the Trash \(at least 5\.0 GB\)/);
  const exact = w.P.trashSummary(body, { [hex(1)]: { ...known[hex(1)], lowerBound: false } });
  w.S.trash = previewed({ token: 'T', summary: exact });
  assert.match(draw(w).dlg.allText(), /would move to the Trash \(5\.0 GB\)/);
});
