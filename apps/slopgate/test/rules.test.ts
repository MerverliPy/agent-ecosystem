// Per-rule fixture tests: every deterministic rule gets at least one positive and
// (where meaningful) one negative fixture. Fixtures are inline strings — no parser,
// purely deterministic pattern matching.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import type { Finding, Rule, ScannedFile } from '../src/types.ts';
import { allFileRules, allCrossFileRules, allTextRules, ruleCount } from '../src/rules/index.ts';

function runRule(rule: Rule, file: string, content: string): Finding[] {
  return rule.run({ file, content, lines: content.split('\n') });
}

function byId(id: string, findings: Finding[]): Finding[] {
  return findings.filter((f) => f.ruleId === id);
}

function crossFile(files: Array<[string, string]>): Finding[] {
  return allCrossFileRules(files.map(([path, content]) => ({ path, content }) as ScannedFile));
}

function findRule(id: string): Rule {
  const r = allFileRules().find((x) => x.id === id);
  assert.ok(r, `rule ${id} registered`);
  return r as Rule;
}

test('rule pack has 30+ rules', () => {
  assert.ok(ruleCount() >= 30, `ruleCount() = ${ruleCount()}`);
});

// ---------------------------------------------------------------- DEAD

test('DEAD-001: unreferenced exported interface/type is flagged', () => {
  const f = crossFile([
    ['src/a.ts', 'export interface NeverUsed { field: string; }\nexport const used = 1;'],
    ['src/b.ts', 'export const other = 2;'],
  ]);
  assert.equal(byId('DEAD-001', f).length, 1);
});

test('DEAD-001: referenced interface is not flagged', () => {
  const f = crossFile([
    ['src/a.ts', 'export interface Used { field: string; }'],
    ['src/b.ts', 'import type { Used } from "./a.ts";\nconst x: Used = { field: "y" };'],
  ]);
  assert.equal(byId('DEAD-001', f).length, 0);
});

test('DEAD-002: empty interface is flagged', () => {
  const f = runRule(findRule('DEAD-002'), 'a.ts', 'interface Marker {}\nexport const x = 1;');
  assert.equal(byId('DEAD-002', f).length, 1);
});

test('DEAD-003: abstract class with no abstract members is flagged', () => {
  const f = runRule(findRule('DEAD-003'), 'a.ts', 'abstract class Base { run(): void {} }');
  assert.equal(byId('DEAD-003', f).length, 1);
});

test('DEAD-003: abstract class with abstract members is not flagged', () => {
  const f = runRule(findRule('DEAD-003'), 'a.ts', 'abstract class Base { abstract run(): void; }');
  assert.equal(byId('DEAD-003', f).length, 0);
});

test('DEAD-004: empty subclass is flagged', () => {
  const f = runRule(findRule('DEAD-004'), 'a.ts', 'class Child extends Parent {}\nexport const x = 1;');
  assert.equal(byId('DEAD-004', f).length, 1);
});

test('DEAD-005: unreferenced pass-through wrapper is flagged', () => {
  const f = crossFile([
    ['src/a.ts', 'export function wrapper(x: number) { return compute(x); }\nfunction compute(x: number): number { return x * 2; }'],
  ]);
  assert.equal(byId('DEAD-005', f).length, 1);
});

test('DEAD-006: unreferenced local class is flagged', () => {
  const f = crossFile([
    ['src/a.ts', 'class Internal {}\nexport function visible() { return 1; }'],
  ]);
  assert.equal(byId('DEAD-006', f).length, 1);
});

// ---------------------------------------------------------------- UNUSED

test('UNUSED-001: exported helper never referenced is flagged', () => {
  const f = crossFile([
    ['src/a.ts', 'export function orphan() { return 1; }'],
    ['src/b.ts', 'export const x = 1;\nconsole.log(x);'],
  ]);
  assert.equal(byId('UNUSED-001', f).length, 1);
});

