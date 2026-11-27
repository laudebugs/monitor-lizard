// The global search's box, menu and keys (the `global-search-view` block in web/search.js) run against a small stand-in
// DOM, so key handling, focus and what is drawn are tested without a browser. The look of the menu is checked by hand.
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { html, webFile } from './page.mjs';
const block = (name) => { const m = html.match(new RegExp('/\\* ' + name + ':begin \\*/([\\s\\S]*?)/\\* ' + name + ':end \\*/')); assert.ok(m, name + ' block not found in the dashboard scripts (web/*.js)'); return m[1]; };
const code = [webFile('core.js').match(/^const unprefixed = .*;$/m)[0], ...['requester', 'url-state', 'guide-search', 'global-search', 'global-search-view'].map(block)].join('\n');
const names = 'SEARCH searchKeys searchSlash searchInit searchUpdate searchRefresh searchSyncBox searchClose searchData'.split(' ');
const make = new Function('env', 'const { S, h, $, render, announce, loadGuide, location, history, window, HashChangeEvent, labelOf, trashOpen, sigOpen, document, setTimeout, clearTimeout } = env;\n' + code + '\nreturn { ' + names.join(', ') + ' };');

class El {
  constructor(tag, doc) { this.tagName = tag.toUpperCase(); this.attrs = {}; this.kids = []; this.on = {}; this.doc = doc; this.value = ''; this.hidden = false; this.isContentEditable = false; }
  setAttribute(k, v) { this.attrs[k] = String(v); }
  removeAttribute(k) { delete this.attrs[k]; }
  getAttribute(k) { return k in this.attrs ? this.attrs[k] : null; }
  addEventListener(t, f) { (this.on[t] ||= []).push(f); }
  fire(t, ev = {}) { const e = Object.assign({ prevented: false, stopped: false, preventDefault() { this.prevented = true; }, stopPropagation() { this.stopped = true; } }, ev); for (const f of this.on[t] || []) f(e); return e; }
  get id() { return this.attrs.id; }
  focus() { this.doc.activeElement = this; }
  select() { this.selected = true; }
  scrollIntoView() {}
  replaceChildren(...k) { this.kids = k; }
  all() { return this.kids.flatMap((k) => (k instanceof El ? [k, ...k.all()] : [])); }
  get text() { return this.kids.map((k) => (typeof k === 'string' ? k : k.text)).join(' '); }
}
function world(S0 = {}) {
  const doc = { activeElement: null }, timers = [], said = [], navs = [], calls = { render: 0, loadGuide: 0, labelOf: 0, guideReads: 0 };
  const q = new El('input', doc), smenu = new El('div', doc), body = new El('body', doc);
  const h = (tag, attrs = {}, ...kids) => {
    const e = new El(tag, doc);
    for (const [k, v] of Object.entries(attrs)) { if (v == null || v === false) continue; if (k.startsWith('on')) e.addEventListener(k.slice(2).toLowerCase(), v); else e.setAttribute(k, v); }
    e.kids = kids.flat(Infinity).filter((c) => c != null && c !== false);
    return e;
  };
  const $ = (id) => (id === 'q' ? q : id === 'smenu' ? smenu : smenu.all().find((e) => e.attrs.id === id) || null);
  const guideData = { commands: [{ id: 'tmutil-list', topic: 'storage', title: 'List local snapshots', command: 'tmutil list', description: 'Lists snapshots.', tags: ['snapshot'], safety: 'read-only' }], terms: [] };
  const grp = (label, c = {}) => ({ key: 'other:/A/' + label, label, trust: 'unknown', conns: [Object.assign({ direction: 'outbound', scope: 'public', proto: 'TCP', remote_addr: '8.8.8.8', remote_port: 443 }, c)] });
  const S = Object.assign({ view: 'overview', q: '', page: {}, modal: null, navOpen: false, copyText: null, menu: null, snap: { groups: [grp('Chrome'), grp('Chromium Helper'), grp('Slack')] }, seen: [{ group_key: 'x', remote: '9.9.9.9' }], pers: [], guide: { state: 'ready' } }, S0);
  Object.defineProperty(S.guide, 'data', { get() { calls.guideReads++; return guideData; }, enumerable: true });
  const env = {
    S, h, $, document: doc, setTimeout: (f) => { timers.push(f); return timers.length; }, clearTimeout: () => {},
    render: () => { calls.render++; if (S.view === 'apps' || S.view === 'guide') api.searchSyncBox(); api.searchRefresh(); },
    announce: (m) => said.push(m), loadGuide: () => { calls.loadGuide++; },
    location: { get hash() { return env.hashNow; }, set hash(v) { navs.push(['push', v]); env.hashNow = v; } }, hashNow: '#' + S.view,
    history: { replaceState: (_, __, v) => { navs.push(['replace', v]); env.hashNow = v; } },
    window: { dispatchEvent: () => navs.push(['hashchange']) }, HashChangeEvent: class {},
    labelOf: (k) => { calls.labelOf++; return k; }, trashOpen: () => !!env.trash, sigOpen: () => false,
  };
  const api = make(env);
  api.searchInit();
  return Object.assign(api, { S, q, smenu, body, doc, timers, said, navs, calls, env, guideData, flush: () => { while (timers.length) timers.shift()(); } });
}
const type = (w, text) => { w.q.value = text; w.q.fire('input'); };
const key = (w, k, o = {}) => w.q.fire('keydown', Object.assign({ key: k }, o));
const options = (w) => w.smenu.all().filter((e) => e.attrs.role === 'option');
const names2 = (w) => options(w).map((o) => o.kids[0].text);

