/* Sorting and filtering shared by every table.
   S.sorts[key] = { col, dir }: no entry means the default order. S.fil[name] holds a dropdown filter's value. */
const fv = (name) => S.fil[name] || 'all';
const setFil = (name, v) => rec({ fil: Object.assign({}, S.fil, { [name]: v }), page: {} });
const clearFilters = () => { $('q').value = ''; rec({ q: '', fil: {}, filter: 'all', lfilter: 'all', pfilter: 'all', nfilter: 'all', gtopic: 'all', page: {} }); };
const clearBtn = () => h('button', { class: 'btn sm', onClick: clearFilters }, 'Clear filters');
const isEmpty = (v) => v == null || v === '';
// Stable sort; empty values go last in both directions; numbers compare as numbers, text ignores case.
function sortRows(rows, key, getters) {
  const s = S.sorts[key];
  if (!s || !Object.prototype.hasOwnProperty.call(getters, s.col)) return rows; // own keys only: 'constructor' and '__proto__' are not columns
  const dir = s.dir === 'desc' ? -1 : 1;
  const empty = (v) => isEmpty(v) || (typeof v === 'number' && Number.isNaN(v));
  return rows.map((r, i) => ({ r, i, v: getters[s.col](r) })).sort((x, y) => {
    if (empty(x.v) || empty(y.v)) return empty(x.v) === empty(y.v) ? x.i - y.i : empty(x.v) ? 1 : -1;
    const c = typeof x.v === 'number' && typeof y.v === 'number' ? x.v - y.v : String(x.v).localeCompare(String(y.v), undefined, { sensitivity: 'base' });
    return c * dir || x.i - y.i;
  }).map((o) => o.r);
}
// Click cycles ascending, descending, then back to the default order.
function nextSort(key, col) {
  const s = S.sorts[key], n = Object.assign({}, S.sorts);
  if (!s || s.col !== col) n[key] = { col, dir: 'asc' }; else if (s.dir === 'asc') n[key] = { col, dir: 'desc' }; else delete n[key];
  return n;
}
/* requester:begin */
/* Pure helpers for the Privilege page's requester detail (no DOM, no storage: the tests load this block alone).
   A hop is { pid, exe, command, started?, tty?, cwd?, detail?, more? }; rows saved by an older build lack the last
   five, and `detail: true` is what separates "could not be read" from "was not recorded". */
const UNKNOWN_REQUESTER = 'no matching launch recorded (launched before Monitor Lizard was running, exited before capture, or older than the records kept)';
const NO_PARENT = '(parent not captured)';
const COMMAND_CUT = 300; // the collector keeps this many characters (code points) of a command line
const MAX_HOPS = 12;     // and this many hops of a chain
// Text a stranger can choose (a command line, a folder) is shown and exported with control and format characters
// (bidi overrides, zero-width characters) as visible escapes, so it cannot reorder or hide what a person reads.
// The Rust side has the same function (watcher::visible); tests/fixtures/visible.json is checked by both.
const visible = (s) => String(s == null ? '' : s).replace(/[\p{Cc}\p{Cf}\u2028\u2029]/gu, (c) => '\\u{' + c.codePointAt(0).toString(16) + '}');
const codePoints = (s) => [...String(s || '')].length;
// A command at the collector's limit was cut; the text says so wherever it is shown or exported.
const cutText = (s) => visible(s) + (codePoints(s) >= COMMAND_CUT ? '  (cut at ' + COMMAND_CUT + ' characters)' : '');
const INTERPRETERS = ['node', 'bun', 'deno', 'ruby', 'perl', 'php', 'bash', 'zsh', 'sh', 'dash', 'fish', 'ksh', 'tcsh', 'csh', 'lua', 'Rscript', 'osascript',
  'npm', 'npx', 'pnpm', 'yarn', 'tsx', 'ts-node', 'vite-node', 'nodemon', 'uv', 'java'];
const VALUE_OPTIONS = ['-r', '--require', '--loader', '--import', '--experimental-loader', '--conditions', '-C', '-W', '-X', '-I', '-o', '-O', '--rcfile', '--init-file',
  '--max-old-space-size', '--stack-size', '--env-file', '--with', '--python', '-cp', '-classpath', '--class-path'];
