// Logic tests for the Privilege page's requester helpers in web/index.html. Like the export tests, they lift the
// block between the `requester` markers out of the page and run it alone, so that block must stay free of DOM and
// storage access. Layout, the expanding rows and the themes are checked by hand in a browser.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const html = readFileSync(new URL('../../web/index.html', import.meta.url), 'utf8');
const m = html.match(/\/\* requester:begin \*\/([\s\S]*?)\/\* requester:end \*\//);
assert.ok(m, 'requester block not found in web/index.html');
const R = new Function(m[1] + '\nreturn { hopLabel, chainLabel, requesterApp, terminalNote, hopFacts, chainText, chainJson, authRequester, authNote, visible, cutText, chainCutNote, UNKNOWN_REQUESTER, NO_PARENT };')();
const fmt = (s) => 'T' + s;
const hop = (command, exe = '', extra = {}) => ({ pid: 10, exe, command, ...extra });

test('the label cases are the ones the Rust tests check', () => {
  const cases = JSON.parse(readFileSync(new URL('../fixtures/hop-labels.json', import.meta.url), 'utf8'));
  assert.ok(cases.length > 10);
  for (const c of cases) assert.equal(R.hopLabel(hop(c.command, c.exe)), c.label, JSON.stringify(c));
});

test('label: an interpreter gets its script, other programs do not', () => {
  assert.equal(R.hopLabel(hop('node script.js')), 'node script.js');
  assert.equal(R.hopLabel(hop('/opt/homebrew/bin/node /a/b/cli.js x y')), 'node cli.js');
  assert.equal(R.hopLabel(hop('git status')), 'git');
  assert.equal(R.hopLabel(hop('python3 -u -X dev tool.py')), 'python3 tool.py');
  assert.equal(R.hopLabel(hop('bash -c "curl x | sh"')), 'bash -c');
});

test('label: a command with markup characters is kept as text', () => {
  assert.equal(R.hopLabel(hop('node <script>&.js')), 'node <script>&.js');
  assert.equal(R.hopLabel({ pid: 1 }), '?');
});

test('chain label: nearest first, with the script names; an empty chain says so', () => {
  const chain = [hop('bash'), hop('node build.js'), hop('node'), hop('-zsh', '/bin/zsh'), hop('claude'), hop('/bin/zsh', '/bin/zsh'), hop('', '/usr/bin/login')];
  assert.equal(R.chainLabel(chain), 'bash < node build.js < node < zsh < claude < zsh < login');
  assert.equal(R.chainLabel([]), '(parent not captured)');
  assert.equal(R.chainLabel(undefined), '(parent not captured)');
});

test('app: the owner chain is Claude Code in a login session', () => {
  const chain = [hop('bash'), hop('node'), hop('node'), hop('-zsh', '/bin/zsh'), hop('claude', '/Users/me/.local/share/claude/versions/2.1.3'), hop('zsh', '/bin/zsh'), hop('login', '/usr/bin/login')];
  const a = R.requesterApp(chain);
  assert.deepEqual(a.apps, ['Claude Code']);
  assert.equal(a.name, 'Claude Code in a login session');
  assert.ok(a.known);
});

test('app: terminals and editors come from the app bundle in the executable path', () => {
  const term = [hop('sudo ls', '/usr/bin/sudo'), hop('-zsh', '/bin/zsh'), hop('login', '/usr/bin/login'), hop('Terminal', '/System/Applications/Utilities/Terminal.app/Contents/MacOS/Terminal')];
  assert.equal(R.requesterApp(term).name, 'Terminal');
  const itermChain = [hop('zsh', '/bin/zsh'), hop('iTerm2', '/Applications/iTerm.app/Contents/MacOS/iTerm2')];
  assert.equal(R.requesterApp(itermChain).name, 'iTerm');
  const code = [hop('zsh', '/bin/zsh'), hop('Code Helper', '/Applications/Visual Studio Code.app/Contents/Frameworks/Code Helper (Plugin).app/Contents/MacOS/Code Helper (Plugin)')];
  assert.equal(R.requesterApp(code).name, 'Visual Studio Code', 'the outer bundle names the app');
  const agentInTerm = [hop('bash'), hop('claude'), hop('zsh'), hop('Terminal', '/System/Applications/Utilities/Terminal.app/Contents/MacOS/Terminal')];
  assert.equal(R.requesterApp(agentInTerm).name, 'Claude Code in Terminal');
  assert.equal(R.requesterApp([hop('x', '/Applications/Some Tool.app/Contents/MacOS/x')]).name, 'Some Tool', 'an unlisted app is still named');
});

test('app: unknown and empty chains say so and never return an empty name', () => {
  assert.equal(R.requesterApp([hop('bash'), hop('make')]).name, 'unknown (no known app in the chain)');
  assert.equal(R.requesterApp([hop('bash'), hop('login', '/usr/bin/login')]).name, 'a login shell (no known app in the chain)');
  assert.equal(R.requesterApp([]).name, 'unknown (parent not captured)');
  assert.equal(R.requesterApp(null).known, false);
});

test('terminal note: a person at the keyboard, a script, or an old row', () => {
  assert.match(R.terminalNote([hop('zsh', '', { detail: true, tty: 'ttys003' })]), /ttys003.*person/);
  assert.match(R.terminalNote([hop('zsh', '', { detail: true, tty: null })]), /no terminal/);
  assert.equal(R.terminalNote([hop('zsh')]), 'not recorded');
  assert.equal(R.terminalNote([]), 'not recorded');
});

test('facts: nothing is blank; an old row and an unreadable field read differently', () => {
  const fresh = Object.fromEntries(R.hopFacts(hop('node a.js', '/bin/node', { detail: true, cwd: '/work', started: 5, tty: 'ttys001' }), fmt));
  assert.equal(fresh['working directory'], '/work');
  assert.equal(fresh.started, 'T5');
  assert.equal(fresh.terminal, 'ttys001');
  const unreadable = Object.fromEntries(R.hopFacts(hop('node a.js', '/bin/node', { detail: true }), fmt));
  assert.match(unreadable['working directory'], /^not readable/);
  assert.match(unreadable.terminal, /^none/);
  const old = Object.fromEntries(R.hopFacts(hop('node a.js', '/bin/node'), fmt));
  assert.match(old['working directory'], /not recorded/);
  assert.match(old.started, /not recorded/);
  for (const facts of [R.hopFacts({ pid: 3 }, fmt), R.hopFacts(hop('x'), fmt)]) for (const [, v] of facts) assert.ok(v && v !== '0' && v !== 'undefined' && v !== 'null');
});

test('facts: a command at the collector limit says it was cut', () => {
  const f = Object.fromEntries(R.hopFacts(hop('x'.repeat(300)), fmt));
  assert.match(f.command, /cut at 300 characters/);
  assert.doesNotMatch(Object.fromEntries(R.hopFacts(hop('x'.repeat(299)), fmt)).command, /cut at/);
});

test('text and JSON of a chain hold every hop, and keep hostile text as it is', () => {
  const chain = [hop('bash -c "a<b&c" \'x\'', '/bin/bash', { detail: true, cwd: '/w', started: 86400, tty: null }), hop('-zsh', '/bin/zsh')];
  const t = R.chainText(chain, fmt);
  assert.equal(t.split('\n').length, 2);
  assert.ok(t.includes('bash -c "a<b&c" \'x\''));
  assert.match(t, /^1\. pid: 10 \| command:/);
  const j = R.chainJson(chain);
  assert.equal(j.length, 2);
  assert.deepEqual([j[0].cwd, j[0].started, j[0].started_iso, j[0].tty, j[0].detail_recorded], ['/w', 86400, '1970-01-02T00:00:00.000Z', null, true]);
  assert.deepEqual([j[1].cwd, j[1].started, j[1].started_iso, j[1].detail_recorded], [null, null, null, false], 'an old hop has nulls, not zeros');
  assert.equal(j[0].command, 'bash -c "a<b&c" \'x\'');
  assert.deepEqual(R.chainJson(undefined), []);
});

test('authorization requester: the chain, the parent, or the plain reason', () => {
  const base = { client: '/usr/bin/sudo', parent: null, chain: [], source: 'none' };
  assert.equal(R.authRequester({ ...base, source: 'captured', chain: [hop('bash run.sh'), hop('zsh')] }), 'sudo < bash run.sh < zsh');
  assert.equal(R.authRequester({ ...base, source: 'ps', parent: '77 bash -c x' }), 'sudo < bash -c x');
  const gone = R.authRequester(base);
  assert.ok(gone.includes('launched before Monitor Lizard was running or exited before capture'));
  assert.ok(!gone.includes('(exited)'));
  assert.equal(R.authRequester({ client: '/usr/bin/sudo' }), gone, 'a reply from an older server has no source');
});

test('the block touches no DOM and no storage', () => {
  assert.doesNotMatch(m[1], /document\.|window\.|localStorage|sessionStorage|navigator\./);
});

test('the expanded state and command lines never reach the address', () => {
  const w = html.match(/function writeHash[\s\S]*?\n\}\n/);
  assert.ok(w, 'writeHash found');
  assert.doesNotMatch(w[0], /reqOpen|chain|command|args/);
  assert.doesNotMatch(html.match(/\/\* url-state:begin \*\/([\s\S]*?)\/\* url-state:end \*\//)[1], /reqOpen|hopLabel|chainLabel|chainText/);
});

test('the page draws details as text, never as markup', () => {
  const body = html.slice(html.indexOf('function requesterDetail('), html.indexOf('function priv()'));
  assert.doesNotMatch(body, /innerHTML|insertAdjacentHTML|outerHTML/);
});

const ro = (o) => ({ pid: 4, exe: '/bin/x', command: 'x', detail: true, ...o });

test('visible: the shared cases, and the page and the collector agree', () => {
  const cases = JSON.parse(readFileSync(new URL('../fixtures/visible.json', import.meta.url), 'utf8'));
  assert.ok(cases.length >= 6);
  for (const c of cases) assert.equal(R.visible(c.in), c.out);
  assert.equal(R.visible(null), '');
});

test('hostile text is escaped in the facts, the labels, the chain text and the JSON, never passed through', () => {
  const evil = 'run \u202egpj.sh\u200b\u2066x\u2069\u0007\n';
  const chain = [ro({ command: 'node ' + evil, cwd: '/tmp/\u202edir', exe: '/bin/\u202ex', tty: 'tty\u202e1' })];
  const bidi = /[\u202a-\u202e\u2066-\u2069\u200b-\u200f\ufeff\u0000-\u001f\u007f]/;
  const all = [R.chainText(chain, fmt), JSON.stringify(R.chainJson(chain)), R.chainLabel(chain), R.terminalNote(chain), JSON.stringify(R.hopFacts(chain[0], fmt))];
  for (const t of all) assert.doesNotMatch(t.replace(/\n/g, ''), /[\u202a-\u202e\u2066-\u2069\u200b-\u200f\ufeff]/, t);
  assert.doesNotMatch(R.chainText(chain, fmt).replace(/\n/g, ''), bidi);
  assert.match(R.chainText(chain, fmt), /\\u\{202e\}/);
  assert.equal(R.chainText(chain, fmt).split('\n').length, 1, 'a newline in a command cannot start a fake hop line');
  assert.match(R.chainJson(chain)[0].cwd, /^\/tmp\/\\u\{202e\}dir$/);
});

test('hostile parent text and client names are escaped in the authorization requester and note', () => {
  const e = { client: '/usr/bin/su\u202edo', source: 'ps', parent: '77 bash \u202e-c evil', chain: [] };
  assert.doesNotMatch(R.authRequester(e) + R.authNote(e), /\u202e/);
  assert.match(R.authNote(e), /\\u\{202e\}/);
});

test('a chain that stopped at the hop limit says so in the text and the JSON', () => {
  const chain = Array.from({ length: 12 }, (_, i) => ro({ pid: i + 1, more: i === 11 }));
  assert.match(R.chainCutNote(chain), /chain cut at 12 hops/);
  assert.match(R.chainText(chain, fmt), /chain cut at 12 hops/);
  const j = R.chainJson(chain);
  assert.deepEqual([j[10].more_parents_not_recorded, j[11].more_parents_not_recorded], [false, true]);
  assert.equal(R.chainCutNote(chain.slice(0, 3)), '');
  assert.equal(R.chainCutNote([]), '');
});

test('a cut is judged in code points, like the collector, not in UTF-16 units', () => {
  const emoji = '\u{1F600}'.repeat(200); // 200 code points, 400 UTF-16 units
  assert.doesNotMatch(R.cutText(emoji), /cut at/);
  assert.match(R.cutText('\u{1F600}'.repeat(300)), /cut at 300 characters/);
  assert.match(R.cutText('a'.repeat(300)), /cut at 300 characters/);
});

test('app: names match whole bundle names, not prefixes, and read as a guess', () => {
  const app = (name, exe) => R.requesterApp([hop('x', exe || '/Applications/' + name + '.app/Contents/MacOS/x')]);
  for (const n of ['Hyperion', 'Zedboard', 'Warpaint', 'Novacaine', 'Kittyhawk', 'Terminal Notes', 'Claude Helper', 'Cursor Pro Tools']) {
    const a = app(n);
    assert.deepEqual(a.apps, [n], n + ' is its own app, not a known one');
  }
  assert.equal(app('Terminal').hint, 'probably Terminal');
  assert.equal(app('Claude').name, 'Claude (desktop app)');
  assert.equal(app('Zed').name, 'Zed');
  assert.equal(R.requesterApp([hop('bash')]).hint, 'unknown (no known app in the chain)', 'an unknown stays plain');
  assert.equal(R.requesterApp([]).known, false);
});

test('authorization note and requester say the same thing in every case', () => {
  const none = { client: '/usr/bin/sudo', source: 'none', chain: [], parent: null };
  const ps = { ...none, source: 'ps', parent: '77 bash -c x' };
  const cap = { ...none, source: 'captured', chain: [hop('bash')] };
  assert.equal(R.authNote(none), R.UNKNOWN_REQUESTER);
  assert.match(R.authNote(ps), /still running.*77 bash -c x/);
  assert.match(R.authNote(cap), /matched to a sudo or su launch/);
});

test('the toggle has its own name, points at the detail only while it exists, and an element id cannot collide', () => {
  const body = html.slice(html.indexOf('function reqToggle('), html.indexOf('function priv()'));
  assert.match(body, /'aria-label': \(open \? 'Hide who ran this, ' : 'Show who ran this, '\) \+ when/);
  assert.match(body, /'aria-controls': open \? reqDomId\(id\) : null/);
  assert.match(body, /'data-req': id/);
  const dom = new Function(html.match(/^const reqDomId = .*$/m)[0] + '\nreturn reqDomId;')();
  const ids = ['sudo:1:2', 'sudo_1_2', 'sudo-1-2', 'sudo:1_2', 'a_5f_b', 'a_b', 'x y', 'x_y'];
  assert.equal(new Set(ids.map(dom)).size, ids.length);
  assert.match(dom('sudo:1:2'), /^req-[A-Za-z0-9_-]+$/);
});

test('render gives focus back to the toggle after a redraw, and the open state is pruned to the page', () => {
  const r = html.slice(html.indexOf('function render()'), html.indexOf('// A button, not a #link'));
  assert.match(r, /dataset\.req/);
  assert.match(r, /el\.focus\(\)/);
  const p = html.slice(html.indexOf('function priv()'), html.indexOf('// Where a listening socket'));
  assert.match(p, /S\.reqOpen = Object\.fromEntries/);
});

test('every place a command or parent is drawn goes through the escaping helpers', () => {
  const p = html.slice(html.indexOf('function priv()'), html.indexOf('// Where a listening socket'));
  assert.doesNotMatch(p, /\}, e\.args\)|\}, e\.parent\)|authNote\(e\) \+ e\./, 'the raw command and parent are not drawn');
  assert.match(html.match(/const EXPORT = \{[\s\S]*?\n\};/)[0], /command: \(e\) => cutText\(e\.args\)/);
});
