// A Reading<T> from the Rust side serializes as {value: T}, {unavailable: "reason"} or the bare
// string "restricted"; `null` is a plain Option<Reading<T>> that was never populated this sample.
/* perf-reading:begin */
// A string is not an object `in` can test: the bare "restricted" would throw, and one such sample would take the whole page's render with it.
const rv = (r) => (r && typeof r === 'object' && 'value' in r ? r.value : null);
const rReason = (r) => (r == null ? 'not sampled yet' : r === 'restricted' ? 'restricted' : (typeof r === 'object' && r.unavailable) || 'unavailable');
/* perf-reading:end */
function perfSeries(samples, pick) { return samples.map((s) => rv(pick(s))); }
// One drawn line: its values, and why sample i has none (the hover tooltip says so instead of showing a blank or a 0).
const perfLine = (win, label, color, pick) => ({ label, color, values: perfSeries(win, pick), reason: (i) => rReason(pick(win[i])) });
const perfWinName = () => (S.perfWin === '1h' ? 'last hour' : 'last 5 minutes');
// What chartHost() needs besides the lines: one stable id per chart (focus and the highlight are kept by it across redraws).
const chartSpec = (win, id, name, fmt, extra) => Object.assign({ id, name: name + ', ' + perfWinName(), ts: win.map((s) => s.ts), fmt }, extra);
// The reason behind the first unreadable reading in a chain, in display order: several tiles and
// cards show one combined value built from several Readings, so there is one "why" to report, not one
// per field. `fields` is `[[value, reading], ...]`; the fields that did read are not reported.
function firstReason(fields) { for (const [v, r] of fields) if (v == null) return rReason(r); return null; }
/* perf-window:begin */
// Nearest-point decimation, not averaging: the six values this page mixes (percent, bytes/sec,
// bytes) would need different combining rules to average sensibly, and a sparkline only needs to
// read as the right shape, not preserve every sample.
function downsample(points, max) {
  if (points.length <= max) return points;
  const out = [];
  for (let i = 0; i < max; i++) out.push(points[Math.floor((i + 1) * points.length / max) - 1]);
  return out;
}
function perfWindowed() {
  const secs = S.perfWin === '1h' ? 3600 : 300;
  const cutoff = (S.metrics.length ? S.metrics[S.metrics.length - 1].ts : 0) - secs;
  const win = S.metrics.filter((s) => s.ts > cutoff);
  // About one point per two pixels of the sparkline's actual rendered width, so density tracks a
  // resized window instead of assuming one fixed layout; the previous render's own .spark element
  // is the best available measurement for the one about to be built.
  const w = document.querySelector('.spark')?.clientWidth || 200;
  return S.perfWin === '1h' ? downsample(win, Math.max(10, Math.round(w / 2))) : win;
}
/* perf-window:end */
// The one place a sample index and a value become an SVG coordinate, shared by every chart below
// so a fixed-viewBox chart (see spark()'s comment on why it's fixed) always maps the same way.
const chartXY = (n, w, max, hgt) => ({ x: (i) => (i * w / n).toFixed(1), y: (v) => (hgt - Math.min(v, max) / max * hgt).toFixed(1) });
function sparkPath(values, max, w, hgt) {
  const pts = values.map((v, i) => [i, v]).filter(([, v]) => v != null);
  if (!pts.length) return null;
  const { x, y } = chartXY(values.length - 1 || 1, w, max, hgt);
  const line = pts.map(([i, v], k) => (k ? 'L' : 'M') + x(i) + ' ' + y(v)).join(' ');
  return { line, area: line + ' L ' + x(pts[pts.length - 1][0]) + ' ' + hgt + ' L ' + x(pts[0][0]) + ' ' + hgt + ' Z' };
}
// viewBox stays a fixed 200x46 regardless of the element's real rendered width (preserveAspectRatio
// "none" stretches it to fill), so the path math doesn't need the live width perfWindowed() measures
// for downsampling density -- the two are independent uses of the same measurement.
function spark(values, max, label, hv) {
  const w = 200, hgt = 46, path = sparkPath(values, max || 1, w, hgt);
  const svg = h('svg', { class: 'spark', viewBox: '0 0 ' + w + ' ' + hgt, preserveAspectRatio: 'none', role: 'img', 'aria-label': label });
  if (path) svg.innerHTML = '<path d="' + path.area + '" fill="var(--acc)" fill-opacity="0.18"></path><path d="' + path.line + '" fill="none" stroke="var(--acc)" stroke-width="1.6" vector-effect="non-scaling-stroke"></path>';
  return hv ? chartHost(svg, { ...hv, max: max || 1, series: [{ label: hv.label, color: 'var(--acc)', values, reason: hv.reason }], tile: true }) : svg;
}
// A plain line chart (no fill) for one or more series sharing one y-axis, taller than the tile
// spark() so per-core bars and the stacked area below it have somewhere to sit; the viewBox stays
// fixed and lets the svg stretch the same way spark() does, for the same reason (see spark()).
function linesChart(series, max, hgt, hv) {
  const w = 600;
  const svg = h('svg', { class: 'spark big', viewBox: '0 0 ' + w + ' ' + hgt, preserveAspectRatio: 'none', role: 'img' });
  svg.innerHTML = series.map(({ values, color }) => {
    const path = sparkPath(values, max || 1, w, hgt);
    return path ? '<path d="' + path.line + '" fill="none" stroke="' + color + '" stroke-width="1.6" vector-effect="non-scaling-stroke"></path>' : '';
  }).join('');
  return hv ? chartHost(svg, { ...hv, max: max || 1, series, keys: true }) : svg;
}
// A stacked area for any number of layers: each layer's own edge is plotted at its running cumulative
// total (itself plus every layer below), not its own raw value. Otherwise, on a sample where an upper
// layer dips below a lower one, its edge would sit below the lower line instead of stacked above it.
// Layers are bottom to top, each `{values, color}`; a sample index is skipped (not just that layer)
// when any layer's value there is null, so the chart never draws a row with one series silently
// treated as zero.
function stackedAreaN(layers, max, hgt, hv) {
  const w = 600, n = layers[0].values.length, pts = [];
  for (let i = 0; i < n; i++) {
    const vals = layers.map((l) => l.values[i]);
    if (vals.some((v) => v == null)) continue;
    let cum = 0;
    pts.push([i, vals.map((v) => (cum += v))]);
  }
  const svg = h('svg', { class: 'spark big', viewBox: '0 0 ' + w + ' ' + hgt, preserveAspectRatio: 'none', role: 'img' });
  if (!pts.length) return hv ? chartHost(svg, { ...hv, max, series: layers, stacked: true, keys: true }) : svg;
  const { x, y } = chartXY(n - 1 || 1, w, max, hgt);
  const paths = layers.map((layer, li) => {
    const topLine = pts.map(([i, tops], k) => (k ? 'L' : 'M') + x(i) + ' ' + y(tops[li])).join(' ');
    const bottomLine = li === 0
      ? 'L' + x(pts[pts.length - 1][0]) + ' ' + hgt + ' L' + x(pts[0][0]) + ' ' + hgt
      : pts.slice().reverse().map(([i, tops]) => 'L' + x(i) + ' ' + y(tops[li - 1])).join(' ');
    return '<path d="' + topLine + ' ' + bottomLine + ' Z" fill="' + layer.color + '" fill-opacity="0.25"></path>';
  });
  const outline = pts.map(([i, tops], k) => (k ? 'L' : 'M') + x(i) + ' ' + y(tops[layers.length - 1])).join(' ');
  svg.innerHTML = paths.join('') + '<path d="' + outline + '" fill="none" stroke="var(--fg)" stroke-width="1.2" vector-effect="non-scaling-stroke"></path>';
  if (!hv) return svg;
  // Each layer is drawn at its running total, so its dot sits there while its tooltip row shows its own value.
  const plot = layers.map(() => new Array(n).fill(null));
  for (const [i, tops] of pts) tops.forEach((t, li) => { plot[li][i] = t; });
  return chartHost(svg, { ...hv, max, stacked: true, keys: true, series: layers.map((l, li) => ({ ...l, plot: plot[li] })) });
}
// A two-layer stacked area for user/system CPU%, the common case of stackedAreaN().
const stackedArea = (bottom, top, max, hgt, hv) => stackedAreaN([bottom, top], max, hgt, hv);
// Colors a per-core load bar the same three-tier way the design calls for everywhere load is shown:
// quiet under 65%, a warning up to 85%, bad above it -- never a fourth color, so this stays one scale.
const loadColor = (pct) => (pct > 85 ? 'var(--bad)' : pct > 65 ? 'var(--warn)' : 'var(--acc)');
// A colored square plus its series name and current value, for the legend under a multi-line chart.
const legendDot = (color, text) => h('span', { class: 'mut' }, h('i', { class: 'legend-dot', style: 'display:inline-block;width:8px;height:8px;background:' + color + ';margin-right:4px' }), text);
function coreBars(label, cores) {
  // A definite flex-basis, not just flex:1: without it this div shrinks to its content's min
  // width inside the wrapping flex row, and the bar's percentage width inside it resolves against
  // that near-zero width instead of a real one (first hit by the bars rendering invisibly here).
  return h('div', { style: 'flex:1 1 260px;min-width:220px' }, h('div', { class: 'small mut', style: 'margin-bottom:6px' }, label + ' (' + cores.length + ')'),
    h('div', { style: 'display:flex;flex-direction:column;gap:6px' }, cores.map(([idx, pct]) =>
      h('div', { style: 'display:flex;align-items:center;gap:8px' },
        h('span', { class: 'mut small', style: 'width:52px;flex:none' }, 'Core ' + idx),
        h('div', { class: 'bar', style: 'flex:1' }, h('i', { style: 'width:' + Math.min(pct, 100) + '%;background:' + loadColor(pct) })),
        h('span', { class: 'small', style: 'width:36px;text-align:right;flex:none' }, pct == null ? '-' : Math.round(pct) + '%')))));
}
// Only tiles with a detail tab pass a `go` handler below.
function perfOverviewTab_(win, last, stale, winLabel) {
  const f1 = (v) => v.toFixed(1), f0 = (v) => Math.round(v);
  const cpu = perfSeries(win, (s) => s.cpu.total_pct), cpuLast = rv(last && last.cpu.total_pct);
  const gpu = perfSeries(win, (s) => s.gpu.device_pct), gpuLast = rv(last && last.gpu.device_pct);
  const memUsed = win.map((s) => { const a = rv(s.memory.app_bytes), w = rv(s.memory.wired_bytes), c = rv(s.memory.compressed_bytes); return a == null || w == null || c == null ? null : a + w + c; });
  const memLast = memUsed[memUsed.length - 1], memTotal = rv(last && last.memory.total_bytes);
  const disk = perfSeries(win, (s) => s.disk.read_bytes_per_sec), diskLast = rv(last && last.disk.read_bytes_per_sec);
  const net = perfSeries(win, (s) => s.network.down_bytes_per_sec), netLast = rv(last && last.network.down_bytes_per_sec);
  const netState = networkState({ interfacesUp: last && last.network.interfaces_up, down: netLast, up: rv(last && last.network.up_bytes_per_sec) });
  const noConnection = netState.kind === 'no-connection';
  // rec() writes the tab into the address, so the tile is a link that Back returns from.
  const openDiskNet = () => { rec({ perfTab: 'disknet' }); loadVolume(); };
  const memReason = () => { const m = last && last.memory; if (!m) return 'not sampled yet'; return firstReason([[rv(m.app_bytes), m.app_bytes], [rv(m.wired_bytes), m.wired_bytes], [rv(m.compressed_bytes), m.compressed_bytes]]); };
  // Each tile passes its own reason thunk rather than being looked up by its label string, so
  // renaming a label can't silently break which reason it reports.
  const tile = (label, values, lastVal, max, fmt, sub, aria, reason, go, reasonAt) => {
    if (lastVal == null) return h('div', { class: 'tile na' }, h('span', { class: 'l' }, label), h('span', { class: 'v mut' }, 'Unavailable'), h('span', { class: 's' }, reason()));
    return h(go ? 'button' : 'div', { class: 'tile' + (stale ? ' stale' : ''), onClick: go }, h('span', { class: 'l' }, h('span', {}, label), stale ? h('span', { class: 'tag warn', title: 'No new sample in a while' }, 'stale') : null), h('span', { class: 'v' }, fmt(lastVal)), h('span', { class: 's' }, sub), spark(values, max, aria, chartSpec(win, 'tile-' + label.toLowerCase(), label, fmt, { label, reason: reasonAt })));
  };
  const at = (pick) => (i) => rReason(pick(win[i]));
  const tiles = [
    tile('CPU', cpu, cpuLast, 100, (v) => f0(v) + '%', 'total, last ' + winLabel, 'CPU utilization, now ' + f0(cpuLast || 0) + '%', () => rReason(last && last.cpu.total_pct), () => rec({ perfTab: 'cpugpu' }), at((s) => s.cpu.total_pct)),
    tile('GPU', gpu, gpuLast, 100, (v) => f0(v) + '%', 'device utilization', 'GPU utilization, now ' + f0(gpuLast || 0) + '%', () => rReason(last && last.gpu.device_pct), () => rec({ perfTab: 'cpugpu' }), at((s) => s.gpu.device_pct)),
    tile('Memory', memUsed, memLast, memTotal || 1, (v) => f1(v / 1e9) + ' GB', 'used' + (memTotal ? ' of ' + f1(memTotal / 1e9) + ' GB' : ''), 'Memory used, now ' + f1((memLast || 0) / 1e9) + ' gigabytes', memReason, () => rec({ perfTab: 'memory' }), (i) => { const m = win[i].memory; return firstReason([[rv(m.app_bytes), m.app_bytes], [rv(m.wired_bytes), m.wired_bytes], [rv(m.compressed_bytes), m.compressed_bytes]]); }),
    tile('Disk', disk, diskLast, Math.max(1, ...disk.filter((v) => v != null), 1e6), formatRate, 'read now', 'Disk read throughput, now ' + formatRate(diskLast), () => rReason(last && last.disk.read_bytes_per_sec), openDiskNet, at((s) => s.disk.read_bytes_per_sec)),
    noConnection ? h('button', { class: 'tile na', onClick: openDiskNet }, h('span', { class: 'l' }, 'Network'), h('span', { class: 'v' }, netState.text), h('span', { class: 's' }, 'No network interface is up.'))
    : tile('Network', net, netLast, Math.max(1, ...net.filter((v) => v != null), 1e6), formatRate, 'down now', 'Network download, now ' + formatRate(netLast), () => rReason(last && last.network.down_bytes_per_sec), openDiskNet, at((s) => s.network.down_bytes_per_sec)),
    h('div', { class: 'tile na' }, h('span', { class: 'l' }, h('span', {}, 'Power and thermals'), h('span', { class: 'tag' }, 'needs administrator')), h('span', { class: 'v mut' }, 'Unavailable'), h('span', { class: 's' }, 'powermetrics needs administrator rights on this Mac; never shown as 0 W.')),
  ];
  return h('div', { class: 'tiles' }, tiles);
}
// Broken into cards one level more detailed than the Overview tile each came from (a single % and
// a thin sparkline there), since that's the level of detail someone clicking through for "why is my
// CPU/GPU busy" actually needs: the user/system split, not just total; per-core, not just an average.
function perfCpuGpuTab_(win, last, winLabel) {
  const f0 = (v) => Math.round(v);
  const totalLast = rv(last && last.cpu.total_pct), userLast = rv(last && last.cpu.user_pct), sysLast = rv(last && last.cpu.system_pct);
  const deviceLast = rv(last && last.gpu.device_pct);
  const cores = rv(last && last.cpu.per_core_pct), effN = rv(last && last.cpu.eff_cores), perfN = rv(last && last.cpu.perf_cores);
  // The counts come from hw.perflevel0/1 (confirmed on this Mac's IORegistry: logical CPU indices
  // run efficiency cores first, then performance -- the same order Apple Silicon uses generally),
  // so the split is by count, not a hardcoded 6+2; if the counts don't add up to the core list this
  // Mac actually reported, cores are shown ungrouped rather than guessing a split that doesn't fit.
  const grouped = cores != null && effN != null && perfN != null && effN + perfN === cores.length;
  const cpuCard = h('section', { class: 'card', style: 'padding:16px;margin-bottom:14px' },
    h('div', { style: 'display:flex;justify-content:space-between;align-items:baseline;margin-bottom:10px' },
      h('h3', { class: 'sec', style: 'margin:0' }, 'CPU: user + system'),
      totalLast == null ? h('span', { class: 'mut small' }, rReason(last && last.cpu.total_pct)) : h('span', { class: 'v', style: 'font-size:20px' }, f0(totalLast) + '%')),
    totalLast == null ? null : stackedArea(perfLine(win, 'User', 'var(--acc)', (s) => s.cpu.user_pct), perfLine(win, 'System', 'var(--s2)', (s) => s.cpu.system_pct), 100, 90, chartSpec(win, 'chart-cpu-split', 'CPU, user and system', (v) => f0(v) + '%')),
    h('div', { style: 'display:flex;gap:16px;margin-top:8px;font-size:12px' },
      legendDot('var(--acc)', 'User ' + (userLast == null ? '-' : f0(userLast) + '%')),
      legendDot('var(--s2)', 'System ' + (sysLast == null ? '-' : f0(sysLast) + '%'))));
  const loadRow = h('div', { style: 'display:flex;gap:24px;margin-bottom:14px' }, [['1 min', 'load1'], ['5 min', 'load5'], ['15 min', 'load15']].map(([l, k]) => {
    const v = rv(last && last[k]);
    return h('div', {}, h('div', { class: 'mut small' }, l), h('div', { style: 'font-size:20px;font-weight:600' }, v == null ? '-' : v.toFixed(2)));
  }));
  const coreCard = h('section', { class: 'card', style: 'padding:16px;margin-bottom:14px' },
    h('h3', { class: 'sec', style: 'margin:0 0 4px' }, 'Per-core load'),
    cores == null ? h('p', { class: 'mut small' }, rReason(last && last.cpu.per_core_pct)) :
      grouped ? h('div', { style: 'display:flex;flex-wrap:wrap;gap:20px' },
          coreBars('Efficiency', cores.slice(0, effN).map((v, i) => [i, v])),
          coreBars('Performance', cores.slice(effN).map((v, i) => [effN + i, v])))
        : coreBars('All cores', cores.map((v, i) => [i, v])),
    h('p', { class: 'mut small', style: 'margin:10px 0 0' }, grouped ? 'Grouping assumes efficiency cores are the first ' + effN + ' logical CPUs reported by the kernel, confirmed on this Mac’s IORegistry but not re-checked on every machine.' : 'Core counts from this Mac did not add up to a confirmed efficiency/performance split, so cores are shown in kernel index order.'));
  const gpuCard = h('section', { class: 'card', style: 'padding:16px' },
    h('div', { style: 'display:flex;justify-content:space-between;align-items:baseline;margin-bottom:10px' },
      h('h3', { class: 'sec', style: 'margin:0' }, 'GPU'),
      deviceLast == null ? h('span', { class: 'mut small' }, rReason(last && last.gpu.device_pct)) : h('span', { class: 'v', style: 'font-size:20px' }, f0(deviceLast) + '%')),
    deviceLast == null ? null : linesChart([
      perfLine(win, 'Device', 'var(--fg)', (s) => s.gpu.device_pct), perfLine(win, 'Renderer', 'var(--acc)', (s) => s.gpu.renderer_pct), perfLine(win, 'Tiler', 'var(--s2)', (s) => s.gpu.tiler_pct),
    ], 100, 90, chartSpec(win, 'chart-gpu', 'GPU utilization', (v) => f0(v) + '%')),
    deviceLast == null ? null : h('div', { style: 'display:flex;gap:16px;margin-top:8px;font-size:12px;flex-wrap:wrap' },
      legendDot('var(--fg)', 'Device ' + f0(deviceLast) + '%'),
      legendDot('var(--acc)', 'Renderer ' + (rv(last.gpu.renderer_pct) == null ? '-' : f0(rv(last.gpu.renderer_pct)) + '%')),
      legendDot('var(--s2)', 'Tiler ' + (rv(last.gpu.tiler_pct) == null ? '-' : f0(rv(last.gpu.tiler_pct)) + '%'))),
    h('p', { class: 'mut small', style: 'margin:10px 0 0' }, 'Per-core frequency, GPU power and per-process GPU use are not measured on this Mac.'));
  return [cpuCard, loadRow, coreCard, gpuCard];
}
// One capacity bar, built from plain numbers so any page can draw it. Pass { used, purgeable, free, unavailable }
// as capacitySegments() documents; a part that could not be read is null, never 0.
function capacityBar(args) {
  const { segments, total, note } = capacitySegments(args);
  // Purgeable is hatched, not just amber: green and amber sit at nearly the same lightness, so the stripes carry the difference for a reader who cannot tell the hues apart.
  const COLORS = { used: 'var(--acc)', purgeable: 'repeating-linear-gradient(135deg,var(--s3) 0 3px,var(--panel2) 3px 5px)', free: 'var(--line)' };
  const NAMES = { used: 'Used', purgeable: 'Purgeable', free: 'Free' };
  if (!segments.length) return h('p', { class: 'mut small', style: 'margin:0' }, 'Capacity unavailable: ' + note);
  const summary = segments.map((x) => NAMES[x.key] + ' ' + formatBytes(x.bytes)).join(', ') + (total ? ', of ' + formatBytes(total) : '');
  return h('div', {},
    h('div', { class: 'capbar', role: 'img', 'aria-label': 'Capacity: ' + summary },
      segments.map((x) => h('i', { style: 'width:' + x.pct.toFixed(2) + '%;background:' + COLORS[x.key] }))),
    h('div', { style: 'display:flex;gap:16px;flex-wrap:wrap;margin-top:8px;font-size:12px' },
      segments.map((x) => legendDot(COLORS[x.key], NAMES[x.key] + ' ' + formatBytes(x.bytes) + ' (' + x.shown + '%)'))),
    note ? h('p', { class: 'mut small', style: 'margin:8px 0 0' }, note) : null);
}
// Reads /api/volume's capacity (a Reading per part) into capacityBar's argument.
function volumeBarArgs(vol) {
  if (!vol) return { used: null, purgeable: null, free: null, unavailable: DISK_REASON_UNKNOWN };
  const reason = [vol.purgeable, vol.used, vol.free].map((r) => (r && 'unavailable' in r ? r.unavailable : null)).find(Boolean);
  return { used: rv(vol.used), purgeable: rv(vol.purgeable), free: rv(vol.free), unavailable: reason || null };
}
// A failed read keeps the card honest (it says so and retries on the next poll) instead of staying on "not read yet".
async function loadVolume() {
  if (document.hidden || !(S.view === 'overview' || (S.view === 'performance' && S.perfTab === 'disknet'))) return;
  try {
    const res = await api('/api/volume');
    if (!res.ok) throw new Error('volume read failed');
    const next = { volume: await res.json(), volumeFailed: false };
    if (S.view === 'overview') { Object.assign(S, next); refreshMachine(); } else quiet(next);
  } catch (e) { const next = { volume: null, volumeFailed: true }; if (S.view === 'overview') { Object.assign(S, next); refreshMachine(); } else quiet(next); }
}
async function loadProcesses() {
  if (document.hidden || S.view !== 'performance' || S.perfTab !== 'topconsumers') return;
  try {
    const rows = await (await api('/api/processes?sort=' + S.procSort + '&limit=50')).json();
    quiet({ procRows: rows.concat(justStoppedRows(S.procRows || [], rows)), procFailed: false });
  } catch (e) { quiet({ procRows: null, procFailed: true }); }
}
const CAPACITY_STATUS_TEXT = { loading: 'Reading capacity...', failed: 'Could not read capacity. Retrying.' };
const card = (...kids) => h('section', { class: 'card', style: 'padding:16px;margin-bottom:14px' }, ...kids);
// Cards use one rate chart each: the series share an axis, and the legend gives the current value in words
// so the colors are never the only carrier.
function rateCard(title, win, picks, lastValues, fmt, floor, id) {
  const lines = picks.map((pick, i) => perfLine(win, lastValues[i].label, lastValues[i].color, pick)), series = lines.map((l) => l.values);
  const nothing = lastValues.every((v) => v.value == null);
  return card(h('h3', { class: 'sec', style: 'margin:0 0 10px' }, title),
    nothing ? h('p', { class: 'mut small', style: 'margin:0' }, lastValues.map((v) => v.reason).find(Boolean) || 'not sampled yet')
      : [linesChart(lines, rateChartMax(series, floor), 90, chartSpec(win, id, title, fmt)),
        h('div', { style: 'display:flex;gap:16px;margin-top:8px;font-size:12px;flex-wrap:wrap' }, lastValues.map((v) => legendDot(v.color, v.label + ' ' + fmt(v.value))))]);
}
function perfDiskNetTab_(win, last) {
  const d = last && last.disk, n = last && last.network;
  const rd = d && d.read_bytes_per_sec, wr = d && d.write_bytes_per_sec, ro = d && d.read_ops_per_sec, wo = d && d.write_ops_per_sec;
  const throughput = rateCard('Disk throughput', win, [(s) => s.disk.read_bytes_per_sec, (s) => s.disk.write_bytes_per_sec],
    [{ label: 'Read', color: 'var(--acc)', value: rv(rd), reason: rReason(rd) }, { label: 'Write', color: 'var(--s2)', value: rv(wr), reason: rReason(wr) }], formatRate, 1e6, 'chart-disk-throughput');
  const ops = rateCard('Operations per second', win, [(s) => s.disk.read_ops_per_sec, (s) => s.disk.write_ops_per_sec],
    [{ label: 'Read', color: 'var(--acc)', value: rv(ro), reason: rReason(ro) }, { label: 'Write', color: 'var(--s2)', value: rv(wo), reason: rReason(wo) }], formatOps, 100, 'chart-disk-ops');
  const status = capacityStatus({ volume: S.volume, failed: S.volumeFailed });
  const capacity = card(h('h3', { class: 'sec', style: 'margin:0 0 10px' }, 'Capacity of the startup disk'),
    status === 'ok' ? capacityBar(volumeBarArgs(S.volume)) : h('p', { class: 'mut small', style: 'margin:0' }, CAPACITY_STATUS_TEXT[status]));
  const nd = n && n.down_bytes_per_sec, nu = n && n.up_bytes_per_sec;
  const state = networkState({ interfacesUp: n ? n.interfaces_up : null, down: rv(nd), up: rv(nu) });
  const network = state.kind === 'ok'
    ? rateCard('Network', win, [(s) => s.network.down_bytes_per_sec, (s) => s.network.up_bytes_per_sec],
      [{ label: 'Download', color: 'var(--acc)', value: rv(nd), reason: rReason(nd) }, { label: 'Upload', color: 'var(--s2)', value: rv(nu), reason: rReason(nu) }], formatRate, 1e6, 'chart-network')
    : card(h('h3', { class: 'sec', style: 'margin:0 0 10px' }, 'Network'),
      h('p', { class: state.kind === 'no-connection' ? '' : 'mut small', style: 'margin:0;font-weight:' + (state.kind === 'no-connection' ? '600' : '400') }, state.kind === 'no-connection' ? state.text : 'Network rates are unavailable: ' + rReason(nd || nu)),
      state.kind === 'no-connection' ? h('p', { class: 'mut small', style: 'margin:6px 0 0' }, 'No network interface is up, so there is nothing to measure.') : null);
  return [capacity, throughput, ops, network,
    h('p', { class: 'mut small' }, 'Totals cover the startup disk and every network interface except loopback. Per-process disk use is not shown here.')];
}
// label, color and the MemorySample field each legend dot and the stacked chart read; one list drives
// both so adding or reordering a layer can't desync a legend dot from the series it describes.
const MEMORY_LAYERS = [['app', 'App', 'var(--acc)'], ['wired', 'Wired', 'var(--s2)'], ['compressed', 'Compressed', 'var(--s3)'], ['cached', 'Cached', 'var(--line)']];
function perfMemoryTab_(win, last) {
  const m = last && last.memory;
  const mem = { app: rv(m && m.app_bytes), wired: rv(m && m.wired_bytes), compressed: rv(m && m.compressed_bytes), cached: rv(m && m.cached_bytes) };
  const totalLast = rv(m && m.total_bytes), haveAll = Object.values(mem).every((v) => v != null);
  const memReason = () => (m ? firstReason(MEMORY_LAYERS.map(([key]) => [mem[key], m[key + '_bytes']])) : 'not sampled yet');
  const usedLast = haveAll ? Object.values(mem).reduce((a, b) => a + b, 0) : null;
  const memCard = h('section', { class: 'card', style: 'padding:16px;margin-bottom:14px' },
    h('div', { style: 'display:flex;justify-content:space-between;align-items:baseline;margin-bottom:10px' },
      h('h3', { class: 'sec', style: 'margin:0' }, 'App, wired, compressed and cached'),
      haveAll ? h('span', { class: 'v', style: 'font-size:20px' }, formatBytes(usedLast) + (totalLast ? ' of ' + formatBytes(totalLast) : '')) : h('span', { class: 'mut small' }, memReason())),
    haveAll ? stackedAreaN(MEMORY_LAYERS.map(([key, label, color]) => perfLine(win, label, color, (s) => s.memory[key + '_bytes'])), totalLast || usedLast || 1, 90, chartSpec(win, 'chart-memory', 'Memory, app, wired, compressed and cached', formatBytes)) : null,
    haveAll ? h('div', { style: 'display:flex;gap:16px;margin-top:8px;font-size:12px;flex-wrap:wrap' },
      MEMORY_LAYERS.map(([key, label, color]) => legendDot(color, label + ' ' + formatBytes(mem[key])))) : null,
    h('p', { class: 'mut small', style: 'margin:10px 0 0' }, '"App, wired and compressed" is this page\'s reading of what Activity Monitor calls Memory Used.'),
    h('p', { class: 'mut small', style: 'margin:4px 0 0' }, 'The two were not compared side by side on a real Mac before this shipped, so treat the mapping as inference until someone records that comparison.'));
  const level = rv(m && m.pressure_level), pct = rv(m && m.pressure_pct_available);
  const tag = level == null ? null : pressureTag(level);
  const pressureCard = h('section', { class: 'card', style: 'padding:16px;margin-bottom:14px' },
    h('div', { style: 'display:flex;justify-content:space-between;align-items:baseline;margin-bottom:6px' },
      h('h3', { class: 'sec', style: 'margin:0' }, 'Memory pressure'),
      tag ? h('span', { class: 'tag ' + tag.cls }, tag.text) : h('span', { class: 'mut small' }, 'Unavailable: ' + rReason(m && m.pressure_level))),
    // Shown next to the level even when it reads "normal": one report had the level stay normal
    // through swap thrash, so the percentage is the number that actually moved.
    h('p', { class: 'mut small', style: 'margin:0' }, pct == null ? (tag ? 'Percent available: unavailable (' + rReason(m && m.pressure_pct_available) + ')' : null) : pct + '% of memory available'));
  const used = rv(m && m.swap_used_bytes), total = rv(m && m.swap_total_bytes), swap = swapState(used, total);
  const swapCard = h('section', { class: 'card', style: 'padding:16px' },
    h('h3', { class: 'sec', style: 'margin:0 0 10px' }, 'Swap'),
    swap.kind === 'unavailable' ? h('p', { class: 'mut small', style: 'margin:0' }, 'Swap is unavailable: ' + rReason(used == null ? m && m.swap_used_bytes : m && m.swap_total_bytes))
      : swap.kind === 'none' ? h('p', { class: 'mut small', style: 'margin:0' }, 'No swap in use.')
      : [h('div', { class: 'capbar', role: 'img', 'aria-label': 'Swap used: ' + formatBytes(used) + ' of ' + formatBytes(total) },
          h('i', { style: 'width:' + swap.pct.toFixed(2) + '%;background:' + loadColor(swap.pct) })),
        h('p', { class: 'mut small', style: 'margin:8px 0 0' }, formatBytes(used) + ' of ' + formatBytes(total) + ' used')]);
  return [memCard, pressureCard, swapCard];
}
const PROC_SORTS = [['cpu', 'CPU'], ['mem', 'Memory'], ['disk', 'Disk I/O']];
const PROC_STATUS_TEXT = { loading: 'Reading processes...', failed: 'Could not read processes. Retrying.' };
function perfTopConsumersTab_() {
  const rows = S.procRows;
  const cols = 'grid-template-columns:minmax(160px,2fr) 70px 90px 100px 100px 130px';
  const header = h('div', { class: 'tr h', style: cols }, ['Process', 'CPU', 'Memory', 'Disk read', 'Disk write', ''].map((l) => h('span', {}, l)));
  const row = (p) => {
    // Shown once more after it drops out of a fresh read (see loadProcesses), so Stop -- or the
    // process exiting on its own -- has something to point to rather than the row just vanishing.
    if (p.stopped) {
      return h('div', { class: 'tr ell', style: cols + ';cursor:default;opacity:.6' },
        h('span', nameAttrs(p.command), p.command + ' (' + p.pid + ')'),
        h('span', {}, '-'), h('span', {}, '-'), h('span', {}, '-'), h('span', {}, '-'),
        h('span', {}, h('span', { class: 'tag' }, 'stopped')));
    }
    const cpu = rv(p.cpu_pct), mem = rv(p.mem_bytes);
    const dr = p.disk_read_bytes_per_sec ? rv(p.disk_read_bytes_per_sec) : null, dw = p.disk_write_bytes_per_sec ? rv(p.disk_write_bytes_per_sec) : null;
    return h('div', { class: 'tr ell', style: cols + ';cursor:default' },
      h('span', nameAttrs(p.command), p.command + ' (' + p.pid + ')'),
      h('span', {}, cpu == null ? '-' : Math.round(cpu) + '%'),
      h('span', {}, mem == null ? '-' : formatBytes(mem)),
      h('span', {}, dr == null ? '-' : formatRate(dr)),
      h('span', {}, dw == null ? '-' : formatRate(dw)),
      // Another user's process never gets a Stop button, matching the Apps detail page's own
      // "root" tag -- `restricted` covers root and every other user, not root alone.
      h('span', {}, p.restricted ? h('span', { class: 'tag', title: 'Monitor Lizard never signals another user’s process' }, 'restricted')
        : h('button', { class: 'btn sm', onClick: () => askStop(p.command + ' ' + p.pid, [p]) }, 'Stop')));
  };
  const status = rows == null ? (S.procFailed ? 'failed' : 'loading') : 'ok';
  const body = status === 'ok'
    ? (rows.length ? h('section', { class: 'card tbl' }, header, rows.map(row)) : h('div', { class: 'card item' }, h('p', {}, 'No processes to show.')))
    : h('p', { class: 'mut small' }, PROC_STATUS_TEXT[status]);
  return [
    h('div', { class: 'seg', role: 'group', 'aria-label': 'Sort by', style: 'margin-bottom:14px' },
      PROC_SORTS.map(([id, l]) => h('button', { class: S.procSort === id ? 'chip on' : 'chip', onClick: () => { rec({ procSort: id }); loadProcesses(); } }, l))),
    body,
    h('p', { class: 'mut small', style: 'margin-top:10px' }, 'Your own processes only; another user’s are restricted. Stop re-checks this is still the same process, from a fresh sample, before signalling it.'),
  ];
}
// Trailing underscore: this view function would otherwise shadow the global window.performance.
function performanceView_() {
  chartFrame();
  const win = perfWindowed(), last = S.metrics[S.metrics.length - 1];
  // No new sample in a while: distinct from Unavailable (a sample came back and said so) -- this is
  // the opposite, no fresh sample to report on at all, so the tiles shown are old news.
  const stale = !!last && (Date.now() / 1000 - last.ts) > 8;
  const winLabel = S.perfWin === '1h' ? 'hour' : '5 min';
  const tabs = [['overview', 'Overview'], ['cpugpu', 'CPU and GPU'], ['disknet', 'Disk and network'], ['memory', 'Memory'], ['topconsumers', 'Top consumers']];
  return pageShell({ title: verdictTitle('performance'), filters: [
      h('div', { class: 'seg', role: 'group', 'aria-label': 'Performance tab' }, tabs.map(([id, l]) => h('button', { class: S.perfTab === id ? 'chip on' : 'chip', style: 'height:34px', onClick: () => { rec({ perfTab: id }); if (id === 'disknet') loadVolume(); if (id === 'topconsumers') loadProcesses(); } }, l))),
      h('div', { class: 'seg', role: 'group', 'aria-label': 'Time window' }, [['5m', 'Last 5 min'], ['1h', 'Last hour']].map(([v, l]) => h('button', { class: S.perfWin === v ? 'chip on' : 'chip', style: 'height:34px', onClick: () => rec({ perfWin: v }) }, l))),
      ],
    actions: [h('span', { class: 'small mut' }, last ? (S.perfLive ? 'live, every 2 s' : 'paused') : 'collecting the first sample...'),
      h('button', { class: 'btn sm', onClick: () => rec({ perfLive: !S.perfLive }) }, S.perfLive ? 'Pause' : 'Resume')],
    body: S.perfTab === 'cpugpu' ? h('div', {}, ...perfCpuGpuTab_(win, last, winLabel)) : S.perfTab === 'disknet' ? h('div', {}, ...perfDiskNetTab_(win, last)) : S.perfTab === 'memory' ? h('div', {}, ...perfMemoryTab_(win, last)) : S.perfTab === 'topconsumers' ? h('div', {}, ...perfTopConsumersTab_()) : perfOverviewTab_(win, last, stale, winLabel) });
}

