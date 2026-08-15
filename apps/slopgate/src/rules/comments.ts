// Cargo-cult comment rules: comments that restate code, add boilerplate, or carry no signal.
import type { Finding, Rule } from '../types.ts';
import { isCodeFile, isProseFile } from '../analysis.ts';

const base = {
  category: 'cargo-cult-comment' as const,
};

const applies = (file: string): boolean => isCodeFile(file);

function normalize(s: string): string {
  return s.toLowerCase().replace(/[^a-z0-9]+/g, ' ');
}

const restatedCode: Rule = {
  id: 'COMM-001',
  name: 'Comment restates the code',
  severity: 'medium',
  description: 'A line comment whose words duplicate the code on the very next line — the comment adds nothing.',
  ...base,
  applies,
  run({ file, lines }) {
    const findings: Finding[] = [];
    for (let i = 0; i < lines.length - 1; i++) {
      const cm = lines[i].match(/^\s*\/\/+\s*(.+?)\s*$/);
      if (!cm) continue;
      const commentWords = normalize(cm[1]).split(' ').filter(Boolean);
      const next = normalize(lines[i + 1].replace(/^\s*\/\/+\s*/, ''));
      if (!next) continue;
      // restatement if every meaningful comment word appears in the next line's tokens
      if (isRestatement(commentWords, next.split(' ').filter(Boolean))) {
        findings.push({
          ruleId: this.id,
          severity: this.severity,
          category: this.category,
          message: `Comment restates the following code: "${cm[1].trim()}".`,
          file,
          line: i + 1,
          evidence: lines[i].trim(),
        });
      }
    }
    return findings;
  },
};

function isRestatement(commentWords: string[], nextWords: string[]): boolean {
  // Restatement = every meaningful comment word also appears in the next code line.
  const meaningful = commentWords.filter((w) => w.length > 2 && !STOP_WORDS.has(w));
  if (meaningful.length < 2) return false;
  return meaningful.every((w) => nextWords.includes(w));
}

const STOP_WORDS = new Set([
  'a', 'an', 'the', 'to', 'of', 'for', 'on', 'with', 'and', 'or', 'in', 'at', 'by',
  'from', 'this', 'that', 'it', 'is', 'are', 'be', 'we', 'our', 'its', 'as', 'into',
]);

const BOILERPLATE_HEADERS: Array<[RegExp, string]> = [
  [/\bDO NOT EDIT\b/i, 'generated-file warning'],
  [/This file (?:was|is) (?:auto-)?generated/i, 'generated-file notice'],
  [/This is a generated file/i, 'generated-file notice'],
  [/\b(?:Copyright|©)\s*\(\s*c\s*\)?\s*\d{4}/i, 'copyright banner'],
  [/Author:\s*<your name>/i, 'placeholder author'],
  [/Created by .{0,40}(?:IDE|editor|scaffold|template)/i, 'tool-created attribution'],
  [/All rights reserved/i, 'legal boilerplate'],
];

const boilerplateHeader: Rule = {
  id: 'COMM-002',
  name: 'Boilerplate header comment',
  severity: 'low',
  description: 'File headers that are pure boilerplate: generated-file notices, bare copyright banners, placeholder authors.',
  ...base,
  applies,
  run({ file, content }) {
    const findings: Finding[] = [];
    const head = content.slice(0, 600);
    for (const [re, what] of BOILERPLATE_HEADERS) {
      const m = head.match(re);
      if (m) {
        findings.push({
          ruleId: this.id,
          severity: this.severity,
          category: this.category,
          message: `Boilerplate header detected: ${what}.`,
          file,
          line: 1,
          evidence: m[0].trim(),
        });
      }
    }
    return findings;
  },
};

const BARE_TODO_WORDS = ['todo', 'fixme', 'hack', 'xxx', 'bug', 'note', 'temp', 'kludge'];

