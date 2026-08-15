// Scoring: aggregate findings into a 0–100 "slop score" where HIGHER = WORSE.
import type { Finding, ScoreBreakdown } from './types.ts';

export const SEVERITY_WEIGHT: Record<Finding['severity'], number> = {
  high: 10,
  medium: 5,
  low: 2,
};

/**
 * Deterministic scoring:
 *   score = min(100, Σ severity weight per finding + density bonus)
 * A repo with zero findings scores 0; a repo saturated with high-severity slop
 * caps at 100. The density bonus (per-file count beyond 5) keeps one giant
 * sloppy file from being drowned out by many small ones.
 */
export function scoreFindings(findings: Finding[]): ScoreBreakdown {
  const byRule: Record<string, { count: number; weight: number }> = {};
  const byFileMap = new Map<string, { file: string; count: number; weight: number }>();

  let total = 0;
  let high = 0;
  let medium = 0;
  let low = 0;

  const perFileCount = new Map<string, number>();

  for (const f of findings) {
    const w = SEVERITY_WEIGHT[f.severity];
    total += w;
    if (f.severity === 'high') high++;
    else if (f.severity === 'medium') medium++;
    else low++;

    const r = byRule[f.ruleId] ?? { count: 0, weight: 0 };
    r.count++;
    r.weight += w;
    byRule[f.ruleId] = r;

    if (f.file) {
      const key = f.file;
      perFileCount.set(key, (perFileCount.get(key) ?? 0) + 1);
      const e = byFileMap.get(key) ?? { file: f.file, count: 0, weight: 0 };
      e.count++;
      e.weight += w;
      byFileMap.set(key, e);
    }
  }

  // Density bonus: files with > 5 findings add 2 points per extra finding (capped).
  for (const [file, count] of perFileCount) {
    if (count > 5) {
      total += Math.min(10, (count - 5) * 2);
    }
  }

  const score = Math.min(100, Math.round(total));
  const byFile = [...byFileMap.values()].sort((a, b) => b.weight - a.weight);

  return {
    score,
    totalFindings: findings.length,
    high,
    medium,
    low,
    byRule,
    byFile,
  };
}

/** CI gate: does the score exceed the threshold? */
export function exceedsThreshold(score: number, threshold: number): boolean {
  return score > threshold;
}
