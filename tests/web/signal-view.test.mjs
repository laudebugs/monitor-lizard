// The freeze/resume/stop layer of the Privilege page: the buttons under a hop and the dialog, drawn against a small stand-in
// DOM (no browser): hostile command lines, the two-step danger confirmation, one request per double click, focus, the inert
// page, and the dialog closing when its row is gone. Layout and the themes are checked by hand in a browser.
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { html } from './page.mjs';
const block = (name) => {
  const m = html.match(new RegExp('/\\* ' + name + ':begin \\*/([\\s\\S]*?)/\\* ' + name + ':end \\*/'));
  assert.ok(m, name + ' block not found in the dashboard scripts (web/*.js)');
  return m[1];
};
const code = ['requester', 'signal', 'signal-view'].map(block).join('\n');
const exported = 'sigOpen sigApply sigDispatch sigRequest sigClose sigDialog sigKeys sigLayer sigControls openSig sigFocusBack sigIdle sigKey loadReqLive'.split(' ');
const make = new Function('env', 'const { S, h, $, render, announce, api, quiet, document, setTimeout, writeHash, reqDomId } = env;\n' + code + '\nreturn { ' + exported.join(', ') + ' };');

class El {
  constructor(tag, doc) { this.tag = tag; this.attrs = {}; this.kids = []; this.on = {}; this.doc = doc; this.dataset = {}; }
  setAttribute(k, v) { this.attrs[k] = String(v); if (k.startsWith('data-')) this.dataset[k.slice(5)] = String(v); }
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
  buttons() { return this.all().filter((e) => e.tag === 'button'); }
}
function world(extra = {}) {
  const doc = { activeElement: null, els: [] }, ids = {}, timers = [], said = [], calls = { render: 0, api: [] };
  const shell = new El('div', doc);
  const nodesOf = (kids) => kids.flat(Infinity).filter((k) => k != null && k !== false);
  const h = (tag, attrs = {}, ...kids) => {
    const e = new El(tag, doc);
    for (const [k, v] of Object.entries(attrs)) {
      if (v == null || v === false) continue;
      if (k.startsWith('on')) e.addEventListener(k.slice(2).toLowerCase(), v); else e.setAttribute(k, v === true ? '' : v);
    }
    e.kids = nodesOf(kids).map((k) => (k instanceof El ? k : String(k)));
    if (attrs.id) ids[attrs.id] = e;
    doc.els.push(e);
    return e;
  };
  doc.querySelector = (sel) => (sel === '.shell' ? shell : sel === '[data-sig]' ? doc.els.find((e) => 'data-sig' in e.attrs) || null : null);
  doc.querySelectorAll = (sel) => { assert.equal(sel, '[data-sig]'); return doc.els.filter((e) => 'data-sig' in e.attrs); };
  const S = { view: 'priv', sig: undefined, reqOpen: { 'sudo:1:2': true }, reqLive: {}, sudo: [], priv: [] };
  const api = extra.api || (async (url, method) => { calls.api.push([url, method]); throw new Error('no api'); });
  const env = { S, h, document: doc, $: (id) => ids[id] || null, render: () => { calls.render++; }, announce: (t) => said.push(t), api, quiet: (m) => Object.assign(S, m),
    setTimeout: (f) => timers.push(f), writeHash: () => '#history', reqDomId: (id) => 'req-' + id.replace(/\W/g, '_') };
  return { P: make(env), S, said, timers, calls, shell, doc, ids, h };
}
const HOSTILE = 'evil\u202egnp\nnext\u200bline <b>&';
const BAD = /[\u202e\u200b\n]/;
const hop = (pid, command, exe, extra = {}) => ({ pid, command, exe, started: 100 + pid, detail: true, ...extra });
const chain = [hop(1, 'node ' + HOSTILE, '/opt/node'), hop(2, '-zsh', '/bin/zsh'), hop(3, 'login -pf me', '/usr/bin/login')];
const previewedState = (w, action, scope, i, body) => {
  w.P.openSig('sudo:1:2', chain, i, action, scope);
  const seq = w.S.sig.seq;
  return w.P.sigDispatch({ type: 'reply', seq, status: 200, body });
};
const body = (pids, extra = {}) => ({ ok: true, dry_run: true, confirm_token: 'T', items: pids.map((p) => ({ pid: p, status: 'would_signal', state: 'running', children: [], children_total: 0 })), ...extra });

test('buttons: a running hop offers every action but resume; a frozen one shows the badge and offers resume but not freeze', () => {
  const w = world();
  w.S.reqLive[w.P.sigKey(chain[0])] = { state: 'running' };
  w.S.reqLive[w.P.sigKey(chain[1])] = { state: 'frozen' };
  const label = (el) => Object.fromEntries(el.buttons().filter((b) => b.dataset.sig.endsWith(':one')).map((b) => [b.text.trim(), !('disabled' in b.attrs)]));
  const a = w.P.sigControls('sudo:1:2', chain, 0), b = w.P.sigControls('sudo:1:2', chain, 1);
  assert.deepEqual(label(a), { Freeze: true, Resume: false, Stop: true, 'Force stop': true });
  assert.deepEqual(label(b), { Freeze: false, Resume: true, Stop: true, 'Force stop': true });
  assert.ok(!/Frozen/.test(a.allText().replace(/Freeze/g, '')) && b.all().some((e) => e.tag === 'span' && e.text === 'Frozen'));
  assert.equal(a.buttons().length, 4, 'the first hop has no chain row: the chain is itself');
  assert.equal(b.buttons().length, 8);
});

