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
const names = 'trashHistoryRows trashSelection trashSummary trashStep trashIdle trashUrl trashKey trashProblem trashHeadline TRASH_PUT_BACK TRASH_MAX_IDS'.split(' ');
const P = new Function(block('url-state') + block('requester') + block('disk-net') + block('page-shell') + block('storage-page') + block('storage-trash') + '\nreturn { ' + names.join(', ') + ' };')();

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
  assert.equal(P.trashHeadline(s.summary, true), '2 folders would move to the Trash (3.0 GB).');
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
  assert.equal(run(s, { type: 'selection', key: sel(2, 1).key }).s, s, 'the same ids in another order change nothing');
  const r = run(s, { type: 'selection', key: sel(1).key });
  assert.equal(r.s.phase, 'idle');
  assert.equal('token' in r.s, false);
  assert.match(r.fx.find((f) => f.do === 'announce').text, /selection or a label changed/);
  assert.ok(kinds(r.fx).includes('focus'), 'focus goes back');
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
  assert.equal(P.trashHeadline(sum, false), 'Moved 1 of 2 folders to the Trash (1.0 GB).');
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
  assert.equal(P.trashHeadline(r.s.summary, false), 'Nothing was moved: all 2 folders were refused.');
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
  const cases = [[409, 'busy', /another move to the Trash is still running/i], [409, 'confirmation_expired', /expired/], [409, 'confirmation_mismatch', /no longer applies/],
    [409, 'confirmation_invalid', /no longer valid/], [400, 'confirmation_required', /needs a preview/], [429, 'too_many_previews', /wait up to 10 minutes.*restart the dashboard/i]];
  for (const [status, code, re] of cases) {
    const s = previewed([1], [item(1, 'would_move')]);
    const m = run(s, { type: 'move' }).s;
    const e = run(m, reply(m, status, { ok: false, code, error: 'x' })).s;
    assert.equal(e.phase, 'error', code);
    assert.match(e.message.text, re, code);
    assert.match(e.message.text, /[Pp]review again|restart the dashboard/, code + ' names the way forward');
    assert.equal(e.message.retry, true, code);
    assert.equal(e.summary, null, 'nothing is shown as moved: ' + code);
  }
  const { s } = run(P.trashIdle(), open(1));
  const busy = run(s, reply(s, 429, { ok: false, code: 'too_many_previews' }));
  assert.equal(busy.s.phase, 'error');
  assert.equal(P.trashProblem(429, null, false).includes('10 minutes'), true);
  for (const code of ['audit_unavailable', 'home_untrusted']) {
    const t = P.trashProblem(500, code, true);
    assert.match(t, /^Nothing was moved\. The dashboard could not confirm .* Restart the dashboard and try again\.$/, code);
    assert.ok(!/trust/i.test(t), 'no jargon');
  }
  for (const code of ['busy', 'confirmation_expired', 'confirmation_mismatch', 'confirmation_invalid', 'confirmation_required', 'unknown_id', 'audit_unavailable', 'home_untrusted', 'too_many_previews']) {
    assert.ok((P.trashProblem(409, code, true).match(/Nothing was moved/g) || []).length <= 1, code + ' says it at most once');
  }
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

// ---- label flips, leaving the page, the server's second account of a request, and History rows
const yoursRow = { ...row(1), label: 'yours' };
test('a rescan that flips a label to YOURS clears the preview, because the key holds the label', () => {
  const before = P.trashSelection([row(1)], [hex(1)]), after = P.trashSelection([yoursRow], [hex(1)]);
  assert.notEqual(before.key, after.key);
  assert.equal(before.key, P.trashSelection([row(1)], [hex(1)]).key, 'the same selection has the same key');
  let { s } = run(P.trashIdle(), { type: 'open', selection: before });
  ({ s } = run(s, reply(s, 200, preview([item(1, 'would_move')]))));
  const r = run(s, { type: 'selection', key: after.key });
  assert.equal(r.s.phase, 'idle');
  assert.equal(run(r.s, { type: 'move' }).fx.length, 0, 'no commit can follow');
  assert.ok(kinds(r.fx).includes('focus'));
  assert.match(r.fx.find((f) => f.do === 'announce').text, /closed because the selection or a label changed/);
});

