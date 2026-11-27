// Logic tests for the freeze/resume/stop dialog on the Privilege page: the severity classifier, which buttons a hop's
// live state allows, and the dialog state machine. They lift the `requester` and `signal` blocks out of the dashboard
// scripts (web/*.js) and run them alone, so those blocks must stay free of DOM and storage access.
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { html } from './page.mjs';
const block = (name) => {
  const m = html.match(new RegExp('/\\* ' + name + ':begin \\*/([\\s\\S]*?)/\\* ' + name + ':end \\*/'));
  assert.ok(m, name + ' block not found in the dashboard scripts (web/*.js)');
  return m[1];
};
const names = 'SIG_ACTIONS SIG_MAX sigSeverity sigLevel sigKey sigItem sigItems sigAvail sigChainAvail sigUrl sigSummary sigProblem sigStep sigIdle sigTitle sigWarning sigHeadline sigSelectionKey'.split(' ');
const S = new Function(block('requester') + '\n' + block('signal') + '\nreturn { ' + names.join(', ') + ' };')();

const hop = (command, exe = '', extra = {}) => ({ pid: 10, exe, command, started: 100, ...extra });
const level = (command, exe, kids = 0) => S.sigSeverity(hop(command, exe), kids).level;

test('severity: shells, login, the terminal app and claude are the owner\'s own session', () => {
  for (const [cmd, exe] of [['-zsh', '/bin/zsh'], ['/bin/bash -l', '/bin/bash'], ['fish', '/opt/homebrew/bin/fish'], ['login -pf me', '/usr/bin/login'], ['sh', '/bin/sh'],
    ['/System/Applications/Utilities/Terminal.app/Contents/MacOS/Terminal', '/System/Applications/Utilities/Terminal.app/Contents/MacOS/Terminal'],
    ['claude --resume', '/Users/me/.local/bin/claude'], ['node /usr/lib/claude/cli.js', '/opt/homebrew/bin/node'], ['tmux new', '/opt/homebrew/bin/tmux']]) {
    const r = S.sigSeverity(hop(cmd, exe), 0);
    assert.equal(r.level, 'danger', cmd);
    assert.ok(r.why.length > 0, 'says why: ' + cmd);
  }
});

test('severity: a script is caution, a leaf program is plain, children raise plain to caution but never lower danger', () => {
  assert.equal(level('node script.js', '/opt/homebrew/bin/node'), 'caution');
  assert.equal(level('python3 tool.py', '/usr/bin/python3'), 'caution');
  assert.equal(level('/usr/bin/git status', '/usr/bin/git'), 'plain');
  assert.equal(level('/usr/bin/git status', '/usr/bin/git', 3), 'caution');
  assert.deepEqual(S.sigSeverity(hop('make', '/usr/bin/make'), 2).why, ['it has 2 child processes']);
  assert.equal(level('-zsh', '/bin/zsh', 9), 'danger');
  assert.equal(S.sigSeverity(hop('make', '/usr/bin/make'), undefined).level, 'plain', 'unknown children count is not a reason');
  assert.equal(level('', ''), 'plain', 'a hop with nothing recorded does not throw');
});

test('severity: a program that merely starts with the same letters is not the terminal or claude', () => {
  assert.equal(level('/Applications/Terminalizer.app/Contents/MacOS/x', '/Applications/Terminalizer.app/Contents/MacOS/x'), 'plain');
  assert.equal(level('claudette --go', '/usr/local/bin/claudette'), 'plain');
});

test('level: a chain takes the strongest item, and resume never asks for more', () => {
  const items = [{ level: 'plain' }, { level: 'caution' }, { level: 'danger' }];
  assert.equal(S.sigLevel('freeze', items), 'danger');
  assert.equal(S.sigLevel('stop', items.slice(0, 2)), 'caution');
  assert.equal(S.sigLevel('force_stop', items.slice(0, 1)), 'plain');
  assert.equal(S.sigLevel('resume', items), 'plain');
});

test('items: one hop, or the chain from the process that ran sudo up to the chosen hop, child first', () => {
  const chain = [hop('git', '/usr/bin/git', { pid: 1 }), hop('-zsh', '/bin/zsh', { pid: 2 }), hop('login', '/usr/bin/login', { pid: 3 })];
  assert.deepEqual(S.sigItems(chain, 1, 'one', {}).map((i) => i.pid), [2]);
  assert.deepEqual(S.sigItems(chain, 1, 'chain', {}).map((i) => i.pid), [1, 2]);
  assert.deepEqual(S.sigItems(chain, 2, 'chain', {}).map((i) => i.level), ['plain', 'danger', 'danger']);
  const live = { [S.sigKey(chain[0])]: { children_total: 2 } };
  assert.equal(S.sigItems(chain, 0, 'one', live)[0].level, 'caution', 'children from the live status count');
});

