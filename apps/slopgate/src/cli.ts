#!/usr/bin/env node
// SlopGate CLI — scan, score, lint, check-text, llm-review.
import path from 'node:path';
import { writeFileSync } from 'node:fs';
import { scanTree, runTextRules } from './scanner.ts';
import { scoreFindings, exceedsThreshold } from './score.ts';
import { buildSarif, formatScore, formatTextFindings } from './report.ts';
import { allFileRules, allTextRules, ruleCount } from './rules/index.ts';
import { catalogReview, llmConfig, mergeReviews, reviewProseWithLlm, type ProseItem } from './llm.ts';
import type { Finding } from './types.ts';

export const VERSION = '0.1.0';

const USAGE = `slop v${VERSION} — deterministic AI-slop detector

Usage:
  slop scan <path> [--json] [--sarif FILE] [--no-prose] [--max-findings N]
      Run the full rule pack over a directory tree and print findings.

  slop score <path> [--json] [--threshold N]
      Score 0-100 (higher = more slop) with per-rule breakdown.

  slop lint <path> [--threshold N] [--block|--no-block] [--commit-msg TEXT] [--json]
      CI entry point. Exit 0 when score <= threshold, 1 when above (with --block).

  slop check-text --text TEXT [--source LABEL] [--json]
      Apply commit/AI text rules to a commit message, PR body or prose blob.

  slop llm-review --text TEXT [--source LABEL] [--json] | --file PATH
      LLM review layer (bring-your-own-key). Deterministic catalog runs always;
      the LLM pass is skipped when SLOPGATE_LLM_KEY / OPENAI_API_KEY is unset.

  slop rules        List every rule in the pack.
  slop version      Print version.
  slop help         This help.

Env:
  SLOPGATE_LLM_KEY / OPENAI_API_KEY   key for llm-review (optional)
  SLOPGATE_LLM_URL                    OpenAI-compatible endpoint (default api.openai.com/v1/chat/completions)
  SLOPGATE_LLM_MODEL                  model id (default gpt-4o-mini)
`;

function argValue(args: string[], flag: string): string | undefined {
  const i = args.indexOf(flag);
  if (i === -1) return undefined;
  return args[i + 1];
}

function hasFlag(args: string[], flag: string): boolean {
  return args.includes(flag);
}

function printJson(obj: unknown): void {
  process.stdout.write(JSON.stringify(obj, null, 2) + '\n');
}

