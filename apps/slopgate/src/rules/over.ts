// Over-engineering rules: indirection, ceremony and patterns that add complexity without payoff.
import type { Finding, Rule } from '../types.ts';
import { extractFunctions, findMatchingBrace, isCodeFile } from '../analysis.ts';

const base = {
  category: 'over-engineering' as const,
};

const applies = (file: string): boolean => isCodeFile(file);

const unnecessaryAsync: Rule = {
  id: 'OVER-001',
  name: 'Unnecessary async function',
  severity: 'medium',
  description: 'An \`async\` function that never awaits — the async keyword adds nothing but promise overhead.',
  ...base,
  applies,
  run({ file, content, lines }) {
    const findings: Finding[] = [];
    const re = /(?:export\s+)?(?:async\s+)(?:function\s+(\w+)|\([^)]*\)\s*=>)/g;
    let m: RegExpExecArray | null;
    while ((m = re.exec(content)) !== null) {
      const braceIdx = content.indexOf('{', m.index);
      if (braceIdx === -1) continue;
      const closeIdx = findMatchingBrace(content, braceIdx);
      if (closeIdx === -1) continue;
      const body = content.slice(braceIdx + 1, closeIdx);
      if (!/\bawait\b|\bfor await\b|\byield\b/.test(body)) {
        const line = content.slice(0, m.index).split('\n').length;
        findings.push({
          ruleId: this.id,
          severity: this.severity,
          category: this.category,
          message: m[1]
            ? `\`async function ${m[1]}\` never awaits — drop \`async\`.`
            : 'Async arrow function never awaits — drop \`async\`.',
          file,
          line,
          evidence: m[0].trim().slice(0, 100),
        });
      }
    }
    return findings;
  },
};

const promiseAntiPattern: Rule = {
  id: 'OVER-002',
  name: 'Promise constructor anti-pattern',
  severity: 'medium',
  description: '\`new Promise((resolve) => resolve(x))\` wraps a value that is already available — just return it.',
  ...base,
  applies,
  run({ file, content }) {
    const findings: Finding[] = [];
    const re = /new\s+Promise\s*\(\s*(?:async\s*)?\(([^)]*)\)\s*=>\s*\{?\s*(\w+)\s*\(\s*[^)]*\)\s*;?\s*\}?\s*\)/g;
    let m: RegExpExecArray | null;
    while ((m = re.exec(content)) !== null) {
      const [resolver] = m[1].split(',').map((s) => s.trim());
      if (resolver && m[2] === resolver) {
        findings.push({
          ruleId: this.id,
          severity: this.severity,
          category: this.category,
          message: 'Promise constructor immediately resolves a synchronous value — return the value directly.',
          file,
          line: content.slice(0, m.index).split('\n').length,
          evidence: m[0].trim().slice(0, 100),
        });
      }
    }
    return findings;
  },
};

const emptyCatch: Rule = {
  id: 'OVER-003',
  name: 'Empty catch block',
  severity: 'medium',
  description: 'A catch block that swallows errors entirely — errors disappear with no trace, log or rethrow.',
  ...base,
  applies,
  run({ file, content }) {
    const findings: Finding[] = [];
    const re = /catch\s*(?:\(\s*\w+\s*\))?\s*\{\s*(?:\/\/[^\n]*)?\s*\}/g;
    let m: RegExpExecArray | null;
    while ((m = re.exec(content)) !== null) {
      findings.push({
        ruleId: this.id,
        severity: this.severity,
        category: this.category,
        message: 'Empty catch block swallows errors silently.',
        file,
        line: content.slice(0, m.index).split('\n').length,
        evidence: m[0].trim(),
      });
    }
    return findings;
  },
};

const excessiveAny: Rule = {
  id: 'OVER-004',
  name: 'Excessive any annotations',
  severity: 'low',
  description: 'Three or more \`: any\` / \`<any>\` annotations in one file — the type system is being bypassed.',
  ...base,
  applies,
  run({ file, content }) {
    const findings: Finding[] = [];
    const matches = content.match(/\bany\b/g);
    const count = matches ? matches.length : 0;
    if (count >= 3) {
      findings.push({
        ruleId: this.id,
        severity: this.severity,
        category: this.category,
        message: `${count} \`any\` occurrences — consider real types.`,
        file,
        line: 1,
        evidence: ': any',
      });
    }
    return findings;
  },
};

