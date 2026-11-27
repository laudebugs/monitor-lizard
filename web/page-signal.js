/* signal:begin */
// Freeze, resume or stop the process that requested sudo, on the Privilege page: the severity of a hop, which buttons
// a hop's live state allows, and the dialog as a pure state machine (`sigStep(state, event)` returns the next state and
// the effects the page must run). No DOM, no storage, no fetch: a Node test loads this block alone with the `requester`
// block it uses (visible, hopLabel, appOfHop, isInterpreter, baseName, TERMINAL_APPS, cutText).
// Phases: idle, previewing, previewed, confirming-danger, sending, done, error. The token lives only in `previewed` and
// `confirming-danger`; every other phase drops it.
const SIG_MAX = 12; // src/requester_signal.rs MAX_ITEMS
const SIG_ACTIONS = {
  freeze: { label: 'Freeze', verb: 'freeze', done: 'frozen', signal: 'SIGSTOP', effect: 'Freeze pauses it where it is (SIGSTOP). It does nothing until you resume it. Anything it started keeps running.' },
  resume: { label: 'Resume', verb: 'resume', done: 'resumed', signal: 'SIGCONT', effect: 'Resume lets it carry on (SIGCONT).' },
  stop: { label: 'Stop', verb: 'stop', done: 'stopped', signal: 'SIGTERM', effect: 'Stop asks it to end (SIGTERM). It may save its work first, or ignore the request. A frozen process is also resumed so it can hear the request.' },
  force_stop: { label: 'Force stop', verb: 'force stop', done: 'force stopped', signal: 'SIGKILL', effect: 'Force stop ends it at once (SIGKILL). It cannot save anything.' },
};
const SIG_ORDER = ['freeze', 'resume', 'stop', 'force_stop'];
// The owner's own session, by program name, app bundle and interpreter. The Rust side holds the same lists
// (src/requester_signal.rs DANGER_PROGRAMS, DANGER_BUNDLES, INTERPRETERS) and enforces the second confirmation on the
// server; tests/fixtures/danger-hops.json is checked by both, lists and cases.
const SIG_DANGER_PROGRAMS = ['sh', 'bash', 'zsh', 'fish', 'dash', 'ksh', 'tcsh', 'csh', 'login', 'su', 'nu', 'pwsh', 'xonsh', 'elvish', 'ssh', 'sshd', 'mosh-server', 'tmux', 'screen', 'zellij', 'claude', 'npx', 'bunx', 'tsx', 'ts-node', 'electron'];
const SIG_DANGER_BUNDLES = ['terminal', 'iterm', 'iterm2', 'ghostty', 'warp', 'alacritty', 'wezterm', 'kitty', 'hyper', 'tabby', 'rio', 'visual studio code', 'visual studio code - insiders', 'cursor', 'windsurf', 'zed', 'zed preview', 'sublime text', 'nova', 'xcode', 'intellij idea', 'intellij idea ce', 'pycharm', 'pycharm ce', 'webstorm', 'goland', 'clion', 'rubymine', 'phpstorm', 'rider', 'datagrip', 'android studio', 'claude', 'claudecode'];
const SIG_INTERPRETERS = ['node', 'bun', 'deno', 'python', 'python3', 'ruby', 'perl'];
const SIG_LEVEL = { plain: 0, caution: 1, danger: 2 };
const sigPlural = (n, one, many) => n + ' ' + (n === 1 ? one : many);
const sigName = (hop) => baseName(String((hop && hop.exe) || String((hop && hop.command) || '').split(/\s+/)[0] || '')).replace(/^-+/, '');
const sigIsInterp = (name) => SIG_INTERPRETERS.includes(name) || /^python[0-9.]*$/.test(name);
// Why the hop is the owner's own session, or null. The same rule as `danger_reason` in Rust: the file name of the executable
// or of the first word of the command, an app bundle anywhere on either path, a word naming Claude Code (a `claude` folder
// counts: a native install runs from `.../claude/versions/<version>`), and, for an interpreter, a command line that is
// missing. What cannot be read is treated as the owner's own session.
const sigClaudeWord = (w) => w === 'claude' || w === 'claude-code' || w.startsWith('claude.');
function sigDangerWhy(hop) {
  const exe = String((hop && hop.exe) || ''), cmd = String((hop && hop.command) || '').trim();
  if (!exe) return 'its program is not known';
  const argv0 = cmd.split(/\s+/)[0] || '';
  if ([baseName(exe), baseName(argv0)].some((n) => SIG_DANGER_PROGRAMS.includes(n.replace(/^-+/, '')))) return 'a shell, login, multiplexer, launcher or Claude session';
  if ([...exe.split('/'), ...cmd.split('/')].some((seg) => seg.endsWith('.app') && SIG_DANGER_BUNDLES.includes(seg.slice(0, -4).toLowerCase()))) return 'a terminal app, an editor or IDE with a terminal, or Claude';
  if ([...exe.split('/'), ...cmd.split(/[\s/]+/)].some(sigClaudeWord)) return 'a Claude session';
  if (sigIsInterp(baseName(exe).replace(/^-+/, '')) && !cmd) return 'its command line could not be read, so what it runs is unknown';
  return null;
}
// What the hop is, as far as the person is concerned. `danger` is the owner's own session; `caution` is a script, anything
// with children, or anything whose children are not known yet (`childCount` not a number: no live status has arrived, and
// nothing is plain by default); the rest is `plain`. `why` says which, in words.
function sigSeverity(hop, childCount) {
  const danger = sigDangerWhy(hop), name = sigName(hop), known = Number.isInteger(childCount), why = [];
  if (danger) return { level: 'danger', why: [danger], unknownKids: !known };
  if (isInterpreter(name)) why.push('a script run by ' + name);
  if (!known) why.push('what runs under it is not known yet');
  else if (childCount > 0) why.push('it has ' + sigPlural(childCount, 'child process', 'child processes'));
  return { level: why.length ? 'caution' : 'plain', why, unknownKids: !known };
}
// Resuming never harms a process, so it never asks for the extra step.
const sigLevel = (action, items) => (action === 'resume' ? 'plain' : items.reduce((m, i) => (SIG_LEVEL[i.level] > SIG_LEVEL[m] ? i.level : m), 'plain'));
const sigKey = (hop) => hop.pid + ':' + (typeof hop.started === 'number' ? hop.started : '') + ':' + (hop.exe || '');
const sigItem = (hop, live) => {
  const sev = sigSeverity(hop, live && live.children_total);
  return { pid: hop.pid, started: typeof hop.started === 'number' ? hop.started : null, exe: String(hop.exe || ''), cmd: String(hop.command || ''), label: hopLabel(hop), command: cutText(hop.command || hop.exe || ''), level: sev.level, why: sev.why, unknownKids: sev.unknownKids };
};
// The hops a request names, child first: the one hop, or the chain from the process that ran sudo up to this hop.
const sigItems = (chain, i, scope, liveMap) => (scope === 'chain' ? chain.slice(0, i + 1) : [chain[i]]).map((hop) => sigItem(hop, liveMap && liveMap[sigKey(hop)]));
const sigSelectionKey = (action, scope, items) => action + '|' + scope + '|' + items.map((i) => i.pid + ':' + i.started + ':' + i.exe).join(',');
// Whether a button can be pressed for this hop, from what the server last said about it (`live`, one item of
// GET /api/requester/status) and what the captured hop recorded. `reason` is plain text the page shows beside the buttons.
function sigAvail(hop, live, action) {
  const verb = SIG_ACTIONS[action].verb;
  if (typeof hop.started !== 'number' || !hop.exe) return { ok: false, reason: 'No start time or executable was recorded for this process, so it cannot be checked' };
  if (!live) return { ok: false, reason: 'Checking whether it is still running', checking: true };
  const code = typeof live.code === 'string' ? live.code : null;
  if (code === 'gone') return { ok: false, reason: 'Already exited, nothing to ' + verb };
  if (code === 'pid_reused') return { ok: false, reason: 'This pid now belongs to a different process' };
  if (code === 'not_a_chain') return { ok: false, reason: 'It is no longer the parent of the process below it' };
  if (code === 'chain_exited') return { ok: false, reason: 'A process below it in this chain has exited, so the chain cannot be checked. Use one process at a time' };
  // Stopped by something else (^Z, a debugger): the server never continues it, so only Freeze's refusal and Force stop apply.
  if (live.frozen_by_us === false && (code === 'already_frozen' || code === 'not_frozen_by_us' || live.state === 'frozen')) {
    if (action === 'force_stop') return { ok: true, reason: '' };
    return { ok: false, reason: action === 'freeze' ? 'Already stopped by something else' : 'Something else stopped it (job control or a debugger), so it will not be continued from here. Force stop still works' };
  }
  if (code === 'already_frozen' || live.state === 'frozen') return action === 'freeze' ? { ok: false, reason: 'Already frozen. Resume it instead' } : { ok: true, reason: '' };
  if (code === 'needs_danger_confirmation') return { ok: true, reason: '' };
  if (code === 'not_frozen' || (live.state === 'running' && !code)) return action === 'resume' ? { ok: false, reason: 'Not frozen, so there is nothing to resume' } : { ok: true, reason: '' };
  if (code) return { ok: false, reason: typeof live.reason === 'string' && live.reason ? visible(live.reason) : 'It cannot be signalled from here' };
  return { ok: false, reason: 'Its state could not be read' };
}
// Chain buttons are enabled when at least one process of the chain can take the action; the dialog says which ones will not.
const SIG_CHAIN_EXITED = 'A process in this chain has exited or been replaced, so the chain cannot be checked. Use one process at a time';
const sigChainAvail = (chain, i, liveMap, action) => {
  const lives = chain.slice(0, i + 1).map((hop) => liveMap && liveMap[sigKey(hop)]);
  // The preview would refuse every item above an exited one (chain_exited, or not_a_chain after a change), so the button is off the same way.
  if (lives.some((l, j) => l && (l.code === 'gone' || l.code === 'pid_reused' || (j > 0 && (l.chain_code === 'chain_exited' || l.chain_code === 'not_a_chain'))))) return { ok: false, reason: SIG_CHAIN_EXITED };
  const parts = chain.slice(0, i + 1).map((hop, j) => sigAvail(hop, lives[j], action));
  return parts.some((p) => p.ok) ? { ok: true, reason: '' } : parts[0];
};
const sigUrl = (fx) => '/api/requester/signal?action=' + fx.action + '&scope=' + fx.scope + fx.items.map((i) => '&item=' + encodeURIComponent(i.pid + ':' + (i.started == null ? '' : i.started) + ':' + i.exe)).join('')
  + (fx.confirmDanger ? '&confirm_danger=1' : '') + (fx.token ? '&dry=0&confirm_token=' + encodeURIComponent(fx.token) : '&dry=1' + (fx.replaces ? '&replaces=' + encodeURIComponent(fx.replaces) : ''));
