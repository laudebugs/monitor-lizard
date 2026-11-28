// Pure logic for the Performance charts' hover and keyboard reading. The test lifts the `chart-hover` block out of
// the dashboard scripts (web/*.js), so the block must stay free of DOM and storage access.
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { html } from './page.mjs';
const m = html.match(/\/\* chart-hover:begin \*\/([\s\S]*?)\/\* chart-hover:end \*\//);
assert.ok(m, 'chart-hover block not found in the dashboard scripts (web/*.js)');
const { chartIndexAt, chartStep, chartIndexForTs, chartTipLeft, chartAgo, chartSample } = new Function(m[1] + '\nreturn { chartIndexAt, chartStep, chartIndexForTs, chartTipLeft, chartAgo, chartSample };')();

test('a chart with no samples has nothing to point at', () => {
  assert.equal(chartIndexAt(0, 0.5), -1);
  assert.equal(chartStep(0, null, 'ArrowLeft'), -1);
  assert.equal(chartIndexForTs([], 100), -1);
  assert.equal(chartSample({ ts: [], series: [], fmt: String }, 0, 100), null);
});

test('a pointer between two samples snaps to the nearer one and never invents a value', () => {
  // 5 samples sit at 0, .25, .5, .75, 1
  assert.equal(chartIndexAt(5, 0.30), 1);
  assert.equal(chartIndexAt(5, 0.45), 2);
  assert.equal(chartIndexAt(5, 0.55), 2);
  assert.equal(chartIndexAt(5, 0.60), 2);
  assert.equal(chartIndexAt(5, 0.70), 3);
});

test('a tie takes the newer sample, and the ends and a one-sample chart hold', () => {
  assert.equal(chartIndexAt(3, 0.25), 1);
  assert.equal(chartIndexAt(5, -0.4), 0);
  assert.equal(chartIndexAt(5, 7), 4);
  assert.equal(chartIndexAt(1, 0.9), 0);
  assert.equal(chartIndexAt(5, NaN), -1);
  assert.equal(chartIndexAt(5, null), -1);
});

test('arrow keys step one sample and stop at the ends; Home and End jump', () => {
  assert.equal(chartStep(5, 2, 'ArrowLeft'), 1);
  assert.equal(chartStep(5, 2, 'ArrowRight'), 3);
  assert.equal(chartStep(5, 0, 'ArrowLeft'), 0);
  assert.equal(chartStep(5, 4, 'ArrowRight'), 4);
  assert.equal(chartStep(5, 2, 'Home'), 0);
  assert.equal(chartStep(5, 2, 'End'), 4);
  assert.equal(chartStep(5, 2, 'a'), 2);
});

test('the first key press with nothing highlighted starts from the newest sample', () => {
  assert.equal(chartStep(5, -1, 'ArrowLeft'), 3);
  assert.equal(chartStep(5, null, 'ArrowRight'), 4);
  assert.equal(chartStep(5, 9, 'ArrowLeft'), 3);
});

test('a keyboard highlight kept as a time finds the nearest sample after the data shifts', () => {
  assert.equal(chartIndexForTs([10, 12, 14, 16], 13.1), 2);
  assert.equal(chartIndexForTs([12, 14, 16, 18], 10), 0);
  assert.equal(chartIndexForTs([12, 14, 16, 18], 99), 3);
  assert.equal(chartIndexForTs([12, 14], undefined), -1);
});

test('relative time reads 32s ago, then minutes, then hours; long form spells it out', () => {
  assert.equal(chartAgo(0.2, false), 'now');
  assert.equal(chartAgo(32, false), '32s ago');
  assert.equal(chartAgo(252, false), '4m 12s ago');
  assert.equal(chartAgo(240, false), '4m ago');
  assert.equal(chartAgo(3720, false), '1h 2m ago');
  assert.equal(chartAgo(-5, false), 'now');
  assert.equal(chartAgo(61, true), '1 minute 1 second ago');
});

const spec = (over) => ({ ts: [100, 102, 104], max: 100, fmt: (v) => v + '%', series: [{ label: 'CPU', color: 'var(--acc)', values: [10, null, 50], reason: (i) => 'reason ' + i }], ...over });

test('a sample reads its value, label, relative time and a dot position', () => {
  const s = chartSample(spec(), 2, 136);
  assert.equal(s.ago, '32s ago');
  assert.equal(s.frac, 1);
  assert.deepEqual(s.rows.map((r) => [r.label, r.text, r.y, r.unavailable]), [['CPU', '50%', 0.5, false]]);
  assert.match(s.say, /^32 seconds ago: CPU 50%$/);
});

test('an unavailable value shows its reason, never a blank or a zero, and gets no dot', () => {
  const s = chartSample(spec(), 1, 110);
  assert.equal(s.rows[0].text, 'unavailable (reason 1)');
  assert.equal(s.rows[0].y, null);
  assert.equal(s.rows[0].unavailable, true);
  assert.equal(chartSample(spec({ series: [{ label: 'CPU', color: 'x', values: [null, null, null] }] }), 0, 110).rows[0].text, 'unavailable (no reading)');
});

test('a stacked sample is dotted at the running total, and not at all when a layer is missing', () => {
  const series = [{ label: 'A', color: 'a', values: [10, 10, null], plot: [10, 10, null] }, { label: 'B', color: 'b', values: [20, null, 5], plot: [30, null, null] }];
  const s = chartSample(spec({ series, stacked: true }), 0, 110);
  assert.deepEqual(s.rows.map((r) => r.y), [0.9, 0.7]);
  assert.deepEqual(chartSample(spec({ series, stacked: true }), 1, 110).rows.map((r) => r.y), [null, null]);
});

test('a value above the chart max sits on the top edge', () => {
  assert.equal(chartSample(spec({ series: [{ label: 'x', color: 'x', values: [500, 1, 1] }] }), 0, 110).rows[0].y, 0);
});

test('an index outside the chart has no sample', () => {
  assert.equal(chartSample(spec(), 3, 110), null);
  assert.equal(chartSample(spec(), -1, 110), null);
});

test('samples that share a time: the one nearest the last highlighted index wins, so Right is never trapped', () => {
  const ts = [1, 2, 2, 3];
  assert.equal(chartIndexForTs(ts, 2, 2), 2);
  assert.equal(chartIndexForTs(ts, 2, 1), 1);
  assert.equal(chartIndexForTs(ts, 2), 1);
  assert.equal(chartStep(4, chartIndexForTs(ts, 2, 2), 'ArrowRight'), 3);
});

test('the tooltip goes right of the guide, else left of it, and never past an edge', () => {
  assert.equal(chartTipLeft(100, 1000, 200, 10), 110);
  assert.equal(chartTipLeft(900, 1000, 200, 10), 690);
  assert.equal(chartTipLeft(1000, 1000, 200, 10), 790);
  assert.equal(chartTipLeft(0, 1000, 200, 10), 10);
  assert.equal(chartTipLeft(150, 300, 200, 10), 0, 'fits on neither side: clamped to the left edge');
  assert.equal(chartTipLeft(250, 300, 200, 10), 40, 'left of the guide');
  assert.equal(chartTipLeft(120, 300, 290, 10), 0, 'wide but narrower than the chart: left edge');
});

test('a tooltip as wide as the chart, a chart of no width or a missing size sits at 0', () => {
  assert.equal(chartTipLeft(50, 200, 200, 10), 0);
  assert.equal(chartTipLeft(50, 200, 500, 10), 0);
  assert.equal(chartTipLeft(50, 0, 100, 10), 0);
  assert.equal(chartTipLeft(50, 200, NaN, 10), 0);
});
