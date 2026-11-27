// The Move to Trash state machine: every rule of the preview-then-confirm flow, with no DOM and no network.
// The test lifts the `storage-trash` block out of the dashboard scripts (web/*.js) with the blocks it reads.
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { html } from './page.mjs';
const block = (name) => {
  const m = html.match(new RegExp('/\\* ' + name + ':begin \\*/([\\s\\S]*?)/\\* ' + name + ':end \\*/'));
  assert.ok(m, name + ' block not found in the dashboard scripts (web/*.js)');
  return m[1];
};
const names = 'trashSelection trashSummary trashStep trashIdle trashUrl trashKey trashProblem trashHeadline TRASH_PUT_BACK TRASH_MAX_IDS'.split(' ');
const P = new Function(block('url-state') + block('disk-net') + block('page-shell') + block('storage-page') + block('storage-trash') + '\nreturn { ' + names.join(', ') + ' };')();

const hex = (n) => n.toString(16).padStart(16, '0');
const row = (n, extra = {}) => ({ kind: 'item', id: hex(n), name: 'cache' + n, path: '/Users/me/Library/Caches/c' + n, label: 'safe', bytes: n * 1e9, lowerBound: false, ...extra });
const rows = [row(1), row(2), row(3, { label: 'yours', path: '/Users/me/Documents/Taxes', name: 'Taxes' }), row(4, { label: 'weird' }), { kind: 'pending', id: hex(5), name: 'later', path: '/x', label: 'review' }];
const sel = (...ns) => P.trashSelection(rows, ns.map(hex));
const item = (n, status, extra = {}) => ({ id: hex(n), path: '/Users/me/Library/Caches/c' + n, label: 'safe', status, code: null, reason: null, bytes: n * 1e9, ...extra });
const preview = (items, extra = {}) => ({ ok: true, dry_run: true, history_id: null, items, confirm_token: 'tok1', note: 'n', ...extra });
const commit = (items, extra = {}) => ({ ok: true, dry_run: false, history_id: 42, items, confirm_token: null, note: 'Server note', ...extra });
const run = (s, ...evs) => { const fx = []; for (const e of evs) { const r = P.trashStep(s, e); s = r.state; fx.push(...r.fx); } return { s, fx }; };
const kinds = (fx) => fx.filter((f) => f.do !== 'announce').map((f) => f.do);
const open = (...ns) => ({ type: 'open', selection: sel(...ns) });
const reply = (s, status, body) => ({ type: 'reply', seq: s.seq, status, body });
// Walk to a shown preview for the given ids.
const previewed = (ns, items, extra) => { let { s } = run(P.trashIdle(), open(...ns)); ({ s } = run(s, reply(s, 200, preview(items, extra)))); return s; };

test('an idle page with no previewable selection stays inert and says why', () => {
  assert.equal(sel().ok, false);
  assert.equal(sel().why, 'Select a folder first');
  assert.equal(P.trashSelection(rows, [hex(5)]).ok, false, 'a location the scan did not reach has nothing to move');
  assert.equal(P.trashSelection(rows, [hex(9)]).ok, false, 'an id not in the latest scan');
  assert.equal(P.trashSelection([{ ...row(1), id: 'p12345678' }], ['p12345678']).ok, false, 'a page-made id is not a server id');
  const many = Array.from({ length: 51 }, (_, i) => row(i + 10));
  assert.equal(P.trashSelection(many, many.map((r) => r.id)).ok, false);
  const { s, fx } = run(P.trashIdle(), open());
  assert.equal(s.phase, 'idle');
  assert.deepEqual(kinds(fx), []);
});

test('opening sends exactly one preview for the selection, with no token and dry=1', () => {
  const { s, fx } = run(P.trashIdle(), open(1, 2));
  assert.equal(s.phase, 'previewing');
  const req = fx.find((f) => f.do === 'preview');
  assert.deepEqual(req.ids, [hex(1), hex(2)]);
  assert.equal(P.trashUrl(req), '/api/trash?ids=' + hex(1) + ',' + hex(2) + '&dry=1');
  assert.equal(req.token, undefined);
  assert.equal(run(s, open(1, 2)).fx.length, 0, 'a second open while previewing starts nothing');
});

test('a label the page does not trust is shown as review, never safe', () => {
  assert.equal(sel(4).items[0].label, 'review');
  assert.deepEqual(sel(1, 3).yours.map((y) => y.name), ['Taxes']);
});

