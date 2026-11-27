/* global-search:begin */
// The global search: one query, results from every page. Pure and free of DOM access so tests can lift it out of the
// page. It reads data the dashboard already holds (snapshot, history, startup items, the Guide table) and sends the
// text nowhere. It needs `visible` (the requester block), the url-state block and the guide-search block.
// Matching: a query is cut to SEARCH_QUERY_MAX characters and split into at most SEARCH_WORDS_MAX words; case, width
// variants and invisible format characters (zero width, bidi marks) are ignored on both sides, so a hostile name
// cannot hide from a search for what it looks like. No word is ever turned into a pattern.
const SEARCH_CAP = 5, SEARCH_KEEP = 200, SEARCH_QUERY_MAX = 200, SEARCH_WORDS_MAX = 8;
const SEARCH_GROUPS = [['apps', 'Apps'], ['ports', 'Ports'], ['hosts', 'Hosts'], ['startup', 'Startup items'], ['guide', 'Guide']];
// Plain printable ASCII (nearly every name) needs only a lower case; the rest is folded and stripped of format characters.
const searchNorm = (s) => { const t = String(s == null ? '' : s); return /^[\x20-\x7e]*$/.test(t) ? t.toLowerCase() : t.normalize('NFKC').replace(/\p{Cf}/gu, '').toLowerCase(); };
const searchWords = (q) => searchNorm(String(q == null ? '' : q).slice(0, SEARCH_QUERY_MAX)).split(/\s+/).filter(Boolean).slice(0, SEARCH_WORDS_MAX);
// 0 exact, 1 starts with the query, 2 a word starts with it, 3 contains every word.
function searchTier(hay, words) {
  const qn = words.join(' ');
  return hay === qn ? 0 : hay.startsWith(qn) ? 1 : (' ' + hay).includes(' ' + qn) ? 2 : 3;
}
// Keeps each tier's first SEARCH_KEEP candidates, so the work after the scan stays small however many items match,
// and an exact or prefix match is never pushed out by weaker ones.
function searchPick(candidates) {
  const tiers = [[], [], [], []];
  for (const c of candidates) if (tiers[c.tier].length < SEARCH_KEEP) tiers[c.tier].push(c);
  return tiers.flat().slice(0, SEARCH_CAP);
}
const searchItem = (kind, page, label, detail, state, tier) => ({ kind, page, label: visible(label), detail: visible(detail), hash: writeHash(state), tier });
const searchAppState = (key) => ({ view: 'apps', filter: 'all', sel: key, q: '' });
const searchHostState = (addr, live) => ({ view: live ? 'network' : 'history', q: addr });
const searchNames = (labels) => { const l = [...labels]; return l.length > 1 ? l[0] + ' and ' + (l.length - 1) + ' more' : l[0] || ''; };
// data: { groups: snapshot groups, seen: host history, pers: startup items, guide: { commands, terms } }.
// Returns { words, groups: [{ id, title, items }], count }: empty groups left out, a group with the better match first.
function searchAll(q, data) {
  const words = searchWords(q), out = { words, groups: [], count: 0 };
  if (!words.length) return out;
  const qn = words.join(' '), snapGroups = ((data && data.groups) || []).filter(Boolean), found = {};
  const hit = (hay) => words.every((w) => hay.includes(w));
  // Apps: by label and key, the same apps the Apps page lists (those with network activity).
  const apps = [];
  for (const g of snapGroups) {
    if (!g || !g.conns || !g.conns.length) continue;
    const label = searchNorm(g.label), hay = label + ' ' + searchNorm(unprefixed(String(g.key)));
    if (hit(hay)) apps.push({ tier: searchTier(label, words), make: () => searchItem('app', 'Apps', g.label, g.signing && g.signing.apple ? 'Apple' : g.trust || '', searchAppState(g.key)) });
  }
  found.apps = apps;
  // Ports: a number, by prefix; one row per port however many sockets and apps hold it.
  const ports = [];
  if (words.length === 1 && /^\d{1,5}$/.test(words[0])) {
    const by = new Map();
    for (const g of snapGroups) for (const c of g.conns || []) {
      if (c.direction !== 'listen' || !Number.isInteger(c.local_port) || !String(c.local_port).startsWith(words[0])) continue;
      const p = by.get(c.local_port) || { labels: new Set(), protos: new Set(), exposed: false };
      p.labels.add(g.label); p.protos.add(c.proto); p.exposed = p.exposed || !!c.exposed; by.set(c.local_port, p);
    }
    for (const [port, p] of by) ports.push({ tier: String(port) === words[0] ? 0 : 1, port, make: () => searchItem('port', 'Listeners', 'Port ' + port, searchNames(p.labels) + ', ' + [...p.protos].join('/') + (p.exposed ? ', exposed' : ''), { view: 'listeners', q: String(port) }) });
    ports.sort((a, b) => a.port - b.port);
  }
  found.ports = ports;
  // Hosts: an address, by prefix, with or without the brackets an IPv6 address is written in. A host contacted now opens
  // Network; one only in the history opens History.
  const hosts = [];
  const hq = words.length === 1 ? words[0].replace(/[[\]]/g, '') : '';
  if (hq) {
    const by = new Map();
    for (const g of snapGroups) for (const c of g.conns || []) {
      if (c.direction !== 'outbound' || !c.remote_addr || c.scope === 'loopback') continue;
      const addr = String(c.remote_addr), key = addr.replace(/[[\]]/g, '').toLowerCase();
      if (!key.startsWith(hq)) continue;
      const e = by.get(key) || { addr, labels: new Set(), live: true }; e.labels.add(g.label); by.set(key, e);
    }
    for (const s of (data && data.seen) || []) {
      const addr = String(s.remote || ''), key = addr.replace(/[[\]]/g, '').toLowerCase();
      if (!key || !key.startsWith(hq) || by.has(key)) continue;
      by.set(key, { addr, labels: new Set([s.label || s.group_key]), live: false });
    }
    for (const [key, e] of by) hosts.push({ tier: key === hq ? 0 : 1, make: () => searchItem('host', e.live ? 'Network' : 'History', e.addr, (e.live ? 'Contacted by ' : 'Seen for ') + searchNames(e.labels), searchHostState(e.addr, e.live)) });
  }
  found.hosts = hosts;
  // Startup items: every word in the label or the program.
  const startup = [];
  for (const i of (data && data.pers) || []) {
    const label = searchNorm(i.label);
    if (hit(label + ' ' + searchNorm(i.program))) startup.push({ tier: searchTier(label, words), make: () => searchItem('startup', 'Persistence', i.label, (i.kind || '') + (i.scope ? ', ' + i.scope : ''), { view: 'persist', filter: 'all', q: String(i.label) }) });
  }
  found.startup = startup;
  // Guide: the Guide page's own search, so what is offered is what the Guide then lists for the same query.
  const guide = [], gd = (data && data.guide) || {};
  const typed = String(q).slice(0, SEARCH_QUERY_MAX);
  // The Guide's search treats a query of only punctuation as empty and lists everything; here that is no query at all.
  if (guideWords(typed).length) for (const e of guideSearch(gd.commands || [], typed, 'all', false).shown) guide.push({ tier: searchNorm(e.title) === qn ? 0 : 3, make: () => searchItem('guide', 'Guide', e.title, e.safety || '', { view: 'guide', q: typed, ex: { gtab: 'commands', gtopic: 'all', gsel: e.id } }) });
  if (guideWords(typed).length) for (const e of guideSearch(gd.terms || [], typed, 'all', false).shown) guide.push({ tier: searchNorm(e.term) === qn ? 0 : 3, make: () => searchItem('guide', 'Guide', e.term, 'Glossary', { view: 'guide', q: typed, ex: { gtab: 'glossary', gtopic: 'all', gsel: null } }) });
  found.guide = guide;
  for (const [id, title] of SEARCH_GROUPS) {
    const picked = searchPick(found[id]); // source order is kept inside a tier
    if (picked.length) out.groups.push({ id, title, best: picked[0].tier, items: picked.map((c) => c.make()) });
  }
  out.groups.sort((a, b) => a.best - b.best); // stable: equal tiers keep the order above
  out.count = out.groups.reduce((n, g) => n + g.items.length, 0);
  return out;
}
const searchFlat = (res) => (res ? res.groups.flatMap((g) => g.items) : []);
// What the box does with a key. `st` is { open, active (-1 for none), count (options, not the "nothing matches" row),
// hasText }. Keys pressed while an input method is composing belong to it (Enter confirms the composition).
// act: 'open' (open the active result), 'filter' (Enter with nothing chosen), 'close' (Escape closed the menu), or null.
function searchKey(st, key, composing) {
  const same = { open: st.open, active: st.active, act: null };
  if (composing || !st.hasText) return same;
  const n = st.count;
  if (key === 'ArrowDown') return st.open ? { open: true, active: n ? (st.active + 1 >= n ? -1 : st.active + 1) : -1, act: null } : { open: true, active: -1, act: null };
  if (key === 'ArrowUp') return st.open ? { open: true, active: n ? (st.active < 0 ? n - 1 : st.active - 1) : -1, act: null } : { open: true, active: -1, act: null };
  if (key === 'Enter') return st.open && st.active >= 0 && st.active < n ? { open: false, active: st.active, act: 'open' } : { open: false, active: -1, act: 'filter' };
  if (key === 'Escape' && st.open) return { open: false, active: -1, act: 'close' };
  return same;
}
// Changes the address the way the rest of the page does: a new page or selection is a place to come back to (push), a
// change on the page you are already on refines it (replace), and the same address does nothing. `apply` runs when the
// address changed without a hashchange event (replace), so the page follows it.
function searchOpen(hash, loc, hist, apply) {
  const mode = historyMode(loc.hash, hash);
  if (mode === 'push') loc.hash = hash;
  else if (mode === 'replace') { hist.replaceState(null, '', hash); apply(); }
  return mode;
}
const searchSummary = (res) => (res.count ? res.count + (res.count === 1 ? ' result' : ' results') + ' in ' + res.groups.map((g) => g.title).join(', ') : 'Nothing matches');
/* global-search:end */

