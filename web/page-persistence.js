// Same three-way signing classification as sigKind(g), reused rather than re-derived: only the
// leaf for "no signing data" differs (null here, so the Trust column can show '-' and not count
// it as unsigned, vs sigKind's 'unchecked' used for the Apps page's signing filter).
const psig = (i) => { const k = sigKind(i); return k === 'unchecked' ? null : k; };
const trustTag = (i) => { const t = psig(i), label = t === 'apple' ? 'Apple' : 'Signed' + (i.signing && i.signing.team ? ' ' + i.signing.team : ''); return t === null ? (i.trust_note ? h('span', { class: 'mut', title: i.trust_note }, i.program_missing ? 'Program missing' : 'Not checked') : h('span', { class: 'mut' }, '-')) : t === 'unsigned' ? h('span', { class: 'tag bad' }, 'UNSIGNED') : h('span', { class: 'tag ok', title: label }, label); };
/* persistence-page:begin */
const PERS_FILTERS = ['all', 'new', 'user', 'system', 'unsigned'];
// Rows carry `sig` (apple, signed, unsigned or null), computed once from the item's signing result.
const persMatches = (r, id) => id === 'all' || (id === 'new' ? r.is_new : id === 'unsigned' ? r.sig === 'unsigned' : r.scope === id);
// Apple items are hidden until asked for, but a search always reaches them.
const persVisible = (rows, showApple, q) => (showApple || q ? rows : rows.filter((r) => r.sig !== 'apple'));
const persCounts = (rows) => Object.fromEntries(PERS_FILTERS.map((id) => [id, rows.filter((r) => persMatches(r, id)).length]));
const PERS_KIND_LABEL = { added: 'ADDED', changed: 'CHANGED', restored: 'RESTORED', removed: 'REMOVED' };
// The change list: entries since the baseline, minus items the owner has since marked expected.
function persChangeRows(changes, items) {
  const expected = new Set(items.filter((i) => i.expected).map((i) => i.path));
  return changes.filter((c) => !expected.has(c.path)).map((c) => ({ path: c.path, label: c.label, kind: c.kind, chip: PERS_KIND_LABEL[c.kind] || String(c.kind).toUpperCase(), at: c.at,
    note: c.kind === 'changed' ? 'program or plist changed' : c.kind === 'removed' ? 'plist no longer present' : c.kind === 'restored' ? 'back after being removed' : c.changed_since_added ? 'changed since it was added' : '' }));
}
// Why an item is flagged, only from fields the server sent. An item with nothing to say says so.
function persWhy(i) {
  const out = [];
  if (i.is_new) out.push(i.was_expected ? 'You marked an earlier version as expected, and it has changed since.' : 'It appeared after your baseline was set.');
  if (i.sig === 'unsigned') out.push('Its program is not signed.');
  if (i.program_missing) out.push('The program it starts is not on disk.');
  if (i.unreadable) out.push('Its plist could not be read: ' + i.unreadable);
  if (i.keep_alive && i.is_new) out.push('It is restarted if it stops.');
  return out.length ? out : ['Nothing about this item is flagged.'];
}
const persSigning = (i) => (i.sig === 'apple' ? 'Signed by Apple' : i.sig === 'signed' ? 'Signed' + (i.signing && i.signing.team ? ', team ' + i.signing.team : '') : i.sig === 'unsigned' ? 'None' : (i.trust_note || 'Not checked'));
const persSubtitle = (rows, baselineAt) => plural(rows.length, 'item runs', 'items run') + ' at login or boot: ' + rows.filter((r) => r.scope === 'user').length + ' yours, ' + rows.filter((r) => r.scope === 'system').length + ' from the system';
/* persistence-page:end */