test('the preview shows sizes and keeps the token only for movable items', () => {
  const s = previewed([1, 2], [item(1, 'would_move'), item(2, 'would_move')]);
  assert.equal(s.phase, 'previewed');
  assert.equal(s.token, 'tok1');
  assert.equal(s.summary.counts.would_move, 2);
  assert.equal(P.trashHeadline(s.summary, true), '2 folders would move to the Trash, 3.0 GB.');
  const none = previewed([1], [item(1, 'refused', { code: 'symlink', reason: 'is a symlink' })], { ok: false, confirm_token: null });
  assert.equal(none.phase, 'previewed');
  assert.equal(none.token, null, 'nothing movable has no token');
  assert.equal(P.trashStep(none, { type: 'move' }).fx.length, 0, 'Move does nothing without a token');
  assert.match(P.trashHeadline(none.summary, true), /None of the 1 folder can be moved/);
  const stray = previewed([1], [item(1, 'refused')], { confirm_token: 'leftover' });
  assert.equal(stray.token, null, 'a token with nothing movable is not kept');
});

test('Move sends the commit with the same ids and the token, once', () => {
  const s = previewed([1, 2], [item(1, 'would_move'), item(2, 'would_move')]);
  const a = run(s, { type: 'move' });
  assert.equal(a.s.phase, 'moving');
  const req = a.fx.find((f) => f.do === 'commit');
  assert.equal(P.trashUrl(req), '/api/trash?ids=' + hex(1) + ',' + hex(2) + '&dry=0&confirm_token=tok1');
  const b = run(a.s, { type: 'move' }, { type: 'move' });
  assert.deepEqual(kinds(b.fx), [], 'double click: still one request');
  assert.equal(b.s, a.s);
});

test('a token is not carried when the page asks again or the dialog closes', () => {
  const s = previewed([1], [item(1, 'would_move')]);
  const closed = run(s, { type: 'close' }).s;
  assert.equal(closed.phase, 'idle');
  assert.equal('token' in closed, false);
  assert.equal(P.trashStep(closed, { type: 'move' }).fx.length, 0, 'Move after close does nothing');
});

test('changing the selection clears the preview and the token', () => {
  const s = previewed([1, 2], [item(1, 'would_move'), item(2, 'would_move')]);
  assert.equal(run(s, { type: 'selection', ids: [hex(2), hex(1)] }).s, s, 'the same ids in another order change nothing');
  const r = run(s, { type: 'selection', ids: [hex(1)] });
  assert.equal(r.s.phase, 'idle');
  assert.equal('token' in r.s, false);
  assert.match(r.fx[0].text, /selection changed/);
  assert.equal(run(r.s, { type: 'move' }).fx.length, 0);
});

test('a late reply for a cleared or older preview is ignored', () => {
  const { s } = run(P.trashIdle(), open(1));
  const stale = { type: 'reply', seq: s.seq, status: 200, body: preview([item(1, 'would_move')]) };
  const closed = run(s, { type: 'close' }).s;
  assert.equal(P.trashStep(closed, stale).state, closed, 'the dialog was closed meanwhile');
  assert.equal(P.trashStep(s, { ...stale, seq: s.seq - 1 }).state, s, 'a reply for an older request');
});

test('Escape closes the dialog without a request, except while a move is running', () => {
  const s = previewed([1], [item(1, 'would_move')]);
  const r = run(s, { type: 'close' });
  assert.equal(r.s.phase, 'idle');
  assert.deepEqual(kinds(r.fx), ['focus']);
  const moving = run(s, { type: 'move' }).s;
  const again = run(moving, { type: 'close' });
  assert.equal(again.s, moving);
  assert.match(again.fx[0].text, /cannot be cancelled/);
});

test('yours items need a second confirmation that names the folder and sends confirm_yours on both calls', () => {
  const first = previewed([1, 3], [item(1, 'would_move'), item(3, 'refused', { code: 'needs_yours_confirmation', reason: 'needs your confirmation' })]);
  assert.equal(first.confirmYours, false);
  assert.equal(P.trashUrl({ ids: first.ids, confirmYours: false }).includes('confirm_yours'), false);
  const a = run(first, { type: 'move' });
  assert.equal(a.s.phase, 'confirming-yours');
  assert.deepEqual(kinds(a.fx), [], 'no request yet');
  assert.deepEqual(a.s.yours.map((y) => y.path), ['/Users/me/Documents/Taxes'], 'the folder is named');
  const b = run(a.s, { type: 'yours-yes' });
  assert.equal(b.s.phase, 'previewing');
  assert.equal(b.s.token, null, 'the first token is dropped');
  const req = b.fx.find((f) => f.do === 'preview');
  assert.match(P.trashUrl(req), /&confirm_yours=1&dry=1$/);
  const c = run(b.s, reply(b.s, 200, preview([item(1, 'would_move'), item(3, 'would_move', { label: 'yours' })], { confirm_token: 'tok2' })));
  assert.equal(c.s.confirmYours, true);
  const d = run(c.s, { type: 'move' });
  assert.equal(d.s.phase, 'moving', 'no third prompt');
  assert.match(P.trashUrl(d.fx.find((f) => f.do === 'commit')), /&confirm_yours=1&dry=0&confirm_token=tok2$/);
});

