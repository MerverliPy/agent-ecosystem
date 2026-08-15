// Shared types for the SlopGate rule pack, scanner, scorer and CLI.

export type Severity = 'low' | 'medium' | 'high';

export type FindingCategory =
  | 'dead-abstraction'
  | 'unused-helper'
  | 'cargo-cult-comment'
  | 'generic-naming'
  | 'over-engineering'
  | 'boilerplate-text'
  | 'ai-phrasing';

export interface Finding {
  ruleId: string;
  severity: Severity;
  category: FindingCategory;
  message: string;
  /** Absolute or relative path of the offending file (absent for pure-text findings). */
  file?: string;
  /** 1-based line number. */
  line?: number;
  /** Matched snippet. */
  evidence?: string;
}

/** A rule applied to whole files (TS/JS, prose). */
export interface Rule {
  id: string;
  name: string;
  category: FindingCategory;
  severity: Severity;
  description: string;
  /** Whether the rule applies to a file (by path/extension). */
  applies(file: string): boolean;
  run(ctx: RuleContext): Finding[];
}

export interface RuleContext {
  /** Path of the file being scanned (as given to the scanner). */
  file: string;
  content: string;
  lines: string[];
}

/** A rule applied to free text (commit messages, PR descriptions, prose). */
export interface TextRule {
  id: string;
  name: string;
  category: FindingCategory;
  severity: Severity;
  description: string;
  /** Return findings for `text`, tagged with `source`. */
  check(text: string, source: string): Finding[];
}

export interface ScannedFile {
  path: string;
  content: string;
}

export interface ScanResult {
  findings: Finding[];
  filesScanned: number;
  filesSkipped: string[];
  scannedAt: string;
}

export interface ScoreBreakdown {
  score: number; // 0–100, higher = more slop
  totalFindings: number;
  high: number;
  medium: number;
  low: number;
  byRule: Record<string, { count: number; weight: number }>;
  byFile: Array<{ file: string; count: number; weight: number }>;
}

export interface LlmReviewResult {
  enabled: boolean;
  reason?: string;
  reviews: Array<{
    source: string;
    slopScore: number; // 0–100
    slopType: string;
    notes: string[];
  }>;
}
