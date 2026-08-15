#!/usr/bin/env node
// SlopGate CLI launcher — spawns node with type-stripping so the TypeScript
// sources run directly (zero build step, zero runtime deps).
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const here = path.dirname(fileURLToPath(import.meta.url));
const cli = path.join(here, '..', 'src', 'cli.ts');
const res = spawnSync(
  process.execPath,
  ['--experimental-strip-types', cli, ...process.argv.slice(2)],
  { stdio: 'inherit' }
);
process.exit(res.status === null ? 1 : res.status);