const statelessSingleton: Rule = {
  id: 'OVER-005',
  name: 'Stateless singleton',
  severity: 'medium',
  description: 'A singleton (private ctor + getInstance) with no instance state — a namespace would do.',
  ...base,
  applies,
  run({ file, content }) {
    const findings: Finding[] = [];
    const re = /class\s+(\w+)\s*\{/g;
    let m: RegExpExecArray | null;
    while ((m = re.exec(content)) !== null) {
      const braceIdx = content.indexOf('{', m.index);
      const closeIdx = findMatchingBrace(content, braceIdx);
      if (closeIdx === -1) continue;
      const cls = content.slice(m.index, closeIdx);
      const hasGetInstance = /getInstance\s*\(/.test(cls);
      const hasPrivateCtor = /private\s+constructor/.test(cls);
      const hasState = /\bthis\.\w+\s*=/.test(cls);
      if (hasGetInstance && hasPrivateCtor && !hasState) {
        findings.push({
          ruleId: this.id,
          severity: this.severity,
          category: this.category,
          message: `Singleton \`${m[1]}\` holds no instance state — replace with module-level functions.`,
          file,
          line: content.slice(0, m.index).split('\n').length,
          evidence: `class ${m[1]}`,
        });
      }
    }
    return findings;
  },
};

const parameterlessFactory: Rule = {
  id: 'OVER-006',
  name: 'Parameterless factory',
  severity: 'medium',
  description: 'A create/build/make function that takes no options and returns a new default instance — call the constructor directly.',
  ...base,
  applies,
  run({ file, content }) {
    const findings: Finding[] = [];
    const re = /(?:export\s+)?function\s+(?:create|make|build|get)[A-Z]\w*\s*\(\s*\)\s*\{/g;
    let m: RegExpExecArray | null;
    while ((m = re.exec(content)) !== null) {
      const braceIdx = content.indexOf('{', m.index);
      const closeIdx = findMatchingBrace(content, braceIdx);
      if (closeIdx === -1) continue;
      const body = content.slice(braceIdx + 1, closeIdx);
      if (/\breturn\s+new\s+\w+\s*\(\s*\)/.test(body)) {
        findings.push({
          ruleId: this.id,
          severity: this.severity,
          category: this.category,
          message: `\`${m[0].replace(/function\s*/, '').replace(/\s*\{$/, '')}\` is a parameterless factory returning a default instance — call the constructor directly.`,
          file,
          line: content.slice(0, m.index).split('\n').length,
          evidence: m[0].trim().slice(0, 100),
        });
      }
    }
    return findings;
  },
};

const emptyBranch: Rule = {
  id: 'OVER-007',
  name: 'Empty if/else branch',
  severity: 'low',
  description: 'An if/else branch with an empty body (comment-only counts as empty).',
  ...base,
  applies,
  run({ file, content }) {
    const findings: Finding[] = [];
    const re = /\b(if|else\s+if|else)\s*(\([^)]*\))?\s*\{\s*(?:\/\/[^\n]*)?\s*\}/g;
    let m: RegExpExecArray | null;
    while ((m = re.exec(content)) !== null) {
      findings.push({
        ruleId: this.id,
        severity: this.severity,
        category: this.category,
        message: `Empty ${m[1].trim()} branch — dead logic.`,
        file,
        line: content.slice(0, m.index).split('\n').length,
        evidence: m[0].trim(),
      });
    }
    return findings;
  },
};

const arrowSoup: Rule = {
  id: 'OVER-008',
  name: 'Deep arrow nesting on one line',
  severity: 'low',
  description: 'Three or more arrow functions on a single line — unreadable callback soup.',
  ...base,
  applies,
  run({ file, lines }) {
    const findings: Finding[] = [];
    lines.forEach((raw, idx) => {
      const arrows = (raw.match(/=>/g) ?? []).length;
      if (arrows >= 3) {
        findings.push({
          ruleId: this.id,
          severity: this.severity,
          category: this.category,
          message: `${arrows} nested arrows on one line — refactor.`,
          file,
          line: idx + 1,
          evidence: raw.trim().slice(0, 100),
        });
      }
    });
    return findings;
  },
};

const unusedTypeParameter: Rule = {
  id: 'OVER-009',
  name: 'Unused type parameter',
  severity: 'low',
  description: 'A generic whose type parameter never appears in its signature or body.',
  ...base,
  applies,
  run({ file, content }) {
    const findings: Finding[] = [];
    const re = /(?:export\s+)?(?:async\s+)?function\s+\w+\s*<([A-Z]\w*)>/g;
    let m: RegExpExecArray | null;
    while ((m = re.exec(content)) !== null) {
      const tp = m[1];
      const braceIdx = content.indexOf('{', m.index);
      if (braceIdx === -1) continue;
      const closeIdx = findMatchingBrace(content, braceIdx);
      if (closeIdx === -1) continue;
      const span = content.slice(m.index, closeIdx);
      const reTp = new RegExp(`\\b${tp}\\b`, 'g');
      const occurrences = span.match(reTp)?.length ?? 0;
      // the declaration itself contains the type parameter once; any more means it is used
      if (occurrences <= 1) {
        findings.push({
          ruleId: this.id,
          severity: this.severity,
          category: this.category,
          message: `Type parameter \`${tp}\` is declared but never used.`,
          file,
          line: content.slice(0, m.index).split('\n').length,
          evidence: `<${tp}>`,
        });
      }
    }
    return findings;
  },
};

export function overEngineeringRules(): Rule[] {
  return [
    unnecessaryAsync,
    promiseAntiPattern,
    emptyCatch,
    excessiveAny,
    statelessSingleton,
    parameterlessFactory,
    emptyBranch,
    arrowSoup,
    unusedTypeParameter,
  ];
}
