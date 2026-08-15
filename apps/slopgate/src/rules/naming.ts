// Generic-naming rules: file and identifier names that carry no information.
import type { Finding, Rule } from '../types.ts';
import { isCodeFile } from '../analysis.ts';

const base = {
  category: 'generic-naming' as const,
};

const applies = (file: string): boolean => isCodeFile(file);

const GENERIC_BASENAMES = [
  'utils', 'util', 'helpers', 'helper', 'common', 'misc', 'miscellany',
  'stuff', 'various', 'lib', 'shared_utils', 'general', 'generic',
];

const genericFileName: Rule = {
  id: 'NAME-001',
  name: 'Generic file name',
  severity: 'low',
  description: 'File names like utils.ts / helpers.ts / misc.ts are catch-all names that hide what the module does.',
  ...base,
  applies,
  run({ file }) {
    const findings: Finding[] = [];
    const baseName = file.split('/').pop()?.replace(/\.[^.]+$/, '').toLowerCase() ?? '';
    if (GENERIC_BASENAMES.includes(baseName)) {
      findings.push({
        ruleId: this.id,
        severity: this.severity,
        category: this.category,
        message: `Generic file name \`${baseName}\` — rename to describe what the module actually contains.`,
        file,
        line: 1,
        evidence: file,
      });
    }
    return findings;
  },
};

const GENERIC_IDS = new Set([
  'data', 'info', 'stuff', 'thing', 'things', 'temp', 'tmp', 'foo', 'bar', 'baz',
  'something', 'blah', 'misc', 'various', 'thething', 'dothis', 'dothat', 'doit',
  'doStuff', 'doThing', 'handleIt', 'processIt', 'whatever', 'placeholder', 'dummy',
]);

const genericIdentifier: Rule = {
  id: 'NAME-002',
  name: 'Generic identifier',
  severity: 'low',
  description: 'Declarations named data / info / temp / foo / doStuff — names that describe nothing.',
  ...base,
  applies,
  run({ file, content }) {
    const findings: Finding[] = [];
    const re = /(?:^|\n)\s*(?:export\s+)?(?:const|let|var|function|class)\s+(\w+)/g;
    let m: RegExpExecArray | null;
    while ((m = re.exec(content)) !== null) {
      if (GENERIC_IDS.has(m[1])) {
        findings.push({
          ruleId: this.id,
          severity: this.severity,
          category: this.category,
          message: `Generic identifier \`${m[1]}\` — use a name that says what it is.`,
          file,
          line: content.slice(0, m.index).split('\n').length,
          evidence: m[0].trim(),
        });
      }
    }
    return findings;
  },
};

const TYPE_SUFFIX_RE = /(Arr|Array|String|Str|Object|Obj|Number|Num|List|Map|Dict|Func|Fn|Bool|Boolean|Val|Value)s?$/i;

const redundantTypeSuffix: Rule = {
  id: 'NAME-003',
  name: 'Redundant type suffix in name',
  severity: 'low',
  description: 'Names like userArray / configObject / nameString repeat the type in the identifier.',
  ...base,
  applies,
  run({ file, content }) {
    const findings: Finding[] = [];
    const re = /(?:^|\n)\s*(?:export\s+)?(?:const|let|var|function|class)\s+(\w+)/g;
    let m: RegExpExecArray | null;
    while ((m = re.exec(content)) !== null) {
      const name = m[1];
      const tm = name.match(TYPE_SUFFIX_RE);
      if (tm && tm[0].length >= 3 && name.length > tm[0].length) {
        findings.push({
          ruleId: this.id,
          severity: this.severity,
          category: this.category,
          message: `Name \`${name}\` embeds its type ("${tm[0]}") — redundant.`,
          file,
          line: content.slice(0, m.index).split('\n').length,
          evidence: m[0].trim(),
        });
      }
    }
    return findings;
  },
};

const CRYPTIC = new Set(['t', 'n', 's', 'c', 'd', 'e', 'f', 'g', 'h', 'r', 'o', 'a', 'v', 'u', 'w', 'p']);

const crypticName: Rule = {
  id: 'NAME-004',
  name: 'Cryptic single-letter name',
  severity: 'low',
  description: 'Single-letter identifiers outside for-loop counters (t, n, s, c…) — cryptic.',
  ...base,
  applies,
  run({ file, lines }) {
    const findings: Finding[] = [];
    lines.forEach((raw, idx) => {
      if (/for\s*\(/.test(raw)) return; // loop counters are fine
      const m = raw.match(/^\s*(?:export\s+)?(?:const|let|var)\s+([a-z])\s*=/);
      if (m && CRYPTIC.has(m[1])) {
        findings.push({
          ruleId: this.id,
          severity: this.severity,
          category: this.category,
          message: `Cryptic single-letter variable \`${m[1]}\`.`,
          file,
          line: idx + 1,
          evidence: raw.trim(),
        });
      }
    });
    return findings;
  },
};

const duplicatedWord: Rule = {
  id: 'NAME-005',
  name: 'Duplicated word in name',
  severity: 'low',
  description: 'Identifiers like data_data, userUser, configConfig — accidental or placeholder duplication.',
  ...base,
  applies,
  run({ file, content }) {
    const findings: Finding[] = [];
    const re = /(?:^|\n)\s*(?:export\s+)?(?:const|let|var|function|class)\s+(\w+)/g;
    let m: RegExpExecArray | null;
    while ((m = re.exec(content)) !== null) {
      const name = m[1];
      if (/(\w+)_\1/.test(name) || /([a-z][a-z0-9]+?)\1[A-Z0-9_]/.test(name)) {
        findings.push({
          ruleId: this.id,
          severity: this.severity,
          category: this.category,
          message: `Duplicated word inside identifier \`${name}\`.`,
          file,
          line: content.slice(0, m.index).split('\n').length,
          evidence: m[0].trim(),
        });
      }
    }
    return findings;
  },
};

export function genericNamingRules(): Rule[] {
  return [genericFileName, genericIdentifier, redundantTypeSuffix, crypticName, duplicatedWord];
}
