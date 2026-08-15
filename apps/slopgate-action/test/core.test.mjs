// SlopGate action core tests: inputs, gate, comment/summary builders, and a real
// integration run of the slop CLI through the action's own spawn path.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  parseInputs,
  decideGate,
  buildCommentBody,
  buildSummary,
  pullRequestNumber,
  writeStepSummary,
  runSlop,
  cliPath,
} from '../lib/core.mjs';

const ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), '..');
const FIXTURES = path.join(ROOT, '..', 'slopgate', 'fixtures');

test('parseInputs applies defaults', () => {
  const inputs = parseInputs({});
  assert.equal(inputs.path, '.');
  assert.equal(inputs.threshold, 50);
  assert.equal(inputs.block, true);
  assert.equal(inputs.sarif, true);
  assert.equal(inputs.comment, true);
  assert.equal(inputs.token, '');
});

test('parseInputs reads INPUT_* overrides', () => {
  const inputs = parseInputs({
    INPUT_PATH: 'src',
    INPUT_THRESHOLD: '80',
    INPUT_BLOCK: 'false',
    INPUT_SARIF: 'false',
    INPUT_COMMENT: 'false',
    INPUT_TOKEN: 'tok',
    GITHUB_REPOSITORY: 'acme/repo',
    GITHUB_WORKSPACE: '/ws',
  });
  assert.equal(inputs.path, 'src');
  assert.equal(inputs.threshold, 80);
  assert.equal(inputs.block, false);
  assert.equal(inputs.sarif, false);
  assert.equal(inputs.comment, false);
  assert.equal(inputs.token, 'tok');
  assert.equal(inputs.repository, 'acme/repo');
  assert.equal(inputs.workspace, '/ws');
});

test('decideGate: pass below/at threshold, warn above with block off, fail above with block on', () => {
  assert.equal(decideGate(40, 50, true).status, 'pass');
  assert.equal(decideGate(50, 50, true).status, 'pass');
  assert.equal(decideGate(60, 50, true).status, 'fail');
  assert.equal(decideGate(60, 50, false).status, 'warn');
});

test('buildCommentBody contains score, breakdown and gate', () => {
  const breakdown = {
    totalFindings: 12,
    high: 2,
    medium: 4,
    low: 6,
    byRule: { 'AI-001': { count: 1, weight: 10 } },
  };
  const body = buildCommentBody(77, breakdown, { status: 'fail', threshold: 50 }, 'abc1234');
  assert.match(body, /77\/100/);
  assert.match(body, /FAIL/);
  assert.match(body, /AI-001/);
  assert.match(body, /abc1234/);
});

test('buildSummary is compact and includes the gate', () => {
  const s = buildSummary(10, { totalFindings: 3, high: 0, medium: 1, low: 2 }, { status: 'pass', threshold: 50 });
  assert.match(s, /10\/100/);
  assert.match(s, /PASS/);
});

test('pullRequestNumber reads the event payload', () => {
  const dir = mkdtempSync(path.join(tmpdir(), 'slopgate-'));
  try {
    const f = path.join(dir, 'event.json');
    writeFileSync(f, JSON.stringify({ pull_request: { number: 42 } }));
    assert.equal(pullRequestNumber('pull_request', f), 42);
    assert.equal(pullRequestNumber('push', f), null);
    assert.equal(pullRequestNumber('pull_request', '/nonexistent.json'), null);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('writeStepSummary appends to the summary file', () => {
  const dir = mkdtempSync(path.join(tmpdir(), 'slopgate-'));
  try {
    const f = path.join(dir, 'summary.md');
    assert.equal(writeStepSummary(f, '## SlopGate\n'), true);
    assert.match(readFileSync(f, 'utf8'), /SlopGate/);
    assert.equal(writeStepSummary('', 'x'), false);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('runSlop resolves the real CLI and scores fixtures (integration)', () => {
  assert.ok(cliPath().endsWith(path.join('slopgate', 'src', 'cli.ts')));
  const heavy = runSlop('score', path.join(FIXTURES, 'heavy'));
  const clean = runSlop('score', path.join(FIXTURES, 'clean'));
  assert.ok(heavy.score >= 60);
  assert.ok(clean.score <= 10);
});

test('end-to-end gate simulation: heavy fails, clean passes at threshold 50', () => {
  const heavyScore = runSlop('score', path.join(FIXTURES, 'heavy')).score;
  const cleanScore = runSlop('score', path.join(FIXTURES, 'clean')).score;
  assert.equal(decideGate(heavyScore, 50, true).status, 'fail');
  assert.equal(decideGate(cleanScore, 50, true).status, 'pass');
});