test('leaving the page closes the dialog with focus and a message, but never a move in flight', () => {
  const s = previewed([1], [item(1, 'would_move')]);
  const r = run(s, { type: 'leave' });
  assert.equal(r.s.phase, 'idle');
  assert.deepEqual(kinds(r.fx), ['focus']);
  assert.match(r.fx.find((f) => f.do === 'announce').text, /left the Storage page/);
  const moving = run(s, { type: 'move' }).s;
  assert.equal(run(moving, { type: 'leave' }).s, moving);
});

test('totals that disagree with the item list raise a visible warning', () => {
  const ok = commit([item(1, 'moved'), item(2, 'moved')], { counts: { moved: 2, refused: 0, failed: 0, not_attempted: 0, would_move: 0 }, failed: null, moved: [{}, {}] });
  assert.equal(P.trashSummary(ok, {}).warning, null);
  const bad = {
    'ok true but one refused': commit([item(1, 'moved'), item(2, 'refused')], { ok: true }),
    'ok false but all moved': commit([item(1, 'moved')], { ok: false }),
    'counts differ': commit([item(1, 'moved'), item(2, 'moved')], { counts: { moved: 1 } }),
    'failed set but none failed': commit([item(1, 'moved')], { failed: { id: hex(1) } }),
    'failed missing but one failed': commit([item(1, 'moved'), item(2, 'failed')], { ok: false, failed: null }),
    'moved list differs': commit([item(1, 'moved')], { moved: [] }),
  };
  for (const [name, body] of Object.entries(bad)) {
    const sum = P.trashSummary(body, {});
    assert.equal(sum.warning, 'The totals and the item list disagree, so this summary may be wrong. Open History to see what was recorded.', name);
  }
  assert.equal(P.trashSummary(commit([item(1, 'moved')], { ok: false, audit_error: 'disk full' }), {}).warning, null, 'ok false with an audit error is consistent');
  const s = previewed([1, 2], [item(1, 'would_move'), item(2, 'would_move')]);
  const m = run(s, { type: 'move' }).s;
  const r = run(m, reply(m, 200, commit([item(1, 'moved'), item(2, 'refused')], { ok: true })));
  assert.equal(r.s.phase, 'done');
  assert.ok(r.s.summary.warning, 'the done state carries it');
  assert.equal(r.s.summary.counts.moved, 1, 'the item list is still what is counted');
});

const HOSTILE = 'evil‮gnp.txt\nnext​line <b>&';
test('hostile names, reasons and notes are escaped wherever the machine hands them on', () => {
  const r = { ...row(1), name: HOSTILE, path: '/Users/me/' + HOSTILE };
  const selected = P.trashSelection([r], [hex(1)]);
  assert.ok(!/[‮\n​]/.test(selected.items[0].name + selected.items[0].path));
  assert.ok(selected.items[0].name.includes('\\u{202e}') && selected.items[0].name.includes('\\u{a}') && selected.items[0].name.includes('\\u{200b}'));
  assert.ok(selected.items[0].name.includes('<b>&'), 'markup characters stay plain text; the page writes text nodes');
  const sum = P.trashSummary(commit([{ id: hex(1), path: '/Users/me/' + HOSTILE, status: 'failed', code: 'a‮b', reason: 'x\ny​z', bytes: 1 }], { note: 'n‮', audit_error: 'e\u0007' }), {});
  const all = JSON.stringify([sum.items, sum.note, sum.auditError]);
  assert.ok(!/[‮​\u0007]|\\n/.test(all.replace(/\\\\n/g, '')), 'no raw control or format character survives');
  assert.ok(P.trashProblem(409, 'a‮b', true).includes('\\u{202e}'));
});

