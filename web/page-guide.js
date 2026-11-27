const SAFETY_TAG = { 'read-only': 'ok', 'changes files': 'warn', 'needs sudo': 'bad' };
// render() rebuilds every control, so a keyboard user would lose their place; put focus back on the one just used.
const guideAct = (id, m) => { rec(m); const el = $(id); if (el) el.focus(); };
// One live region that render() never rebuilds, so a screen reader hears changes made after the content is replaced.
function announce(msg) { const el = $('announcer'); if (!el) return; el.textContent = ''; setTimeout(() => { el.textContent = msg; }, 60); }
async function loadGuide() {
  S.guide = { state: 'loading', data: null };
  try { const data = await (await api('/api/guide')).json(); rec({ guide: { state: 'ready', data } }); }
  catch (e) { rec({ guide: { state: 'error', data: null } }); }
}
const copyCommand = (e) => copyFlow(e.command, {
  write: (t) => navigator.clipboard.writeText(t),
  show: (state) => { rec({ gcopy: { id: e.id, state } }); announce(state === 'copied' ? 'Copied to the clipboard' : 'The clipboard is blocked. The command is selected: press Cmd+C'); },
  byId: $,
  select: (el) => { const r = document.createRange(); r.selectNodeContents(el); const sel = getSelection(); sel.removeAllRanges(); sel.addRange(r); },
});
function guideDetail(e) {
  const page = NAV.find((n) => n.label === e.appears_in), c = S.gcopy && S.gcopy.id === e.id ? S.gcopy.state : '';
  const part = (title, ...kids) => h('div', { class: 'insp-sec' }, h('h3', { class: 'sec' }, title), ...kids);
  return h('aside', { class: 'drawer card', 'aria-label': 'Command detail' },
    h('div', { class: 'insp-head' }, h('h2', { class: 'grow', style: 'font-size:16px;font-weight:600;margin:0' }, e.title), h('span', { class: 'tag ' + (SAFETY_TAG[e.safety] || ''), style: 'flex:none' }, e.safety)),
    h('div', { style: 'padding:0 16px 14px;display:flex;flex-direction:column;gap:8px' },
      h('div', { class: 'code', id: 'guide-cmd' }, e.command),
      h('div', { style: 'display:flex;align-items:center;gap:10px' }, h('button', { class: 'btn btn-acc sm', id: 'guide-copy', onClick: () => copyCommand(e) }, c === 'copied' ? 'Copied' : c === 'manual' ? 'Press Cmd+C' : 'Copy'),
        h('span', { class: 'small mut', 'aria-hidden': 'true' }, c === 'copied' ? 'Copied to the clipboard.' : c === 'manual' ? 'The clipboard is blocked, so the command is selected.' : ''))),
    part('What it does', h('p', { style: 'margin:0;font-size:13px;line-height:1.45' }, e.description)),
    e.example ? part('Example output (' + e.example.macos + ')', h('pre', { class: 'code', style: 'margin:0;white-space:pre-wrap' }, e.example.output)) : null,
    e.appears_in ? part('Where it appears', page ? h('a', { href: '#' + page.id }, e.appears_in) : h('span', {}, e.appears_in)) : null,
    e.undo ? part('How to undo', h('p', { style: 'margin:0;font-size:13px;line-height:1.45' }, e.undo)) : null);
}
// The line under a list that hides weak matches. The button reveals them and keeps focus on the first one.
const weakLine = (hidden, kind, focusId) => h('div', { class: 'small mut', style: 'padding:12px 16px;display:flex;align-items:center;gap:10px;flex-wrap:wrap' },
  h('span', {}, hidden.length + ' weaker ' + (hidden.length === 1 ? 'match' : 'matches') + ' hidden.'),
  h('button', { class: 'btn sm', id: 'g-weak', onClick: () => guideAct(focusId, { gall: S.q }) }, 'Show ' + (hidden.length === 1 ? 'it' : 'them')));