test('UNUSED-002: local function never called is flagged', () => {
  const f = crossFile([
    ['src/a.ts', 'function helper() { return 1; }\nexport function main() { return 2; }'],
  ]);
  assert.equal(byId('UNUSED-002', f).length, 1);
});

test('UNUSED-003: unused import is flagged, used import is not', () => {
  const unused = runRule(findRule('UNUSED-003'), 'a.ts', 'import { unusedThing } from "./m.ts";\nexport const y = 1;');
  assert.equal(byId('UNUSED-003', unused).length, 1);
  const used = runRule(findRule('UNUSED-003'), 'a.ts', 'import { usedThing } from "./m.ts";\nconsole.log(usedThing);');
  assert.equal(byId('UNUSED-003', used).length, 0);
});

test('UNUSED-004: declared-but-never-read variable is flagged', () => {
  const f = runRule(findRule('UNUSED-004'), 'a.ts', 'const orphan = 5;\nexport const y = 1;\nconsole.log(y);');
  assert.equal(byId('UNUSED-004', f).length, 1);
});

test('UNUSED-005: duplicate module import is flagged', () => {
  const f = runRule(findRule('UNUSED-005'), 'a.ts', 'import { a } from "./m.ts";\nimport { b } from "./m.ts";\nconsole.log(a, b);');
  assert.equal(byId('UNUSED-005', f).length, 1);
});

// ---------------------------------------------------------------- COMM

test('COMM-001: restating comment is flagged', () => {
  const f = runRule(findRule('COMM-001'), 'a.ts', '// add item to cart\ncart.add(item);');
  assert.equal(byId('COMM-001', f).length, 1);
});

test('COMM-001: explanatory comment is not flagged', () => {
  const f = runRule(findRule('COMM-001'), 'a.ts', '// prices are stored in cents to avoid float drift\nconst total = sum(prices);');
  assert.equal(byId('COMM-001', f).length, 0);
});

test('COMM-002: boilerplate header is flagged', () => {
  const f = runRule(findRule('COMM-002'), 'a.ts', '// Copyright (c) 2021 ACME Corp. All rights reserved.\nexport const x = 1;');
  assert.equal(byId('COMM-002', f).length, 2);
});

test('COMM-003: bare TODO is flagged, detailed TODO is not', () => {
  const bare = runRule(findRule('COMM-003'), 'a.ts', '// TODO\nexport const x = 1;');
  assert.equal(byId('COMM-003', bare).length, 1);
  const detailed = runRule(findRule('COMM-003'), 'a.ts', '// TODO: add retry with exponential backoff to the fetch wrapper\nexport const x = 1;');
  assert.equal(byId('COMM-003', detailed).length, 0);
});

test('COMM-004: placeholder comment is flagged', () => {
  const f = runRule(findRule('COMM-004'), 'a.ts', '// John Doe implemented this\nexport const x = 1;');
  assert.equal(byId('COMM-004', f).length, 1);
});

test('COMM-005: vacuous comment is flagged', () => {
  const f = runRule(findRule('COMM-005'), 'a.ts', '// this is fine\nexport const x = 1;');
  assert.equal(byId('COMM-005', f).length, 1);
});

test('COMM-006: commented-out code block is flagged', () => {
  const f = runRule(findRule('COMM-006'), 'a.ts', '// const oldValue = compute();\n// const other = oldValue + 1;\nexport const x = 1;');
  assert.equal(byId('COMM-006', f).length, 1);
});

// ---------------------------------------------------------------- NAME

test('NAME-001: generic file name is flagged', () => {
  const f = runRule(findRule('NAME-001'), 'src/utils.ts', 'export const x = 1;');
  assert.equal(byId('NAME-001', f).length, 1);
});

test('NAME-001: specific file name is not flagged', () => {
  const f = runRule(findRule('NAME-001'), 'src/pricing.ts', 'export const x = 1;');
  assert.equal(byId('NAME-001', f).length, 0);
});

