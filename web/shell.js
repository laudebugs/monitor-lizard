// The cell stays one element: the short text for the eye, the exact time as visible-on-hover title, and a
// full sentence for a screen reader and for touch, where there is no hover.
function timeCell(sec, cls) {
  if (typeof sec !== 'number' || !Number.isFinite(sec) || Number.isNaN(new Date(sec * 1000).getTime())) return h('span', { class: cls || '' }, '-');
  const now = Date.now() / 1000, abs = new Date(sec * 1000).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' });
  return h('time', { class: 'time ' + (cls || ''), datetime: new Date(sec * 1000).toISOString(), title: abs },
    h('span', { 'aria-hidden': 'true' }, relTime(sec, now)), h('span', { class: 'sr-only' }, abs + ', ' + relTime(sec, now, true)));
}
const crumbNodes = () => crumb().flat().filter(Boolean);
// Every page is drawn through this. The breadcrumb and search sit in the fixed top bar (the search box must keep focus
// while a poll redraws the page), so the shell returns the breadcrumb separately from the page nodes. `filters` are the
// children of the filter row and `actions` sit at the right of the title (Performance puts its live toggle there);
// `inspector` sits beside `body`, and below it on a narrow screen. Without an inspector the body takes the full width.
function pageShell({ title, subtitle, actions, filters, body, inspector }) {
  const head = h('div', { class: 'ptitle' }, h('div', { class: 'grow' }, h('h1', { id: 'title' }, title), subtitle ? h('p', { class: 'psub mut' }, subtitle) : null), actions ? h('div', { class: 'pactions' }, actions) : null);
  const bar = filters && nodesOf([filters]).length ? h('div', { class: 'filters' }, filters) : null;
  return { crumb: crumbNodes(), nodes: [head, bar, inspector ? h('div', { class: 'split' }, body, inspector) : body] };
}
// Each part of the trail is a link, so a selected item can be opened in a new tab and lands on the same item.
function crumb() {
  const g = S.snap && S.snap.groups.find((x) => x.key === S.sel);
  const parts = crumbParts(toHashState(S), g ? g.label : S.sel && unprefixed(S.sel));
  return parts.map((p, i) => [i ? h('span', { 'aria-hidden': 'true' }, '\u203A') : null, i === parts.length - 1 ? h('span', { 'aria-current': 'page' }, p.text) : h('a', { href: p.href }, p.text)]);
}

