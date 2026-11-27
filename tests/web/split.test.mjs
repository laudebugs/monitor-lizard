// The dashboard is one page split into web/app.css and classic scripts that share globals. These tests
// guard the split: nothing changed in the move, the load order holds, and the token stays in one script.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import vm from 'node:vm';
import { shell, css, script, scriptFiles, scriptTexts, webFile } from './page.mjs';

const fixture = (name) => readFileSync(new URL('../fixtures/web-before/' + name, import.meta.url), 'utf8');

test('split: the stylesheet and the scripts, joined in load order, equal the page they were moved from byte for byte', () => {
  assert.equal(css === fixture('old.css'), true, 'web/app.css differs from the old <style> block');
  assert.equal(script === fixture('old-script.js'), true, 'the scripts joined in load order differ from the old <script> block');
});

test('split: every script is under the 800-line budget and web/ holds no file the shell does not load', () => {
  for (const [i, f] of scriptFiles.entries()) assert.ok(scriptTexts[i].split('\n').length - 1 <= 800, f + ' is over 800 lines');
  assert.ok(css.split('\n').length <= 800);
  const onDisk = readdirSync(new URL('../../web/', import.meta.url)).filter((n) => n.endsWith('.js')).sort();
  assert.deepEqual(onDisk, [...scriptFiles].sort(), 'a script in web/ is not loaded by index.html, or the reverse');
});

test('split: core.js loads first, and the shell is plain markup with no inline script but the theme bootstrap', () => {
  assert.equal(scriptFiles[0], 'core.js');
  const inline = [...shell.matchAll(/<script(?![^>]*\bsrc=)[^>]*>([\s\S]*?)<\/script>/g)];
  assert.equal(inline.length, 1, 'only the theme bootstrap is inline');
  assert.match(inline[0][1], /monitor-lizard-theme/);
  assert.ok(!/<style/.test(shell), 'styles live in app.css');
  assert.ok(!/https?:\/\/(?!www\.w3\.org)/.test(shell), 'no remote asset in the shell');
});

// A stand-in browser: any property, call or construction answers with another stand-in, so a script's own
// top-level code runs and only a name that no earlier script defined throws a ReferenceError.
function standIn() {
  const target = function () {};
  const p = new Proxy(target, {
    get: (_, k) => (k === Symbol.toPrimitive ? () => '' : k === 'then' ? undefined : k === Symbol.iterator ? function* () {} : k === 'length' ? 0 : p),
    apply: () => p, construct: () => p, set: () => true, has: () => true,
  });
  return p;
}
function runInOrder(files) {
  const names = ['document', 'window', 'matchMedia', 'localStorage', 'location', 'history', 'navigator', 'fetch', 'setTimeout', 'setInterval', 'clearTimeout', 'requestAnimationFrame', 'console', 'URL', 'Blob', 'Node', 'Intl'];
  const ctx = vm.createContext({ ...Object.fromEntries(names.map((n) => [n, standIn()])), localStorage: { getItem: () => null, setItem() {}, removeItem() {} } });
  for (const f of files) vm.runInContext(webFile(f), ctx, { filename: f });
  return ctx;
}

test('load order: every script runs, in the order the shell lists them, with no name used before its script loaded', () => {
  assert.doesNotThrow(() => runInOrder(scriptFiles));
});

test('load order: the test notices a wrong order (core.js last fails on its globals)', () => {
  const wrong = [...scriptFiles.slice(1), scriptFiles[0]];
  assert.throws(() => runInOrder(wrong), (e) => e.name === 'ReferenceError');
});

test('token: only core.js declares it, the shell never holds it, and the only use is the request header', () => {
  assert.ok(!/__TOKEN__|TOKEN/.test(shell), 'the shell must not mention the token');
  const holders = scriptFiles.filter((f, i) => scriptTexts[i].includes('__TOKEN__'));
  assert.deepEqual(holders, ['core.js']);
  assert.equal([...script.matchAll(/const TOKEN = "__TOKEN__";/g)].length, 1);
  assert.ok(!/__TOKEN__/.test(css));
  // Every other mention is the header on a request: never a URL, the hash, storage or markup.
  const uses = script.split('\n').filter((l) => /\bTOKEN\b/.test(l) && !l.startsWith('const TOKEN = '));
  assert.ok(uses.length >= 1);
  for (const l of uses) assert.ok(l.includes("headers: { 'X-MonitorLizard-Token': TOKEN }"), 'TOKEN used outside a request header: ' + l.trim());
});