const INLINE_OPTIONS = ['-c', '-e', '-p', '-E'];
const SUBCOMMAND_TOOLS = ['npm', 'pnpm', 'yarn', 'bun', 'deno', 'uv'];
const RUN_WORDS = ['run', 'exec', 'dlx', 'x', 'run-script'];
const WRAPPERS = ['tsx', 'ts-node', 'ts-node-esm', 'vite-node', 'nodemon', 'npx', 'npm', 'pnpm', 'yarn'];
const baseName = (p) => String(p).split('/').pop();
const isInterpreter = (n) => INTERPRETERS.includes(n) || /^python[0-9.]*$/.test(n);
// Index of the first word that is not an option or an option's value, from `from`.
function firstOperand(words, from) {
  let i = from;
  while (i < words.length) {
    const w = words[i];
    if (VALUE_OPTIONS.includes(w)) { i += 2; continue; }
    if (w.startsWith('--') && !w.includes('=') && /^\d+$/.test(words[i + 1] || '')) { i += 2; continue; }
    if (w.startsWith('-')) { i++; continue; }
    return i;
  }
  return -1;
}
function rawLabel(command, exe) {
  command = String(command || '').trim(); exe = String(exe || '');
  let first, rest;
  if (exe && command.startsWith(exe) && (command.length === exe.length || command[exe.length] === ' ')) { first = exe; rest = command.slice(exe.length); }
  else { const m = /^(\S*)\s*([\s\S]*)$/.exec(command); first = m[1]; rest = m[2]; }
  let words = rest.split(/\s+/).filter(Boolean);
  let name = baseName(first).replace(/^-+/, '') || baseName(exe);
  if (!name) return '?';
  if (name === 'env') {
    let skip = 0;
    while (skip < words.length && (words[skip].startsWith('-') || words[skip].includes('='))) skip++;
    if (skip < words.length) { name = baseName(words[skip]); words = words.slice(skip + 1); }
  }
  if (!isInterpreter(name)) return name;
  let i = 0;
  while (i < words.length) {
    const w = words[i];
    if (INLINE_OPTIONS.includes(w)) return name + ' ' + w;
    if (w === '-m') return words[i + 1] ? name + ' -m ' + words[i + 1] : name + ' -m';
    if (w === '-jar') return words[i + 1] ? name + ' -jar ' + baseName(words[i + 1]) : name + ' -jar';
    const k = firstOperand(words, i);
    if (k < 0) break;
    if (k !== i) { i = k; continue; }
    if (SUBCOMMAND_TOOLS.includes(name) && RUN_WORDS.includes(w)) {
      const n = firstOperand(words, i + 1);
      return n < 0 ? name + ' ' + w : name + ' ' + w + ' ' + baseName(words[n]);
    }
    if (WRAPPERS.includes(baseName(w))) return rawLabel(words.slice(i).join(' '), '');
    return name + ' ' + baseName(w);
  }
  return name;
}
// A short name for one hop: the program and, when it is an interpreter, the script or module it runs. The Rust side
// has the same function (hop_label); tests/fixtures/hop-labels.json is checked by both.
const hopLabel = (hop) => visible(rawLabel(hop && hop.command, hop && hop.exe));
// The compact chain, nearest parent first: `bash run.sh < zsh < claude`.
const chainLabel = (chain) => (Array.isArray(chain) && chain.length ? chain.map(hopLabel).join(' < ') : NO_PARENT);
// Said when the walk stopped at the hop limit with parents still above it.
const chainCutNote = (chain) => (Array.isArray(chain) && chain.length && chain[chain.length - 1].more ? 'chain cut at ' + MAX_HOPS + ' hops: older parents were not recorded' : '');