// The sidebar is rebuilt only when what it shows changes, so a poll does not drop keyboard focus from it; after a
// real change, focus goes back to the control that had it.
let navSig = '';
const badgeHash = (id) => (id === 'apps' ? writeHash({ view: 'apps', filter: 'review' }) : id === 'priv' ? writeHash({ view: 'priv', dd: { 'auth-result': 'failed' } }) : writeHash({ view: 'persist', filter: 'new' }));
function drawNav() {
  const badges = navBadges({ review: reviewCount(), failed: failedAuth(), fresh: S.pers.filter((i) => i.is_new).length });
  const sig = JSON.stringify([S.view, badges]);
  if (sig === navSig) return;
  navSig = sig;
  const nav = $('nav'), had = nav.contains(document.activeElement) ? document.activeElement.dataset.nav : null;
  const item = ({ id, label, icon }) => {
    const b = own(badges, id), svg = h('svg', { width: 16, height: 16, viewBox: '0 0 24 24', fill: 'none', stroke: 'currentColor', 'stroke-width': 1.8, 'aria-hidden': 'true' });
    svg.innerHTML = '<path d="' + icon + '" stroke-linecap="round" stroke-linejoin="round"/>'; // createElement has no SVG namespace for innerHTML children
    return h('div', { class: 'navrow' },
      h('button', { class: 'nav' + (S.view === id ? ' on' : '') + (b ? ' has-badge' : ''), type: 'button', 'data-nav': id, 'aria-current': S.view === id ? 'page' : null, onClick: () => { const wasOpen = S.navOpen; rec({ view: id, navOpen: false }); if (wasOpen) $('main').focus(); if (id === 'performance') loadMetrics(); if (id === 'storage') loadStorage(); } }, svg, h('span', { class: 'grow' }, label)),
      b ? h('a', { class: 'badge ' + b.tone, href: badgeHash(id), 'data-nav': 'badge-' + id, title: b.label, 'aria-label': b.label }, b.text) : null);
  };
  nav.replaceChildren(...NAV_GROUPS.map((g) => h('div', { role: 'group', 'aria-labelledby': 'navg-' + g.toLowerCase() }, h('div', { class: 'navgroup', id: 'navg-' + g.toLowerCase() }, g), NAV.filter((n) => n.group === g).map(item))));
  if (had) { const el = nav.querySelector('[data-nav="' + had + '"]'); if (el) el.focus({ preventScroll: true }); }
}
// The top bar's breadcrumb and the page below it change together.
const show = (page) => { $('crumb').replaceChildren(...nodesOf(page.crumb)); $('content').replaceChildren(...nodesOf(page.nodes)); };
function render() {
  pruneTables(TABLES, Object.keys(PAGE_SORTS[S.view] || {})); pruneTables(HEADERS, Object.keys(PAGE_SORTS[S.view] || {}));
  drawNav();
  document.querySelector('.shell').classList.toggle('nav-open', S.navOpen);
  $('navtoggle').setAttribute('aria-expanded', String(S.navOpen));
  const hadReq = document.activeElement && document.activeElement.dataset ? document.activeElement.dataset.req : null;
  const hint = { guide: ['Search commands and terms', 'Search commands and terms, or apps, ports, hosts, startup items and the Guide'], storage: ['Search folders', 'Search folders, or apps, ports, hosts, startup items and the Guide'] }[S.view]
    || (FILTERABLE.includes(S.view) ? ['Search this page, or everything', 'Search this page, or apps, ports, hosts, startup items and the Guide'] : ['Search everything', 'Search apps, ports, hosts, startup items and the Guide']);
  $('q').placeholder = hint[0] + ' (press /)'; $('q').setAttribute('aria-label', hint[1]);
  searchSyncBox();
  $('notif').replaceChildren(h('div', { class: 'theme-row small mut' }, 'Notify', dropdown('notif', S.settings.notifications, [['off', 'Off'], ['high', 'High only'], ['all', 'All']], setNotifications, 'Notification level')));
  $('theme').replaceChildren(h('div', { class: 'theme-row small mut' }, 'Theme', dropdown('theme', S.theme, [['system', 'System'], ['dark', 'Dark'], ['light', 'Light']], setTheme, 'Color theme')));
  const c = $('content');
  if (S.view !== 'guide') S.gsig = null;
  if (S.view !== 'storage') { S.stMulti = []; S.stAll = false; S.stAsk = false; }
  let page = null;
  if (!S.snap && S.view !== 'guide' && S.view !== 'storage') page = pageShell({ title: TITLES[S.view] || '', body: h('p', { class: 'mut' }, 'Collecting the first snapshot...') }); // the Guide needs no snapshot
  else if (S.view === 'overview') page = overview();
  else if (S.view === 'apps') page = apps();
  else if (S.view === 'priv') page = priv();
  else if (S.view === 'listeners') page = listeners();
  else if (S.view === 'persist') page = persistence();
  else if (S.view === 'performance') page = performanceView_();
  else if (S.view === 'network') page = network();
  else if (S.view === 'history') page = historyView();
  else if (S.view === 'storage') page = storage();
  if (page) show(page);
  else if (S.view === 'guide') {
    // Rebuilt only when what the Guide shows changes: a poll must not drop focus or the selected command.
    if (guideSig(S) !== S.gsig || !c.hasChildNodes()) {
      const g = guide();
      show(g);
      const prev = S.gsig && JSON.parse(S.gsig); S.gsig = guideSig(S);
      const now = JSON.parse(S.gsig);
      if (prev && (prev[0] !== now[0] || prev[1] !== now[1] || prev[2] !== now[2] || prev[4] !== now[4]) && S.gnote) announce(S.gnote);
    }
  }
  else show(pageShell({ title: 'Unknown view', body: h('p', { class: 'mut' }, 'Unknown view.') })); // every NAV id has a branch above; this only guards a future NAV entry added without one
  const m = S.modal, layer = [];
  if (m) layer.push(h('div', { class: 'overlay' }, h('div', { class: 'modal', role: 'dialog', 'aria-modal': 'true', 'aria-label': 'Confirm stop' },
    h('h2', {}, (m.count > 1 ? 'Quit ' : 'Stop ') + m.label + '?'),
    h('p', { class: 'mut small', style: 'margin:0' }, evidenceLine(m.label, m.cpuPct)),
    h('p', { class: 'mut', style: 'margin:0;line-height:1.5' }, m.count > 1 ? 'All ' + m.count + ' of your processes in this app receive SIGTERM and get a chance to save. Unsaved work may be lost.' : 'This process receives SIGTERM. If it ignores it, use Force quit.'),
    h('div', { class: 'code' }, 'kill -TERM ' + m.pids.slice(0, 6).join(' ') + (m.pids.length > 6 ? ' ... (+' + (m.pids.length - 6) + ')' : '')),
    h('div', { style: 'display:flex;gap:8px;justify-content:flex-end;flex-wrap:wrap' }, h('button', { class: 'btn', onClick: () => rec({ modal: null }) }, 'Cancel'), h('button', { class: 'btn btn-danger quiet', onClick: () => stop('TERM') }, 'Quit (SIGTERM)'), h('button', { class: 'btn btn-danger', onClick: () => stop('KILL') }, 'Force quit (SIGKILL)')))));
  // The collapsed-nav drawer's own backdrop (shares .overlay's look, stacked above it): clicking it is the same
  // as Escape. CSS hides both the backdrop and the open drawer above the breakpoint where the sidebar is never
  // collapsed, so this is inert there.
  if (S.navOpen) layer.push(h('div', { class: 'overlay navbackdrop', onClick: () => closeNav() }));
  const trash = trashLayer();
  if (trash) layer.push(trash);
  const sig = sigLayer();
  if (sig) layer.push(sig);
  if (S.toast) layer.push(h('div', { class: 'toast', role: 'status' }, h('span', {}, S.toast), h('button', { class: 'btn sm', onClick: () => rec({ toast: null }) }, 'Dismiss')));
  if (S.copyText) layer.push(h('div', { class: 'copybox', role: 'dialog', 'aria-label': 'Copy blocked' }, h('p', { class: 'small', style: 'margin:0' }, 'The browser did not allow copying. The text is selected: press Cmd+C.'),
    h('textarea', { readonly: true, rows: 8, 'aria-label': 'Row as JSON' }, S.copyText), h('button', { class: 'btn sm', onClick: () => rec({ copyText: null }) }, 'Close')));
  $('layer').replaceChildren(...layer);
  decorateTables();
  const af = (trash || sig || document).querySelector('[data-autofocus]'); if (af) af.focus();
  else if (hadReq) { const el = [...document.querySelectorAll('[data-req]')].find((x) => x.dataset.req === hadReq); if (el) el.focus({ preventScroll: true }); } // a poll redraws the page; keep the keyboard where it was
  syncHash(S, location, history, S.hashReplace);
  S.hashReplace = false;
  searchRefresh();
}
// A button, not a #link: a link would change the address, and the address is the page's state.
$('skip').addEventListener('click', () => $('main').focus());
// The box filters the page you are on where it can and offers results from every page (web/search.js).
searchInit();
// Below the breakpoint where the sidebar collapses behind this button (860px, also in web/app.css -- change both
// together); above it the button is hidden by CSS and neither of these fires from a click.
function openNav() { rec({ navOpen: true }); $('nav').querySelector('[data-nav]')?.focus(); }
// Closing (Escape, the backdrop, or the toggle itself) returns focus to the button that opened it; a nav item's
// own click closes with a different focus target (the new page), so it sets navOpen directly instead of calling this.
function closeNav() { rec({ navOpen: false }); $('navtoggle').focus(); }
$('navtoggle').addEventListener('click', () => (S.navOpen ? closeNav() : openNav()));
// Widening past the breakpoint while the drawer is open would otherwise strand its backdrop over the page: the
// toggle that would close it is hidden by CSS above this width, and the sidebar itself no longer needs collapsing.
window.addEventListener('resize', () => { if (S.navOpen && window.innerWidth > 860) rec({ navOpen: false }); });
// Back and Forward, a pasted link and a clicked crumb all arrive here; the address is the source of truth.
const fromHash = () => { Object.assign(S, patchFromHash(parseHash(location.hash), S)); Object.assign(S, selPatch(S, S.snap)); $('q').value = S.q; };
window.addEventListener('hashchange', () => { searchClose(); fromHash(); render(); loadVolume(); loadProcesses(); loadStorage(); loadMetrics(); if (S.view === 'overview') loadStorageHistory(); });
document.addEventListener('keydown', (e) => { if (e.key === 'Escape' && closeTopLayer()) e.preventDefault(); }); // one Escape closes one layer
document.addEventListener('keydown', searchSlash);
document.addEventListener('click', () => { if (S.menu) rec({ menu: null }); });
applyTheme(); fromHash(); render(); if (S.view === 'overview') loadStorageHistory(); loadVolume(); loadProcesses(); loadStorage(); load(); loadPriv(); loadSudo(); loadPers(); loadHist(); loadSettings(); loadMetrics();
setInterval(load, 10000); setInterval(loadPriv, 30000); setInterval(loadSudo, 10000); setInterval(loadPers, 60000); setInterval(loadHist, 30000); setInterval(loadReqLive, 5000);
setInterval(loadVolume, 30000);
setInterval(loadProcesses, 5000); // matches the server's own 5 s procusage sampler interval
setInterval(loadMetrics, 2000); // loadMetrics itself is the one place that checks S.perfLive, document.hidden and the active view
// A tab backgrounded for a while is caught up in this one listener-driven fetch, not by the interval
// above firing a burst once visible again (it only ever requests what's new since the last ts held).
document.addEventListener('visibilitychange', loadMetrics);
document.addEventListener('visibilitychange', () => loadStorage(false)); // the Event would read as fresh and start a Rescan
document.addEventListener('visibilitychange', loadVolume); // the first read is skipped while the page is hidden