test('History rows: every state reads honestly, and nothing unknown is shown as moved', () => {
  const moves = [{ request: 7, ts: 100, bytes: 5, items: [
    { id: 'a', path: '~/a', label: 'safe', state: 'moved', code: null, reason: null, bytes: 5 },
    { id: 'b', path: '~/b', label: 'review', state: 'refused', code: 'changed', reason: 'changed since the scan', bytes: null },
    { id: 'c', path: '~/c', label: 'safe', state: 'interrupted', code: null, reason: 'the app stopped', bytes: null },
    { id: 'd', path: '~/d', label: 'safe', state: 'pending', code: null, reason: null, bytes: null },
    { id: 'e', path: '~/e', label: 'safe', state: 'moving', code: null, reason: null, bytes: null },
    { id: 'f', path: '~/f', label: 'safe', state: 'teleported‮', code: null, reason: null, bytes: null }] },
  { request: null, ts: 50, bytes: 0, items: [{ id: '', path: '', label: 'unknown', state: 'unreadable', code: null, reason: 'this old record is damaged', bytes: null }] }];
  const rows = P.trashHistoryRows(moves);
  assert.equal(rows.length, 7);
  assert.equal(rows[0].requestText, 'Request 7');
  assert.match(rows[2].stateText, /may or may not have moved\. Check your Trash/);
  assert.match(rows[3].stateText, /not started yet/);
  assert.match(rows[4].stateText, /stopped while moving\. Check your Trash/);
  assert.match(rows[5].stateText, /^Unknown state: teleported\\u\{202e\}$/);
  assert.equal(rows[6].requestText, 'Older record');
  assert.equal(rows[6].stateText, 'Could not read this record. Check your Trash');
  assert.equal(rows[6].path, '(no path)');
  assert.equal(rows[1].why, 'changed since the scan');
  assert.equal(rows[0].sizeText, '5 B');
  assert.equal(rows[1].sizeText, '-');
  assert.deepEqual(P.trashHistoryRows(null), []);
  assert.deepEqual(P.trashHistoryRows([null, { items: 'x' }, { request: 1, items: [null, 5] }]), []);
});

test('History rows escape a hostile folder name and reason', () => {
  const rows = P.trashHistoryRows([{ request: 1, ts: 1, items: [{ id: 'a', path: '~/' + HOSTILE, label: 'safe', state: 'refused', code: 'x', reason: HOSTILE, bytes: null }] }]);
  const text = JSON.stringify(rows[0]);
  assert.ok(!/[‮​]/.test(text) && !text.includes('\\n'.replace('\\n', '\n')), 'no raw bidi, zero-width or newline');
  assert.ok(rows[0].path.includes('\\u{202e}') && rows[0].why.includes('\\u{a}'));
});

test('the result summary matches the History rows built from the same request', () => {
  const items = [item(1, 'moved'), item(2, 'refused', { code: 'changed', reason: 'changed', bytes: null }), item(4, 'failed', { code: 'move_failed', reason: 'busy', bytes: null }), item(5, 'not_attempted', { code: 'earlier_move_failed', bytes: null })];
  const sum = P.trashSummary(commit(items, { ok: false, history_id: 12 }), {});
  // What GET /api/history returns for that request: the same items, with `state` where the reply says `status`.
  const moves = [{ request: sum.historyId, ts: 1, bytes: 0, items: items.map((i) => ({ id: i.id, path: i.path, label: i.label, state: i.status, code: i.code, reason: i.reason, bytes: i.bytes })) }];
  const rows = P.trashHistoryRows(moves);
  const tally = (st) => rows.filter((r) => r.state === st).length;
  for (const st of ['moved', 'refused', 'failed', 'not_attempted']) assert.equal(tally(st), sum.counts[st], st);
  assert.equal(rows.length, sum.items.length);
  assert.equal(rows.filter((r) => r.state === 'moved').reduce((n, r) => n + (r.bytes || 0), 0), sum.moved.bytes);
  assert.deepEqual(rows.map((r) => r.requestText), Array(4).fill('Request 12'));
  assert.deepEqual(rows.map((r) => r.state), sum.items.map((i) => i.status));
});