// Terminals, editors and agents, matched on the whole name of the app bundle (lower case) or of the program, never
// on a prefix, so an unrelated app that starts with the same letters is not mislabelled. A hint, not an answer.
const BUNDLE_APPS = {
  'terminal': 'Terminal', 'iterm': 'iTerm', 'iterm2': 'iTerm', 'ghostty': 'Ghostty', 'warp': 'Warp', 'alacritty': 'Alacritty', 'wezterm': 'WezTerm', 'kitty': 'kitty', 'hyper': 'Hyper',
  'visual studio code': 'Visual Studio Code', 'visual studio code - insiders': 'Visual Studio Code', 'cursor': 'Cursor', 'zed': 'Zed', 'zed preview': 'Zed', 'xcode': 'Xcode', 'xcode-beta': 'Xcode',
  'sublime text': 'Sublime Text', 'nova': 'Nova', 'intellij idea': 'IntelliJ IDEA', 'intellij idea ce': 'IntelliJ IDEA', 'pycharm': 'PyCharm', 'pycharm ce': 'PyCharm', 'webstorm': 'WebStorm',
  'goland': 'GoLand', 'clion': 'CLion', 'rubymine': 'RubyMine', 'phpstorm': 'PhpStorm', 'android studio': 'Android Studio', 'claude': 'Claude (desktop app)',
};
const TERMINAL_APPS = ['Terminal', 'iTerm', 'Ghostty', 'Warp', 'Alacritty', 'WezTerm', 'kitty', 'Hyper', 'tmux', 'screen'];
const PROGRAM_APPS = { claude: 'Claude Code', tmux: 'tmux', screen: 'screen', sshd: 'a remote SSH session', cron: 'cron (a scheduled job)' };
const hasOwn2 = (o, k) => Object.prototype.hasOwnProperty.call(o, k);
function appOfHop(hop) {
  const exe = String((hop && hop.exe) || ''), cmd = String((hop && hop.command) || '');
  for (const prog of [baseName(exe), baseName(cmd.split(/\s+/)[0]).replace(/^-+/, '')]) if (hasOwn2(PROGRAM_APPS, prog)) return PROGRAM_APPS[prog];
  const m = /(?:^|\/)([^/]+)\.app(?:\/|$)/.exec(exe || cmd);
  if (!m) return null;
  const low = m[1].toLowerCase();
  return hasOwn2(BUNDLE_APPS, low) ? BUNDLE_APPS[low] : m[1];
}
// A guess at which app the request came from: every recognised app in the chain, nearest first. `name` is the bare
// answer and `hint` the wording to show ("probably ..."), because a name in a process list proves little.
function requesterApp(chain) {
  if (!Array.isArray(chain) || !chain.length) return { name: 'unknown (parent not captured)', hint: 'unknown (parent not captured)', apps: [], known: false };
  const apps = [];
  for (const hop of chain) { const a = appOfHop(hop); if (a && !apps.includes(a)) apps.push(a); }
  const login = chain.some((hop) => baseName(hop.exe || String(hop.command || '').split(/\s+/)[0]) === 'login');
  if (!apps.length) { const n = login ? 'a login shell (no known app in the chain)' : 'unknown (no known app in the chain)'; return { name: n, hint: n, apps, known: false }; }
  const inTerminal = apps.some((a) => TERMINAL_APPS.includes(a));
  const name = visible(apps.join(' in ') + (login && !inTerminal ? ' in a login session' : ''));
  return { name, hint: 'probably ' + name, apps, known: true };
}
// Whether the nearest hop (the process that ran sudo) had a terminal: a hint of a person at the keyboard, not proof.
function terminalNote(chain) {
  const hop = Array.isArray(chain) ? chain.find((x) => x && x.detail) : null;
  if (!hop) return 'not recorded';
  return hop.tty ? 'terminal ' + visible(hop.tty) + ' (a person may have typed it)' : 'no terminal (probably a script or background job)';
}
const UNREADABLE = 'not readable (the process had exited, or belongs to another user)';
const NOT_RECORDED = 'not recorded (captured by an older version)';
// The facts shown for one hop, as [label, text] pairs. `fmt` formats epoch seconds. Nothing is ever blank, and
// nothing a stranger chose is shown without `visible`.
function hopFacts(hop, fmt) {
  const seen = !!hop.detail, cmd = String(hop.command || '');
  return [
    ['pid', String(hop.pid)],
    ['command', cmd ? cutText(cmd) : UNREADABLE],
    ['executable', hop.exe ? visible(hop.exe) : UNREADABLE],
    ['working directory', hop.cwd ? visible(hop.cwd) : (seen ? UNREADABLE : NOT_RECORDED)],
    ['started', typeof hop.started === 'number' ? fmt(hop.started) : (seen ? UNREADABLE : NOT_RECORDED)],
    ['terminal', hop.tty ? visible(hop.tty) : (seen ? 'none, or ' + UNREADABLE : NOT_RECORDED)],
  ];
}
// The whole chain as plain text, one hop per line, for CSV cells and the clipboard.
function chainText(chain, fmt) {
  if (!Array.isArray(chain) || !chain.length) return NO_PARENT;
  const lines = chain.map((hop, i) => (i + 1) + '. ' + hopFacts(hop, fmt).map(([k, v]) => k + ': ' + v).join(' | '));
  const cut = chainCutNote(chain);
  return (cut ? lines.concat(cut) : lines).join('\n');
}
// The whole chain as data for the JSON export: unreadable fields are null, never invented.
function chainJson(chain) {
  const iso = (s) => { const d = typeof s === 'number' ? new Date(s * 1000) : null; return d && !Number.isNaN(d.getTime()) ? d.toISOString() : null; };
  const v = (x) => (x ? visible(x) : null);
  return (Array.isArray(chain) ? chain : []).map((hop) => ({
    pid: hop.pid, label: hopLabel(hop), command: hop.command ? cutText(hop.command) : null, exe: v(hop.exe), cwd: v(hop.cwd),
    started: typeof hop.started === 'number' ? hop.started : null, started_iso: iso(hop.started), tty: v(hop.tty), detail_recorded: !!hop.detail, more_parents_not_recorded: !!hop.more,
  }));
}
// What a failed authorization knows about its requester, in words: the same text in the table, the CSV, the JSON and the copied row.
function authNote(e) {
  if (e.source === 'captured' && e.chain && e.chain.length) return 'matched to a sudo or su launch Monitor Lizard recorded for this process' + (chainCutNote(e.chain) ? '; ' + chainCutNote(e.chain) : '');
  if (e.source === 'ps' && e.parent) return 'process still running; only its parent could be read: ' + visible(e.parent);
  return UNKNOWN_REQUESTER;
}
// The Requester cell of a failed authorization: its program, then where the request came from.
function authRequester(e) {
  const who = visible(baseName(e.client));
  if (e.source === 'captured' && e.chain && e.chain.length) return who + ' < ' + chainLabel(e.chain);
  if (e.source === 'ps' && e.parent) return who + ' < ' + visible(e.parent.split(' ').slice(1).join(' ').slice(0, 60));
  return who + ': ' + UNKNOWN_REQUESTER;
}
/* requester:end */
/* export:begin */
/* Pure helpers for table export and the column chooser (no DOM, no storage: the tests load this block alone).
   A column is { id, label, get(row), time? }; a time column's value is epoch seconds. */