function guide() {
  const g = S.guide;
  if (g.state === 'idle') loadGuide();
  const title = verdictTitle('guide');
  if (g.state === 'error') return pageShell({ title, body: h('div', { class: 'card' }, h('div', { class: 'item' }, h('p', {}, 'The guide did not load. '), h('button', { class: 'btn sm', onClick: loadGuide }, 'Try again'))) });
  if (g.state !== 'ready') return pageShell({ title, body: h('p', { class: 'mut' }, 'Loading the guide...') });
  const commands = g.data.commands || [], terms = g.data.terms || [], gloss = S.gtab === 'glossary', all = S.q.trim() !== '' && S.gall === S.q;
  const topics = guideTopics(commands), topic = topics.includes(S.gtopic) ? S.gtopic : 'all';
  const tabs = [['commands', 'Commands'], ['glossary', 'Glossary']].map(([id, l]) => h('button', { class: 'chip' + (S.gtab === id ? ' on' : ''), id: 'g-tab-' + id, 'aria-pressed': String(S.gtab === id), onClick: () => guideAct('g-tab-' + id, { gtab: id, gcopy: null, page: {} }) }, l));
  const chips = gloss ? [] : [['all', 'All'], ...topics.map((t) => [t, topicLabel(t)])].map(([id, l]) => h('button', { class: 'chip' + (topic === id ? ' on' : ''), id: 'g-topic-' + id, 'aria-pressed': String(topic === id), onClick: () => guideAct('g-topic-' + id, { gtopic: id, gcopy: null }) }, l + ' ', h('span', { class: 'mut' }, guideSearch(commands, S.q, id, all).shown.length)));
  const bar = [h('div', { class: 'seg', role: 'group', 'aria-label': 'Guide section' }, tabs), chips];
  const { shown: rows, hidden } = guideSearch(gloss ? terms : commands, S.q, gloss ? 'all' : topic, all);
  S.gnote = guideSummary(rows.length, hidden.length, gloss ? 'term' : 'command');
  if (!rows.length && !hidden.length) return pageShell({ title, filters: bar, body: h('div', { class: 'card' }, h('div', { class: 'item' }, h('p', {}, 'Nothing matches that. '), h('button', { class: 'btn sm', onClick: () => { clearFilters(); $('q').focus(); } }, 'Clear filters'))) });
  if (gloss) return pageShell({ title, filters: bar, body: h('section', { class: 'card', 'aria-label': 'Glossary' }, h('h2', { class: 'sec', style: 'margin:0;padding:12px 16px 0' }, 'Glossary'), rows.map((t) => h('div', { class: 'item' }, h('div', {}, h('h3', { id: 'g-term-' + t.id, tabindex: '-1' }, t.term), h('p', {}, t.meaning)))), hidden.length ? weakLine(hidden, 'term', 'g-term-' + hidden[0].id) : null) });
  const cur = guidePick(rows, S.gsel);
  // A selection the filters hide gives way to the first result; the address follows without a new history entry.
  if (S.gsel && (!cur || cur.id !== S.gsel)) { S.gsel = null; S.hashReplace = true; }
  const row = (e) => h('li', {}, h('button', { class: 'tr ell' + (cur && e.id === cur.id ? ' on' : ''), id: 'g-row-' + e.id, style: 'grid-template-columns:minmax(0,1fr) auto;min-width:0', 'aria-current': cur && e.id === cur.id ? 'true' : null, onClick: () => guideAct('g-row-' + e.id, { gsel: e.id, gcopy: null }) }, h('span', {}, e.title), h('span', { class: 'tag ' + (SAFETY_TAG[e.safety] || '') }, e.safety)));
  const groups = S.q.trim() ? [[null, rows]] : guideGroups(rows);
  const plain = 'list-style:none;margin:0;padding:0';
  const list = h('section', { class: 'card', style: 'flex:1 1 360px;min-width:0', 'aria-label': 'Commands' }, groups.map(([t, rs]) => [t ? h('h2', { class: 'sec', style: 'margin:0;padding:12px 16px 6px' }, topicLabel(t)) : null, h('ul', { style: plain }, rs.map(row))]), hidden.length ? weakLine(hidden, 'command', 'g-row-' + hidden[0].id) : null);
  return pageShell({ title, filters: bar, body: list, inspector: cur ? guideDetail(cur) : null });
}