const bareTodo: Rule = {
  id: 'COMM-003',
  name: 'Bare TODO/FIXME without context',
  severity: 'medium',
  description: 'TODO/FIXME/HACK markers with no explanation of what needs doing — cargo-cult markers.',
  ...base,
  applies,
  run({ file, lines }) {
    const findings: Finding[] = [];
    lines.forEach((raw, idx) => {
      const m = raw.match(/^\s*\/\/+\s*(TODO|FIXME|HACK|XXX|BUG)\b[:\s-]*(.*)$/i);
      if (!m) return;
      const detail = m[2].trim();
      const wordCount = detail.split(/\s+/).filter(Boolean).length;
      if (wordCount < 4 || BARE_TODO_WORDS.includes(detail.toLowerCase())) {
        findings.push({
          ruleId: this.id,
          severity: this.severity,
          category: this.category,
          message: `Bare \`${m[1].toUpperCase()}\` marker with no explanation.`,
          file,
          line: idx + 1,
          evidence: raw.trim(),
        });
      }
    });
    return findings;
  },
};

const placeholderComment: Rule = {
  id: 'COMM-004',
  name: 'Placeholder comment',
  severity: 'low',
  description: 'Comments containing placeholder identities (Your Name, John Doe, ACME) left behind by templates.',
  ...base,
  applies,
  run({ file, lines }) {
    const findings: Finding[] = [];
    const re = /(your name|john doe|jane doe|acme|your_?company|xxx|yyy)\b/i;
    lines.forEach((raw, idx) => {
      const cm = raw.match(/^\s*\/\//);
      if (!cm) return;
      const m = raw.match(re);
      if (m) {
        findings.push({
          ruleId: this.id,
          severity: this.severity,
          category: this.category,
          message: `Placeholder text in comment: "${m[1]}".`,
          file,
          line: idx + 1,
          evidence: raw.trim(),
        });
      }
    });
    return findings;
  },
};

const VACUOUS: Array<[RegExp, string]> = [
  [/this is fine/i, 'this is fine'],
  [/works on my machine/i, 'works on my machine'],
  [/nothing to see here/i, 'nothing to see here'],
  [/should be fine/i, 'should be fine'],
  [/do nothing/i, 'do nothing'],
  [/no[- ]?op/i, 'no-op'],
  [/trust me/i, 'trust me'],
  [/it just works/i, 'it just works'],
  [/move along/i, 'move along'],
  [/don'?t (?:touch|look at) this/i, 'don\'t touch this'],
];

const vacuousComment: Rule = {
  id: 'COMM-005',
  name: 'Vacuous comment',
  severity: 'low',
  description: 'Comments with no informational content ("this is fine", "do nothing", "trust me").',
  ...base,
  applies,
  run({ file, lines }) {
    const findings: Finding[] = [];
    lines.forEach((raw, idx) => {
      if (!/^\s*\/\//.test(raw)) return;
      for (const [re, what] of VACUOUS) {
        if (re.test(raw)) {
          findings.push({
            ruleId: this.id,
            severity: this.severity,
            category: this.category,
            message: `Vacuous comment ("${what}") — remove or explain.`,
            file,
            line: idx + 1,
            evidence: raw.trim(),
          });
          break;
        }
      }
    });
    return findings;
  },
};

const commentedOutCode: Rule = {
  id: 'COMM-006',
  name: 'Commented-out code',
  severity: 'medium',
  description: 'Two or more consecutive lines of commented-out code — dead code in the comment graveyard.',
  ...base,
  applies,
  run({ file, lines }) {
    const findings: Finding[] = [];
    const isCodeish = (l: string) =>
      /^\s*\/\/\s*\w/.test(l) && !/^\s*\/\/\s*[a-z\s:,.!?]{1,80}$/i.test(l) || /^\s*\/\/\s*(if|for|while|return|const|let|var|function|class|import|export)\b/i.test(l);
    let run = 0;
    let start = 0;
    for (let i = 0; i <= lines.length; i++) {
      const line = lines[i] ?? '';
      if (isCodeish(line)) {
        if (run === 0) start = i;
        run++;
      } else {
        if (run >= 2) {
          findings.push({
            ruleId: this.id,
            severity: this.severity,
            category: this.category,
            message: `Commented-out code block (${run} lines) — delete it.`,
            file,
            line: start + 1,
            evidence: lines[start].trim(),
          });
        }
        run = 0;
      }
    }
    return findings;
  },
};

export function cargoCultCommentRules(): Rule[] {
  return [restatedCode, boilerplateHeader, bareTodo, placeholderComment, vacuousComment, commentedOutCode];
}

export const proseCommentRules: Rule[] = [];