test('going back from the yours confirmation sends nothing and keeps the preview', () => {
  const first = previewed([3], [item(3, 'refused', { code: 'needs_yours_confirmation' })], { ok: false, confirm_token: null });
  const a = run(first, { type: 'move' });
  assert.equal(a.s.phase, 'confirming-yours', 'a yours-only selection can still continue; the token comes after the confirmation');
  const b = run(a.s, { type: 'yours-back' });
  assert.equal(b.s.phase, 'previewed');
  assert.deepEqual(kinds(b.fx), []);
  assert.equal(run(first, { type: 'yours-yes' }).s, first, 'yes outside the confirmation does nothing');
});

test('a refusal lists the item with its reason while the others still move', () => {
  const s = previewed([1, 2], [item(1, 'would_move'), item(2, 'refused', { code: 'changed', reason: 'changed since the scan' })], { ok: false });
  const m = run(s, { type: 'move' });
  const r = run(m.s, reply(m.s, 200, commit([item(1, 'moved'), item(2, 'refused', { code: 'changed', reason: 'changed since the scan', bytes: null })], { ok: false })));
  assert.equal(r.s.phase, 'done');
  const sum = r.s.summary;
  assert.deepEqual([sum.counts.moved, sum.counts.refused, sum.counts.failed, sum.counts.not_attempted], [1, 1, 0, 0]);
  assert.equal(sum.items[1].reason, 'changed since the scan');
  assert.equal(sum.historyId, 42);
  assert.equal(P.trashHeadline(sum, false), 'Moved 1 of 2 items to the Trash, 1.0 GB.');
  assert.ok(kinds(r.fx).includes('rescan'), 'a rescan follows');
  assert.equal('token' in r.s && r.s.token, null);
});

test('a partial failure counts only what moved and lists the rest', () => {
  const s = previewed([1, 2, 4], [item(1, 'would_move'), item(2, 'would_move'), item(4, 'would_move')]);
  const m = run(s, { type: 'move' }).s;
  const r = run(m, reply(m, 200, commit([item(1, 'moved'), item(2, 'failed', { code: 'move_not_permitted', reason: 'not permitted' }), item(4, 'not_attempted', { code: 'earlier_move_failed', bytes: null })], { ok: false })));
  assert.deepEqual([r.s.summary.counts.moved, r.s.summary.counts.failed, r.s.summary.counts.not_attempted], [1, 1, 1]);
  assert.equal(r.s.summary.moved.bytes, 1e9);
});

test('a status the page does not know is never counted as moved', () => {
  const sum = P.trashSummary(commit([{ id: hex(1), status: 'surprise', bytes: 5 }, { id: hex(2), status: 'moved', bytes: 7 }]), {});
  assert.equal(sum.counts.moved, 1);
  assert.equal(sum.counts.failed, 1);
});

test('ok:false with everything refused is shown as nothing moved', () => {
  const s = previewed([1, 2], [item(1, 'would_move'), item(2, 'would_move')]);
  const m = run(s, { type: 'move' }).s;
  const r = run(m, reply(m, 200, commit([item(1, 'refused', { code: 'changed', reason: 'x' }), item(2, 'refused', { code: 'gone', reason: 'y' })], { ok: false, history_id: 7 })));
  assert.equal(r.s.phase, 'done');
  assert.equal(P.trashHeadline(r.s.summary, false), 'Nothing was moved: all 2 items were refused.');
  assert.equal(r.s.summary.okFlag, false);
});

test('a network error shows nothing as moved and offers a new preview', () => {
  const { s } = run(P.trashIdle(), open(1));
  const e = run(s, reply(s, 0, null));
  assert.equal(e.s.phase, 'error');
  assert.equal(e.s.summary, null);
  assert.match(e.s.message.text, /Nothing was moved/);
  const again = run(e.s, { type: 'again' });
  assert.equal(again.s.phase, 'previewing');
  assert.equal(again.s.confirmYours, false, 'the yours confirmation is asked for again');
});

test('a network error on the commit says it cannot tell, and shows nothing as moved', () => {
  const s = previewed([1], [item(1, 'would_move')]);
  const m = run(s, { type: 'move' }).s;
  const e = run(m, reply(m, 0, null)).s;
  assert.equal(e.phase, 'error');
  assert.equal(e.summary, null);
  assert.match(e.message.text, /cannot say whether anything moved/);
  assert.match(e.message.text, /Nothing is shown as moved/);
});