test('typing opens the menu with grouped options, the box is a combobox pointing at the active one', () => {
  const w = world(); w.q.focus(); type(w, 'chrom');
  assert.equal(w.smenu.hidden, false); assert.equal(w.q.attrs['aria-expanded'], 'true');
  assert.deepEqual(names2(w), ['Chrome', 'Chromium Helper']);
  assert.ok(w.smenu.all().some((e) => e.attrs.role === 'group' && e.attrs['aria-labelledby'] === 'sg-apps'));
  key(w, 'ArrowDown');
  assert.equal(w.q.attrs['aria-activedescendant'], 'sr-0'); assert.equal(options(w)[0].attrs['aria-selected'], 'true');
});
test('no match: a "Nothing matches" row with the query, escaped, that is not selectable and not a result', () => {
  const w = world(); w.q.focus(); type(w, 'zz\u202e<b>');
  const [row, ...rest] = options(w);
  assert.equal(rest.length, 0); assert.equal(row.attrs['aria-disabled'], 'true');
  assert.equal(row.text, 'Nothing matches \u201czz\\u{202e}<b>\u201d');
  key(w, 'ArrowDown'); assert.equal(w.q.attrs['aria-activedescendant'], undefined);
});
test('Enter on a result opens it through the address; Enter with none chosen on a page with no filter opens Apps with the text', () => {
  const w = world(); w.q.focus(); type(w, 'slack'); key(w, 'ArrowDown');
  const e = key(w, 'Enter'); assert.ok(e.prevented);
  assert.deepEqual(w.navs, [['push', '#apps?sel=other%3A%2FA%2FSlack']]);
  const w2 = world(); w2.q.focus(); type(w2, ' chr ');
  assert.ok(key(w2, 'Enter').prevented);
  assert.deepEqual(w2.navs, [['push', '#apps?q=chr']]); assert.equal(w2.smenu.hidden, true);
});
test('Enter with none chosen on a page that filters in place changes no address and closes the menu', () => {
  const w = world({ view: 'apps' }); w.q.focus(); type(w, 'chr'); assert.equal(w.S.q, 'chr');
  assert.ok(key(w, 'Enter').prevented); assert.deepEqual(w.navs, []); assert.equal(w.smenu.hidden, true);
});
test('on a page with no filter, typing sets no filter and does not leave the page', () => {
  const w = world(); w.q.focus(); type(w, 'chr');
  assert.equal(w.S.view, 'overview'); assert.equal(w.S.q, ''); assert.equal(w.calls.render, 0); assert.deepEqual(w.navs, []);
});
test('the box text is read when a key is pressed, so a value set in code is searched', () => {
  const w = world(); w.q.focus(); w.q.value = 'slack'; // as fromHash does for #apps?q=slack
  const e = key(w, 'ArrowDown');
  assert.ok(e.prevented); assert.equal(w.smenu.hidden, false); assert.deepEqual(names2(w), ['Slack']);
  w.q.value = ''; key(w, 'Escape'); // the Storage reset clears the box in code: a stale menu must not come back
  w.q.value = 'chrom'; key(w, 'ArrowUp'); assert.equal(w.q.attrs['aria-activedescendant'], 'sr-1', 'Up on a closed menu opens on the last row');
});
test('one search per keystroke on a page that filters in place, and the data is not rebuilt while it is unchanged', () => {
  const w = world({ view: 'apps' }); w.q.focus();
  type(w, 'c'); w.calls.guideReads = 0; w.calls.render = 0; type(w, 'ch'); assert.equal(w.calls.guideReads, 1); assert.equal(w.calls.render, 1);
  type(w, 'chr');
  assert.equal(w.calls.labelOf, 1, 'the history rows are mapped once for unchanged data');
  w.S.seen = [...w.S.seen]; type(w, 'chro'); assert.equal(w.calls.labelOf, 2);
});
test('a poll keeps focus and the highlighted row, and announces only when what is offered changes', () => {
  const w = world(); w.q.focus(); type(w, 'chrom'); w.flush(); key(w, 'ArrowDown'); key(w, 'ArrowDown');
  assert.equal(w.SEARCH.active, 1); w.said.length = 0;
  w.S.snap = { groups: [...w.S.snap.groups] }; w.searchRefresh(); w.flush();
  assert.equal(w.doc.activeElement, w.q); assert.equal(w.SEARCH.active, 1); assert.deepEqual(w.said, [], 'same results, nothing said');
  w.S.snap = { groups: [{ key: 'other:/A/Chrome Beta', label: 'Chrome Beta', conns: [{ direction: 'outbound' }] }, ...w.S.snap.groups] };
  w.searchRefresh(); w.flush();
  assert.equal(options(w)[w.SEARCH.active].kids[0].text, 'Chromium Helper', 'the active row follows its result');
  assert.equal(w.said.length, 1);
});
test('a row whose detail changes is redrawn', () => {
  const w = world(); w.S.snap.groups[2].conns.push({ direction: 'listen', local_port: 3000, proto: 'TCP', exposed: false });
  w.q.focus(); type(w, '3000'); assert.match(options(w)[0].kids[1].text, /^Slack, TCP$/);
  w.S.snap = { groups: w.S.snap.groups.map((g) => (g.label === 'Slack' ? Object.assign({}, g, { conns: [{ direction: 'listen', local_port: 3000, proto: 'TCP', exposed: true }] }) : g)) };
  w.searchRefresh(); assert.match(options(w)[0].kids[1].text, /exposed/);
});
test('keys: composition (keyCode 229 or isComposing) and modifiers pass through untouched', () => {
  const w = world(); w.q.focus(); type(w, 'chr');
  for (const o of [{ keyCode: 229 }, { isComposing: true }, { metaKey: true }, { ctrlKey: true }, { altKey: true }, { shiftKey: true }]) for (const k of ['Enter', 'ArrowDown', 'ArrowUp', 'Escape']) {
    const e = key(w, k, o); assert.equal(e.prevented, false, k + ' ' + JSON.stringify(o)); assert.equal(e.stopped, false);
  }
  assert.deepEqual(w.navs, []); assert.equal(w.smenu.hidden, false); assert.equal(w.q.value, 'chr');
});
test('keys: nothing is stopped when the box is empty, or for a key that is not ours', () => {
  const w = world(); w.q.focus();
  for (const k of ['Enter', 'ArrowDown', 'ArrowUp', 'Escape', 'a', 'Tab']) assert.equal(key(w, k).prevented, false, k);
});
test('Escape closes the menu and stops there; a second Escape empties the box and the filter; a layer takes the Escape first', () => {
  const w = world({ view: 'apps' }); w.q.focus(); type(w, 'chr');
  let e = key(w, 'Escape'); assert.ok(e.prevented && e.stopped); assert.equal(w.smenu.hidden, true); assert.equal(w.q.value, 'chr');
  e = key(w, 'Escape'); assert.ok(e.prevented); assert.equal(w.q.value, ''); assert.equal(w.S.q, '');
  type(w, 'chr'); key(w, 'Escape'); w.S.navOpen = true;
  e = key(w, 'Escape'); assert.equal(e.prevented, false); assert.equal(w.q.value, 'chr');
});
test('/ focuses and selects the box, but types a slash in a field and stays out of the way of dialogs and composition', () => {
  const w = world(); const ev = (o) => Object.assign({ key: '/', target: w.body, prevented: false, preventDefault() { this.prevented = true; } }, o);
  let e = ev(); assert.equal(w.searchSlash(e), true); assert.ok(e.prevented); assert.equal(w.doc.activeElement, w.q); assert.ok(w.q.selected);
  for (const o of [{ target: new El('input', w.doc) }, { target: new El('textarea', w.doc) }, { target: new El('select', w.doc) }, { target: Object.assign(new El('div', w.doc), { isContentEditable: true }) }, { isComposing: true }, { keyCode: 229 }, { metaKey: true }, { ctrlKey: true }, { altKey: true }, { key: 'a' }]) {
    w.doc.activeElement = null; e = ev(o); assert.equal(w.searchSlash(e), false, JSON.stringify(Object.keys(o))); assert.equal(e.prevented, false); assert.equal(w.doc.activeElement, null);
  }
  for (const s of [{ modal: {} }, { navOpen: true }, { copyText: 'x' }, { menu: 'f-apps' }]) { Object.assign(w.S, { modal: null, navOpen: false, copyText: null, menu: null }, s); e = ev(); assert.equal(w.searchSlash(e), false, JSON.stringify(s)); assert.equal(e.prevented, false); }
  Object.assign(w.S, { modal: null, navOpen: false, copyText: null, menu: null }); w.env.trash = true;
  assert.equal(w.searchSlash(ev()), false, 'the Trash dialog');
});
test('the box shows the page filter: it is cleared where the page has none, and never changed under the caret', () => {
  const w = world({ view: 'overview', q: 'chr' }); w.q.value = 'chr';
  w.searchSyncBox(); assert.equal(w.q.value, '');
  w.S.view = 'apps'; w.searchSyncBox(); assert.equal(w.q.value, 'chr');
  w.q.focus(); w.q.value = 'chrx'; w.S.view = 'overview'; w.searchSyncBox(); assert.equal(w.q.value, 'chrx', 'focus is the person typing');
});
test('a click or a scroll anywhere in the menu does not take focus from the box', () => {
  const w = world(); w.q.focus(); type(w, 'chr');
  assert.ok(w.smenu.fire('mousedown').prevented);
});
test('blur closes the menu; clicking a result opens it', () => {
  const w = world(); w.q.focus(); type(w, 'slack');
  options(w)[0].fire('click'); assert.equal(w.smenu.hidden, true); assert.deepEqual(w.navs, [['push', '#apps?sel=other%3A%2FA%2FSlack']]);
  type(w, 'slack'); w.q.fire('blur'); assert.equal(w.smenu.hidden, true); assert.equal(w.q.attrs['aria-expanded'], 'false');
});
test('the Guide table is requested the first time the search runs, and not when the box is empty', () => {
  const w = world({ guide: { state: 'idle' } }); w.q.focus(); type(w, ''); assert.equal(w.calls.loadGuide, 0);
  type(w, 'snap'); assert.equal(w.calls.loadGuide, 1);
});
