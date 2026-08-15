// SlopGate GitHub Action entry point.
// Runs `slop scan` + `slop score`, posts a PR comment, writes the step summary,
// emits slopgate.sarif, and fails the job when the score exceeds `threshold` and
// `block` is true.
import {
  parseInputs,
  runSlop,
  decideGate,
  buildCommentBody,
  buildSummary,
  postComment,
  pullRequestNumber,
  writeStepSummary,
  cliPath,
} from './lib/core.mjs';
import { spawnSync } from 'node:child_process';
import path from 'node:path';

async function main() {
  const inputs = parseInputs(process.env);
  console.log(`SlopGate: scanning ${inputs.path} (threshold ${inputs.threshold}, block ${inputs.block})`);

  const scan = runSlop('scan', inputs.path);
  const scoreResult = runSlop('score', inputs.path);

  const gate = decideGate(scoreResult.score, inputs.threshold, inputs.block);

  if (inputs.sarif) {
    const sarifPath = path.join(inputs.workspace, 'slopgate.sarif');
    const res = spawnSync(
      process.execPath,
      ['--experimental-strip-types', cliPath(), 'scan', inputs.path, '--sarif', sarifPath],
      { encoding: 'utf8' }
    );
    if (res.status === 0) {
      console.log(`SARIF written to ${sarifPath}`);
    } else {
      console.warn(`SARIF write failed (exit ${res.status})`);
    }
  }

  const issueNumber = pullRequestNumber(inputs.eventName, inputs.eventPath);
  if (inputs.comment && issueNumber) {
    const body = buildCommentBody(scoreResult.score, scoreResult, gate, inputs.sha);
    try {
      const res = await postComment({
        repository: inputs.repository,
        token: inputs.token,
        issueNumber,
        body,
      });
      if (res.posted) console.log(`Commented on PR #${issueNumber}`);
      else console.warn(`PR comment skipped: ${res.reason}`);
    } catch (err) {
      console.warn(`PR comment failed: ${err instanceof Error ? err.message : err}`);
    }
  }

  const summary = buildSummary(scoreResult.score, scoreResult, gate);
  if (writeStepSummary(inputs.stepSummary, summary)) {
    console.log('Step summary written.');
  }

  console.log(`Slop score: ${scoreResult.score}/100 (threshold ${inputs.threshold})`);
  console.log(`Gate: ${gate.status.toUpperCase()} (${scan.findings.length} findings)`);

  if (gate.status === 'fail') {
    console.error(`SlopGate gate FAIL: score ${scoreResult.score} exceeds threshold ${inputs.threshold}`);
    process.exitCode = 1;
  }
}

main().catch((err) => {
  console.error(`SlopGate action failed: ${err instanceof Error ? err.message : err}`);
  process.exitCode = 2;
});
