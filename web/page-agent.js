/* agent-suggestion:begin */
// The local classifier's suggestion for one app, shown in the Apps drawer only when the agent is On. Pure: no DOM,
// no storage, no fetch. `agentSuggestion(reply)` takes the reply of POST /api/classify and returns what to show, or
// null for "show nothing": the agent is off or shadow, the model server is not ready, or the model gave no answer
// (the server then sends the rule label with an empty rationale and low confidence, which is not a suggestion).
// The server already clamped the verdict to the rules (src/classify.rs validate_verdict); this only decides how it reads.
const AGENT_ASKED_MS = 10 * 60 * 1000; // an app is asked about again no sooner than this, so a 2 s refresh never re-asks
const AGENT_RETRY_MS = 60 * 1000; // after a failed or not-ready answer, ask again after this
const AGENT_SETTINGS_MS = 30 * 1000; // the mode can change under an open page (`monitor-lizard agent ...`), so it is re-read this often
const AGENT_ACTIONS = {
  approve: { text: 'Approve app', kind: 'approve' },
  quit: { text: 'Quit app...', kind: 'stop' },
  force_quit: { text: 'Quit app...', kind: 'stop' }, // the same dialog; Force quit is its own button there
  investigate: { text: 'Investigate', kind: 'link' },
  ignore: { text: 'Ignore', kind: 'note' }, // nothing to run yet: marking an app expected or suspicious is a later todo
};
// `mode` is the mode the server reports now, not the one in the cached reply, so turning the agent off or to shadow hides
// a suggestion that is already on screen.
function agentSuggestion(reply, mode) {
  if (mode !== 'on' || !reply || reply.agent_mode !== 'on' || reply.agent_reason || !reply.verdict) return null;
  const v = reply.verdict;
  if (!v.rationale) return null;
  // Low confidence, or a label that differs from the rules', is not a basis for anything beyond looking closer.
  const unsure = v.confidence === 'low' || v.label !== reply.rule_label;
  const ids = unsure ? ['investigate'] : [...new Set((v.actions || []).filter((a) => AGENT_ACTIONS[a]).map((a) => (a === 'force_quit' ? 'quit' : a)))];
  return {
    unsure,
    headline: unsure ? 'Unsure, investigate' : 'The agent reads this as ' + v.label,
    label: v.label,
    confidence: v.confidence,
    rationale: String(v.rationale),
    actions: ids.map((id) => ({ id, text: AGENT_ACTIONS[id].text, kind: AGENT_ACTIONS[id].kind })),
  };
}
// Whether to ask the server about this app now: the agent must be on or shadow, and the app not asked about lately.
// `askedAt[key]` is the time before which the app is not asked again.
function agentShouldAsk(mode, key, askedAt, now) {
  if (mode !== 'on' && mode !== 'shadow') return false;
  const t = askedAt && askedAt[key];
  return !(t && now < t);
}
/* agent-suggestion:end */

const agentAskedAt = {}, agentBusy = new Set();
let agentSettingsAt = 0;
// Asks the server for a verdict. In shadow mode the answer is only recorded, never drawn (agentSuggestion returns null
// for it); a failure shows nothing, not an error banner, since the agent is optional. A failed or not-ready answer is
// asked again after a minute, a real one after ten.
async function agentAsk(g) {
  if (agentBusy.has(g.key)) return;
  agentBusy.add(g.key);
  const later = (ms) => { agentAskedAt[g.key] = Date.now() + ms; };
  try {
    if (Date.now() - agentSettingsAt > AGENT_SETTINGS_MS) { agentSettingsAt = Date.now(); await loadSettings(); }
    if (!agentShouldAsk(S.settings && S.settings.agent_mode, g.key, agentAskedAt, Date.now())) return;
    later(AGENT_RETRY_MS);
    const reply = await (await api('/api/classify?group_key=' + encodeURIComponent(g.key), 'POST')).json();
    if (!reply.agent_reason) later(AGENT_ASKED_MS);
    quiet({ agent: Object.assign({}, S.agent, { [g.key]: reply }) });
  } catch (e) { /* no banner: the agent being down is its normal state */ }
  finally { agentBusy.delete(g.key); }
}
function agentBlock(g, mine) {
  agentAsk(g);
  const s = agentSuggestion(S.agent && S.agent[g.key], S.settings && S.settings.agent_mode);
  if (!s) return null;
  const cmd = (g.procs[0] && g.procs[0].command) || '';
  const run = (a) => a.kind === 'approve' ? approve(g) : a.kind === 'stop' ? (mine.length ? askStop(g.label, mine) : toast('None of this app\'s processes are yours to quit.')) : null;
  return h('div', { class: 'insp-sec' },
    h('h3', { class: 'sec' }, 'Suggestion (AI-generated, unverified)'),
    h('p', { style: 'margin:0 0 6px;font-size:13px;font-weight:600' }, s.headline, ' ', h('span', { class: 'tag ' + (s.unsure ? 'warn' : trustCls(s.label)) }, s.confidence + ' confidence')),
    h('p', { style: 'margin:0 0 8px;font-size:13px;line-height:1.45' }, visible(s.rationale)),
    h('div', { class: 'code', style: 'margin:0 0 8px;white-space:pre-wrap' }, visible(g.label) + '\n' + visible(unprefixed(g.key)) + (cmd ? '\n' + visible(cmd) : '')),
    h('div', { style: 'display:flex;gap:8px;flex-wrap:wrap' }, s.actions.map((a) => a.kind === 'link'
      ? h('a', { class: 'btn sm', href: writeHash({ view: 'network', q: g.label }) }, a.text)
      : a.kind === 'note' ? h('span', { class: 'tag' }, a.text)
      : h('button', { class: 'btn sm' + (a.kind === 'stop' ? ' btn-danger quiet' : ''), onClick: () => run(a) }, a.text))));
}
