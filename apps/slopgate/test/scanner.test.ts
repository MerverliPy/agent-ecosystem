// Scanner tests: file collection, skipping, prose handling, cross-file pass.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { scanTree, collectFiles, runTextRules, runFileRules } from '../src/scanner.ts';
import { allFileRules, allTextRules } from '../src/rules/index.ts';

const FIXTURES = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', 'fixtures');

test('collectFiles skips node_modules, .git, dist and lockfiles', () => {
  const root = path.join(FIXTURES, 'clean');
  const ctx = collectFiles(root);
  const names = ctx.files.map((f) => path.basename(f.path));
  assert.ok(names.includes('config.ts'));
  assert.ok(!names.some((n) => n.includes('node_modules')));
  assert.ok(!names.some((n) => n.includes('.git')));
  assert.ok(!names.some((n) => n.includes('package-lock.json')));
});

test('collectFiles picks up prose files', () => {
  const ctx = collectFiles(path.join(FIXTURES, 'heavy'));
  assert.ok(ctx.files.some((f) => f.path.endsWith('README.md')));
});

test('scanTree finds no findings in the clean fixture', () => {
  const result = scanTree(path.join(FIXTURES, 'clean'));
  assert.equal(result.findings.length, 0);
});

test('scanTree flags AI phrasing in heavy README', () => {
  const result = scanTree(path.join(FIXTURES, 'heavy'));
  const ai = result.findings.filter((f) => f.category === 'ai-phrasing');
  assert.ok(ai.length >= 5, `expected >=5 ai-phrasing findings, got ${ai.length}`);
  assert.ok(ai.some((f) => f.ruleId === 'AI-001'));
});

test('scanTree respects --no-prose (prose:false)', () => {
  const withProse = scanTree(path.join(FIXTURES, 'heavy'));
  const withoutProse = scanTree(path.join(FIXTURES, 'heavy'), { prose: false });
  assert.ok(withProse.findings.length > withoutProse.findings.length);
});

test('runFileRules applies only applicable rules to a file', () => {
  const rules = allFileRules();
  const content = 'export function f(x: number): number { return x; }';
  const findings = runFileRules({ path: 'a.ts', content }, rules, 0);
  // a clean one-liner: no findings
  assert.equal(findings.length, 0);
});

test('runTextRules applies text rules to free text', () => {
  const findings = runTextRules('wip', 'commit-msg', allTextRules());
  assert.ok(findings.some((f) => f.ruleId === 'COMMIT-004'));
});

test('rule errors never crash a scan', () => {
  // A pathological file (deeply nested braces) must not throw.
  const root = path.join(FIXTURES, 'heavy');
  const result = scanTree(root);
  assert.ok(Array.isArray(result.findings));
});
