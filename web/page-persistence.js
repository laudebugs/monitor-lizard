// Same three-way signing classification as sigKind(g), reused rather than re-derived: only the
// leaf for "no signing data" differs (null here, so the Trust column can show '-' and not count
// it as unsigned, vs sigKind's 'unchecked' used for the Apps page's signing filter).
const psig = (i) => { const k = sigKind(i); return k === 'unchecked' ? null : k; };
const trustTag = (i) => { const t = psig(i), label = t === 'apple' ? 'Apple' : 'Signed' + (i.signing && i.signing.team ? ' ' + i.signing.team : ''); return t === null ? (i.trust_note ? h('span', { class: 'mut', title: i.trust_note }, i.program_missing ? 'Program missing' : 'Not checked') : h('span', { class: 'mut' }, '-')) : t === 'unsigned' ? h('span', { class: 'tag bad' }, 'UNSIGNED') : h('span', { class: 'tag ok', title: label }, label); };
function persistence() {
  const q = S.q.toLowerCase(), f = S.pfilter;
  const all = S.pers;
  const inFilter = (i, id) => id === 'all' || (id === 'new' ? i.is_new : i.scope === id);
  const list = all.filter((i) => inFilter(i, f) && (!q || i.label.toLowerCase().includes(q) || i.program.toLowerCase().includes(q)));
  const shown = sortRows(list, 'pers', { label: (i) => i.label, kind: (i) => i.kind, scope: (i) => i.scope, program: (i) => i.program, trust: (i) => psig(i) || '', added: (i) => i.first_seen });
  const pg = paginate(shown, 'pers');
  const chips = [['all', 'All'], ['new', 'New'], ['user', 'User'], ['system', 'System']].map(([id, l]) => h('button', { class: 'chip ' + (f === id ? 'on' : ''), onClick: () => rec({ pfilter: id, page: {} }) }, l + ' ', all.filter((i) => inFilter(i, id)).length));
  const cols = 'grid-template-columns:minmax(120px,1.3fr) 110px 70px minmax(80px,1.6fr) 100px 100px 190px';
  const table = h('section', { class: 'card tbl', style: 'min-width:0' },
    sortHeader('pers', [{ label: 'Label', sort: 'label' }, { label: 'Kind', sort: 'kind' }, { label: 'Scope', sort: 'scope' }, { label: 'Program', sort: 'program' }, { label: 'Trust', sort: 'trust' }, { label: 'Added', sort: 'added' }, { label: 'Expected', id: 'actions', action: true }], cols, 'ell'),
    pg.slice.map((i) => h('div', { class: 'tr ell', style: cols },
      h('span', nameAttrs(i.label, 'display:flex;align-items:center;gap:6px'), i.is_new ? h('span', { class: 'tag warn', title: 'First seen after the first scan' }, 'NEW') : null, i.label),
      h('span', { class: 'mut small' }, i.kind), h('span', { class: 'mut small' }, i.scope),
      h('span', { class: 'mut small', title: i.program }, i.program), h('span', {}, trustTag(i)),
      timeCell(i.first_seen, 'mut small'), h('span', { style: 'display:flex;align-items:center;gap:6px' }, i.expected ? expectedTag() : null, markBtn([['startup', { key: i.mark_key }]], i.expected, i.label)))),
    shown.length ? null : h('div', { class: 'item' }, h('p', {}, all.length ? 'No items match the current filters. ' : 'No startup items found.'), all.length ? clearBtn() : null), pg.pager);
  return pageShell({ title: verdict('persist', { fresh: all.filter((i) => i.is_new).length }), subtitle: plural(all.length, 'startup item', 'startup items') + ' found', filters: chips, body: table });
}