// A spreadsheet treats a cell that starts with these as a formula, and a file can hold text a stranger chose
// (an app name, a host). A leading apostrophe makes it text; it is the same rule Excel and Numbers use.
const FORMULA_START = /^[=+\-@\t\r]/;
function csvCell(v) {
  if (v == null || (typeof v === 'number' && !Number.isFinite(v))) return '';
  let s = String(v);
  if (typeof v !== 'number' && FORMULA_START.test(s)) s = "'" + s;
  return /[",\r\n]/.test(s) ? '"' + s.replace(/"/g, '""') + '"' : s;
}
function colId(c) { return c.id || c.sort || String(c.label).toLowerCase().replace(/[^a-z0-9]+/g, '_').replace(/^_+|_+$/g, ''); }
function isoOf(sec) { const d = typeof sec === 'number' ? new Date(sec * 1000) : null; return d && !Number.isNaN(d.getTime()) ? d.toISOString() : null; }
function toCsv(cols, rows) {
  const out = [cols.map((c) => csvCell(c.label)).join(',')];
  for (const r of rows) out.push(cols.map((c) => csvCell(c.time ? isoOf(c.get(r)) : c.get(r))).join(','));
  return out.join('\r\n') + '\r\n';
}
// No prototype, so a column id such as __proto__ is an ordinary key and cannot be lost or reach Object.prototype.
function rowObject(cols, row) {
  const o = Object.create(null);
  for (const c of cols) {
    const v = c.getJson ? c.getJson(row) : c.get(row);
    o[c.id] = v === undefined || (typeof v === 'number' && !Number.isFinite(v)) ? null : v;
    if (c.time) o[c.id + '_iso'] = isoOf(v);
  }
  return o;
}
function toJson(cols, rows) { return JSON.stringify(rows.map((r) => rowObject(cols, r)), null, 2); }
// What the person hid is stored as a list of column ids. Anything unreadable or hostile means "show everything".
function hiddenFrom(raw) {
  try {
    const v = JSON.parse(raw);
    return Array.isArray(v) ? v.filter((x) => typeof x === 'string').slice(0, 64) : [];
  } catch (e) { return []; }
}
function visibleCols(cols, hidden) {
  const shown = cols.filter((c) => !hidden.includes(c.id));
  return shown.length ? shown : cols;
}
// One column always stays: hiding the last visible one is refused, so an empty table cannot be saved.
function toggleHidden(cols, hidden, id) {
  const known = hidden.filter((x) => cols.some((c) => c.id === x));
  if (known.includes(id)) return known.filter((x) => x !== id);
  return cols.length - known.length <= 1 ? known : known.concat(id);
}
function fileName(page, ext, now) {
  const p = (n) => String(n).padStart(2, '0');
  return page + '-' + now.getFullYear() + '-' + p(now.getMonth() + 1) + '-' + p(now.getDate()) + '.' + ext;
}
// Copy buttons and hidden columns are matched to rows by position, so they are applied only when the drawn
// table is exactly what the page rows predict: one DOM row per page row, each with one cell per header column.
function tableAligned(headCells, rowCells, pageRows) {
  return rowCells.length === pageRows && rowCells.every((n) => n === headCells);
}
// The note beside the export buttons: one line that is always shown, and the full text behind a button. The facts a
// reader must not lose are in the full text: the file stays on this Mac until shared, commands can hold secrets,
// and the CSV apostrophe guard. `meta` is { info, warn? }.
function exportNote(meta) {
  const info = (meta && meta.info) || 'the rows shown', warn = meta && meta.warn;
  return {
    line: (warn ? 'May contain secrets. ' : '') + 'Stays on this Mac until you share it.',
    full: 'The file holds ' + info + '. ' + (warn ? warn + ' ' : '') + 'In the CSV, a cell starting with = + - or @ gets a leading \' so spreadsheets do not run it; the JSON is unchanged. The file is made in this page and is a copy: it stays on this Mac until you share it.',
  };
}
// Each page keeps only the tables it draws, so rows of tables left behind are not held in memory.
function pruneTables(tables, keep) {
  for (const k of Object.keys(tables)) if (!keep.includes(k)) delete tables[k];
}
/* export:end */
/* export-cols:begin */
// Export value of every column, per table key: the text the table shows, not the number used for sorting.
// Differences are deliberate: a "none" dash is exported as empty (a leading '-' would be written as text with an
// apostrophe), times are epoch seconds (the file adds an ISO string), Actions "sent" is the bare process IDs, and
// Authorization time is the full time stamp. Ids must match the sortHeader columns; a test checks it.
const noDash = (s) => (s === '-' ? '' : s);
const EXPORT = {
  apps: { app: (g) => g.label, procs: (g) => g.procs.length, out: (g) => g.conns.filter((c) => c.direction === 'outbound').length, listening: (g) => noDash(listenText(g)), signing: (g) => noDash(signText(g)), status: (g) => status(g), label: (g) => g.trust },
  sudo: { time: (e) => e.ts, command: (e) => cutText(e.args), app: (e) => requesterApp(e.chain).hint, requester_chain: (e) => chainLabel(e.chain), requester_detail: (e) => chainText(e.chain, fmtWhen) },
  auth: { time: (e) => e.time, result: (e) => (e.ok ? 'ok' : 'failed'), right: (e) => e.right + (e.error ? ' ' + e.error : ''), requester: (e) => authRequester(e), app: (e) => requesterApp(e.source === 'captured' ? e.chain : []).hint, requester_detail: (e) => (e.source === 'captured' ? chainText(e.chain, fmtWhen) : authNote(e)), requester_note: (e) => authNote(e) },
  listen: { port: (r) => r.c.local_port, proto: (r) => r.c.proto, bind: (r) => r.c.local_addr, app: (r) => r.g.label, pid: (r) => r.c.pid, reach: (r) => reachOf(r) },
  pers: { label: (i) => i.label, kind: (i) => i.kind, scope: (i) => i.scope, program: (i) => i.program, trust: (i) => psig(i) || '', added: (i) => i.first_seen },
  net: { app: (r) => (r.apps ? r.apps.map((g) => g.label).join(', ') : r.g.label), remote: (r) => r.remote, ports: (r) => r.ports.join(', '), scope: (r) => r.scope, sockets: (r) => r.sockets, seen: (r) => r.first_seen },
  hist: { when: (s) => s.first_seen, app: (s) => labelOf(s.group_key), host: (s) => s.remote, hits: (s) => s.hits },
  store: { folder: (r) => r.path, section: (r) => r.category || '', label: (r) => r.label || '', size: (r) => sizeText(r), share: (r) => noDash(shareText(r)), measured: (r) => r.measuredText },
  act: { when: (a) => a.ts, signal: (a) => a.sig, sent: (a) => jsonOr(a.sent, []).join(' '), refused: (a) => noDash(refusedText(a)) },
  trash: { when: (r) => r.ts, request: (r) => r.requestText, folder: (r) => r.path, state: (r) => r.stateText, size: (r) => noDash(r.sizeText), why: (r) => r.why },
};
/* export-cols:end */
// Columns whose JSON is structured data (the full chain) rather than the text the CSV holds.
const EXPORT_JSON = {
  sudo: { requester_detail: (e) => chainJson(e.chain) },
  auth: { requester_detail: (e) => (e.source === 'captured' ? chainJson(e.chain) : []) }, // an unmatched row has no hops; requester_note says why
};
// A time inside an exported chain: the full local time, to the second, which stays readable in a spreadsheet.
const fmtWhen = (sec) => new Date(sec * 1000).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'medium' });
// One descriptor per table: file name, what the file holds (shown beside the buttons, since a shared file shows
// habits such as apps, hosts and paths), which columns are times, and an optional warning where one is needed.
const SECRETS = 'Commands can contain secrets in their arguments.';
const TABLE_META = {
  apps: { file: 'apps', info: 'app names, signing details and listening ports', time: [] },
  sudo: { file: 'sudo-launches', info: 'the commands run with sudo and, for each, the programs and scripts that ran it, with their full command lines, folders, start times and terminals', warn: SECRETS, time: ['time'] },
  auth: { file: 'authorization-requests', info: 'authorization rights and the programs that asked, with the full chain of programs and scripts where one was captured', warn: SECRETS, time: [] },
  listen: { file: 'listeners', info: 'app names, ports and bind addresses', time: [] },
  pers: { file: 'persistence', info: 'startup item names and program paths', time: ['added'] },
  net: { file: 'network', info: 'app names and the remote hosts they contacted', time: ['seen'] },
  hist: { file: 'new-hosts', info: 'app names and the hosts they first contacted', time: ['when'] },
  store: { file: 'storage', info: 'folder paths and their sizes as shown, with the words "at least" or the reason where a size is a lower bound or unavailable', time: [] },
  act: { file: 'actions', info: 'which processes you stopped and when', time: ['when'] },
  trash: { file: 'trash-moves', info: 'folder paths you moved or tried to move to the Trash, with the result and reason for each', time: ['when'] },
};
// The filtered, sorted rows of each table (all pages), left by paginate() for the toolbar and the row buttons.
const TABLES = {};
const HEADERS = {}; // the columns each table's last sortHeader drew
const HIDDEN = {};
const COLS_STORE = 'table-cols:';
function hiddenOf(key) {
  if (!hasOwn(HIDDEN, key)) {
    let raw = null;
    try { raw = localStorage.getItem(COLS_STORE + key); } catch (e) { /* storage can be blocked; show every column */ }
    HIDDEN[key] = hiddenFrom(raw);
  }
  return HIDDEN[key];
}
function setHidden(key, list) {
  HIDDEN[key] = list;
  try { localStorage.setItem(COLS_STORE + key, JSON.stringify(list)); } catch (e) { /* the choice then lasts until reload */ }
}
const WIDTHS = {};
const COLW_STORE = 'table-colw:';
// Column id -> px, for columns a person has dragged wider or narrower. Missing or stale (a since-renamed or
// removed column id) just means that column uses its default track; applyColumnWidths ignores an id it no longer has.
function widthsOf(key) {
  if (!hasOwn(WIDTHS, key)) {
    let parsed = {};
    try {
      const raw = localStorage.getItem(COLW_STORE + key);
      if (raw) { const o = JSON.parse(raw); if (o && typeof o === 'object') parsed = o; }
    } catch (e) { /* storage blocked or corrupt; use default widths */ }
    WIDTHS[key] = parsed;
  }
  return WIDTHS[key];
}
function setWidth(key, id, px) {
  const cur = Object.assign({}, widthsOf(key));
  if (px == null) delete cur[id]; else cur[id] = px;
  WIDTHS[key] = cur;
  try { localStorage.setItem(COLW_STORE + key, JSON.stringify(cur)); } catch (e) { /* the choice then lasts until reload */ }
}
// An action column (buttons, not data) is part of the drawn table but is never exported or offered in the column chooser.
const exportCols = (key, header) => header.filter((c) => !c.action).map((c) => {
  const id = colId(c);
  return { id, label: c.label, get: (EXPORT[key] && EXPORT[key][id]) || (() => null), getJson: (EXPORT_JSON[key] && EXPORT_JSON[key][id]) || null, time: ((TABLE_META[key] || {}).time || []).includes(id) };
});
function download(name, mime, text) {
  const url = URL.createObjectURL(new Blob([text], { type: mime + ';charset=utf-8' }));
  const a = h('a', { href: url, download: name, style: 'display:none' });
  document.body.append(a); a.click(); a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 10000);
}
const rowsOf = (key) => (TABLES[key] ? TABLES[key].rows : []);
function exportTable(key, header, kind) {
  const cols = visibleCols(exportCols(key, header), hiddenOf(key)), rows = rowsOf(key);
  const text = kind === 'csv' ? toCsv(cols, rows) : toJson(cols, rows);
  download(fileName((TABLE_META[key] || {}).file || key, kind, new Date()), kind === 'csv' ? 'text/csv' : 'application/json', text);
}
let lastColToggle = null;
function columnsMenu(key, header) {
  const id = 'cols:' + key, open = S.menu === id, all = exportCols(key, header), hidden = hiddenOf(key), shown = visibleCols(all, hidden);
  return h('div', { class: 'sel' },
    h('button', { class: 'sel-btn', type: 'button', 'aria-haspopup': 'true', 'aria-expanded': String(open), 'data-trigger': id, title: 'Choose which columns this table shows and exports', onClick: (e) => { e.stopPropagation(); lastColToggle = null; rec({ menu: open ? null : id }); } }, h('span', {}, 'Columns'), h('span', { class: 'caret', 'aria-hidden': 'true' }, '▾')),
    open ? h('div', { class: 'sel-menu down colmenu', role: 'group', 'aria-label': 'Columns', onClick: (e) => e.stopPropagation(), onKeydown: (e) => { if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); rec({ menu: null }); const t = document.querySelector('[data-trigger="' + id + '"]'); if (t) t.focus(); } } },
      all.map((c, i) => h('label', { class: 'colopt' },
        h('input', { type: 'checkbox', checked: shown.includes(c), disabled: shown.length === 1 && shown[0] === c, 'data-autofocus': (lastColToggle === null ? i === 0 : lastColToggle === key + ':' + c.id) ? '' : null, onChange: () => { lastColToggle = key + ':' + c.id; setHidden(key, toggleHidden(all, hidden, c.id)); rec({}); } }), c.label))) : null);
}
function exportBar(key, header) {
  const n = rowsOf(key).length, meta = TABLE_META[key] || {}, note = exportNote(meta), open = !!S.xopen[key];
  const btn = (kind) => h('button', { class: 'btn sm', type: 'button', title: (n ? 'Saves ' + n + (n === 1 ? ' row' : ' rows') : 'Saves only the column names') + ' as ' + kind.toUpperCase() + ', with the shown columns and your filters and sort. ' + note.full, onClick: () => exportTable(key, header, kind) },
    n ? 'Export ' + kind.toUpperCase() + ' (' + n + (n === 1 ? ' row' : ' rows') + ')' : 'Nothing to export (' + kind.toUpperCase() + ')');
  // A real button, not hover text: it works from the keyboard, and the full text stays in the page (hidden, not removed).
  const more = h('button', { class: 'btn sm', type: 'button', id: 'xmore-' + key, 'aria-expanded': String(open), 'aria-controls': 'xinfo-' + key,
    onClick: () => { rec({ xopen: Object.assign({}, S.xopen, { [key]: !open }) }); const b = $('xmore-' + key); if (b) b.focus({ preventScroll: true }); } }, open ? 'Hide file details' : 'File details');
  return h('div', { class: 'xbar' }, btn('csv'), btn('json'), columnsMenu(key, header), h('span', { class: 'mut small xnote' }, note.line), more,
    h('p', { class: 'mut small xinfo', id: 'xinfo-' + key, hidden: !open }, note.full));
}
// Hidden columns and the row buttons are applied to the drawn table after each render, so every table gets
// both from its sortHeader call and no page draws them itself.
const GRID_DEFAULT = 'minmax(120px,1.6fr) 56px 56px minmax(80px,1.4fr) minmax(80px,1.2fr) 96px 96px';
// Splits a grid-template-columns string on top-level spaces only, so a track's own parentheses (minmax(...)) stay whole.
function splitTracks(str) {
  const out = []; let depth = 0, cur = '';
  for (const ch of str) {
    if (ch === '(') depth++; else if (ch === ')') depth--;
    if (ch === ' ' && !depth) { if (cur) out.push(cur); cur = ''; } else cur += ch;
  }
  return cur ? out.concat(cur) : out;
}
// The first call keeps the row's original grid, so a later pass (a window resize) never trims it twice.
function tracksOf(el) {
  if (el.dataset.grid == null) { const m = /grid-template-columns:\s*([^;]+)/.exec(el.getAttribute('style') || ''); el.dataset.grid = m ? m[1].trim() : GRID_DEFAULT; }
  return splitTracks(el.dataset.grid);
}
/* floor:begin */
// A track's smallest width: a fixed width, or the first argument of minmax(). The row's minimum width is the sum
// of these, the gaps and the padding (the last two are in the CSS), so no column is squeezed below its minimum.
const trackMin = (t) => { const m = /^(?:minmax\()?(\d+(?:\.\d+)?)px\b/.exec(t); return m ? Number(m[1]) : 0; };
const tracksMin = (tracks) => tracks.reduce((n, t) => n + trackMin(t), 0);
// At this width and up the layout puts the sidebar beside the page and the CSS clips tables (to keep the sticky header).
const SIDE_BY_SIDE = 1101;
// A table whose row is wider than its card must scroll sideways and be reachable by keyboard. Clipped tables
// (SIDE_BY_SIDE and up) also need the xscroll class to turn real scrolling back on.
const scrollState = (rowW, cardW, viewportW) => ({ region: rowW > cardW, xscroll: rowW > cardW && viewportW >= SIDE_BY_SIDE });
// Sizes the table's rows by their own tracks and makes a table wider than its card a labelled, focusable scroll region.
function setFloor(sec, head, tracks, name, viewportW) {
  sec.style.setProperty('--tracks-min', tracksMin(tracks) + 'px'); sec.style.setProperty('--cols', tracks.length);
  sec.classList.add('floor');
  const st = scrollState(head.offsetWidth, sec.clientWidth, viewportW);
  sec.classList.toggle('xscroll', st.xscroll);
  if (st.region) { sec.setAttribute('tabindex', '0'); sec.setAttribute('role', 'region'); sec.setAttribute('aria-label', name + ', scrollable'); }
  else { sec.removeAttribute('tabindex'); sec.removeAttribute('role'); sec.removeAttribute('aria-label'); }
}
// The name cell is cut with an ellipsis in a narrow column, so its full text is also the tooltip.
const nameAttrs = (text, style) => ({ style: 'font-weight:500' + (style ? ';' + style : ''), title: String(text) });
/* floor:end */
/* colresize:begin */
// Only a flexible (fr) track is resizable by hand; a fixed-width track (an icon or action column) stays put.
const isFlexTrack = (t) => t.includes('fr');
// Replaces a flexible track with a fixed px width from `widths` (column id -> px), clamped to that track's own
// minimum. A fixed track, or an id missing from `widths` (never dragged, or a stale id from a renamed column), is
// returned unchanged. The override itself then becomes that column's new minimum for setFloor.
function applyColumnWidths(tracks, ids, widths) {
  return tracks.map((t, i) => {
    const w = widths[ids[i]];
    if (w == null || !isFlexTrack(t)) return t;
    return Math.max(trackMin(t), Math.round(w)) + 'px';
  });
}
// A dragged or arrow-keyed width never goes below the column's own minimum.
const clampDrag = (startW, minW, delta) => Math.max(minW, startW + delta);
/* colresize:end */
const warned = new Set();
const TABLE_NAME = { sudo: 'sudo and su launches', auth: 'Authorization requests', apps: 'Apps', listen: 'Listeners', pers: 'Persistence startup items', net: 'Network connections', hist: 'New hosts', act: 'Actions', trash: 'Moves to the Trash', store: 'Storage folders' };
function decorateTables() {
  for (const head of document.querySelectorAll('.tr.h[data-table]')) {
    const key = head.dataset.table, sec = head.parentElement, ids = head.dataset.cols.split(',');
    const rows = [...sec.querySelectorAll(':scope > .tr:not(.h)')], slice = TABLES[key] ? TABLES[key].page : [];
    for (const old of sec.querySelectorAll(':scope > .rowcopy-layer')) old.remove();
    const headTracks = applyColumnWidths(tracksOf(head), ids, widthsOf(key));
    setFloor(sec, head, headTracks, TABLE_NAME[key] || key, window.innerWidth);
    // Position decides which cell is hidden and which record a button copies, so a table that does not line up is left alone.
    if (head.children.length !== ids.length || !tableAligned(ids.length, rows.map((r) => r.children.length), slice.length)) {
      if (!warned.has(key)) { warned.add(key); console.warn('table "' + key + '": drawn rows do not match its data; column hiding and row copy are off'); }
      continue;
    }
    const shown = visibleCols(ids.map((id) => ({ id })), hiddenOf(key)).map((c) => c.id);
    for (const r of [head, ...rows]) {
      const tracks = applyColumnWidths(tracksOf(r), ids, widthsOf(key));
      [...r.children].forEach((c, i) => { c.style.display = shown.includes(ids[i]) ? '' : 'none'; });
      r.style.gridTemplateColumns = tracks.filter((_, i) => shown.includes(ids[i])).join(' ');
    }
    sec.classList.add('has-copy');
    setFloor(sec, head, headTracks.filter((_, i) => shown.includes(ids[i])), TABLE_NAME[key] || key, window.innerWidth);
    sec.append(h('div', { class: 'rowcopy-layer', style: 'width:' + sec.scrollWidth + 'px' }, rows.map((r, i) =>
      h('button', { class: 'rowcopy', type: 'button', style: 'top:' + (r.offsetTop + Math.max(0, (r.offsetHeight - 24) / 2)) + 'px', title: 'Copy this row as JSON', 'aria-label': 'Copy row ' + (i + 1) + ' as JSON', onClick: () => copyRow(key, slice[i]) }, 'Copy'))));
  }
}
let decoRaf = 0;
window.addEventListener('resize', () => { if (!decoRaf) decoRaf = requestAnimationFrame(() => { decoRaf = 0; decorateTables(); }); });
// The shown columns of one row, as JSON a person can paste elsewhere.
function copyRow(key, row) {
  copyText(JSON.stringify(rowObject(visibleCols(exportCols(key, HEADERS[key] || []), hiddenOf(key)), row), null, 2));
}
function copyText(text) {
  const done = () => { rec({ toast: 'Copied as JSON.', copyText: null }); setTimeout(() => { if (S.toast === 'Copied as JSON.') quiet({ toast: null }); }, 3000); };
  // Clipboard access can be refused (no permission, page not focused): then show the text selected, ready for Cmd+C.
  const fallback = () => { rec({ copyText: text, toast: null }); const t = document.querySelector('.copybox textarea'); if (t) { t.focus(); t.select(); } };
  try { navigator.clipboard.writeText(text).then(done, fallback); } catch (e) { fallback(); }
}
function sortHeader(key, cols, style, extra) {
  const s = S.sorts[key];
  HEADERS[key] = cols;
  // An export-only column is in the export and the column chooser but is not drawn.
  const drawn = cols.filter((c) => !c.exportOnly);
  const rowTracks = splitTracks((style && (/grid-template-columns:\s*([^;]+)/.exec(style) || [])[1] || GRID_DEFAULT).trim());
  const head = h('div', { class: 'tr h ' + (extra || ''), style: style || '', 'data-table': key, 'data-cols': drawn.map(colId).join(',') }, drawn.map((c, i) => {
    const id = colId(c), grip = colGrip(key, id, c.label, rowTracks[i]);
    if (!c.sort) return h('span', { class: 'cwrap ' + (c.cls || '') }, c.label, grip);
    const on = !!s && s.col === c.sort;
    return h('span', { class: 'cwrap ' + (c.cls || '') }, h('button', { class: 'th' + (on ? ' on' : ''), type: 'button', title: c.title || 'Sort by ' + c.label, onClick: () => rec({ sorts: nextSort(key, c.sort), page: {} }) },
      c.label, h('span', { class: 'arrow', 'aria-hidden': 'true' }, on ? (s.dir === 'asc' ? '\u25B2' : '\u25BC') : '')), grip);
  }));
  return [exportBar(key, cols), head];
}
// A drag/keyboard handle on a flexible column's right edge; a fixed-width column (an icon or action column) gets none.
function colGrip(key, id, label, track) {
  if (!track || !isFlexTrack(track)) return null;
  return h('span', { class: 'colgrip', role: 'separator', 'aria-orientation': 'vertical', 'aria-label': 'Resize the ' + label + ' column', tabindex: '0',
    onMousedown: (e) => startColResize(e, key, id),
    onKeydown: (e) => colHandleKeys(e, key, id),
    onDblclick: (e) => { e.stopPropagation(); setWidth(key, id, null); render(); } });
}
const COL_STEP = 16;
// Shared by every pointer drag on this page (a column grip, the drawer handle): wires the mousemove/mouseup/blur
// listeners and guards against a second mousedown or a missed mouseup leaking the previous drag's listeners. Only
// one drag can be in progress at a time, so one guard covers the drawer and every column.
let activeDragEnd = null;
function beginDrag(onMove, onUp) {
  if (activeDragEnd) activeDragEnd();
  const stop = () => {
    document.removeEventListener('mousemove', onMove); document.removeEventListener('mouseup', stop); window.removeEventListener('blur', stop);
    activeDragEnd = null;
    onUp();
  };
  activeDragEnd = stop;
  document.addEventListener('mousemove', onMove); document.addEventListener('mouseup', stop); window.addEventListener('blur', stop);
}
// Drag a column's right edge; mirrors startDrawerResize (live DOM patch per frame, one render() on release). Every
// row of the table is patched at once, from each row's own tracks (tracksOf caches per node, same as decorateTables).
function startColResize(e, key, id) {
  e.preventDefault(); e.stopPropagation();
  const handleEl = e.currentTarget, headEl = handleEl.closest('.tr.h'); if (!headEl) return;
  const sec = headEl.parentElement, ids = headEl.dataset.cols.split(','), i = ids.indexOf(id);
  if (i < 0) return;
  const rows = [headEl, ...sec.querySelectorAll(':scope > .tr:not(.h)')];
  const startX = e.clientX, startW = headEl.children[i].getBoundingClientRect().width, minW = trackMin(tracksOf(headEl)[i]);
  const shown = visibleCols(ids.map((cid) => ({ id: cid })), hiddenOf(key)).map((c) => c.id);
  let raf = 0, pending = startW;
  const paint = () => {
    raf = 0;
    const widths = Object.assign({}, widthsOf(key), { [id]: pending });
    for (const r of rows) r.style.gridTemplateColumns = applyColumnWidths(tracksOf(r), ids, widths).filter((_, k) => shown.includes(ids[k])).join(' ');
    handleEl.setAttribute('aria-valuenow', Math.round(pending));
  };
  beginDrag(
    (ev) => { pending = clampDrag(startW, minW, ev.clientX - startX); if (!raf) raf = requestAnimationFrame(paint); },
    () => { if (raf) cancelAnimationFrame(raf); setWidth(key, id, Math.round(pending)); render(); },
  );
}
function colHandleKeys(e, key, id) {
  if (e.key !== 'ArrowLeft' && e.key !== 'ArrowRight') return;
  e.preventDefault();
  const headEl = e.currentTarget.closest('.tr.h'); if (!headEl) return;
  const ids = headEl.dataset.cols.split(','), i = ids.indexOf(id); if (i < 0) return;
  const cur = Math.round(headEl.children[i].getBoundingClientRect().width), minW = trackMin(tracksOf(headEl)[i]);
  setWidth(key, id, clampDrag(cur, minW, e.key === 'ArrowLeft' ? -COL_STEP : COL_STEP));
  render();
}
const sigKind = (g) => (!g.signing ? 'unchecked' : g.signing.apple ? 'apple' : g.signing.signed ? 'signed' : 'unsigned');
const listenText = (g) => [...new Set(g.conns.filter((c) => c.direction === 'listen').map((c) => c.local_port + (c.exposed ? '*' : '')))].join(', ') || '-';
const signText = (g) => (!g.signing ? '-' : g.signing.apple ? 'Apple' : g.signing.signed ? 'Signed ' + (g.signing.team || '') : 'UNSIGNED');
const TRUST_RANK = { suspicious: 0, unknown: 1, trusted: 2 }; // ascending puts the most worrying first
const DRAWER_MIN = 320, DRAWER_MAX = 720, DRAWER_STEP = 16;
const clampDrawer = (w) => Math.min(DRAWER_MAX, Math.max(DRAWER_MIN, w));
function setDrawerWidth(w) { S.drawerW = clampDrawer(w); localStorage.setItem('drawer-width', S.drawerW); render(); }
// Drag the boundary between the table and the app detail panel; both resize together because the table has flex-grow
// and the panel's width is fixed while dragging. Keyboard: ArrowLeft/ArrowRight on the focused handle.
// A full render() per mousemove would replace the handle's DOM node and drop its focus, and is wasteful at mouse
// rate; instead the live elements are nudged directly while dragging, and one render() settles everything on release.
function startDrawerResize(e) {
  e.preventDefault();
  const startX = e.clientX, startW = S.drawerW || document.querySelector('.drawer')?.offsetWidth || 380;
  const handleEl = e.currentTarget, hadFocus = document.activeElement === handleEl;
  let raf = 0, pending = startW;
  const paint = () => {
    raf = 0;
    const el = document.querySelector('.drawer'); if (el) { el.classList.add('sized'); el.style.setProperty('--dw', pending + 'px'); }
    handleEl.setAttribute('aria-valuenow', pending);
  };
  beginDrag(
    (ev) => { pending = clampDrawer(startW + (startX - ev.clientX)); if (!raf) raf = requestAnimationFrame(paint); },
    () => {
      if (raf) cancelAnimationFrame(raf);
      S.drawerW = pending; localStorage.setItem('drawer-width', S.drawerW); render();
      if (hadFocus) document.querySelector('.handle')?.focus({ preventScroll: true });
    },
  );
}
function drawerHandleKeys(e) {
  if (e.key === 'ArrowLeft') { e.preventDefault(); setDrawerWidth((S.drawerW || 380) + DRAWER_STEP); }
  else if (e.key === 'ArrowRight') { e.preventDefault(); setDrawerWidth((S.drawerW || 380) - DRAWER_STEP); }
}

