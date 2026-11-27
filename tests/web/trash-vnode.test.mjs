// Spike: the Trash dialog as a plain function of state, tested without any DOM stand-in. `h` returns a vnode-shaped object.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { html } from './page.mjs';
const block = (name) => html.match(new RegExp('/\\* ' + name + ':begin \\*/([\\s\\S]*?)/\\* ' + name + ':end \\*/'))[1];
const code = ['url-state', 'requester', 'disk-net', 'page-shell', 'storage-page', 'storage-trash', 'storage-trash-view'].map(block).join('\n');
const flat = (k) => k.flat(Infinity).filter((c) => c != null && c !== false);
const h = (type, props, ...kids) => ({ type, props: props || {}, kids: flat(kids) });
const walk = (v) => (typeof v === 'object' ? [v, ...v.kids.flatMap(walk)] : []);
const text = (v) => (typeof v === 'object' ? v.kids.map(text).join(' ') : String(v));
const make = new Function('env', 'const { S, h, $, render, announce, api, loadStorage, labelTag, rec, document, setTimeout, vnodeMode } = env;\n' + code + '\nreturn { trashDialog };');
const S = { view: 'storage' };
const P = make({ S, h, $: () => null, render() {}, announce() {}, api: async () => { throw new Error('no'); }, loadStorage() {}, labelTag: (l) => h('span', {}, l), rec() {}, document: {}, setTimeout() {}, vnodeMode: () => true });
const HOSTILE = 'evil\u202egnp\nnext\u200bline <b>&';

test('the yours step escapes a hostile name and names it on the confirming button', () => {
  S.trash = { phase: 'confirming-yours', seq: 2, ids: ['a'], yours: [{ id: 'a', name: HOSTILE, path: '/Users/me/' + HOSTILE, label: 'yours', bytes: 1 }], known: {}, confirmYours: false, summary: null };
  const dlg = P.trashDialog(), nodes = walk(dlg), all = text(dlg) + nodes.map((n) => Object.values(n.props).filter((x) => typeof x === 'string').join(' ')).join(' ');
  assert.ok(!/[\u202e\u200b\n]/.test(all));
  assert.ok(nodes.filter((n) => n.type === 'button').some((b) => text(b).startsWith('Include') && text(b).includes('\\u{202e}')));
});

test('the dialog is a labelled modal, its heading is keyed by phase and carries a ref for focus', () => {
  const dlg = P.trashDialog(), nodes = walk(dlg), modal = nodes.find((n) => n.props.role === 'dialog'), heading = nodes.find((n) => n.type === 'h2');
  assert.equal(modal.props['aria-modal'], 'true');
  assert.equal(heading.props.key, 'confirming-yours');
  assert.equal(typeof heading.props.ref, 'function');
  assert.equal(typeof dlg.props.ref, 'function', 'the overlay ref sets inert and returns focus');
});
