/* page-shell:begin */
// Pure helpers for the page shell: navigation groups, the sidebar counts, the verdict sentence that titles each
// page, and relative time. No DOM or storage access, so tests load this block together with url-state (TITLES).
const NAV_GROUPS = ['Security', 'Machine', 'Help'];
// A group heads its items in the sidebar; a test checks that none is missing.
const NAV = [
  { id: 'overview', label: 'Overview', icon: 'M3 12l9-8 9 8M5 10v10h14V10', group: 'Security' },
  { id: 'apps', label: 'Apps', icon: 'M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z', group: 'Security' },
  { id: 'network', label: 'Network', icon: 'M2 12h4l3-8 6 16 3-8h4', group: 'Security' },
  { id: 'listeners', label: 'Listeners', icon: 'M9 12a3 3 0 1 0 6 0a3 3 0 1 0-6 0M6 6a8 8 0 0 0 0 12M18 6a8 8 0 0 1 0 12', group: 'Security' },
  { id: 'priv', label: 'Privilege', icon: 'M12 3l8 3v6c0 5-3.5 8-8 9-4.5-1-8-4-8-9V6z', group: 'Security' },
  { id: 'persist', label: 'Persistence', icon: 'M4 6h16M4 12h16M4 18h10', group: 'Security' },
  { id: 'history', label: 'History', icon: 'M12 7v5l3 2M12 3a9 9 0 1 0 0 18a9 9 0 0 0 0-18', group: 'Security' },
  { id: 'performance', label: 'Performance', icon: 'M3 17l5-9 4 5 3-6 6 10', group: 'Machine' },
  { id: 'storage', label: 'Storage', icon: 'M4 7c0-1.7 3.6-3 8-3s8 1.3 8 3-3.6 3-8 3-8-1.3-8-3zM4 7v5c0 1.7 3.6 3 8 3s8-1.3 8-3V7M4 12v5c0 1.7 3.6 3 8 3s8-1.3 8-3v-5', group: 'Machine' },
  { id: 'guide', label: 'Guide', icon: 'M6 4h12v16H6zM6 16h12M9 8h6', group: 'Help' },
];
const countText = (n) => (n > 99 ? '99+' : String(n));
const plural = (n, one, many) => n + ' ' + (n === 1 ? one : many);
// The counts for items that need a decision. Each one is the number of rows on the page its link opens (see badgeHash).
// A count of zero or less shows no badge at all.
function navBadges(f) {
  const out = {}, x = f || {};
  // The accessible name starts with what is shown, so a screen reader and the screen agree: "99+ apps need review, 120 in total".
  const add = (id, n, tone, one, many) => { if (Number.isFinite(n) && n > 0) out[id] = { n, text: countText(n), tone, label: n > 99 ? '99+ ' + many + ', ' + n + ' in total' : plural(n, one, many) }; };
  add('apps', x.review, 'warn', 'app needs review', 'apps need review');
  add('priv', x.failed, 'bad', 'failed authorization request', 'failed authorization requests');
  add('persist', x.fresh, 'warn', 'new startup item', 'new startup items');
  return out;
}
// One count of apps that need review, used by the title, the badge and the Overview tile. `statusOf` is the page's status().
const countReview = (groups, statusOf) => groups.filter((g) => statusOf(g) === 'review').length;
// Which data each verdict title counts. A title says "nothing" only after all of it has loaded; before that the page name is used.
const TITLE_NEEDS = { overview: ['snap', 'priv', 'pers'], apps: ['snap'], priv: ['priv'], listeners: ['snap'], persist: ['pers'], network: ['snap', 'hist'], storage: ['stor'] };
const titleReady = (view, loaded) => !Object.prototype.hasOwnProperty.call(TITLE_NEEDS, view) || TITLE_NEEDS[view].every((k) => !!(loaded && loaded[k]));
// The page title is one sentence that says what the page found. A page with no verdict yet, and a page whose data has
// not loaded (facts missing), uses its name instead, so a title never claims "nothing" before anything was read.
function verdictTitle(view, f) {
  const name = Object.prototype.hasOwnProperty.call(TITLES, view) ? TITLES[view] : '';
  const get = (k) => (f && Number.isFinite(f[k]) && f[k] >= 0 ? f[k] : null);
  const n = view === 'storage' ? get('measured') : view === 'overview' ? get('attention') : view === 'apps' ? get('review') : view === 'priv' ? get('failed') : view === 'listeners' ? get('exposed') : view === 'persist' ? get('fresh') : view === 'network' ? get('newHosts') : null;
  if (n === null) return name;
  if (view === 'storage') {
    const ago = typeof f.ago === 'string' ? f.ago : '', done = f.complete === true;
    const when = f.stale ? 'Scan from ' + ago : (done ? 'Scan complete' : 'Scan partial') + (ago ? ', ' + ago : '');
    return when + ': ' + (done ? plural(n, 'location', 'locations') : n + (get('total') === null ? '' : ' of ' + get('total')) + ' locations') + ' measured';
  }
  if (view === 'overview') return n ? plural(n, 'thing needs', 'things need') + ' a decision' : 'Nothing needs a decision';
  if (view === 'apps') return n ? plural(n, 'app needs', 'apps need') + ' review' : 'Nothing needs review';
  if (view === 'priv') return n ? plural(n, 'failed authorization request', 'failed authorization requests') : 'No failed authorization requests';
  if (view === 'listeners') return n ? plural(n, 'listening socket is', 'listening sockets are') + ' open to your network' : 'No listening socket is open to your network';
  if (view === 'persist') {
    if (n) return plural(n, 'new startup item', 'new startup items') + ' since your baseline';
    const ch = get('changed');
    return ch ? 'No new startup items, ' + plural(ch, 'change', 'changes') + ' since your baseline' : 'No changes since your baseline';
  }
  return n ? plural(n, 'new host', 'new hosts') + ' in the last hour' : 'No new hosts in the last hour';
}
// "5m ago" for a time in epoch seconds; `long` gives "5 minutes ago" for a screen reader. The cell carries the exact time as well.
function relTime(sec, now, long) {
  if (typeof sec !== 'number' || !Number.isFinite(sec)) return '-';
  const d = Math.max(0, now - sec);
  const ago = (n, short, one, many) => (long ? plural(n, one, many) + ' ago' : n + short + ' ago');
  if (d < 60) return 'just now';
  if (d < 3600) return ago(Math.floor(d / 60), 'm', 'minute', 'minutes');
  if (d < 86400) return ago(Math.floor(d / 3600), 'h', 'hour', 'hours');
  if (d < 2592000) return ago(Math.floor(d / 86400), 'd', 'day', 'days');
  if (d < 31536000) return ago(Math.floor(d / 2592000), 'mo', 'month', 'months');
  return ago(Math.floor(d / 31536000), 'y', 'year', 'years');
}
// Links from one app to the pages that know about it, each only when it opens at least one row. Hashes come from
// writeHash, so they stay inside the allow-lists. The filter is the app's name (or its path for startup items), so an
// app with a similar name may also show. counts: { hosts, sockets, startup, newHosts }.
function relatedLinks(counts, label, path) {
  const c = counts || {}, out = [];
  const add = (n, text, hash) => { if (n > 0) out.push({ text, hash }); };
  add(c.hosts, plural(c.hosts, 'remote host', 'remote hosts') + ' on Network', writeHash({ view: 'network', q: label }));
  add(c.sockets, plural(c.sockets, 'listening socket', 'listening sockets') + ' on Listeners', writeHash({ view: 'listeners', q: label }));
  add(c.startup, plural(c.startup, 'startup item', 'startup items') + ' on Persistence', writeHash({ view: 'persist', q: path }));
  add(c.newHosts, plural(c.newHosts, 'new host', 'new hosts') + ' on History', writeHash({ view: 'history', q: label }));
  return out;
}
/* page-shell:end */