test('NAME-002: generic identifier is flagged', () => {
  const f = runRule(findRule('NAME-002'), 'a.ts', 'const data = "x";\nexport const y = data;');
  assert.equal(byId('NAME-002', f).length, 1);
});

test('NAME-003: redundant type suffix is flagged', () => {
  const f = runRule(findRule('NAME-003'), 'a.ts', 'const userArray = [1, 2];\nexport const y = userArray.length;');
  assert.equal(byId('NAME-003', f).length, 1);
});

test('NAME-004: cryptic single-letter variable is flagged', () => {
  const f = runRule(findRule('NAME-004'), 'a.ts', 'const t = "x";\nexport const y = t;');
  assert.equal(byId('NAME-004', f).length, 1);
});

test('NAME-004: loop counters are not flagged', () => {
  const f = runRule(findRule('NAME-004'), 'a.ts', 'for (let i = 0; i < 10; i++) { console.log(i); }');
  assert.equal(byId('NAME-004', f).length, 0);
});

test('NAME-005: duplicated word in name is flagged', () => {
  const f = runRule(findRule('NAME-005'), 'a.ts', 'const data_data = 1;\nexport const y = data_data;');
  assert.equal(byId('NAME-005', f).length, 1);
});

// ---------------------------------------------------------------- OVER

test('OVER-001: async without await is flagged', () => {
  const f = runRule(findRule('OVER-001'), 'a.ts', 'async function noAwait() { return 1; }\nexport const y = noAwait();');
  assert.equal(byId('OVER-001', f).length, 1);
});

test('OVER-001: async with await is not flagged', () => {
  const f = runRule(findRule('OVER-001'), 'a.ts', 'async function withAwait() { return await load(); }\nexport const y = withAwait();');
  assert.equal(byId('OVER-001', f).length, 0);
});

test('OVER-002: promise anti-pattern is flagged', () => {
  const f = runRule(findRule('OVER-002'), 'a.ts', 'export const p = new Promise((resolve) => resolve(42));');
  assert.equal(byId('OVER-002', f).length, 1);
});

test('OVER-003: empty catch is flagged', () => {
  const f = runRule(findRule('OVER-003'), 'a.ts', 'try { work(); } catch (e) {}');
  assert.equal(byId('OVER-003', f).length, 1);
});

test('OVER-004: excessive any is flagged', () => {
  const f = runRule(findRule('OVER-004'), 'a.ts', 'const a: any = 1;\nconst b: any = 2;\nconst c: any = 3;\nexport const y = a + b + c;');
  assert.equal(byId('OVER-004', f).length, 1);
});

test('OVER-005: stateless singleton is flagged', () => {
  const f = runRule(findRule('OVER-005'), 'a.ts', 'class S { private static i: S; private constructor() {} static getInstance(): S { if (!S.i) S.i = new S(); return S.i; } work(): void {} }');
  assert.equal(byId('OVER-005', f).length, 1);
});

test('OVER-006: parameterless factory is flagged', () => {
  const f = runRule(findRule('OVER-006'), 'a.ts', 'function makeThing() { return new Thing(); }\nexport const y = makeThing();');
  assert.equal(byId('OVER-006', f).length, 1);
});

test('OVER-007: empty branch is flagged', () => {
  const f = runRule(findRule('OVER-007'), 'a.ts', 'if (x) {\n  // placeholder\n} else {\n  run();\n}');
  assert.equal(byId('OVER-007', f).length, 1);
});

test('OVER-008: deep arrow nesting on one line is flagged', () => {
  const f = runRule(findRule('OVER-008'), 'a.ts', 'export const r = arr.map((a) => b.filter((c) => c.list.map((d) => d.id)));');
  assert.equal(byId('OVER-008', f).length, 1);
});