test('availability: exited, reused, frozen, running, unreadable and not yet checked', () => {
  const h = hop('sleep 9', '/bin/sleep');
  const a = (live, action) => S.sigAvail(h, live, action);
  assert.deepEqual(a({ code: 'gone' }, 'freeze'), { ok: false, reason: 'Already exited, nothing to freeze' });
  assert.equal(a({ code: 'gone' }, 'force_stop').reason, 'Already exited, nothing to force stop');
  assert.equal(a({ code: 'pid_reused' }, 'stop').reason, 'This pid now belongs to a different process');
  assert.deepEqual(a({ state: 'frozen' }, 'freeze'), { ok: false, reason: 'Already frozen. Resume it instead' });
  assert.equal(a({ state: 'frozen' }, 'resume').ok, true);
  assert.equal(a({ state: 'frozen' }, 'stop').ok, true);
  assert.equal(a({ code: 'already_frozen', state: 'frozen' }, 'resume').ok, true, 'the freeze preview says already_frozen; resume is still offered');
  assert.equal(a({ state: 'running' }, 'freeze').ok, true);
  assert.equal(a({ state: 'running' }, 'resume').ok, false);
  assert.equal(a({ code: 'root_owned', reason: 'Owned by root\u202e' }, 'stop').reason, 'Owned by root\\u{202e}', 'a server reason is escaped');
  assert.equal(a(undefined, 'freeze').checking, true);
  assert.equal(S.sigAvail({ pid: 1, exe: '/x' }, { state: 'running' }, 'freeze').ok, false, 'no recorded start time');
  assert.equal(S.sigAvail({ pid: 1, started: 5, exe: '' }, { state: 'running' }, 'freeze').ok, false, 'no recorded executable');
});

test('availability: the chain button works when any process of the chain can take the action', () => {
  const chain = [hop('a', '/a', { pid: 1 }), hop('b', '/b', { pid: 2 })];
  const live = { [S.sigKey(chain[0])]: { code: 'gone' }, [S.sigKey(chain[1])]: { state: 'running' } };
  assert.equal(S.sigChainAvail(chain, 1, live, 'freeze').ok, true);
  assert.equal(S.sigChainAvail(chain, 1, live, 'resume').ok, false);
});

test('url: every item carries pid, start time and executable, encoded; a token makes it a real request; nothing leaves in a hash', () => {
  const items = [{ pid: 7, started: 9, exe: '/a b/c:d,e?&' }, { pid: 8, started: null, exe: '/x' }];
  const u = S.sigUrl({ action: 'freeze', scope: 'chain', items });
  assert.ok(u.startsWith('/api/requester/signal?action=freeze&scope=chain&item=7%3A9%3A'));
  assert.ok(u.includes('item=8%3A%3A%2Fx') && u.endsWith('&dry=1') && !u.includes('#'));
  assert.ok(S.sigUrl({ action: 'stop', scope: 'one', items: items.slice(0, 1), token: 't/k' }).endsWith('&dry=0&confirm_token=t%2Fk'));
});

// ---- the state machine
const item = (pid, level = 'plain', extra = {}) => ({ pid, started: 5, exe: '/bin/x' + pid, label: 'prog' + pid, command: 'prog' + pid + ' --a', level, why: level === 'plain' ? [] : ['why'], ...extra });
const reply = (items, extra = {}) => ({ ok: true, dry_run: true, confirm_token: 'tok', items: items.map((i) => ({ pid: i.pid, status: 'would_signal', state: 'running', children: [], children_total: 0, ...(i.over || {}) })), ...extra });
function run(events, start = S.sigIdle()) {
  let st = start; const fx = [];
  for (const e of events) { const r = S.sigStep(st, e); st = r.state; fx.push(...r.fx); }
  return { st, fx };
}
const open = (items, action = 'freeze', scope = items.length > 1 ? 'chain' : 'one') => ({ type: 'open', action, scope, items, rowId: 'sudo:1:2', origin: 'o' });

