// Report formatting: human text, JSON and SARIF 2.1.0 output.
import type { Finding, ScoreBreakdown } from './types.ts';

export function formatTextFindings(findings: Finding[]): string {
  if (findings.length === 0) return 'No slop findings. 🎉';
  const sorted = [...findings].sort((a, b) => (a.file ?? '').localeCompare(b.file ?? ''));
  const lines: string[] = [];
  for (const f of sorted) {
    const loc = f.file ? `${f.file}${f.line ? `:${f.line}` : ''}` : f.file ?? '(text)';
    const sev = f.severity.toUpperCase().padEnd(6);
    lines.push(`[${sev}] ${loc}  ${f.ruleId} — ${f.message}`);
    if (f.evidence && f.evidence !== f.message) {
      lines.push(`        │ ${f.evidence.replace(/\n/g, ' ').slice(0, 140)}`);
    }
  }
  return lines.join('\n');
}

export function formatScore(b: ScoreBreakdown, threshold?: number): string {
  const lines: string[] = [];
  lines.push(`Slop score: ${b.score}/100${threshold !== undefined ? ` (threshold ${threshold})` : ''}`);
  lines.push(`Findings: ${b.totalFindings} (high ${b.high} / medium ${b.medium} / low ${b.low})`);
  const rules = Object.entries(b.byRule).sort((a, b2) => b2[1].weight - a[1].weight);
  if (rules.length > 0) {
    lines.push('Per-rule breakdown:');
    for (const [id, { count, weight }] of rules) {
      lines.push(`  ${id.padEnd(12)} ${String(count).padStart(3)}×  weight ${weight}`);
    }
  }
  if (b.byFile.length > 0) {
    lines.push('Worst files:');
    for (const { file, count, weight } of b.byFile.slice(0, 8)) {
      lines.push(`  ${weight.toString().padStart(3)}  ${file} (${count} findings)`);
    }
  }
  return lines.join('\n');
}

/** Write a SARIF 2.1.0 document from findings. */
export function buildSarif(findings: Finding[], toolVersion: string): Record<string, unknown> {
  const severityToLevel: Record<Finding['severity'], string> = {
    high: 'error',
    medium: 'warning',
    low: 'note',
  };
  return {
    $schema: 'https://json.schemastore.org/sarif-2.1.0.json',
    version: '2.1.0',
    runs: [
      {
        tool: {
          driver: {
            name: 'slopgate',
            informationUri: 'https://github.com/MerverliPy/agent-ecosystem/tree/main/apps/slopgate',
            version: toolVersion,
            rules: [...new Set(findings.map((f) => f.ruleId))].map((id) => ({
              id,
              shortDescription: { text: `slopgate rule ${id}` },
            })),
          },
        },
        results: findings.map((f) => ({
          ruleId: f.ruleId,
          level: severityToLevel[f.severity],
          message: { text: f.message },
          ...(f.file
            ? {
                locations: [
                  {
                    physicalLocation: {
                      artifactLocation: { uri: f.file },
                      ...(f.line ? { region: { startLine: f.line } } : {}),
                    },
                  },
                ],
              }
            : {}),
        })),
      },
    ],
  };
}
