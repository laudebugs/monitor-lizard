/* global-search:begin */
// The global search: one query, results from every page. Pure and free of DOM access so tests can lift it out of the
// page. It reads data the dashboard already holds (snapshot, history, startup items, the Guide table) and sends the
// text nowhere. It needs `visible` (the requester block), the url-state block and the guide-search block.
// Matching: a query is cut to SEARCH_QUERY_MAX characters and split into at most SEARCH_WORDS_MAX words; case, width
// variants and invisible format characters (zero width, bidi marks) are ignored on both sides, so a hostile name
// cannot hide from a search for what it looks like. No word is ever turned into a pattern.
const SEARCH_CAP = 5, SEARCH_KEEP = 200, SEARCH_QUERY_MAX = 200, SEARCH_WORDS_MAX = 8;
const SEARCH_GROUPS = [['apps', 'Apps'], ['ports', 'Ports'], ['hosts', 'Hosts'], ['startup', 'Persistence'], ['guide', 'Guide']];
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
  const kept = tiers.flat();
  return { picked: kept.slice(0, SEARCH_CAP), kept: kept.length };
}
// A lone surrogate cannot be percent-encoded, and writeHash would silently drop the parameter. The page then opens with
// nothing selected and no word why; with the replacement character the app's "no longer running" notice shows instead.
const searchWell = (v) => (typeof v === 'string' ? v.replace(/[\ud800-\udbff](?![\udc00-\udfff])|(?<![\ud800-\udbff])[\udc00-\udfff]/g, '\ufffd') : v);

