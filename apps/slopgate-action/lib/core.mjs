// SlopGate GitHub Action — core logic (pure, testable without a GitHub runner).
// Uses only Node built-ins: child_process, fs, global fetch. Zero runtime deps.

import { spawnSync } from 'node:child_process';
import { appendFileSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const ACTION_DIR = path.dirname(path.dirname(fileURLToPath(import.meta.url)));

/** The slop CLI entry, resolved relative to this action (works in any checkout). */
export function cliPath() {
  return path.join(ACTION_DIR, '..', 'slopgate', 'src', 'cli.ts');
}

/** Parse GitHub Actions inputs from the environment. */
export function parseInputs(env) {
  const num = (name, dflt) => {
    const v = env[`INPUT_${name}`] ?? dflt;
    const n = Number(v);
    return Number.isFinite(n) ? n : Number(dflt);
  };
  const bool = (name, dflt) => {
    const v = (env[`INPUT_${name}`] ?? String(dflt)).toLowerCase();
    return v === 'true' || v === '1' || v === 'yes';
  };
  return {
    path: env.INPUT_PATH ?? '.',
    threshold: num('THRESHOLD', 50),
    block: bool('BLOCK', true),
    token: env.INPUT_TOKEN ?? env.GITHUB_TOKEN ?? '',
    sarif: bool('SARIF', true),
    comment: bool('COMMENT', true),
    workspace: env.GITHUB_WORKSPACE ?? process.cwd(),
    repository: env.GITHUB_REPOSITORY ?? '',
    eventName: env.GITHUB_EVENT_NAME ?? '',
    eventPath: env.GITHUB_EVENT_PATH ?? '',
    stepSummary: env.GITHUB_STEP_SUMMARY ?? '',
    sha: env.GITHUB_SHA ?? '',
  };
}

/** Run `slop <cmd> <path> --json` and return the parsed result (throws on CLI failure). */
export function runSlop(cmd, target, extraArgs = []) {
  const res = spawnSync(
    process.execPath,
    ['--experimental-strip-types', cliPath(), cmd, target, '--json', ...extraArgs],
    { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 }
  );
  if (res.status !== 0) {
    throw new Error(`slop ${cmd} failed (exit ${res.status}): ${res.stderr || res.stdout}`);
  }
  return JSON.parse(res.stdout);
}

/** CI gate decision: block when the score exceeds the threshold and block is enabled. */
export function decideGate(score, threshold, block) {
  const failed = score > threshold;
  return { failed, block, status: failed ? (block ? 'fail' : 'warn') : 'pass' };
}

/** Markdown comment body for a PR. */
export function buildCommentBody(score, breakdown, gate, sha) {
  const icon = gate.status === 'fail' ? '❌' : gate.status === 'warn' ? '⚠️' : '✅';
  const lines = [
    `## SlopGate report ${icon}`,
    '',
    `| Metric | Value |`,
    `|---|---|`,
    `| Slop score | **${score}/100** (threshold ${gate.threshold}) |`,
    `| Findings | ${breakdown.totalFindings} (high ${breakdown.high} / medium ${breakdown.medium} / low ${breakdown.low}) |`,
    `| Gate | **${gate.status.toUpperCase()}** |`,
    '',
  ];
  const topRules = Object.entries(breakdown.byRule ?? {})
    .sort((a, b) => b[1].weight - a[1].weight)
    .slice(0, 8);
  if (topRules.length > 0) {
    lines.push('Top rules:', '');
    lines.push('| Rule | Count | Weight |', '|---|---|---|');
    for (const [id, { count, weight }] of topRules) lines.push(`| ${id} | ${count} | ${weight} |`);
    lines.push('');
  }
  if (sha) lines.push(`_scan of \`${sha.slice(0, 7)}\`_`);
  return lines.join('\n');
}

/** Step-summary markdown. */
export function buildSummary(score, breakdown, gate) {
  return [
    `### SlopGate — score **${score}/100** (threshold ${gate.threshold})`,
    `Gate: **${gate.status.toUpperCase()}** · ${breakdown.totalFindings} findings (${breakdown.high} high / ${breakdown.medium} medium / ${breakdown.low} low)`,
    '',
  ].join('\n');
}

/** Post a PR comment via the GitHub REST API (uses global fetch). Non-fatal on error. */
export async function postComment({ repository, token, issueNumber, body }) {
  if (!repository || !token || !issueNumber) {
    return { posted: false, reason: 'no repository/token/issueNumber' };
  }
  const url = `https://api.github.com/repos/${repository}/issues/${issueNumber}/comments`;
  const res = await fetch(url, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${token}`,
      'Content-Type': 'application/json',
      'User-Agent': 'slopgate-action',
      'X-GitHub-Api-Version': '2022-11-28',
    },
    body: JSON.stringify({ body }),
  });
  if (!res.ok) {
    return { posted: false, reason: `HTTP ${res.status}` };
  }
  return { posted: true };
}

/** Extract the PR number from the event payload if this is a pull_request event. */
export function pullRequestNumber(eventName, eventPath) {
  if (eventName !== 'pull_request' || !eventPath) return null;
  try {
    const payload = JSON.parse(readFileSync(eventPath, 'utf8'));
    return payload?.pull_request?.number ?? null;
  } catch {
    return null;
  }
}

/** Write the step summary file (GITHUB_STEP_SUMMARY). */
export function writeStepSummary(file, text) {
  if (!file) return false;
  appendFileSync(file, text);
  return true;
}
