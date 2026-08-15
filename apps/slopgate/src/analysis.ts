// Deterministic regex/brace-based analysis helpers.
// These are heuristic by design (documented in each rule) but fully deterministic:
// the same input always yields the same findings.

import type { ScannedFile } from './types.ts';

/** Code file extensions the deterministic rule pack inspects. */
export const CODE_EXTENSIONS = ['.ts', '.tsx', '.js', '.jsx', '.mjs', '.cjs'];

/** Prose extensions scanned with the text rule packs. */
export const PROSE_EXTENSIONS = ['.md', '.mdx', '.txt', '.rst'];

export function isCodeFile(file: string): boolean {
  const lower = file.toLowerCase();
  return CODE_EXTENSIONS.some((ext) => lower.endsWith(ext));
}

export function isProseFile(file: string): boolean {
  const lower = file.toLowerCase();
  return PROSE_EXTENSIONS.some((ext) => lower.endsWith(ext));
}

export function isMinified(content: string): boolean {
  // A "minified" file is dominated by very long lines with no newlines between statements.
  const lines = content.split('\n');
  if (lines.length < 3) return false;
  const longLines = lines.filter((l) => l.length > 1000).length;
  return longLines / lines.length > 0.5;
}

/** Find the index of the matching close brace starting at `openIndex` (content[openIndex] === '{'). */
export function findMatchingBrace(content: string, openIndex: number): number {
  let depth = 0;
  let inString: '"' | "'" | '`' | null = null;
  let escaped = false;
  for (let i = openIndex; i < content.length; i++) {
    const ch = content[i];
    if (inString) {
      if (escaped) {
        escaped = false;
      } else if (ch === '\\') {
        escaped = true;
      } else if (ch === inString) {
        inString = null;
      }
      continue;
    }
    if (ch === '"' || ch === "'" || ch === '`') {
      inString = ch;
      continue;
    }
    if (ch === '/') {
      const next = content[i + 1];
      if (next === '/') {
        const nl = content.indexOf('\n', i);
        i = nl === -1 ? content.length : nl;
        continue;
      }
      if (next === '*') {
        const end = content.indexOf('*/', i + 2);
        i = end === -1 ? content.length : end + 1;
        continue;
      }
    }
    if (ch === '{') depth++;
    else if (ch === '}') {
      depth--;
      if (depth === 0) return i;
    }
  }
  return -1;
}

/** Extract a function's parameter list and body given its signature start index. */
export interface FunctionShape {
  name: string;
  params: string;
  body: string;
  line: number;
}

/** Find function-like declarations: `function name(...) {...}`, `const name = (...) => {...}`, methods. */
export function extractFunctions(content: string, lines: string[]): FunctionShape[] {
  const out: FunctionShape[] = [];
  const re =
    /(?:\b(?:export\s+)?(?:async\s+)?function\s+(\w+)\s*\(([^)]*)\)\s*\{)|(?:\b(?:const|let)\s+(\w+)\s*=\s*(?:async\s*)?\(([^)]*)\)\s*=>\s*\{)|(?:\b(\w+)\s*\(([^)]*)\)\s*\{\s*$)/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(content)) !== null) {
    const name = m[1] ?? m[3] ?? m[5] ?? '(anonymous)';
    const params = (m[2] ?? m[4] ?? m[6] ?? '').trim();
    const braceIdx = content.indexOf('{', m.index);
    if (braceIdx === -1) continue;
    const closeIdx = findMatchingBrace(content, braceIdx);
    if (closeIdx === -1) continue;
    const body = content.slice(braceIdx + 1, closeIdx);
    const line = lineOf(content, m.index, lines);
    // Skip one-line getters/setters and trivial object methods like `foo() {}` only when the
    // body is empty — those are legitimate stubs in interfaces.
    out.push({ name, params, body, line });
  }
  return out;
}

/** 1-based line number of a character offset. */
export function lineOf(content: string, offset: number, lines: string[]): number {
  let line = 1;
  let i = 0;
  const n = Math.min(offset, content.length);
  while (i < n) {
    if (content[i] === '\n') line++;
    i++;
  }
  return line;
}

export interface TopLevelDecl {
  kind: 'function' | 'class' | 'interface' | 'type' | 'const';
  name: string;
  exported: boolean;
  file: string;
  line: number;
}

/**
 * Extract top-level declarations from a code file. Used for cross-file "unreferenced
 * declaration" analysis. Regex-based and deliberately conservative: destructuring,
 * default-export objects and inline IIFEs are not captured.
 */
export function extractDeclarations(file: string, content: string): TopLevelDecl[] {
  const decls: TopLevelDecl[] = [];
  const lines = content.split('\n');

  const re =
    /(?:(?:^|\n)\s*(?:export\s+)?(?:default\s+)?(?:async\s+)?function\s+(\w+))|(?:(?:^|\n)\s*(?:export\s+)?(?:default\s+)?class\s+(\w+))|(?:(?:^|\n)\s*export\s+interface\s+(\w+))|(?:(?:^|\n)\s*export\s+type\s+(\w+)\s*=)|(?:(?:^|\n)\s*export\s+const\s+(\w+)\s*(?::[^=;\n]*)?\s*=)|(?:(?:^|\n)\s*const\s+(\w+)\s*=\s*(?:async\s*)?(?:function|\())/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(content)) !== null) {
    const name = (m[1] ?? m[2] ?? m[3] ?? m[4] ?? m[5] ?? m[6]) as string;
    const exported = m[0].includes('export');
    decls.push({
      kind: (m[1] ? 'function' : m[2] ? 'class' : m[3] ? 'interface' : m[4] ? 'type' : 'const') as TopLevelDecl['kind'],
      name,
      exported,
      file,
      line: lineOf(content, m.index, lines),
    });
  }
  return decls;
}

/** Count occurrences of `name` as a whole word across `files` (excluding string contents when cheap). */
export function countReferences(name: string, files: ScannedFile[]): number {
  const re = new RegExp(`\\b${escapeRegExp(name)}\\b`, 'g');
  let count = 0;
  for (const f of files) {
    // Strip line comments to reduce false positives from prose-like comments. Keep it cheap:
    // this is a heuristic, not a parser.
    const code = f.content.replace(/\/\/[^\n]*/g, ' ');
    const mm = code.match(re);
    if (mm) count += mm.length;
  }
  return count;
}

export function escapeRegExp(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

/** Whole-word occurrence count of `name` inside a single file body. */
export function countInFile(name: string, content: string): number {
  const re = new RegExp(`\\b${escapeRegExp(name)}\\b`, 'g');
  const mm = content.match(re);
  return mm ? mm.length : 0;
}
