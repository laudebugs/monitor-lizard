// Logic tests for the Apps drawer's agent suggestion: when it shows, how an unsure verdict reads, and when the page asks
// the server. They lift the `agent-suggestion` block out of the dashboard scripts (web/*.js), so the block must stay free
// of DOM, storage and fetch access.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { html } from './page.mjs';

const block = (name) => {
  const m = html.match(new RegExp('/\\* ' + name + ':begin \\*/([\\s\\S]*?)/\\* ' + name + ':end \\*/'));
  assert.ok(m, name + ' block not found in the dashboard scripts (web/*.js)');
  return m[1];
};
const A = new Function(block('agent-suggestion') + '\nreturn { agentSuggestion, agentShouldAsk, AGENT_ASKED_MS, AGENT_RETRY_MS };')();

const reply = (over = {}, verdict = {}) => ({
  rule_label: 'unknown', allowed_actions: ['approve', 'investigate', 'quit', 'force_quit'], agent_reason: null, agent_mode: 'on',
  verdict: { label: 'unknown', confidence: 'high', actions: ['investigate', 'quit'], rationale: 'Unsigned and listening on a new port.', ...verdict }, ...over,
});

test('suggestion: shown only when the agent is on, the model answered, and a rationale came back', () => {
  assert.ok(A.agentSuggestion(reply(), 'on'));
  for (const mode of ['off', 'shadow', undefined]) assert.equal(A.agentSuggestion(reply({ agent_mode: mode }), 'on'), null, 'reply mode ' + mode);
  assert.equal(A.agentSuggestion(reply({ agent_reason: 'not_ready' }), 'on'), null, 'model server unavailable: no block, no banner');
  assert.equal(A.agentSuggestion(reply({}, { rationale: '' }), 'on'), null, 'the rules-only fallback has an empty rationale');
  assert.equal(A.agentSuggestion(null, 'on'), null);
  assert.equal(A.agentSuggestion({ agent_mode: 'on' }, 'on'), null);
});

test('suggestion: a reply cached while the agent was on is hidden once the mode is off or shadow, or not yet known', () => {
  for (const now of ['off', 'shadow', undefined]) assert.equal(A.agentSuggestion(reply(), now), null, 'current mode ' + now);
});

test('suggestion: low confidence or a label that differs from the rules reads "Unsure, investigate" and offers no quit', () => {
  for (const verdict of [{ confidence: 'low' }, { label: 'suspicious' }, { label: 'trusted' }]) {
    const s = A.agentSuggestion(reply({}, { actions: ['quit', 'force_quit', 'approve'], ...verdict }), 'on');
    assert.equal(s.unsure, true, JSON.stringify(verdict));
    assert.equal(s.headline, 'Unsure, investigate');
    assert.deepEqual(s.actions.map((a) => a.id), ['investigate'], JSON.stringify(verdict));
  }
});

test('suggestion: a confident verdict that agrees with the rules offers its allowed actions, quit and force quit as one button', () => {
  const s = A.agentSuggestion(reply({}, { actions: ['approve', 'quit', 'force_quit', 'ignore', 'block_network', 'approve'] }), 'on');
  assert.equal(s.unsure, false);
  assert.deepEqual(s.actions.map((a) => [a.id, a.kind]), [['approve', 'approve'], ['quit', 'stop'], ['ignore', 'note']], 'no block_network, no duplicates');
  assert.match(s.headline, /unknown/);
});

test('suggestion: the rationale is kept as plain text, never markup', () => {
  const s = A.agentSuggestion(reply({}, { rationale: '<img src=x onerror=alert(1)>' }), 'on');
  assert.equal(s.rationale, '<img src=x onerror=alert(1)>');
});

test('asking: only when the agent is on or shadow, and not before the time set for that app', () => {
  const now = 1_000_000_000;
  assert.equal(A.agentShouldAsk('off', 'k', {}, now), false);
  assert.equal(A.agentShouldAsk(undefined, 'k', {}, now), false);
  for (const mode of ['on', 'shadow']) assert.equal(A.agentShouldAsk(mode, 'k', {}, now), true, mode);
  assert.equal(A.agentShouldAsk('shadow', 'k', { k: now + A.AGENT_ASKED_MS }, now), false, 'a 2 s refresh must not re-ask');
  assert.equal(A.agentShouldAsk('shadow', 'k', { k: now + A.AGENT_RETRY_MS }, now + 1000), false, 'a failed ask waits the retry time');
  assert.equal(A.agentShouldAsk('shadow', 'k', { k: now + A.AGENT_RETRY_MS }, now + A.AGENT_RETRY_MS), true, 'then asks again');
  assert.equal(A.agentShouldAsk('shadow', 'k', { other: now + A.AGENT_ASKED_MS }, now), true, 'another app is asked about separately');
});
