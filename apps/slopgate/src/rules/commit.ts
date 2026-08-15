// Boilerplate-text rules: generic commit messages and PR descriptions that carry no information.
import type { Finding, TextRule } from '../types.ts';

const category = 'boilerplate-text' as const;

function norm(text: string): string {
  return text.toLowerCase().replace(/[^a-z0-9\s]/g, ' ').replace(/\s+/g, ' ').trim();
}

function finding(ruleId: string, severity: Finding['severity'], message: string, text: string, source: string, evidence?: string): Finding {
  return { ruleId, severity, category, message, evidence: evidence ?? text.trim().slice(0, 120) };
}

interface TextRuleSpec {
  id: string;
  name: string;
  severity: Finding['severity'];
  description: string;
  test: (n: string) => boolean;
  message: (n: string) => string;
}

const SPECS: TextRuleSpec[] = [
  {
    id: 'COMMIT-001',
    name: 'Bare "fix typo"',
    severity: 'medium',
    description: 'A commit that only says "fix typo" — no scope, no what/why.',
    test: (n) => n === 'fix typo' || n === 'typo' || n === 'fixed typo' || n === 'typo fix',
    message: () => 'Bare "fix typo" commit — say what was fixed and where.',
  },
  {
    id: 'COMMIT-002',
    name: 'Bare "update README"',
    severity: 'low',
    description: 'A commit that only says it touched the README.',
    test: (n) => n === 'update readme' || n === 'updated readme' || n === 'readme' || n === 'update readme.md',
    message: () => 'Bare "update README" — describe what changed in the docs.',
  },
  {
    id: 'COMMIT-003',
    name: 'Vague "minor/small"',
    severity: 'low',
    description: '"minor changes", "small fix" — adjectives instead of content.',
    test: (n) =>
      /^(minor|small|tiny|slight|little|quick)\b/.test(n) &&
      n.split(' ').length <= 4,
    message: () => '"minor"/"small" commit with no substance — what changed?',
  },
  {
    id: 'COMMIT-004',
    name: 'WIP-only message',
    severity: 'medium',
    description: 'A commit message that is only "wip" / "work in progress".',
    test: (n) => n === 'wip' || n === 'work in progress' || n === 'in progress' || /^wip\b/.test(n) && n.split(' ').length <= 2,
    message: () => 'WIP-only commit — split into a meaningful unit or say what is in progress.',
  },
  {
    id: 'COMMIT-005',
    name: 'Bare "refactor"',
    severity: 'medium',
    description: '"refactor" with no description of what was restructured or why.',
    test: (n) => n === 'refactor' || n === 'refactoring' || n === 'refactored' || /^refactor\b/.test(n) && n.split(' ').length <= 3,
    message: () => 'Bare "refactor" — say what was restructured and why.',
  },
  {
    id: 'COMMIT-006',
    name: '"initial commit" / add boilerplate',
    severity: 'low',
    description: 'Initial-commit boilerplate with no inventory of what the commit introduces.',
    test: (n) => n === 'initial commit' || n === 'add files' || n === 'add stuff' || n === 'first commit' || n === 'init',
    message: () => 'Initial/add boilerplate — enumerate what is being introduced.',
  },
  {
    id: 'COMMIT-007',
    name: 'Empty or punctuation-only message',
    severity: 'high',
    description: 'An empty commit message or one that is only punctuation.',
    test: (n) => n.length === 0 || n.replace(/[^a-z0-9]/g, '').length === 0,
    message: () => 'Empty or punctuation-only commit message.',
  },
  {
    id: 'COMMIT-008',
    name: 'Bare "cleanup"',
    severity: 'low',
    description: '"cleanup" / "tidy" without saying what was cleaned.',
    test: (n) => n === 'cleanup' || n === 'clean up' || n === 'tidy' || n === 'tidy up' || n === 'clean-up',
    message: () => 'Bare "cleanup" — what was cleaned and why?',
  },
  {
    id: 'COMMIT-009',
    name: '"stuff" as content',
    severity: 'medium',
    description: 'Messages whose only substantive word is "stuff" or "things".',
    test: (n) => /\b(stuff|things|thing|misc|miscellaneous|various)\b/.test(n) && n.split(' ').length <= 5,
    message: () => 'Vague commit content ("stuff") — be specific.',
  },
  {
    id: 'COMMIT-010',
    name: 'Dismissive placeholder',
    severity: 'low',
    description: '"no message", "n/a", "-", "." as a commit message.',
    test: (n) => n === 'no message' || n === 'na' || n === 'n a' || n === '-' || n === '.' || n === '...',
    message: () => 'Dismissive placeholder commit message.',
  },
  {
    id: 'COMMIT-011',
    name: 'Missing PR description',
    severity: 'medium',
    description: 'A PR description that is empty or contains only issue references.',
    test: (n) => {
      const stripped = n.replace(/fixes?\s+#\d+/gi, '').replace(/#\d+/g, '').trim();
      return stripped.length < 10;
    },
    message: () => 'PR description is empty or only references issue numbers — describe the change.',
  },
];

export function commitTextRules(): TextRule[] {
  return SPECS.map((s) => ({
    id: s.id,
    name: s.name,
    category,
    severity: s.severity,
    description: s.description,
    check(text: string, source: string): Finding[] {
      const n = norm(text);
      if (s.test(n)) {
        return [finding(s.id, s.severity, s.message(n), text, source)];
      }
      return [];
    },
  }));
}