const persRows = () => S.pers.map((i) => Object.assign({}, i, { sig: psig(i) }));
const persSelect = (path) => { rec({ pItem: S.pItem === path ? null : path }); const el = document.querySelector('[data-pers-item="' + CSS.escape(path) + '"]'); if (el) el.focus(); };
function persInspector(i) {
  const kv = (...pairs) => h('dl', { class: 'kv' }, pairs.filter(Boolean).map(([k, v]) => [h('dt', {}, k), h('dd', { style: 'word-break:break-all' }, v)]));
  const g = S.snap && S.snap.groups.find((x) => i.program && i.program.startsWith(unprefixed(x.key) + '/'));
  const tags = [i.is_new ? h('span', { class: 'tag warn' }, 'new') : null, i.sig === 'unsigned' ? h('span', { class: 'tag bad' }, 'unsigned') : null, i.expected ? expectedTag() : null, h('span', { class: 'tag' }, i.scope + ' ' + i.kind)];
  return h('aside', { class: 'drawer card', 'aria-label': 'Startup item detail' },
    h('div', { class: 'insp-head' }, h('div', { class: 'grow mono', style: 'font-size:16px;font-weight:600;word-break:break-all' }, i.label), h('button', { class: 'btn sm', type: 'button', onClick: () => { const p = S.pItem; rec({ pItem: null }); const el = document.querySelector('[data-pers-item="' + CSS.escape(p) + '"]'); if (el) el.focus(); } }, 'Close')),
    h('div', { class: 'insp-sec', style: 'display:flex;gap:6px;flex-wrap:wrap' }, tags),
    h('div', { class: 'insp-sec' }, h('h3', { class: 'sec' }, 'Why it is flagged'), persWhy(i).map((t) => h('p', { style: 'margin:4px 0' }, t))),
    kv(['Runs', i.schedule || 'unknown'], ['Program', i.program || 'none'], ['Defined in', i.path], ['Signing', persSigning(i)], ['First seen', timeCell(i.first_seen)]),
    h('div', { class: 'insp-sec' }, h('h3', { class: 'sec' }, 'Related'),
      g ? h('p', { style: 'margin:4px 0' }, h('a', { href: writeHash({ view: 'apps', sel: g.key }) }, 'Open app ' + g.label)) : null,
      g ? h('p', { style: 'margin:4px 0' }, h('a', { href: writeHash({ view: 'network', q: g.label }) }, 'Open its connections on Network')) : null,
      h('p', { style: 'margin:4px 0' }, h('a', { href: writeHash({ view: 'guide', q: 'launch' }) }, 'Open the Guide on startup items'))),
    h('div', { class: 'insp-sec', style: 'display:flex;gap:8px;flex-wrap:wrap' },
      markBtn([['startup', { key: i.mark_key }]], i.expected, i.label, i.expected ? '' : 'btn-acc'),
      h('button', { class: 'btn sm', type: 'button', onClick: () => navigator.clipboard.writeText(i.path).then(() => toast('Copied the plist path.'), () => toast('Could not copy. Select the path and copy it yourself.')) }, 'Copy plist path')),
    h('p', { class: 'mut small', style: 'padding:0 16px 14px;margin:0' }, 'Monitor Lizard does not disable startup items. Remove one yourself in Finder.'));
}
function persistence() {
  const q = S.q.toLowerCase(), f = S.pfilter, all = persRows();
  const pool = persVisible(all, S.pApple === '1', q), counts = persCounts(pool), appleN = all.filter((r) => r.sig === 'apple').length;
  const list = pool.filter((i) => persMatches(i, f) && (!q || i.label.toLowerCase().includes(q) || i.program.toLowerCase().includes(q)));
  const shown = sortRows(list, 'pers', { label: (i) => i.label, kind: (i) => i.kind, scope: (i) => i.scope, program: (i) => i.program, trust: (i) => psig(i) || '', added: (i) => i.first_seen });
  const pg = paginate(shown, 'pers');
  const sel = S.pItem ? all.find((i) => i.path === S.pItem) : null;
  const labels = { all: 'All', new: 'New', user: 'User', system: 'System', unsigned: 'Unsigned' };
  const chips = [PERS_FILTERS.map((id) => h('button', { class: 'chip ' + (f === id ? 'on' : ''), 'aria-pressed': String(f === id), onClick: () => rec({ pfilter: id, page: {} }) }, labels[id] + ' \u00b7 ' + counts[id])),
    h('button', { class: 'chip ' + (S.pApple === '1' ? 'on' : ''), 'aria-pressed': String(S.pApple === '1'), title: 'Apple items are hidden by default. A search always includes them.', onClick: () => rec({ pApple: S.pApple === '1' ? '0' : '1', page: {} }) }, 'Apple items \u00b7 ' + (S.pApple === '1' ? 'shown' : 'hidden') + ' \u00b7 ' + appleN)];
  const cols = 'grid-template-columns:minmax(120px,1.3fr) 110px 70px minmax(80px,1.6fr) 100px 100px 190px';
  const table = h('section', { class: 'card tbl', style: 'min-width:0' },
    sortHeader('pers', [{ label: 'Label', sort: 'label' }, { label: 'Kind', sort: 'kind' }, { label: 'Scope', sort: 'scope' }, { label: 'Program', sort: 'program' }, { label: 'Trust', sort: 'trust' }, { label: 'Added', sort: 'added' }, { label: 'Expected', id: 'actions', action: true }], cols, 'ell'),
    pg.slice.map((i) => h('div', { class: 'tr ell' + (S.pItem === i.path ? ' on' : ''), style: cols },
      h('span', nameAttrs(i.label, 'display:flex;align-items:center;gap:6px'), i.is_new ? h('span', { class: 'tag warn', title: 'First seen after the first scan' }, 'NEW') : null,
        h('button', { class: 'linkbtn', type: 'button', 'data-pers-item': i.path, 'aria-pressed': String(S.pItem === i.path), title: i.label, style: 'background:none;border:0;padding:0;text-align:left;font:inherit;color:inherit;min-width:0;overflow:hidden;text-overflow:ellipsis', onClick: () => persSelect(i.path) }, i.label)),
      h('span', { class: 'mut small' }, i.kind), h('span', { class: 'mut small' }, i.scope),
      h('span', { class: 'mut small', title: i.program }, i.program), h('span', {}, trustTag(i)),
      timeCell(i.first_seen, 'mut small'), h('span', { style: 'display:flex;align-items:center;gap:6px' }, i.expected ? expectedTag() : null, markBtn([['startup', { key: i.mark_key }]], i.expected, i.label)))),
    shown.length ? null : h('div', { class: 'item' }, h('p', {}, all.length ? 'No items match the current filters. ' : 'No startup items found.'), all.length ? clearBtn() : null), pg.pager);
  const changes = persChangeRows(S.persChanges || [], S.pers);
  const changeList = h('section', { class: 'card', 'aria-label': 'Changed since baseline' },
    h('div', { class: 'item', style: 'display:flex;justify-content:space-between' }, h('h2', { class: 'sec', style: 'margin:0' }, 'Changed since baseline'), h('span', { class: 'mut small' }, 'Added, removed or changed on disk')),
    changes.length ? changes.slice(0, 20).map((c) => h('div', { class: 'item' }, h('span', { class: 'tag' + (c.kind === 'added' ? ' warn' : '') }, c.chip),
      h('div', { class: 'grow', style: 'min-width:0;overflow-wrap:anywhere' }, h('b', {}, c.label), c.note ? h('span', { class: 'mut small' }, ' \u00b7 ' + c.note) : null), timeCell(c.at, 'mut small')))
      : h('div', { class: 'item' }, h('p', {}, 'No changes since your baseline.')),
    changes.length > 20 ? h('p', { class: 'mut small', style: 'padding:0 16px 12px;margin:0' }, (changes.length - 20) + ' older changes are not shown.') : null);
  return pageShell({ title: verdict('persist', { fresh: all.filter((i) => i.is_new).length }), subtitle: all.length ? persSubtitle(all) : 'No startup items found', filters: chips, body: [changeList, table], inspector: sel ? persInspector(sel) : null });
}