test('buttons: exited, reused, unrecorded and not-yet-checked hops are disabled with the reason beside them', () => {
  const w = world();
  const sig = (i) => w.P.sigControls('sudo:1:2', chain, i);
  w.S.reqLive[w.P.sigKey(chain[0])] = { code: 'gone' };
  w.S.reqLive[w.P.sigKey(chain[1])] = { code: 'pid_reused' };
  const gone = sig(0), reused = sig(1), checking = sig(2);
  assert.ok(gone.buttons().every((b) => 'disabled' in b.attrs));
  assert.match(gone.text, /Already exited, nothing to freeze/);
  assert.match(reused.text, /This pid now belongs to a different process/);
  assert.match(checking.text, /Checking whether it is still running/);
  const note = gone.all().find((e) => /-note$/.test(e.attrs.id || ''));
  assert.ok(note && gone.buttons().every((b) => b.attrs['aria-describedby'] === note.attrs.id), 'a disabled button points at its reason');
  const bare = w.P.sigControls('sudo:1:2', [{ pid: 9, command: 'x', exe: '/x' }], 0);
  assert.match(bare.text, /No start time or executable was recorded/);
  assert.equal(gone.attrs.role, 'group');
  assert.match(gone.attrs['aria-label'], /pid 1/);
});

test('buttons: every command is escaped and no label carries a raw bidi or newline character', () => {
  const w = world();
  w.S.reqLive[w.P.sigKey(chain[0])] = { state: 'running' };
  const all = w.P.sigControls('sudo:1:2', chain, 0).allText();
  assert.ok(!BAD.test(all) && all.includes('\\u{202e}'));
});

test('the dialog names the full command line, escaped, grades each process and shows what runs under it', () => {
  const w = world();
  w.S.reqLive[w.P.sigKey(chain[0])] = { state: 'running' };
  const b = body([1, 2]);
  b.items[0].children = [{ pid: 7, command: 'child ' + HOSTILE, frozen: true }];
  b.items[0].children_total = 9;
  previewedState(w, 'stop', 'chain', 1, b);
  const dlg = w.P.sigDialog(), all = dlg.allText();
  assert.ok(!BAD.test(all), 'no raw character anywhere in text or attributes');
  assert.ok(all.includes('\\u{202e}') && all.includes('<b>&'));
  assert.match(all, /Stop 2 processes in this chain\?/);
  assert.match(all, /Your session/);
  assert.match(all, /and 8 more/);
  assert.match(all, /your own session/);
  const box = dlg.all().find((e) => e.attrs.role === 'dialog');
  assert.equal(box.attrs['aria-modal'], 'true');
  assert.equal(box.attrs['aria-labelledby'], 'sig-title');
  assert.ok(dlg.querySelector('[data-autofocus]'));
  assert.equal(w.P.sigDialog.name, 'sigDialog');
});

test('a danger action takes two confirmations, previews again with the flag, and sends once; nothing is sent before the second', async () => {
  const w = world({ api: async (url, m) => {
    w.calls.api.push([url, m]);
    const real = url.includes('dry=0'), flagged = url.includes('confirm_danger=1');
    const b = body([2], { dry_run: !real, confirm_token: flagged && !real ? 'T' : null });
    if (!flagged) { b.items[0].status = 'refused'; b.items[0].code = 'needs_danger_confirmation'; }
    if (real) b.items[0].status = 'signalled';
    return { status: 200, json: async () => b };
  } });
  w.P.openSig('sudo:1:2', chain, 1, 'freeze', 'one');
  await new Promise((r) => setTimeout(r, 0));
  assert.equal(w.calls.api.length, 1, 'the preview is the only request so far');
  let dlg = w.P.sigDialog();
  const go = dlg.buttons().find((b) => b.text === 'Continue');
  assert.ok(go && !('disabled' in go.attrs), 'Continue works with no token yet: the server holds your session back until the second confirmation');
  go.on.click();
  assert.equal(w.S.sig.phase, 'confirming-danger');
  assert.equal(w.calls.api.length, 1, 'still only the first preview');
  dlg = w.P.sigDialog();
  assert.match(dlg.allText(), /Freeze your own session\?/);
  const yes = dlg.buttons().find((b) => /^Yes, freeze/.test(b.text));
  yes.on.click(); yes.on.click(); yes.on.click();
  await new Promise((r) => setTimeout(r, 0)); await new Promise((r) => setTimeout(r, 0));
  assert.equal(w.calls.api.length, 3, 'one flagged preview and one real request, however many clicks');
  assert.match(w.calls.api[1][0], /confirm_danger=1&dry=1$/);
  assert.match(w.calls.api[2][0], /confirm_danger=1&dry=0&confirm_token=T$/);
  assert.equal(w.calls.api[2][1], 'POST');
  assert.ok(!w.calls.api.some(([u]) => u.includes('#')));
});