async function main(argv: string[]): Promise<number> {
  const [cmd, ...rest] = argv;

  if (cmd === 'version' || cmd === '--version' || cmd === '-v') {
    console.log(VERSION);
    return 0;
  }
  if (cmd === 'help' || cmd === '--help' || cmd === '-h' || cmd === undefined) {
    console.log(USAGE);
    return cmd === undefined ? 1 : 0;
  }

  if (cmd === 'rules') {
    const fileRules = allFileRules();
    const textRules = allTextRules();
    console.log(`SlopGate rule pack — ${ruleCount()} rules (${fileRules.length} file rules + ${textRules.length} text rules)\n`);
    for (const r of fileRules) console.log(`${r.id.padEnd(12)} ${r.severity.toUpperCase().padEnd(6)} ${r.name}`);
    for (const r of textRules) console.log(`${r.id.padEnd(12)} ${r.severity.toUpperCase().padEnd(6)} ${r.name}`);
    return 0;
  }

  if (cmd === 'scan') {
    const target = rest[0] ?? '.';
    const result = scanTree(path.resolve(target), {
      prose: !hasFlag(rest, '--no-prose'),
      maxFindingsPerFile: Number(argValue(rest, '--max-findings') ?? 0),
    });
    if (hasFlag(rest, '--json')) {
      printJson({ ...result, ruleCount: ruleCount() });
    } else {
      console.log(`Scanned ${result.filesScanned} files (${result.filesSkipped.length} skipped).`);
      console.log(formatTextFindings(result.findings));
    }
    const sarif = argValue(rest, '--sarif');
    if (sarif) {
      writeFileSync(sarif, JSON.stringify(buildSarif(result.findings, VERSION), null, 2) + '\n');
      if (!hasFlag(rest, '--json')) console.log(`\nSARIF written to ${sarif}`);
    }
    return 0;
  }

  if (cmd === 'score') {
    const target = rest[0] ?? '.';
    const result = scanTree(path.resolve(target), { prose: !hasFlag(rest, '--no-prose') });
    const breakdown = scoreFindings(result.findings);
    if (hasFlag(rest, '--json')) {
      printJson({ ...breakdown, filesScanned: result.filesScanned, scannedAt: result.scannedAt });
    } else {
      console.log(formatScore(breakdown));
    }
    return 0;
  }

  if (cmd === 'lint') {
    const target = rest[0] ?? '.';
    const threshold = Number(argValue(rest, '--threshold') ?? 50);
    const block = hasFlag(rest, '--block') ? true : hasFlag(rest, '--no-block') ? false : true;
    const commitMsg = argValue(rest, '--commit-msg');

    const result = scanTree(path.resolve(target), { prose: !hasFlag(rest, '--no-prose') });
    const findings: Finding[] = [...result.findings];
    if (commitMsg !== undefined) {
      findings.push(...runTextRules(commitMsg, 'commit-msg', allTextRules()));
    }
    const breakdown = scoreFindings(findings);
    const above = exceedsThreshold(breakdown.score, threshold);

    if (hasFlag(rest, '--json')) {
      printJson({
        ...breakdown,
        threshold,
        block,
        gate: above ? 'fail' : 'pass',
      });
    } else {
      console.log(formatScore(breakdown, threshold));
      console.log(above ? `\n✗ Gate FAIL — score ${breakdown.score} exceeds threshold ${threshold}` : `\n✓ Gate PASS — score ${breakdown.score} <= threshold ${threshold}`);
      if (findings.length > 0) console.log('\n' + formatTextFindings(findings));
    }
    return block && above ? 1 : 0;
  }

  if (cmd === 'check-text') {
    const text = argValue(rest, '--text');
    if (text === undefined) {
      console.error('check-text requires --text "<content>"');
      return 2;
    }
    const source = argValue(rest, '--source') ?? 'text';
    const findings = runTextRules(text, source, allTextRules());
    if (hasFlag(rest, '--json')) {
      printJson({ source, findings, count: findings.length });
    } else {
      console.log(formatTextFindings(findings));
    }
    return findings.length > 0 ? 1 : 0;
  }

  if (cmd === 'llm-review') {
    const text = argValue(rest, '--text');
    const file = argValue(rest, '--file');
    const source = argValue(rest, '--source') ?? 'text';
    let items: ProseItem[];
    if (file) {
      const content = (await import('node:fs')).readFileSync(path.resolve(file), 'utf8');
      items = [{ source: file, text: content }];
    } else if (text !== undefined) {
      items = [{ source, text }];
    } else {
      console.error('llm-review requires --text "<content>" or --file <path>');
      return 2;
    }

    const catalog = catalogReview(items);
    const cfg = llmConfig();
    if (!cfg) {
      if (hasFlag(rest, '--json')) {
        printJson({
          enabled: false,
          reason: 'No LLM key (SLOPGATE_LLM_KEY / OPENAI_API_KEY unset) — deterministic catalog only (DEC-0005).',
          catalog,
          combinedScore: mergeReviews(catalog, { enabled: false, reviews: [] }).combinedScore,
        });
      } else {
        console.log('LLM review disabled: no SLOPGATE_LLM_KEY / OPENAI_API_KEY set (DEC-0005 — deterministic core only).');
        console.log(formatTextFindings(catalog));
      }
      return 0;
    }

    let llm;
    try {
      llm = await reviewProseWithLlm(items, cfg);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error(`LLM review failed: ${msg}`);
      if (hasFlag(rest, '--json')) {
        printJson({ enabled: true, error: msg, catalog, combinedScore: mergeReviews(catalog, { enabled: false, reviews: [] }).combinedScore });
      }
      return 1;
    }
    const merged = mergeReviews(catalog, llm);
    if (hasFlag(rest, '--json')) {
      printJson({ enabled: true, ...merged });
    } else {
      console.log(`Combined slop score (catalog + LLM): ${merged.combinedScore}/100`);
      for (const r of llm.reviews) {
        console.log(`  [${r.source}] LLM: ${r.slopScore}/100 (${r.slopType})${r.notes.length ? ' — ' + r.notes.join('; ') : ''}`);
      }
      console.log(formatTextFindings(catalog));
    }
    return 0;
  }

  console.error(`Unknown command: ${cmd}\n${USAGE}`);
  return 2;
}

// Allow import for tests; run when executed directly.
if (import.meta.url === `file://${process.argv[1]}` || process.argv[1]?.endsWith('cli.ts')) {
  main(process.argv.slice(2)).then((code) => {
    process.exitCode = code;
  });
}