// The box, its menu and its keys. Built once at load by searchInit(); render() never rebuilds the box (it must keep
// focus while a poll redraws the page), so the menu is drawn here and only when what it shows changes.
const SEARCH = { text: '', res: null, open: false, active: -1, sig: '', timer: 0 };
const searchData = () => ({ groups: S.snap ? S.snap.groups : [], seen: S.seen.map((s) => Object.assign({}, s, { label: labelOf(s.group_key) })), pers: S.pers, guide: S.guide.data });
function searchDraw() {
  const box = $('smenu'), q = $('q'), res = SEARCH.res, flat = searchFlat(res);
  q.setAttribute('aria-expanded', String(SEARCH.open));
  box.hidden = !SEARCH.open;
  if (!SEARCH.open || !res) { q.removeAttribute('aria-activedescendant'); return; }
  let n = 0;
  const opt = (it) => { const i = n++; return h('div', { class: 'sopt', role: 'option', id: 'sr-' + i, 'aria-selected': String(i === SEARCH.active), onMousedown: (e) => e.preventDefault(), onClick: () => searchGo(it) },
    h('span', { class: 'slabel' }, it.label), h('span', { class: 'sdetail mut small' }, it.detail), h('span', { class: 'spage tag' }, it.page)); };
  box.replaceChildren(...(flat.length ? res.groups.map((g) => h('div', { role: 'group', 'aria-labelledby': 'sg-' + g.id }, h('div', { class: 'sgroup', id: 'sg-' + g.id }, g.title), g.items.map(opt)))
    : [h('div', { class: 'sopt', role: 'option', 'aria-disabled': 'true', id: 'sr-none' }, 'Nothing matches')]));
  const cur = SEARCH.active >= 0 ? $('sr-' + SEARCH.active) : null;
  if (cur) { q.setAttribute('aria-activedescendant', cur.id); cur.scrollIntoView({ block: 'nearest' }); } else q.removeAttribute('aria-activedescendant');
}
// Said after the typing pauses, so a screen reader hears the count once and not once per key.
function searchAnnounce() { clearTimeout(SEARCH.timer); SEARCH.timer = setTimeout(() => { if (SEARCH.open && SEARCH.res) announce(searchSummary(SEARCH.res)); }, 300); }
function searchUpdate(open) {
  const had = SEARCH.res ? searchFlat(SEARCH.res).map((x) => x.hash + x.label).join('|') : '';
  if (S.guide.state === 'idle' && SEARCH.text.trim()) loadGuide(); // the Guide table is fetched the first time it is searched
  SEARCH.res = searchAll(SEARCH.text, searchData());
  const now = searchFlat(SEARCH.res).map((x) => x.hash + x.label).join('|');
  SEARCH.open = open !== undefined ? open : SEARCH.open;
  if (!SEARCH.text.trim()) SEARCH.open = false;
  if (now !== had) { SEARCH.active = -1; searchAnnounce(); }
  const sig = JSON.stringify([SEARCH.open, SEARCH.active, now]);
  if (sig !== SEARCH.sig) { SEARCH.sig = sig; searchDraw(); }
}
// A poll can change what matches; redraw only when it does, so the highlighted row stays where the keyboard put it.
const searchRefresh = () => { if (SEARCH.open) searchUpdate(); };
function searchClose() { SEARCH.open = false; SEARCH.active = -1; SEARCH.sig = ''; searchDraw(); }
function searchGo(it) {
  searchClose();
  searchOpen(it.hash, location, history, () => window.dispatchEvent(new HashChangeEvent('hashchange')));
  announce('Opened ' + it.page);
}
function searchKeys(e) {
  const flat = searchFlat(SEARCH.res), st = { open: SEARCH.open, active: SEARCH.active, count: flat.length, hasText: !!SEARCH.text.trim() };
  const next = searchKey(st, e.key, e.isComposing || e.keyCode === 229);
  if (!['ArrowDown', 'ArrowUp', 'Enter', 'Escape'].includes(e.key) || e.isComposing || e.keyCode === 229) return;
  if (e.key === 'Escape' && next.act !== 'close') return; // not ours: the page's own Escape handling runs
  if (!st.hasText) return;
  e.preventDefault(); e.stopPropagation();
  if (next.act === 'open') { searchGo(flat[next.active]); return; }
  if (next.act === 'filter') {
    searchClose();
    // Where the page filters in place the text is already applied; elsewhere Enter takes it to Apps, as typing there used to.
    if (!FILTERABLE.includes(S.view)) searchOpen(writeHash({ view: 'apps', filter: 'all', q: SEARCH.text.trim() }), location, history, () => window.dispatchEvent(new HashChangeEvent('hashchange')));
    return;
  }
  SEARCH.open = next.open; SEARCH.active = next.active;
  if (next.open && !st.open) searchUpdate(true); else { SEARCH.sig = ''; searchDraw(); }
}
function searchInit() {
  const q = $('q');
  q.addEventListener('input', (e) => {
    SEARCH.text = e.target.value;
    if (FILTERABLE.includes(S.view)) { S.q = SEARCH.text; S.page = {}; render(); }
    searchUpdate(true);
  });
  q.addEventListener('keydown', searchKeys);
  q.addEventListener('blur', () => searchClose());
}