test('OVER-009: unused type parameter is flagged', () => {
  const f = runRule(findRule('OVER-009'), 'a.ts', 'export function f<TState>(x: number): number { return x; }');
  assert.equal(byId('OVER-009', f).length, 1);
});

test('OVER-009: used type parameter is not flagged', () => {
  const f = runRule(findRule('OVER-009'), 'a.ts', 'export function identity<T>(x: T): T { return x; }');
  assert.equal(byId('OVER-009', f).length, 0);
});

// ---------------------------------------------------------------- TEXT RULES

test('COMMIT-001..011: boilerplate commit messages are flagged', () => {
  const textRules = allTextRules();
  const runText = (text: string) => {
    const out: Finding[] = [];
    for (const r of textRules) out.push(...r.check(text, 'commit-msg'));
    return out;
  };
  assert.ok(byId('COMMIT-001', runText('fix typo')).length === 1);
  assert.ok(byId('COMMIT-002', runText('update README')).length === 1);
  assert.ok(byId('COMMIT-003', runText('minor changes')).length === 1);
  assert.ok(byId('COMMIT-004', runText('wip')).length === 1);
  assert.ok(byId('COMMIT-005', runText('refactor')).length === 1);
  assert.ok(byId('COMMIT-006', runText('initial commit')).length === 1);
  assert.ok(byId('COMMIT-007', runText('   ')).length === 1);
  assert.ok(byId('COMMIT-008', runText('cleanup')).length === 1);
  assert.ok(byId('COMMIT-009', runText('add stuff')).length === 1);
  assert.ok(byId('COMMIT-010', runText('n/a')).length === 1);
});

test('COMMIT rules: a substantive message is not flagged', () => {
  const textRules = allTextRules();
  const msg = 'Add retry with exponential backoff to the fetch wrapper so transient 502s do not fail deploys';
  const out: Finding[] = [];
  for (const r of textRules) out.push(...r.check(msg, 'commit-msg'));
  assert.equal(out.length, 0);
});

test('AI-001..014: AI-assistant phrasing is flagged', () => {
  const textRules = allTextRules();
  const runText = (text: string) => {
    const out: Finding[] = [];
    for (const r of textRules) out.push(...r.check(text, 'docs'));
    return out;
  };
  assert.ok(byId('AI-001', runText('As an AI language model, I cannot do that.')).length === 1);
  assert.ok(byId('AI-002', runText('As a language model, I have no opinion.')).length === 1);
  assert.ok(byId('AI-003', runText("I'm sorry, but I cannot assist with that.")).length === 1);
  assert.ok(byId('AI-004', runText("I don't have personal opinions on this.")).length === 1);
  assert.ok(byId('AI-005', runText('As of my last knowledge update, this was true.')).length === 1);
  assert.ok(byId('AI-006', runText('Let me know if you have any other questions.')).length === 1);
  assert.ok(byId('AI-007', runText("Certainly! Here's how to do it.")).length === 1);
  assert.ok(byId('AI-008', runText('Is there anything else I can help with?')).length === 1);
  assert.ok(byId('AI-009', runText("It's important to note that this matters.")).length === 1);
  assert.ok(byId('AI-010', runText("I'd be happy to help.")).length === 1);
  assert.ok(byId('AI-011', runText('Great question!')).length === 1);
  assert.ok(byId('AI-012', runText('In conclusion, ship it.')).length === 1);
  assert.ok(byId('AI-013', runText('Here is a comprehensive guide to testing.')).length === 1);
  assert.ok(byId('AI-014', runText('Please note that this is versioned.')).length === 1);
});

test('AI rules: normal technical prose is not flagged', () => {
  const textRules = allTextRules();
  const prose = 'The retry loop waits 200ms between attempts and backs off exponentially. Errors are logged with request ids.';
  const out: Finding[] = [];
  for (const r of textRules) out.push(...r.check(prose, 'docs'));
  assert.equal(out.length, 0);
});
