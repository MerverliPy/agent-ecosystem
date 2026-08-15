// Unused-helper rules: helpers, imports and variables that are never used.
import type { Finding, Rule } from '../types.ts';
import { countInFile, countReferences, extractDeclarations, isCodeFile } from '../analysis.ts';
import type { ScannedFile } from '../types.ts';

const applies = (file: string): boolean => isCodeFile(file);

const base = {
  category: 'unused-helper' as const,
};

/** Exported function/const never referenced outside its own file (cross-file). */
function exportedUnusedRule(files: ScannedFile[]): Finding[] {
  const findings: Finding[] = [];
  const all = files.filter((f) => isCodeFile(f.path));
  for (const f of all) {
    for (const d of extractDeclarations(f.path, f.content)) {
      if (d.kind === 'function' || d.kind === 'const') {
        if (d.exported && countReferences(d.name, all) === 1) {
          findings.push({
            ruleId: 'UNUSED-001',
            severity: 'medium',
            category: 'unused-helper',
            message: `Exported ${d.kind} \`${d.name}\` is never imported or used anywhere — unused helper.`,
            file: f.path,
            line: d.line,
            evidence: `${d.kind} ${d.name}`,
          });
        }
      }
    }
  }
  return findings;
}

/** Local (non-exported) function declared but never called in its own file. */
function localFunctionUnusedRule(files: ScannedFile[]): Finding[] {
  const findings: Finding[] = [];
  const all = files.filter((f) => isCodeFile(f.path));
  for (const f of all) {
    for (const d of extractDeclarations(f.path, f.content)) {
      if (d.kind === 'function' && !d.exported) {
        // count occurrences in this file only; 1 == declaration only
        if (countInFile(d.name, f.content) === 1) {
          findings.push({
            ruleId: 'UNUSED-002',
            severity: 'medium',
            category: 'unused-helper',
            message: `Local function \`${d.name}\` is never called.`,
            file: f.path,
            line: d.line,
            evidence: `function ${d.name}`,
          });
        }
      }
    }
  }
  return findings;
}

const unusedImport: Rule = {
  id: 'UNUSED-003',
  name: 'Unused import',
  severity: 'medium',
  description: 'A symbol imported from another module but never used in this file.',
  ...base,
  applies,
  run({ file, content }) {
    const findings: Finding[] = [];
    const importRe = /import\s+([^;]+?)\s+from\s+['"][^'"]+['"]/g;
    let m: RegExpExecArray | null;
    while ((m = importRe.exec(content)) !== null) {
      const spec = m[1].trim();
      // default import
      let defaultName: string | null = null;
      let names: string[] = [];
      let nsName: string | null = null;
      const dm = spec.match(/^(\w+)$/);
      if (dm) defaultName = dm[1];
      else {
        const brace = spec.match(/^\{([^}]*)\}$/);
        if (brace) {
          names = brace[1].split(',').map((s) => s.trim()).filter(Boolean)
            .map((s) => {
              const am = s.match(/^(\w+)\s+as\s+(\w+)$/);
              return am ? am[2] : s;
            });
        } else {
          const nm = spec.match(/^\*\s+as\s+(\w+)$/);
          if (nm) nsName = nm[1];
          else if (spec === '') { /* side-effect import */ }
        }
      }
      const bodyAfter = content.slice(m.index + m[0].length);
      const line = content.slice(0, m.index).split('\n').length;
      const evidence = m[0].trim();
      const check = (name: string, what: string) => {
        if (countInFile(name, bodyAfter) === 0 && !name.startsWith('_')) {
          findings.push({
            ruleId: this.id,
            severity: this.severity,
            category: this.category,
            message: `Imported ${what} \`${name}\` is never used in this file.`,
            file,
            line,
            evidence,
          });
        }
      };
      if (defaultName) check(defaultName, 'default');
      for (const n of names) check(n, 'symbol');
      if (nsName) check(nsName, 'namespace');
    }
    return findings;
  },
};

const unusedVariable: Rule = {
  id: 'UNUSED-004',
  name: 'Unused variable',
  severity: 'low',
  description: 'A const/let whose name appears exactly once in its file (the declaration) — it is never read.',
  ...base,
  applies,
  run({ file, content }) {
    const findings: Finding[] = [];
    const re = /(?:^|\n)\s*(?:export\s+)?(?:const|let)\s+(\w+)\s*=/g;
    let m: RegExpExecArray | null;
    while ((m = re.exec(content)) !== null) {
      const name = m[1];
      if (name.startsWith('_')) continue;
      if (countInFile(name, content) === 1) {
        findings.push({
          ruleId: this.id,
          severity: this.severity,
          category: this.category,
          message: `Variable \`${name}\` is declared but never used.`,
          file,
          line: content.slice(0, m.index).split('\n').length,
          evidence: `${m[0].trim()}`,
        });
      }
    }
    return findings;
  },
};

const duplicateImport: Rule = {
  id: 'UNUSED-005',
  name: 'Duplicate import of the same module',
  severity: 'low',
  description: 'The same module is imported twice in one file — merge the imports.',
  ...base,
  applies,
  run({ file, content }) {
    const findings: Finding[] = [];
    const seen = new Map<string, number>();
    const re = /import\s+[^;]+?\s+from\s+['"]([^'"]+)['"]/g;
    let m: RegExpExecArray | null;
    while ((m = re.exec(content)) !== null) {
      // `import type` is a separate, idiomatic import kind — not a duplicate.
      const spec = content.slice(m.index, m.index + m[0].length);
      if (/^import\s+type\b/.test(spec)) continue;
      const mod = m[1];
      if (seen.has(mod)) {
        findings.push({
          ruleId: this.id,
          severity: this.severity,
          category: this.category,
          message: `Module \`${mod}\` is imported twice (first at line ${seen.get(mod)}).`,
          file,
          line: content.slice(0, m.index).split('\n').length,
          evidence: m[0].trim(),
        });
      } else {
        seen.set(mod, content.slice(0, m.index).split('\n').length);
      }
    }
    return findings;
  },
};

export function unusedHelperRules(): Rule[] {
  return [unusedImport, unusedVariable, duplicateImport];
}

export function unusedHelperCrossFileRules(files: ScannedFile[]): Finding[] {
  return [...exportedUnusedRule(files), ...localFunctionUnusedRule(files)];
}