test('flow: open previews, the reply allows the move, go sends once, the reply is the result', () => {
  const items = [item(1)];
  let r = run([open(items)]);
  assert.equal(r.st.phase, 'previewing');
  assert.deepEqual(r.fx.filter((f) => f.do === 'preview').map((f) => [f.seq, f.action, f.scope]), [[1, 'freeze', 'one']]);
  r = run([{ type: 'reply', seq: 1, status: 200, body: reply(items) }], r.st);
  assert.equal(r.st.phase, 'previewed');
  assert.equal(r.st.token, 'tok');
  r = run([{ type: 'go' }, { type: 'go' }, { type: 'go' }], r.st);
  assert.equal(r.st.phase, 'sending');
  assert.equal(r.fx.filter((f) => f.do === 'commit').length, 1, 'a double click is one request');
  assert.equal(r.fx.find((f) => f.do === 'commit').token, 'tok');
  r = run([{ type: 'reply', seq: 2, status: 200, body: reply(items, { dry_run: false, items: [{ pid: 1, status: 'signalled' }] }) }], r.st);
  assert.equal(r.st.phase, 'done');
  assert.equal(r.st.token, null);
  assert.ok(r.fx.some((f) => f.do === 'refresh'));
  assert.equal(r.st.summary.counts.signalled, 1);
});

test('danger: Continue asks a second question, and only the second yes sends', () => {
  const items = [item(1, 'danger')];
  let r = run([open(items), { type: 'reply', seq: 1, status: 200, body: reply(items) }, { type: 'go' }]);
  assert.equal(r.st.phase, 'confirming-danger');
  assert.equal(r.fx.filter((f) => f.do === 'commit').length, 0, 'the first confirmation sends nothing');
  r = run([{ type: 'danger-back' }], r.st);
  assert.equal(r.st.phase, 'previewed');
  r = run([{ type: 'go' }, { type: 'danger-yes' }, { type: 'danger-yes' }], r.st);
  assert.equal(r.fx.filter((f) => f.do === 'commit').length, 1);
  assert.equal(r.st.phase, 'sending');
  assert.equal(S.sigStep(S.sigStep(S.sigIdle(), open(items)).state, { type: 'danger-yes' }).state.phase, 'previewing', 'danger-yes outside the question does nothing');
});

test('danger on resume asks nothing extra', () => {
  const items = [item(1, 'danger')];
  const r = run([open(items, 'resume'), { type: 'reply', seq: 1, status: 200, body: reply(items) }, { type: 'go' }]);
  assert.equal(r.st.phase, 'sending');
});

test('level: fresh children in the preview raise a plain process to caution', () => {
  const items = [item(1)];
  const body = reply([{ pid: 1, over: { children: [{ pid: 9, command: 'sleep 1', frozen: false }], children_total: 1 } }]);
  const r = run([open(items), { type: 'reply', seq: 1, status: 200, body }]);
  assert.equal(r.st.level, 'caution');
  assert.deepEqual(r.st.items[0].why, ['it has 1 child process']);
});

test('chain: the strongest warning applies and every process is listed with its own level', () => {
  const items = [item(1), item(2, 'caution'), item(3, 'danger')];
  const r = run([open(items), { type: 'reply', seq: 1, status: 200, body: reply(items) }]);
  assert.equal(r.st.level, 'danger');
  assert.deepEqual(r.st.items.map((i) => i.level), ['plain', 'caution', 'danger']);
  assert.match(S.sigTitle(r.st), /^Freeze 3 processes in this chain\?$/);
  assert.match(S.sigWarning(r.st, 0)[0], /your own session/);
  assert.equal(S.sigTitle(run([open([item(1)])]).st), 'Freeze prog1?');
});

test('selection: a changed selection closes the dialog and clears the token; the same one does not', () => {
  const items = [item(1)];
  let r = run([open(items), { type: 'reply', seq: 1, status: 200, body: reply(items) }]);
  const key = r.st.key;
  assert.equal(S.sigStep(r.st, { type: 'selection', key }).state.phase, 'previewed');
  const out = S.sigStep(r.st, { type: 'selection', key: 'other' });
  assert.deepEqual(out.state, { phase: 'idle', seq: r.st.seq });
  assert.ok(out.fx.some((f) => f.do === 'focus'));
  assert.equal(S.sigStep(r.st, { type: 'leave' }).state.phase, 'idle');
  // A reply for a closed dialog is ignored, so a token can never arrive late.
  assert.equal(S.sigStep(out.state, { type: 'reply', seq: 1, status: 200, body: reply(items) }).state.phase, 'idle');
});

test('stale replies: a reply for an older request is ignored', () => {
  const items = [item(1)];
  const r = run([open(items), { type: 'reply', seq: 99, status: 200, body: reply(items) }]);
  assert.equal(r.st.phase, 'previewing');
});

