// Dead-abstraction rules: abstractions that add no value or are never used.
import type { Finding, Rule } from '../types.ts';
import { countReferences, extractDeclarations, isCodeFile } from '../analysis.ts';
import type { ScannedFile } from '../types.ts';

const applies = (file: string): boolean => isCodeFile(file);

const base = {
  category: 'dead-abstraction' as const,
};

/** Unreferenced exported interface/type alias across the project (cross-file rule). */
function unreferencedInterfaceRule(files: ScannedFile[]): Finding[] {
  const findings: Finding[] = [];
  const all = files.filter((f) => isCodeFile(f.path));
  for (const f of all) {
    for (const d of extractDeclarations(f.path, f.content)) {
      if ((d.kind === 'interface' || d.kind === 'type') && d.exported) {
        if (countReferences(d.name, all) === 1) {
          findings.push({
            ruleId: 'DEAD-001',
            severity: 'high',
            category: 'dead-abstraction',
            message: `Exported ${d.kind} \`${d.name}\` is never referenced anywhere in the project — dead abstraction.`,
            file: f.path,
            line: d.line,
            evidence: `export ${d.kind} ${d.name}`,
          });
        }
      }
    }
  }
  return findings;
}

const emptyInterface: Rule = {
  id: 'DEAD-002',
  name: 'Empty interface',
  severity: 'medium',
  description: 'Interface with zero members — a marker/flag interface that documents nothing and cannot be implemented meaningfully.',
  ...base,
  applies,
  run({ file, content, lines }) {
    const findings: Finding[] = [];
    const re = /interface\s+(\w+)\s*\{\s*\}/g;
    let m: RegExpExecArray | null;
    while ((m = re.exec(content)) !== null) {
      const line = content.slice(0, m.index).split('\n').length;
      findings.push({
        ruleId: this.id,
        severity: this.severity,
        category: this.category,
        message: `Interface \`${m[1]}\` has no members — an empty abstraction.`,
        file,
        line,
        evidence: m[0].trim(),
      });
    }
    return findings;
  },
};

const abstractWithoutAbstract: Rule = {
  id: 'DEAD-003',
  name: 'Abstract class with no abstract members',
  severity: 'medium',
  description: 'A class declared \`abstract\` but containing zero abstract members can be a concrete class — the abstraction is cosmetic.',
  ...base,
  applies,
  run({ file, content, lines }) {
    const findings: Finding[] = [];
    const re = /\babstract\s+class\s+(\w+)/g;
    let m: RegExpExecArray | null;
    while ((m = re.exec(content)) !== null) {
      const braceIdx = content.indexOf('{', m.index);
      if (braceIdx === -1) continue;
      const closeIdx = findMatchingBraceSafe(content, braceIdx);
      if (closeIdx === -1) continue;
      const body = content.slice(m.index, closeIdx);
      const abstractMembers = (body.match(/\babstract\s+(?:class|get|set|readonly)?\s*(?:\w+\s*[:(])/g) ?? []).length;
      if (abstractMembers === 0) {
        findings.push({
          ruleId: this.id,
          severity: this.severity,
          category: this.category,
          message: `Class \`${m[1]}\` is \`abstract\` but declares no abstract members.`,
          file,
          line: content.slice(0, m.index).split('\n').length,
          evidence: m[0],
        });
      }
    }
    return findings;
  },
};

const emptySubclass: Rule = {
  id: 'DEAD-004',
  name: 'Empty subclass',
  severity: 'medium',
  description: 'A class that extends another and adds no members of its own contributes nothing.',
  ...base,
  applies,
  run({ file, content }) {
    const findings: Finding[] = [];
    const re = /class\s+(\w+)\s+extends\s+(\w+)\s*\{\s*\}/g;
    let m: RegExpExecArray | null;
    while ((m = re.exec(content)) !== null) {
      findings.push({
        ruleId: this.id,
        severity: this.severity,
        category: this.category,
        message: `Class \`${m[1]}\` extends \`${m[2]}\` but adds no members — empty subclass.`,
        file,
        line: content.slice(0, m.index).split('\n').length,
        evidence: m[0].trim(),
      });
    }
    return findings;
  },
};

/** Pass-through wrapper function that forwards all params and is never called (cross-file). */
function passThroughWrapperRule(files: ScannedFile[]): Finding[] {
  const findings: Finding[] = [];
  const all = files.filter((f) => isCodeFile(f.path));
  const re = /(?:export\s+)?(?:async\s+)?function\s+(\w+)\s*\(([^)]*)\)\s*\{\s*return\s+(\w+)\s*\(\s*([^)]*)\s*\)\s*;?\s*\}/g;
  for (const f of all) {
    let m: RegExpExecArray | null;
    while ((m = re.exec(f.content)) !== null) {
      const params = m[2].split(',').map((p) => p.trim().split(/\s*[:=]/)[0]).filter(Boolean);
      const args = m[4].split(',').map((a) => a.trim()).filter(Boolean);
      const sameSet = params.length === args.length && params.every((p) => args.includes(p));
      if (!sameSet) continue;
      // Called anywhere (including its own file) more than the declaration itself?
      if (countReferences(m[1], all) > 1) continue;
      findings.push({
        ruleId: 'DEAD-005',
        severity: 'high',
        category: 'dead-abstraction',
        message: `\`${m[1]}\` is a pass-through wrapper around \`${m[3]}\` and is never called — delete the wrapper.`,
        file: f.path,
        line: f.content.slice(0, m.index).split('\n').length,
        evidence: m[0].trim().slice(0, 120),
      });
    }
  }
  return findings;
}

/** Local (non-exported) class that is never referenced (cross-file). */
function unreferencedLocalClassRule(files: ScannedFile[]): Finding[] {
  const findings: Finding[] = [];
  const all = files.filter((f) => isCodeFile(f.path));
  for (const f of all) {
    for (const d of extractDeclarations(f.path, f.content)) {
      if (d.kind === 'class' && !d.exported) {
        if (countReferences(d.name, all) === 1) {
          findings.push({
            ruleId: 'DEAD-006',
            severity: 'medium',
            category: 'dead-abstraction',
            message: `Local class \`${d.name}\` is never referenced — dead abstraction.`,
            file: f.path,
            line: d.line,
            evidence: `class ${d.name}`,
          });
        }
      }
    }
  }
  return findings;
}

function findMatchingBraceSafe(content: string, open: number): number {
  // local mirror to avoid importing the helper twice
  let depth = 0;
  for (let i = open; i < content.length; i++) {
    const ch = content[i];
    if (ch === '{') depth++;
    else if (ch === '}') {
      depth--;
      if (depth === 0) return i;
    }
  }
  return -1;
}

export function deadAbstractionRules(): Rule[] {
  return [emptyInterface, abstractWithoutAbstract, emptySubclass];
}

export function deadAbstractionCrossFileRules(files: ScannedFile[]): Finding[] {
  return [
    ...unreferencedInterfaceRule(files),
    ...passThroughWrapperRule(files),
    ...unreferencedLocalClassRule(files),
  ];
}
