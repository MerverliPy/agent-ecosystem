// Scoring tests: fixture ordering, threshold gating, breakdown shape.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { scanTree } from '../src/scanner.ts';
import { scoreFindings, exceedsThreshold, SEVERITY_WEIGHT } from '../src/score.ts';

const FIXTURES = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', 'fixtures');

function scoreFixture(name: string) {
  const result = scanTree(path.join(FIXTURES, name));
  return scoreFindings(result.findings);
}

test('score ordering: clean < mild < heavy', () => {
  const clean = scoreFixture('clean');
  const mild = scoreFixture('mild');
  const heavy = scoreFixture('heavy');
  assert.ok(clean.score <= 10, `clean score ${clean.score} should be <= 10`);
  assert.ok(mild.score > clean.score, `mild (${mild.score}) > clean (${clean.score})`);
  assert.ok(heavy.score > mild.score, `heavy (${heavy.score}) > mild (${mild.score})`);
  assert.ok(heavy.score >= 60, `heavy score ${heavy.score} should be >= 60`);
});

test('score is bounded 0..100', () => {
  for (const name of ['clean', 'mild', 'heavy']) {
    const s = scoreFixture(name);
    assert.ok(s.score >= 0 && s.score <= 100, `${name} score ${s.score} out of bounds`);
  }
});

test('score breakdown counts severities and rules', () => {
  const heavy = scoreFixture('heavy');
  assert.equal(heavy.totalFindings, heavy.high + heavy.medium + heavy.low);
  assert.ok(heavy.high >= 3, `expected >=3 high-severity findings, got ${heavy.high}`);
  assert.ok(Object.keys(heavy.byRule).length >= 10, 'expected >=10 distinct rules to fire');
  assert.ok(heavy.byFile.length > 0, 'expected per-file breakdown');
});

test('severity weights are deterministic', () => {
  assert.deepEqual(SEVERITY_WEIGHT, { high: 10, medium: 5, low: 2 });
});

test('exceedsThreshold gates on score', () => {
  assert.equal(exceedsThreshold(60, 50), true);
  assert.equal(exceedsThreshold(50, 50), false);
  assert.equal(exceedsThreshold(49, 50), false);
  assert.equal(exceedsThreshold(0, 0), false);
});

test('threshold gating: heavy fails at 50, clean passes', () => {
  const heavy = scoreFixture('heavy');
  const clean = scoreFixture('clean');
  assert.equal(exceedsThreshold(heavy.score, 50), true);
  assert.equal(exceedsThreshold(clean.score, 50), false);
});