test('a plain action needs one confirmation and the button says how many processes', () => {
  const w = world();
  w.S.reqLive[w.P.sigKey({ ...chain[2], command: 'git status', exe: '/usr/bin/git' })] = { state: 'running', children_total: 0 };
  w.P.openSig('sudo:1:2', [chain[2]].map((x) => ({ ...x, command: 'git status', exe: '/usr/bin/git' })), 0, 'force_stop', 'one');
  w.P.sigDispatch({ type: 'reply', seq: w.S.sig.seq, status: 200, body: body([3]) });
  assert.ok(w.P.sigDialog().buttons().some((b) => b.text === 'Force stop 1 process'));
});

test('a refused preview disables the action and says why for each process', () => {
  const w = world();
  w.P.openSig('sudo:1:2', chain, 0, 'freeze', 'one');
  w.P.sigDispatch({ type: 'reply', seq: w.S.sig.seq, status: 200, body: { ok: false, dry_run: true, confirm_token: null, items: [{ pid: 1, status: 'refused', code: 'root_owned', reason: 'Owned by root\u202e', children: [] }] } });
  const dlg = w.P.sigDialog();
  const act = dlg.buttons().find((b) => /Nothing can be/.test(b.text));
  assert.ok(act && 'disabled' in act.attrs);
  assert.ok(dlg.allText().includes('Owned by root\\u{202e}') && !/\u202e/.test(dlg.allText()));
});

test('Tab stays inside the dialog and Shift+Tab from the heading goes to the last button', () => {
  const w = world();
  w.P.openSig('sudo:1:2', chain, 0, 'freeze', 'one');
  w.P.sigDispatch({ type: 'reply', seq: w.S.sig.seq, status: 200, body: body([1]) });
  const dlg = w.P.sigDialog(), box = dlg.all().find((e) => e.attrs.role === 'dialog'), f = box.querySelectorAll('button:not([disabled]), a[href]');
  let prevented = 0; const ev = (shiftKey) => ({ key: 'Tab', shiftKey, currentTarget: box, preventDefault: () => prevented++ });
  w.doc.activeElement = f.at(-1); w.P.sigKeys(ev(false));
  assert.equal(w.doc.activeElement, f[0], 'wraps forward');
  w.doc.activeElement = box.querySelector('[data-autofocus]'); w.P.sigKeys(ev(true));
  assert.equal(w.doc.activeElement, f.at(-1), 'wraps backward from the heading');
  assert.equal(prevented, 2);
});

test('the page behind the dialog is inert while it is open, and the dialog closes when its row closes or the page changes', () => {
  const w = world();
  w.P.openSig('sudo:1:2', chain, 0, 'freeze', 'one');
  assert.ok(w.P.sigLayer());
  assert.equal(w.shell.inert, true);
  w.S.reqOpen = {};
  assert.equal(w.P.sigLayer(), null, 'the row is no longer open');
  assert.equal(w.S.sig.phase, 'idle');
  assert.match(w.said.at(-1), /selection changed\. Nothing was signalled/);
  assert.equal(w.timers.length, 1, 'focus goes back after the render');
  w.S.reqOpen = { 'sudo:1:2': true };
  w.P.openSig('sudo:1:2', chain, 0, 'freeze', 'one');
  w.S.view = 'guide';
  w.P.sigLayer();
  assert.match(w.said.at(-1), /left the Privilege page/);
  assert.equal(w.P.sigOpen(), false);
});

test('focus returns to the button that opened the dialog', () => {
  const w = world();
  w.S.reqLive[w.P.sigKey(chain[0])] = { state: 'running' };
  const ctl = w.P.sigControls('sudo:1:2', chain, 0);
  const opener = ctl.buttons().find((b) => b.dataset.sig.endsWith(':stop:one'));
  opener.on.click();
  w.P.sigDispatch({ type: 'close' });
  assert.equal(w.doc.activeElement, opener);
});

test('a dialog that cannot be drawn is closed with a message instead of freezing the page', () => {
  let boom = false;
  const w = world();
  w.P.openSig('sudo:1:2', chain, 0, 'freeze', 'one');
  const real = w.S.sig;
  w.S.sig = { ...real, items: null }; // drawing a dialog with no items throws
  assert.equal(w.P.sigLayer(), null);
  assert.equal(w.S.sig.phase, 'idle');
  assert.match(w.said.at(-1), /could not be drawn and was closed\. Nothing was signalled\./);
  assert.equal(boom, false);
});

test('background polls are held while the dialog is open (the quiet() guard)', () => {
  assert.match(html, /!trashOpen\(\) && !sigOpen\(\)\) render\(\);/);
});
