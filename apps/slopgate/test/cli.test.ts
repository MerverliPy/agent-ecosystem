// CLI integration tests: run the compiled-away TypeScript CLI directly via node.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { readFileSync, existsSync, rmSync } from 'node:fs';

const ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), '..');
const CLI = path.join(ROOT, 'src', 'cli.ts');
const FIXTURES = path.join(ROOT, 'fixtures');

function slop(args: string[], opts: { env?: Record<string, string> } = {}): { code: number; stdout: string; stderr: string } {
  const res = spawnSync(process.execPath, ['--experimental-strip-types', CLI, ...args], {
    cwd: ROOT,
    encoding: 'utf8',
    env: { ...process.env, ...opts.env },
  });
  return { code: res.status ?? -1, stdout: res.stdout, stderr: res.stderr };
}

test('slop version prints 0.1.0', () => {
  const r = slop(['version']);
  assert.equal(r.code, 0);
  assert.match(r.stdout.trim(), /^0\.1\.0/);
});

test('slop rules lists 30+ rules', () => {
  const r = slop(['rules']);
  assert.equal(r.code, 0);
  const ids = r.stdout.match(/[A-Z]+-\d{3}/g) ?? [];
  assert.ok(ids.length >= 30, `expected >=30 rule ids, got ${ids.length}`);
});

test('slop scan on clean fixture exits 0 with no findings', () => {
  const r = slop(['scan', path.join(FIXTURES, 'clean')]);
  assert.equal(r.code, 0);
  assert.match(r.stdout, /No slop findings/);
});

test('slop scan --json on heavy fixture returns parseable JSON with findings', () => {
  const r = slop(['scan', path.join(FIXTURES, 'heavy'), '--json']);
  assert.equal(r.code, 0);
  const data = JSON.parse(r.stdout);
  assert.ok(Array.isArray(data.findings));
  assert.ok(data.findings.length > 20);
  assert.ok(data.filesScanned > 0);
});

test('slop scan writes SARIF artifact', () => {
  const out = path.join(ROOT, 'test', 'tmp-slopgate.sarif');
  try {
    const r = slop(['scan', path.join(FIXTURES, 'heavy'), '--sarif', out]);
    assert.equal(r.code, 0);
    assert.ok(existsSync(out));
    const sarif = JSON.parse(readFileSync(out, 'utf8'));
    assert.equal(sarif.version, '2.1.0');
    assert.equal(sarif.runs[0].tool.driver.name, 'slopgate');
    assert.ok(sarif.runs[0].results.length > 0);
  } finally {
    rmSync(out, { force: true });
  }
});

test('slop score --json returns bounded score with breakdown', () => {
  const r = slop(['score', path.join(FIXTURES, 'heavy'), '--json']);
  assert.equal(r.code, 0);
  const data = JSON.parse(r.stdout);
  assert.ok(data.score >= 0 && data.score <= 100);
  assert.ok(data.byRule && data.byFile);
});

test('slop lint: heavy blocks at threshold 50 (exit 1), clean passes (exit 0)', () => {
  const heavy = slop(['lint', path.join(FIXTURES, 'heavy'), '--threshold', '50']);
  assert.equal(heavy.code, 1, 'heavy should fail CI gate');
  const clean = slop(['lint', path.join(FIXTURES, 'clean'), '--threshold', '50']);
  assert.equal(clean.code, 0, 'clean should pass CI gate');
});

test('slop lint --no-block exits 0 even above threshold', () => {
  const r = slop(['lint', path.join(FIXTURES, 'heavy'), '--threshold', '50', '--no-block']);
  assert.equal(r.code, 0);
  assert.match(r.stdout, /Gate FAIL/);
});

test('slop lint --commit-msg folds commit-text findings into the gate', () => {
  const r = slop(['lint', path.join(FIXTURES, 'clean'), '--threshold', '0', '--commit-msg', 'fix typo']);
  assert.equal(r.code, 1);
});

test('slop check-text flags slop text (exit 1) and passes clean text (exit 0)', () => {
  const sloppy = slop(['check-text', '--text', 'As an AI language model, I cannot help.']);
  assert.equal(sloppy.code, 1);
  const clean = slop(['check-text', '--text', 'Add retry with backoff to the fetch wrapper']);
  assert.equal(clean.code, 0);
});

test('slop llm-review without a key degrades to the deterministic catalog', () => {
  const r = slop(['llm-review', '--text', 'wip', '--json'], { env: { SLOPGATE_LLM_KEY: '', OPENAI_API_KEY: '' } });
  assert.equal(r.code, 0);
  const data = JSON.parse(r.stdout);
  assert.equal(data.enabled, false);
  assert.ok(data.catalog.some((f: { ruleId: string }) => f.ruleId === 'COMMIT-004'));
});

test('unknown command exits 2', () => {
  const r = slop(['frobnicate']);
  assert.equal(r.code, 2);
});

test('slop help exits 0', () => {
  const r = slop(['help']);
  assert.equal(r.code, 0);
  assert.match(r.stdout, /Usage:/);
});