const searchItem = (kind, page, label, detail, state) => ({ kind, page, label: visible(label), detail: visible(detail), hash: writeHash(Object.assign({}, state, { sel: searchWell(state.sel), q: searchWell(state.q) })) });
const searchAppState = (key) => ({ view: 'apps', filter: 'all', sel: key, q: '' });
const searchHostState = (addr, live) => ({ view: live ? 'network' : 'history', q: addr });
const searchNames = (labels) => { const l = [...labels]; return l.length > 1 ? l[0] + ' and ' + (l.length - 1) + ' more' : l[0] || ''; };
// data: { groups: snapshot groups, seen: host history, pers: startup items, guide: { commands, terms } }.
// Returns { words, groups: [{ id, title, items }], count }: empty groups left out, a group with the better match first.
function searchAll(q, data) {
  const words = searchWords(q), out = { words, groups: [], count: 0, query: visible(searchWell(String(q == null ? '' : q).slice(0, SEARCH_QUERY_MAX)).trim()) };
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
    const { picked, kept } = searchPick(found[id]); // source order is kept inside a tier
    if (picked.length) out.groups.push({ id, title, best: picked[0].tier, kept, items: picked.map((c) => c.make()) });
  }
  out.groups.sort((a, b) => a.best - b.best); // stable: equal tiers keep the order above
  out.count = out.groups.reduce((n, g) => n + g.items.length, 0);
  return out;
}
const searchFlat = (res) => (res ? res.groups.flatMap((g) => g.items) : []);
// What the box does with a key. `st` is { open, active (-1 for none), count (options, not the "nothing matches" row),
// hasText }. `handled` says the key is ours, so the caller stops it; a key pressed while an input method is composing
// (Enter confirms the composition) or with a modifier (Cmd+Arrow moves the caret) is never ours.
// act: 'open' (open the active result), 'filter' (Enter with nothing chosen), 'close' (Escape closed the menu), 'clear'
// (Escape with the menu already closed empties the box), or null.
function searchKey(st, key, composing, mod) {
  const none = { open: st.open, active: st.active, act: null, handled: false };
  if (composing || mod) return none;
  if (key === 'Escape') return st.open ? { open: false, active: -1, act: 'close', handled: true } : st.hasText ? { open: false, active: -1, act: 'clear', handled: true } : none;
  if (!st.hasText) return none;
  const n = st.count, go = (open, active, act) => ({ open, active, act: act || null, handled: true });
  if (key === 'ArrowDown') return go(true, st.open && n ? (st.active + 1 >= n ? -1 : st.active + 1) : -1);
  if (key === 'ArrowUp') return go(true, n ? (st.open ? (st.active < 0 ? n - 1 : st.active - 1) : n - 1) : -1);
  if (key === 'Enter') return st.open && st.active >= 0 && st.active < n ? go(false, st.active, 'open') : go(false, -1, 'filter');
  return none;
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
const searchNothing = (res) => 'Nothing matches \u201c' + res.query + '\u201d';
const searchSummary = (res) => (res.count ? res.count + (res.count === 1 ? ' result: ' : ' results: ') + res.groups.map((g) => g.title).join(', ') : searchNothing(res));
/* global-search:end */

/* global-search-view:begin */
// The box, its menu and its keys. Built once at load by searchInit(). render() never rebuilds the box (it must keep
// focus while a poll redraws the page), so the menu is drawn here and only when what it shows changes. The text is read
// from the box each time: code sets it too (the Storage reset, a link, Back).
const SEARCH = { res: null, open: false, active: -1, sig: '', timer: 0, said: '', data: null };
const searchText = () => $('q').value;
const searchLayers = () => !!(S.modal || S.navOpen || S.copyText || S.menu || trashOpen() || sigOpen());
// The data the search reads, rebuilt only when a poll replaces one of its sources.
function searchData() {
  const c = SEARCH.data;
  if (c && c.snap === S.snap && c.seen === S.seen && c.pers === S.pers && c.guide === S.guide.data) return c.out;
  const out = { groups: S.snap ? S.snap.groups : [], seen: S.seen.map((s) => Object.assign({}, s, { label: labelOf(s.group_key) })), pers: S.pers, guide: S.guide.data };
  SEARCH.data = { snap: S.snap, seen: S.seen, pers: S.pers, guide: S.guide.data, out };
  return out;
}
function searchDraw() {
  const box = $('smenu'), q = $('q'), res = SEARCH.res, flat = searchFlat(res);
  q.setAttribute('aria-expanded', String(SEARCH.open));
  box.hidden = !SEARCH.open;
  if (!SEARCH.open || !res) { q.removeAttribute('aria-activedescendant'); return; }
  let n = 0;
  const opt = (it) => { const i = n++; return h('div', { class: 'sopt', role: 'option', id: 'sr-' + i, 'aria-selected': String(i === SEARCH.active), onClick: () => searchGo(it) },
    h('span', { class: 'slabel' }, it.label), h('span', { class: 'sdetail mut small' }, it.detail), h('span', { class: 'spage tag' }, it.page)); };
  box.replaceChildren(...(flat.length ? res.groups.map((g) => h('div', { role: 'group', 'aria-labelledby': 'sg-' + g.id }, h('div', { class: 'sgroup', id: 'sg-' + g.id }, g.title), g.items.map(opt)))
    : [h('div', { class: 'sopt', role: 'option', 'aria-disabled': 'true', id: 'sr-none' }, searchNothing(res))]));
  const cur = SEARCH.active >= 0 ? $('sr-' + SEARCH.active) : null;
  if (cur) { q.setAttribute('aria-activedescendant', cur.id); cur.scrollIntoView({ block: 'nearest' }); } else q.removeAttribute('aria-activedescendant');
}
// Said after the typing pauses, so a screen reader hears the count once and not once per key.
function searchAnnounce() { clearTimeout(SEARCH.timer); SEARCH.timer = setTimeout(() => { if (SEARCH.open && SEARCH.res) announce(searchSummary(SEARCH.res)); }, 300); }
// Searches the box's current text. `open` sets whether the menu is shown; without it the menu stays as it is. The active
// row follows its result when a poll reorders the list, and a change is announced only when the summary changes.
function searchUpdate(open) {
  const text = searchText(), before = searchFlat(SEARCH.res)[SEARCH.active];
  if (S.guide.state === 'idle' && text.trim()) loadGuide(); // the Guide table is fetched the first time it is searched
  SEARCH.res = searchAll(text, searchData());
  const flat = searchFlat(SEARCH.res);
  SEARCH.open = (open !== undefined ? open : SEARCH.open) && !!text.trim();
  SEARCH.active = before ? flat.findIndex((x) => x.hash === before.hash) : -1;
  const said = searchSummary(SEARCH.res);
  if (SEARCH.open && said !== SEARCH.said) { SEARCH.said = said; searchAnnounce(); }
  const sig = JSON.stringify([SEARCH.open, SEARCH.active, SEARCH.res.query, flat.map((x) => [x.hash, x.label, x.detail, x.page])]);
  if (sig !== SEARCH.sig) { SEARCH.sig = sig; searchDraw(); }
}
// A poll can change what matches; redraw only when it does, so the highlighted row stays where the keyboard put it.
const searchRefresh = () => { if (SEARCH.open) searchUpdate(); };
function searchClose() { SEARCH.open = false; SEARCH.active = -1; SEARCH.sig = ''; SEARCH.said = ''; searchDraw(); }
// The box shows what the page filters by. Where the page has no filter it is empty, so the box never says a filter is
// on when it is not. Not while it has focus: the person is typing, and a poll must not move the caret.
function searchSyncBox() {
  const q = $('q'), want = FILTERABLE.includes(S.view) ? S.q : '';
  if (document.activeElement !== q && q.value !== want) q.value = want;
}
const searchHashChange = () => window.dispatchEvent(new HashChangeEvent('hashchange'));
function searchGo(it) {
  searchClose();
  searchOpen(it.hash, location, history, searchHashChange);
  announce('Opened ' + it.page);
}
function searchKeys(e) {
  const composing = e.isComposing || e.keyCode === 229, mod = e.metaKey || e.ctrlKey || e.altKey || e.shiftKey, hasText = !!searchText().trim();
  if (hasText && !composing && !mod && ['ArrowDown', 'ArrowUp', 'Enter'].includes(e.key)) searchUpdate(); // results for the text as it is now
  const flat = searchFlat(SEARCH.res), next = searchKey({ open: SEARCH.open, active: SEARCH.active, count: flat.length, hasText }, e.key, composing, mod);
  if (!next.handled || (next.act === 'clear' && searchLayers())) return; // not ours: the page's own Escape handling runs
  e.preventDefault(); e.stopPropagation();
  if (next.act === 'open') { searchGo(flat[next.active]); return; }
  if (next.act === 'clear') { $('q').value = ''; if (FILTERABLE.includes(S.view)) { S.q = ''; S.page = {}; render(); } searchClose(); return; }
  if (next.act === 'filter') {
    searchClose();
    // Where the page filters in place the text is already applied; elsewhere Enter takes it to Apps, as typing there used to.
    if (!FILTERABLE.includes(S.view)) searchOpen(writeHash({ view: 'apps', filter: 'all', q: searchText().trim() }), location, history, searchHashChange);
    return;
  }
  const opened = next.open && !SEARCH.open;
  SEARCH.open = next.open; SEARCH.active = next.active; SEARCH.sig = '';
  searchDraw();
  if (opened) { SEARCH.said = searchSummary(SEARCH.res); searchAnnounce(); }
}
// "/" focuses the box from anywhere, except where it must type a slash or a layer owns the keyboard.
function searchSlash(e) {
  const t = e.target, typing = t && (['INPUT', 'TEXTAREA', 'SELECT'].includes(t.tagName) || t.isContentEditable);
  if (e.key !== '/' || typing || e.isComposing || e.keyCode === 229 || e.metaKey || e.ctrlKey || e.altKey || searchLayers()) return false;
  e.preventDefault(); $('q').focus(); $('q').select();
  return true;
}
function searchInit() {
  const q = $('q');
  q.addEventListener('input', () => {
    if (FILTERABLE.includes(S.view)) { S.q = searchText(); S.page = {}; SEARCH.open = true; render(); } // render() refreshes the menu once
    else searchUpdate(true);
  });
  q.addEventListener('keydown', searchKeys);
  q.addEventListener('blur', () => searchClose());
  $('smenu').addEventListener('mousedown', (e) => e.preventDefault()); // a click or a scroll in the menu must not take focus from the box
}
/* global-search-view:end */