// What a preview showed, for comparing the flagged preview with the one the person confirmed: the processes, their levels, what
// each would do, their state and what runs under them. A held-back hop reads the same as one that goes.
const sigSignature = (sum, items) => JSON.stringify([items.map((i) => [i.pid, i.level]), sum.items.map((i) => [i.pid, i.status, i.waiting ? null : i.code, i.state, i.childrenTotal, i.children.map((c) => c.command)])]);
const SIG_STATUSES = ['would_signal', 'signalled', 'refused', 'failed'];
const sigNum = (n) => (typeof n === 'number' && Number.isFinite(n) && n >= 0 ? n : null);
// One reply read defensively. Counts come from the listed items, so the list and the numbers cannot disagree, and only a
// `signalled` status is ever counted as done.
function sigSummary(body, known) {
  const b = body && typeof body === 'object' && !Array.isArray(body) ? body : {};
  const items = (Array.isArray(b.items) ? b.items : []).filter((i) => i && typeof i === 'object').map((i) => {
    const mine = known && known[i.pid];
    const kids = (Array.isArray(i.children) ? i.children : []).filter((c) => c && typeof c === 'object').map((c) => ({ pid: sigNum(c.pid), command: visible(c.command), frozen: c.frozen === true }));
    return { pid: i.pid, status: SIG_STATUSES.includes(i.status) ? i.status : 'failed', code: typeof i.code === 'string' ? visible(i.code) : null, reason: typeof i.reason === 'string' ? visible(i.reason) : null,
      state: i.state === 'frozen' || i.state === 'running' ? i.state : null, signals: (Array.isArray(i.signals) ? i.signals : []).filter((x) => typeof x === 'string').map(visible), children: kids,
      childrenTotal: sigNum(i.children_total) == null ? kids.length : i.children_total, label: mine ? mine.label : 'pid ' + visible(i.pid), command: mine ? mine.command : '', level: mine ? mine.level : 'plain', why: mine ? mine.why : [] };
  });
  // A preview that holds back your own session until the second confirmation is not a refusal: the item waits for it.
  const dry = b.dry_run !== false;
  items.forEach((i) => { i.waiting = dry && i.status === 'refused' && i.code === 'needs_danger_confirmation'; if (i.waiting) i.status = 'would_signal'; });
  const counts = Object.fromEntries(SIG_STATUSES.map((s) => [s, items.filter((i) => i.status === s).length]));
  return { items, counts, waiting: items.some((i) => i.waiting), token: typeof b.confirm_token === 'string' && b.confirm_token ? b.confirm_token : null, dryRun: dry, okFlag: b.ok === true,
    auditError: typeof b.audit_error === 'string' && b.audit_error ? visible(b.audit_error) : null, hasItems: items.length > 0 };
}
// A refusal of the whole request, as a plain sentence.
function sigProblem(status, code, committing) {
  const again = ' Nothing was signalled. Preview again to continue.';
  const table = {
    confirmation_expired: 'The preview is older than 10 minutes and has expired.' + again,
    confirmation_mismatch: 'The action or the processes changed after the preview, so it no longer applies.' + again,
    confirmation_invalid: 'The preview is no longer valid (it may already have been used).' + again,
    confirmation_required: 'The server needs a preview of exactly these processes first.' + again,
    too_many_previews: 'Too many previews are waiting. Wait up to 10 minutes for old previews to expire, or restart the dashboard.',
    audit_unavailable: 'Nothing was signalled. The dashboard could not confirm it can keep a record in History. Restart the dashboard and try again.',
    busy: 'Another request to stop or freeze is still running. Nothing was signalled. Wait for it to finish, then preview again.',
    too_many_items: 'A request can name at most ' + SIG_MAX + ' processes. Nothing was signalled.',
  };
  if (typeof code === 'string' && Object.prototype.hasOwnProperty.call(table, code)) return table[code];
  if (status === 0) return committing ? 'The page could not read the server’s reply, so it cannot say whether anything was signalled. Open History to check, then preview again.' : 'The page could not reach the server. Nothing was signalled. Preview again when the dashboard is running.';
  if (status === 429) return table.too_many_previews;
  if (status === 409) return 'The server could not use the preview (' + visible(code || 'conflict') + ').' + again;
  return (committing ? 'The server reported an error (' + status + (code ? ', ' + visible(code) : '') + ') without saying what happened. Open History to check, then preview again.'
    : 'The server could not preview this (' + status + (code ? ', ' + visible(code) : '') + '). Nothing was signalled.');
}
const sigRetryable = (code) => !['bad_action', 'bad_scope', 'bad_items', 'too_many_items', 'bad_dry', 'repeated_parameter'].includes(code);
const sigIdle = () => ({ phase: 'idle', seq: 0 });
function sigTitle(s) {
  const a = SIG_ACTIONS[s.action], n = s.items.length;
  return n === 1 ? a.label + ' ' + s.items[0].label + '?' : a.label + ' ' + n + ' processes in this chain?';
}
// What each level says before anything is sent. `kids` is the fresh list of children from the preview.
function sigWarning(s, kids) {
  const a = SIG_ACTIONS[s.action], danger = s.items.filter((i) => i.level === 'danger'), caution = s.items.filter((i) => i.level === 'caution'), out = [];
  if (s.level === 'danger') {
    const names = danger.map((i) => i.label + ' (' + i.why.join(', ') + ')').join('; ');
    out.push(s.action === 'freeze' ? 'This is your own session: ' + names + '. Freezing it hangs the window it runs in until you resume it, and you may have to do that from another terminal or from this page.'
      : 'This is your own session: ' + names + '. Stopping it ends the session, and everything running in it ends too.');
    if (kids > 0) out.push('Altogether ' + sigPlural(kids, 'process runs', 'processes run') + ' under the ' + (s.items.length === 1 ? 'process' : 'processes') + ' listed below.');
  } else if (s.level === 'caution') {
    out.push(caution.map((i) => i.label + ': ' + i.why.join(', ')).join('; ') + '.');
    out.push(s.action === 'freeze' ? 'Freezing leaves its child processes running while it waits.' : 'Stopping it may end its child processes too, and a script may lose work that was not saved.');
  }
  out.push(a.effect);
  return out;
}
function sigHeadline(sum, s, preview) {
  const a = SIG_ACTIONS[s.action], c = sum.counts, n = sum.items.length;
  if (preview) return c.would_signal ? sigPlural(c.would_signal, 'process', 'processes') + ' will be ' + a.done + '.' : 'None of the ' + sigPlural(n, 'process', 'processes') + ' can be ' + a.done + '.';
  if (c.signalled === 0) return 'Nothing was ' + a.done + (n ? ': ' + (n === 1 ? 'it was ' : 'none of the ' + n + ' processes was ') + (c.failed ? 'not signalled' : 'refused') : '') + '.';
  return c.signalled + ' of ' + sigPlural(n, 'process', 'processes') + ' ' + a.done + '.';
}
function sigStep(s, ev) {
  const none = { state: s, fx: [] }, say = (text) => ({ do: 'announce', text }), phase = s.phase;
  const ask = (state, confirmDanger) => ({ state: Object.assign({}, state, { phase: 'previewing', seq: s.seq + 1, token: null, summary: null, message: null, changed: false, confirmDanger: !!confirmDanger }),
    fx: [{ do: 'preview', seq: s.seq + 1, action: state.action, scope: state.scope, items: state.items, confirmDanger: !!confirmDanger, replaces: confirmDanger ? state.token : null }, say('Checking ' + sigPlural(state.items.length, 'process', 'processes') + '. Nothing is signalled yet.')] });
  const send = (state) => ({ state: Object.assign({}, state, { phase: 'sending', changed: false, seq: s.seq + 1 }),
    fx: [{ do: 'commit', seq: s.seq + 1, action: state.action, scope: state.scope, items: state.items, token: state.token, confirmDanger: !!state.confirmDanger }, say(SIG_ACTIONS[state.action].label + ' sent to ' + sigPlural(state.items.length, 'process', 'processes') + '. Waiting for the result.')] });
  switch (ev.type) {
    case 'open': {
      if (phase !== 'idle' && phase !== 'done') return none;
      const items = ev.items || [];
      if (!SIG_ACTIONS[ev.action] || !items.length || items.length > SIG_MAX) return { state: s, fx: [say('Nothing to preview')] };
      const known = Object.fromEntries(items.map((i) => [i.pid, i]));
      return ask({ phase: 'previewing', seq: s.seq, action: ev.action, scope: ev.scope, items, known, level: sigLevel(ev.action, items), key: sigSelectionKey(ev.action, ev.scope, items), rowId: ev.rowId || null, origin: ev.origin || null }, false);
    }
    case 'reply': {
      if ((phase !== 'previewing' && phase !== 'sending') || ev.seq !== s.seq) return none;
      const committing = phase === 'sending', body = ev.body, sum = sigSummary(body, s.known), status = ev.status | 0;
      const code = body && typeof body === 'object' && typeof body.code === 'string' ? body.code : null;
      const readable = status === 200 && body && typeof body === 'object' && sum.hasItems;
      const partial = committing && status >= 500 && sum.hasItems; // a 500 that still lists items says what was sent
      if (readable && !committing && sum.dryRun) {
        // What the server just read decides the level again: fresh children make a process caution, and a hop the server
        // held back for the second confirmation is your own session whatever the page guessed.
        const items = s.items.map((i) => {
          const r = sum.items.find((x) => x.pid === i.pid);
          if (!r) return i;
          if (r.waiting) return Object.assign({}, i, { level: 'danger', why: i.why.length && i.level === 'danger' ? i.why : ['the server treats it as your own session'] });
          if (r.status !== 'would_signal') return i;
          if (i.level === 'danger') return Object.assign({}, i, { unknownKids: false });
          const sev = sigSeverity({ exe: i.exe, command: i.cmd }, r.childrenTotal);
          return Object.assign({}, i, { level: sev.level, why: sev.why, unknownKids: false });
        });
        const level = sigLevel(s.action, items), token = sum.token && sum.counts.would_signal > 0 ? sum.token : null;
        const state = Object.assign({}, s, { phase: 'previewed', summary: sum, items, level, token, message: null });
        // The second confirmation was given before this preview, which carried the flag. If it shows what the person confirmed,
        // send it now, once. If anything differs, show it and ask again: the confirmation was for the other list.
        if (s.confirmDanger && token && sum.counts.would_signal > 0) {
          if (sigSignature(sum, items) === s.sig0) return send(state);
          return { state: Object.assign({}, state, { changed: true }), fx: [say('The check changed after you confirmed. Review the list, then continue.')] };
        }
        return { state, fx: [say(sigHeadline(sum, s, true) + (sum.counts.refused ? ' ' + sigPlural(sum.counts.refused, 'process is', 'processes are') + ' refused.' : ''))] };
      }
      if ((readable && (committing || !sum.dryRun)) || partial) {
        return { state: Object.assign({}, s, { phase: 'done', summary: sum, token: null, message: null }), fx: [say(sigHeadline(sum, s, false)), { do: 'refresh' }] };
      }
      const text = committing && status === 200 ? 'The server’s reply could not be read, so it cannot say whether anything was signalled. Open History to check, then preview again.' : sigProblem(status, code, committing);
      return { state: Object.assign({}, s, { phase: 'error', token: null, summary: null, message: { text, retry: sigRetryable(code) } }), fx: [say(text), committing ? { do: 'refresh' } : null].filter(Boolean) };
    }
    case 'go': {
      if (phase !== 'previewed' || !(s.token || (s.summary && s.summary.waiting))) return none; // a second click while sending, or before a preview, does nothing
      // Your own session needs a second, separate confirmation, and the server wants the flag on a fresh preview as well.
      if (s.level === 'danger' && !s.confirmDanger) return { state: Object.assign({}, s, { phase: 'confirming-danger', sig0: s.summary ? sigSignature(s.summary, s.items) : '' }), fx: [say('This is your own session. Confirm it separately.')] };
      return s.token ? send(s) : none;
    }
    case 'danger-yes': return phase === 'confirming-danger' ? ask(s, true) : none;
    case 'danger-back': return phase === 'confirming-danger' ? { state: Object.assign({}, s, { phase: 'previewed' }), fx: [] } : none;
    case 'again': return phase === 'error' ? ask(s) : none;
    case 'close': {
      if (phase === 'idle') return none;
      if (phase === 'sending') return { state: s, fx: [say('The request is running. Wait for the result.')] };
      return { state: { phase: 'idle', seq: s.seq }, fx: [{ do: 'focus', origin: s.origin }, say(phase === 'done' ? 'Closed.' : 'Closed. Nothing was signalled.')] };
    }
    case 'selection': {
      if (!['previewing', 'previewed', 'confirming-danger', 'error'].includes(phase) || ev.key === s.key) return none;
      return { state: { phase: 'idle', seq: s.seq }, fx: [{ do: 'focus', origin: s.origin }, say('The dialog closed because the selection changed. Nothing was signalled.')] };
    }
    case 'leave': {
      if (phase === 'idle' || phase === 'sending') return none;
      return { state: { phase: 'idle', seq: s.seq }, fx: [{ do: 'focus', origin: s.origin }, say('The dialog closed because you left the Privilege page. Nothing was signalled.')] };
    }
    default: return none;
  }
}
/* signal:end */
/* signal-view:begin */
// The buttons under each hop and the dialog around the state machine above. Text a stranger can choose (a command line,
// an executable path, a child's command, a server reason) goes through visible() and into text nodes, never markup. The
// page sends the executable path and start time of each hop as the collector captured them, through api() (the per-run
// token header); nothing about a request is put in the address.
const sigOpen = () => !!(S.sig && S.sig.phase !== 'idle');
function sigFocusBack(origin) {
  const el = (origin && [...document.querySelectorAll('[data-sig]')].find((x) => x.dataset.sig === origin)) || document.querySelector('[data-sig]') || $('main');
  if (el) el.focus();
}
// Runs the machine and its effects without drawing. Returns the origin to give focus back to, or null.
function sigApply(ev) {
  const r = sigStep(S.sig || sigIdle(), ev);
  S.sig = r.state;
  let back;
  for (const fx of r.fx) {
    if (fx.do === 'announce') announce(fx.text);
    else if (fx.do === 'preview' || fx.do === 'commit') sigRequest(fx);
    else if (fx.do === 'refresh') loadReqLive();
    else if (fx.do === 'focus') back = fx.origin || '';
  }
  return back;
}
function sigDispatch(ev) {
  const back = sigApply(ev);
  render();
  if (back != null) sigFocusBack(back);
}
async function sigRequest(fx) {
  let status = 0, body = null;
  try { const r = await api(sigUrl(fx), 'POST'); status = r.status; body = await r.json(); }
  catch (e) { if (e && e.status) { status = e.status; try { body = JSON.parse(e.text); } catch (_) { body = null; } } else body = null; }
  sigDispatch({ type: 'reply', seq: fx.seq, status, body });
}
const sigClose = () => sigDispatch({ type: 'close' });
// The dialog's live-status lookups; the page stores one item of GET /api/requester/status per hop key.
function openSig(rowId, chain, i, action, scope) {
  const items = sigItems(chain, i, scope, S.reqLive);
  sigDispatch({ type: 'open', action, scope, items, rowId, origin: sigKey(chain[i]) + ':' + action + ':' + scope });
}
const SIG_STATUS_TEXT = { would_signal: 'Will be signalled', signalled: 'Done', refused: 'Refused', failed: 'Failed' };
const sigP = (text, cls) => h('p', { class: cls || '', style: 'margin:0;line-height:1.5' }, text);
function sigItemRow(i, showStatus) {
  const kids = i.children.slice(0, 5), more = Math.max(0, i.childrenTotal - kids.length);
  return h('li', { class: 'tr-item', 'data-status': i.status === 'would_signal' || i.status === 'signalled' ? null : i.status, 'data-level': i.level },
    h('div', { class: 'tr-name' }, h('strong', { class: 'tr-path mono' }, h('bdi', {}, i.label)), h('span', { class: 'tag ' + (i.level === 'danger' ? 'bad' : i.level === 'caution' ? 'warn' : '') }, i.level === 'danger' ? 'Your session' : i.level === 'caution' ? 'Caution' : 'Plain')),
    i.command ? h('div', { class: 'small tr-meta mono' }, h('bdi', {}, i.command)) : null,
    i.why.length ? h('div', { class: 'small mut tr-meta' }, h('bdi', {}, i.why.join('; '))) : null,
    showStatus ? h('div', { class: 'small tr-meta' }, h('strong', {}, i.waiting ? 'Waits for your second confirmation' : SIG_STATUS_TEXT[i.status] || 'Unknown'), i.reason && i.status !== 'would_signal' && i.status !== 'signalled' ? ': ' + i.reason : '', i.state === 'frozen' ? ' (frozen now)' : '') : null,
    kids.length ? h('div', { class: 'small tr-meta' }, h('span', { class: 'mut' }, 'Running under it: '), kids.map((k, n) => [n ? '; ' : '', h('bdi', { class: 'mono' }, k.command || 'pid ' + k.pid), k.frozen ? ' (frozen)' : ''])) : null,
    more ? h('div', { class: 'small mut tr-meta' }, 'and ' + more + ' more') : null);
}
function sigBody(s) {
  const sum = s.summary, a = SIG_ACTIONS[s.action];
  if (s.phase === 'previewing') return [sigP('Checking ' + sigPlural(s.items.length, 'process', 'processes') + ' with the server. Nothing is signalled yet.', 'mut'), h('ul', { class: 'tr-list' }, s.items.map((i) => sigItemRow(Object.assign({ children: [], childrenTotal: 0, status: 'would_signal', reason: null, state: null }, i), false)))];
  if (s.phase === 'sending') return [sigP(a.label + ' is being sent to ' + sigPlural(s.items.length, 'process', 'processes') + '. It cannot be cancelled once it starts.', 'mut'), h('ul', { class: 'tr-list' }, sum.items.map((i) => sigItemRow(i, false)))];
  if (s.phase === 'error') return [sigP(s.message.text), s.message.retry ? sigP('Preview again to start over. Nothing carries across.', 'small mut') : null];
  if (s.phase === 'confirming-danger') {
    const names = s.items.filter((i) => i.level === 'danger');
    return [sigP(s.action === 'freeze' ? 'You are about to freeze your own session: ' : 'You are about to end your own session: ', 'tr-head'), h('ul', { class: 'tr-list' }, names.map((i) => sigItemRow(sum.items.find((x) => x.pid === i.pid) || Object.assign({}, i, { children: [], childrenTotal: 0, status: 'would_signal', reason: null, state: null }), false))),
      sigP(s.action === 'freeze' ? 'Anything you type in it will not appear until it is resumed. Resume it from this page.' : 'Unsaved work in it is lost.'), sigP('Nothing has been sent yet.', 'small mut')];
  }
  const done = s.phase === 'done', kidsTotal = sum.items.reduce((n, i) => n + i.childrenTotal, 0), lines = [sigP(sigHeadline(sum, s, !done), 'tr-head')];
  if (s.changed) lines.unshift(sigP('The check changed after you confirmed. Review the list below, then continue.', 'tr-warn'));
  if (!done) sigWarning(s, kidsTotal).forEach((t, n) => lines.push(sigP(t, n === 0 && s.level !== 'plain' ? 'tr-warn' : 'small')));
  if (s.action === 'stop' && done) lines.push(sigP('A process can ignore a stop request. If it is still running, use Force stop.', 'small'));
  if (sum.auditError) lines.push(sigP('History could not be written: ' + sum.auditError, 'small'));
  if (!done && sum.counts.refused) lines.push(sigP(sigPlural(sum.counts.refused, 'process is', 'processes are') + ' refused and is left alone. The rest can still go.', 'small'));
  lines.push(h('ul', { class: 'tr-list', 'aria-label': done ? 'Result per process' : 'What would be signalled' }, sum.items.map((i) => sigItemRow(i, true))));
  if (done) { lines.push(sigP('Recorded in History before it was sent.', 'small mut')); lines.push(h('a', { class: 'small', href: writeHash({ view: 'history' }) }, 'Open History')); }
  if (done && s.action === 'freeze' && sum.counts.signalled) lines.push(sigP('Frozen processes stay frozen until you press Resume.', 'small'));
  return lines;
}
function sigButtons(s) {
  const cancel = (label) => h('button', { class: 'btn', type: 'button', onClick: sigClose }, label || 'Cancel'), a = SIG_ACTIONS[s.action];
  if (s.phase === 'previewing') return [cancel()];
  if (s.phase === 'sending') return [h('button', { class: 'btn', type: 'button', 'aria-disabled': 'true', onClick: (e) => e.preventDefault() }, 'Sending...')];
  if (s.phase === 'error') return [cancel('Close'), s.message.retry ? h('button', { class: 'btn btn-acc', type: 'button', onClick: () => sigDispatch({ type: 'again' }) }, 'Preview again') : null];
  if (s.phase === 'done') return [h('button', { class: 'btn btn-acc', type: 'button', onClick: sigClose }, 'Close')];
  if (s.phase === 'confirming-danger') {
    const names = s.items.filter((i) => i.level === 'danger').map((i) => i.label).join(', ');
    return [h('button', { class: 'btn', type: 'button', onClick: () => sigDispatch({ type: 'danger-back' }) }, 'Go back'), h('button', { class: 'btn btn-danger', type: 'button', onClick: () => sigDispatch({ type: 'danger-yes' }) }, 'Yes, ' + a.verb + ' ' + names)];
  }
  const can = !!s.token || !!s.summary.waiting, n = s.summary.counts.would_signal;
  return [cancel(), h('button', { class: 'btn btn-danger', type: 'button', disabled: can ? null : true, 'aria-describedby': can ? null : 'sig-desc', onClick: () => sigDispatch({ type: 'go' }) },
    can ? (s.level === 'danger' && !s.confirmDanger ? 'Continue' : a.label + ' ' + sigPlural(n, 'process', 'processes')) : 'Nothing can be ' + a.done)];
}
function sigDialog() {
  const s = S.sig;
  if (!s || s.phase === 'idle') return null;
  const a = SIG_ACTIONS[s.action], title = { previewing: 'Checking before anything is sent', previewed: sigTitle(s), 'confirming-danger': a.label + ' your own session?', sending: a.label + ' is being sent', done: 'Result', error: 'Could not finish' }[s.phase];
  return h('div', { class: 'overlay' }, h('div', { class: 'modal trash-modal', role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'sig-title', 'aria-describedby': 'sig-desc',
    'aria-busy': s.phase === 'previewing' || s.phase === 'sending' ? 'true' : null, tabindex: '-1', onKeydown: sigKeys },
  h('h2', { id: 'sig-title', tabindex: '-1', 'data-autofocus': '' }, title),
  h('div', { id: 'sig-desc', class: 'trash-body' }, sigBody(s)),
  h('div', { class: 'trash-buttons' }, sigButtons(s))));
}
// Tab stays inside the dialog: it wraps at both ends, and Shift+Tab from the heading goes to the last button.
function sigKeys(e) {
  if (e.key !== 'Tab') return;
  const box = e.currentTarget, f = [...box.querySelectorAll('button:not([disabled]), a[href]')], a = document.activeElement;
  if (!f.length) { e.preventDefault(); box.focus(); return; }
  if (e.shiftKey && (a === f[0] || a === box || a.getAttribute('tabindex') === '-1' || !box.contains(a))) { e.preventDefault(); f[f.length - 1].focus(); }
  else if (!e.shiftKey && (a === f[f.length - 1] || !box.contains(a))) { e.preventDefault(); f[0].focus(); }
}
// What render() puts in the layer. The dialog closes (clearing its token) when the row it belongs to is no longer open
// or the page changed. A dialog that cannot be drawn is closed with a message: a page frozen under an invisible dialog is worse.
function sigLayer() {
  const shell = document.querySelector('.shell');
  if (sigOpen() && S.sig.phase !== 'sending') {
    const gone = S.view !== 'priv' ? 'leave' : S.sig.rowId && !S.reqOpen[S.sig.rowId] ? 'selection' : null;
    if (gone) { const back = sigApply(gone === 'leave' ? { type: 'leave' } : { type: 'selection', key: '' }); if (back != null) setTimeout(() => sigFocusBack(back), 0); }
  }
  let node = null;
  try { node = sigDialog(); }
  catch (e) {
    const sending = S.sig && S.sig.phase === 'sending';
    S.sig = sigIdle();
    announce('The dialog could not be drawn and was closed. ' + (sending ? 'A request may have started: open History to check.' : 'Nothing was signalled.'));
  }
  if (shell && node) shell.inert = true; // the page behind a dialog is out of reach of the keyboard and screen readers
  return node;
}
// The status of every hop in the open details, one request per open detail (the server takes up to 12 processes).
let reqLiveBusy = false;
const reqChainFor = (id) => {
  const m = /^(sudo|auth):/.exec(id);
  if (!m) return [];
  const e = m[1] === 'sudo' ? S.sudo.find((x) => 'sudo:' + x.ts + ':' + x.pid === id) : S.priv.find((x) => 'auth:' + x.time + ':' + x.pid + ':' + x.right === id);
  return e && Array.isArray(e.chain) && (m[1] === 'sudo' || e.source === 'captured') ? e.chain : [];
};
async function loadReqLive() {
  if (reqLiveBusy || S.view !== 'priv') return;
  const chains = Object.keys(S.reqOpen).filter((k) => S.reqOpen[k]).map(reqChainFor).filter((c) => c.length);
  if (!chains.length) return;
  reqLiveBusy = true;
  try {
    const next = Object.assign({}, S.reqLive);
    for (const chain of chains) {
      const q = chain.map((hop) => '&item=' + encodeURIComponent(sigKey(hop))).join('');
      try { const body = await (await api('/api/requester/status?' + q.slice(1))).json(); (body.items || []).forEach((it, n) => { if (chain[n]) next[sigKey(chain[n])] = it; }); } catch (e) { /* the buttons say it could not be checked */ }
    }
    quiet({ reqLive: next });
  } finally { reqLiveBusy = false; }
}
// The controls under one hop of an open requester detail: a Frozen badge, a line saying why a button is unavailable, and
// the four actions on this process and, below the first hop, on the whole chain from the process that ran sudo up to here.
function sigControls(rowId, chain, i) {
  const hop = chain[i], live = S.reqLive[sigKey(hop)], domId = reqDomId(rowId) + '-sig-' + i, label = hopLabel(hop);
  const frozen = !!live && live.state === 'frozen' && !live.code;
  const one = SIG_ORDER.map((a) => [a, sigAvail(hop, live, a)]), chainRow = i > 0 ? SIG_ORDER.map((a) => [a, sigChainAvail(chain, i, S.reqLive, a)]) : null;
  const note = one.find(([, v]) => !v.ok && !v.checking && v.reason !== 'Not frozen, so there is nothing to resume' && v.reason !== 'Already frozen. Resume it instead') || one.find(([, v]) => v.checking);
  const foreign = frozen && live.frozen_by_us === false;
  const noteText = note ? note[1].reason : frozen ? 'Frozen: it does nothing until you resume it.' : '';
  const btn = (a, v, scope) => h('button', { class: 'btn sm', type: 'button', disabled: v.ok ? null : true, 'aria-describedby': v.ok || !noteText ? null : domId + '-note', 'data-sig': sigKey(hop) + ':' + a + ':' + scope,
    'aria-label': SIG_ACTIONS[a].label + (scope === 'chain' ? ' the whole chain from ' : ' ') + label + ', pid ' + hop.pid, onClick: () => openSig(rowId, chain, i, a, scope) }, SIG_ACTIONS[a].label);
  return h('div', { class: 'sigctl', role: 'group', 'aria-label': 'Actions for ' + label + ', pid ' + hop.pid },
    h('div', { class: 'sigrow' }, h('span', { class: 'mut' }, 'This process'), frozen ? h('span', { class: 'tag warn' }, foreign ? 'Stopped by something else' : 'Frozen') : null, one.map(([a, v]) => btn(a, v, 'one'))),
    chainRow ? h('div', { class: 'sigrow' }, h('span', { class: 'mut' }, 'Whole chain from here (' + (i + 1) + ' processes)'), chainRow.map(([a, v]) => btn(a, v, 'chain'))) : null,
    noteText ? h('p', { class: 'mut sum', id: domId + '-note', style: 'margin:4px 0 0' }, h('bdi', {}, noteText)) : null);
}
/* signal-view:end */
