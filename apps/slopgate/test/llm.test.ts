// LLM review layer tests (deterministic parts — no network, no keys required).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  catalogReview,
  llmConfig,
  mergeReviews,
  parseLlmJson,
  reviewProseWithLlm,
  patternCatalog,
  type LlmConfig,
} from '../src/llm.ts';

test('llmConfig returns null when no key is set', () => {
  const prev = { ...process.env };
  delete process.env.SLOPGATE_LLM_KEY;
  delete process.env.OPENAI_API_KEY;
  try {
    assert.equal(llmConfig(), null);
  } finally {
    process.env = prev;
  }
});

test('llmConfig reads key from SLOPGATE_LLM_KEY and defaults URL/model', () => {
  const prev = { ...process.env };
  process.env.SLOPGATE_LLM_KEY = 'test-key';
  delete process.env.SLOPGATE_LLM_URL;
  delete process.env.SLOPGATE_LLM_MODEL;
  try {
    const cfg = llmConfig();
    assert.ok(cfg);
    assert.equal(cfg?.key, 'test-key');
    assert.equal(cfg?.url, 'https://api.openai.com/v1/chat/completions');
    assert.equal(cfg?.model, 'gpt-4o-mini');
  } finally {
    process.env = prev;
  }
});

test('patternCatalog lists commit + AI rules', () => {
  const catalog = patternCatalog();
  assert.ok(catalog.length >= 20);
  assert.ok(catalog.some((r) => r.id === 'AI-001'));
  assert.ok(catalog.some((r) => r.id === 'COMMIT-001'));
});

test('catalogReview flags slop prose deterministically', () => {
  const findings = catalogReview([
    { source: 'commit-msg', text: 'wip' },
    { source: 'docs', text: 'As an AI language model, I cannot do that.' },
    { source: 'docs', text: 'This is a normal technical sentence about retry loops.' },
  ]);
  assert.ok(findings.some((f) => f.ruleId === 'COMMIT-004'));
  assert.ok(findings.some((f) => f.ruleId === 'AI-001'));
  assert.equal(findings.filter((f) => f.ruleId === 'AI-001').length, 1);
});

test('parseLlmJson handles array, wrapped, fenced and garbage input', () => {
  assert.equal(parseLlmJson('[{"source":"a","slopScore":80,"slopType":"x","notes":["n"]}]')[0].source, 'a');
  assert.equal(parseLlmJson('{"reviews":[{"source":"b","slopScore":10}]}')[0].source, 'b');
  assert.equal(parseLlmJson('```json\n[{"source":"c","slopScore":50}]\n```')[0].source, 'c');
  assert.equal(parseLlmJson('not json at all').length, 0);
});

test('parseLlmJson clamps slopScore to 0..100', () => {
  const parsed = parseLlmJson('[{"source":"a","slopScore":250}]');
  assert.equal(parsed[0].slopScore, 100);
});

test('mergeReviews: catalog-only when LLM disabled', () => {
  const catalog = catalogReview([{ source: 'docs', text: 'As an AI language model, I cannot help.' }]);
  const merged = mergeReviews(catalog, { enabled: false, reviews: [] });
  // catalog: AI-001 high(10) + AI-003 high(10) = 20
  assert.equal(merged.combinedScore, 20);
});

test('mergeReviews: blends catalog with LLM scores when enabled', () => {
  const catalog = catalogReview([{ source: 'docs', text: 'As an AI language model, I cannot help.' }]);
  const llm = { enabled: true, reviews: [{ source: 'docs', slopScore: 80, slopType: 'refusal', notes: [] }] };
  const merged = mergeReviews(catalog, llm);
  // 20 * 0.6 + 80 * 0.4 = 12 + 32 = 44
  assert.equal(merged.combinedScore, 44);
});

test('reviewProseWithLlm posts to the endpoint and parses the reply (mock fetch)', async () => {
  const cfg: LlmConfig = { key: 'k', url: 'https://example.test/chat', model: 'm' };
  const calls: Array<{ url: string; body: unknown }> = [];
  const mockFetch = async (url: string, init: unknown) => {
    calls.push({ url, body: JSON.parse((init as { body: string }).body) });
    return {
      ok: true,
      status: 200,
      json: async () => ({ choices: [{ message: { content: '[{"source":"docs","slopScore":90,"slopType":"ai","notes":["yes"]}]' } }] }),
    };
  };
  const res = await reviewProseWithLlm([{ source: 'docs', text: 'hello' }], cfg, mockFetch);
  assert.equal(res.enabled, true);
  assert.equal(res.reviews[0].slopScore, 90);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, 'https://example.test/chat');
  assert.equal((calls[0].body as { model: string }).model, 'm');
});

test('reviewProseWithLlm throws on HTTP error', async () => {
  const cfg: LlmConfig = { key: 'k', url: 'https://example.test/chat', model: 'm' };
  const mockFetch = async () => ({ ok: false, status: 500, json: async () => ({}) });
  await assert.rejects(() => reviewProseWithLlm([{ source: 'docs', text: 'x' }], cfg, mockFetch));
});