test('refusals: nothing signalable gives no token and a disabled action; a mix keeps the rest', () => {
  const items = [item(1), item(2)];
  const none = reply(items, { ok: false, confirm_token: null }); none.items.forEach((i) => { i.status = 'refused'; i.code = 'gone'; i.reason = 'Already exited, nothing to freeze'; });
  let r = run([open(items), { type: 'reply', seq: 1, status: 200, body: none }]);
  assert.equal(r.st.phase, 'previewed');
  assert.equal(r.st.token, null);
  assert.equal(S.sigStep(r.st, { type: 'go' }).state.phase, 'previewed', 'no token, no request');
  assert.equal(S.sigHeadline(r.st.summary, r.st, true), 'None of the 2 processes can be frozen.');
  const mix = reply(items); mix.items[1].status = 'refused'; mix.items[1].code = 'pid_reused';
  r = run([open(items), { type: 'reply', seq: 1, status: 200, body: mix }]);
  assert.equal(r.st.token, 'tok');
  assert.equal(r.st.summary.counts.refused, 1);
});

test('errors: a refused request, an unreadable reply and a 500 that lists items are each told apart', () => {
  const items = [item(1)];
  const opened = run([open(items)]).st;
  let r = run([{ type: 'reply', seq: 1, status: 409, body: { code: 'confirmation_expired' } }], opened);
  assert.equal(r.st.phase, 'error');
  assert.match(r.st.message.text, /expired/);
  assert.equal(r.st.message.retry, true);
  r = run([{ type: 'reply', seq: 1, status: 0, body: null }], opened);
  assert.match(r.st.message.text, /could not reach/);
  r = run([{ type: 'reply', seq: 1, status: 400, body: { code: 'bad_items' } }], opened);
  assert.equal(r.st.message.retry, false);
  const previewed = run([{ type: 'reply', seq: 1, status: 200, body: reply(items) }], opened).st;
  const sending = run([{ type: 'go' }], previewed).st;
  r = run([{ type: 'reply', seq: 2, status: 200, body: null }], sending);
  assert.match(r.st.message.text, /cannot say whether anything was signalled/);
  assert.ok(r.fx.some((f) => f.do === 'refresh'), 'the page re-reads the live state after an unclear send');
  r = run([{ type: 'reply', seq: 2, status: 500, body: reply(items, { dry_run: false, ok: false, audit_error: 'disk\u202e', items: [{ pid: 1, status: 'signalled' }] }) }], sending);
  assert.equal(r.st.phase, 'done', 'a 500 that lists what happened is a result, not a plain error');
  assert.equal(r.st.summary.auditError, 'disk\\u{202e}');
  r = run([{ type: 'again' }], run([{ type: 'reply', seq: 1, status: 409, body: { code: 'confirmation_invalid' } }], opened).st);
  assert.equal(r.st.phase, 'previewing');
  assert.equal(r.fx.filter((f) => f.do === 'preview').length, 1);
});

test('close: while sending it cannot be closed; otherwise it says nothing was signalled and returns focus', () => {
  const items = [item(1)];
  const sending = run([open(items), { type: 'reply', seq: 1, status: 200, body: reply(items) }, { type: 'go' }]).st;
  assert.equal(S.sigStep(sending, { type: 'close' }).state.phase, 'sending');
  const r = S.sigStep(run([open(items)]).st, { type: 'close' });
  assert.equal(r.state.phase, 'idle');
  assert.deepEqual(r.fx.find((f) => f.do === 'focus'), { do: 'focus', origin: 'o' });
  assert.ok(r.fx.some((f) => f.do === 'announce' && /Nothing was signalled/.test(f.text)));
});

test('open: more than the server takes, nothing named, or an unknown action opens nothing; a dialog already open is not replaced', () => {
  for (const ev of [open(Array.from({ length: S.SIG_MAX + 1 }, (_, n) => item(n + 1))), open([]), { ...open([item(1)]), action: 'nuke' }]) assert.equal(S.sigStep(S.sigIdle(), ev).state.phase, 'idle');
  const busy = run([open([item(1)])]).st;
  assert.equal(S.sigStep(busy, open([item(2)])).state, busy);
});

test('summary: hostile text in a reply is escaped, an unknown status is a failure, counts come from the items', () => {
  const sum = S.sigSummary({ ok: true, dry_run: false, items: [{ pid: 1, status: 'signalled', children: [{ pid: 2, command: 'rm\u202e -rf', frozen: true }] }, { pid: 3, status: 'weird', reason: 'r\u200b' }] }, {});
  assert.equal(sum.counts.signalled, 1);
  assert.equal(sum.counts.failed, 1, 'a status the page does not know is never counted as done');
  assert.equal(sum.items[0].children[0].command, 'rm\\u{202e} -rf');
  assert.equal(sum.items[1].reason, 'r\\u{200b}');
  assert.equal(S.sigSummary(null, {}).hasItems, false);
  assert.equal(S.sigSummary([], {}).hasItems, false);
});