test('each server refusal gets a plain message and a way forward', () => {
  const cases = [[409, 'busy', /another removal is still running/i], [409, 'confirmation_expired', /expired/], [409, 'confirmation_mismatch', /no longer applies/],
    [409, 'confirmation_invalid', /no longer valid/], [400, 'confirmation_required', /needs a preview/], [429, 'too_many_previews', /preview again/i]];
  for (const [status, code, re] of cases) {
    const s = previewed([1], [item(1, 'would_move')]);
    const m = run(s, { type: 'move' }).s;
    const e = run(m, reply(m, status, { ok: false, code, error: 'x' })).s;
    assert.equal(e.phase, 'error', code);
    assert.match(e.message.text, re, code);
    assert.match(e.message.text, /[Pp]review again/, code + ' names the way forward');
    assert.equal(e.message.retry, true, code);
    assert.equal(e.summary, null, 'nothing is shown as moved: ' + code);
  }
  const { s } = run(P.trashIdle(), open(1));
  const busy = run(s, reply(s, 429, { ok: false, code: 'too_many_previews' }));
  assert.equal(busy.s.phase, 'error');
  assert.equal(P.trashProblem(429, null, false).includes('10 minutes'), true);
});

test('a scan that no longer has the folder offers no blind retry', () => {
  const { s } = run(P.trashIdle(), open(1));
  const e = run(s, reply(s, 400, { ok: false, code: 'unknown_id', error: 'x' })).s;
  assert.equal(e.message.retry, false);
  assert.match(e.message.text, /Rescan/);
});

test('a 500 with a body shows what moved; a 500 without one shows nothing moved', () => {
  const s = previewed([1, 2], [item(1, 'would_move'), item(2, 'would_move')]);
  const m = run(s, { type: 'move' }).s;
  const r = run(m, reply(m, 500, commit([item(1, 'moved'), item(2, 'failed', { code: 'audit_failed', reason: 'could not write History', bytes: null })], { ok: false, audit_error: 'disk full' })));
  assert.equal(r.s.phase, 'done');
  assert.equal(r.s.summary.counts.moved, 1);
  assert.equal(r.s.summary.auditError, 'disk full');
  const bare = run(m, reply(m, 500, { ok: false, code: 'audit_unavailable', error: 'x' })).s;
  assert.equal(bare.phase, 'error');
  assert.equal(bare.summary, null);
  const blank = run(m, reply(m, 500, null)).s;
  assert.equal(blank.phase, 'error');
  assert.match(blank.message.text, /Nothing is shown as moved/);
});

test('a 200 reply that cannot be read is not treated as a move', () => {
  const s = previewed([1], [item(1, 'would_move')]);
  const m = run(s, { type: 'move' }).s;
  const e = run(m, reply(m, 200, 'garbage')).s;
  assert.equal(e.phase, 'error');
  assert.match(e.message.text, /could not be read/);
});

test('a preview reply that reports a real move is shown honestly as a result', () => {
  const { s } = run(P.trashIdle(), open(1));
  const r = run(s, reply(s, 200, commit([item(1, 'moved')])));
  assert.equal(r.s.phase, 'done');
  assert.equal(r.s.summary.counts.moved, 1);
});

test('a new preview can start after a result, and the old one is gone', () => {
  const s = previewed([1], [item(1, 'would_move')]);
  const m = run(s, { type: 'move' }).s;
  const d = run(m, reply(m, 200, commit([item(1, 'moved')]))).s;
  const n = run(d, open(2));
  assert.equal(n.s.phase, 'previewing');
  assert.deepEqual(n.s.ids, [hex(2)]);
  assert.equal(n.s.summary, null);
});

test('the Put Back note is always present and server text is read as plain data', () => {
  assert.match(P.TRASH_PUT_BACK, /Put Back may not be offered/);
  assert.match(P.TRASH_PUT_BACK, /History keeps the original path/);
  const sum = P.trashSummary(commit([item(1, 'moved')], { note: '' }), {});
  assert.equal(sum.note, P.TRASH_PUT_BACK);
  const evil = P.trashSummary(preview([{ id: '<img onerror=x>', path: '/Users/me/<b>', status: 'refused', reason: '<script>', bytes: 1 }]), {});
  assert.equal(evil.items[0].reason, '<script>', 'kept as text; the page builds it with textContent');
  assert.equal(P.trashUrl({ ids: ['a&b=c'], confirmYours: false }), '/api/trash?ids=a%26b%3Dc&dry=1', 'ids are encoded');
});
