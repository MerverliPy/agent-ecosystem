// Scanner: walks a tree, runs per-file rules, then cross-file analysis.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';
import type { Finding, Rule, ScannedFile, ScanResult, TextRule } from './types.ts';
import { isCodeFile, isMinified, isProseFile } from './analysis.ts';
import { allCrossFileRules, allFileRules, allTextRules } from './rules/index.ts';

const SKIP_DIRS = new Set([
  'node_modules', '.git', 'dist', 'build', 'out', '.next', 'coverage', 'target',
  '.cache', '.turbo', '.venv', 'venv', '__pycache__', '.idea', '.vscode',
]);

const SKIP_FILES = new Set([
  'package-lock.json', 'pnpm-lock.yaml', 'yarn.lock', 'Cargo.lock', 'go.sum',
  '.DS_Store', 'npm-shrinkwrap.json', 'bun.lockb', 'bun.lock',
]);

export interface ScanOptions {
  /** Scan prose files (.md/.mdx/.txt/.rst) with the text rule packs. Default true. */
  prose?: boolean;
  /** Limit per-file findings (0 = no limit). Default 0. */
  maxFindingsPerFile?: number;
}

export interface ScanContext {
  files: ScannedFile[];
  skipped: string[];
}

/** Recursively list eligible files under `root`. */
export function collectFiles(root: string): ScanContext {
  const files: ScannedFile[] = [];
  const skipped: string[] = [];
  const walk = (dir: string) => {
    let entries: string[];
    try {
      entries = readdirSync(dir);
    } catch {
      skipped.push(dir);
      return;
    }
    for (const entry of entries) {
      const full = path.join(dir, entry);
      let st: ReturnType<typeof statSync>;
      try {
        st = statSync(full);
      } catch {
        skipped.push(full);
        continue;
      }
      if (st.isDirectory()) {
        if (!SKIP_DIRS.has(entry)) walk(full);
        continue;
      }
      if (SKIP_FILES.has(entry)) continue;
      if (!isCodeFile(entry) && !isProseFile(entry)) continue;
      let content: string;
      try {
        content = readFileSync(full, 'utf8');
      } catch {
        skipped.push(full);
        continue;
      }
      if (isMinified(content)) {
        skipped.push(full);
        continue;
      }
      files.push({ path: full, content });
    }
  };
  walk(root);
  return { files, skipped };
}

/** Run all per-file rules over a single file. */
export function runFileRules(file: ScannedFile, rules: Rule[], maxPerFile: number): Finding[] {
  const lines = file.content.split('\n');
  const findings: Finding[] = [];
  for (const rule of rules) {
    if (!rule.applies(file.path)) continue;
    try {
      const found = rule.run({ file: file.path, content: file.content, lines });
      for (const f of found) findings.push({ ...f, file: f.file ?? file.path });
    } catch {
      // a rule must never crash a scan — skip it for this file
    }
    if (maxPerFile > 0 && findings.length >= maxPerFile) break;
  }
  return findings;
}

/** Run text rules against a prose file (one finding set per file). */
export function runTextRulesOnFile(file: ScannedFile, textRules: TextRule[], maxPerFile: number): Finding[] {
  const findings: Finding[] = [];
  for (const rule of textRules) {
    // COMMIT rules are for commit messages/PR bodies, not arbitrary prose files;
    // only AI-phrasing (and generic prose) rules apply to repo docs.
    if (rule.category === 'boilerplate-text') continue;
    try {
      const found = rule.check(file.content, file.path);
      for (const f of found) findings.push({ ...f, file: file.path });
    } catch {
      // no-op
    }
    if (maxPerFile > 0 && findings.length >= maxPerFile) break;
  }
  return findings;
}

/** Run text rules against a free-text blob (commit message, PR body, arbitrary string). */
export function runTextRules(text: string, source: string, textRules: TextRule[]): Finding[] {
  const findings: Finding[] = [];
  for (const rule of textRules) {
    try {
      findings.push(...rule.check(text, source));
    } catch {
      // no-op
    }
  }
  return findings;
}

export function scanTree(root: string, opts: ScanOptions = {}): ScanResult {
  const prose = opts.prose ?? true;
  const maxPerFile = opts.maxFindingsPerFile ?? 0;
  const { files, skipped } = collectFiles(root);
  const findings: Finding[] = [];

  const fileRules = allFileRules();
  const textRules = allTextRules();

  for (const file of files) {
    if (isCodeFile(file.path)) {
      findings.push(...runFileRules(file, fileRules, maxPerFile));
    }
    if (prose && isProseFile(file.path)) {
      findings.push(...runTextRulesOnFile(file, textRules, maxPerFile));
    }
  }

  // Cross-file analysis over all code files.
  findings.push(...allCrossFileRules(files));

  return {
    findings,
    filesScanned: files.length,
    filesSkipped: skipped,
    scannedAt: new Date().toISOString(),
  };
}
