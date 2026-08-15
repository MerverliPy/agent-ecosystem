// Rule registry — every deterministic rule in the pack.
import type { Finding, Rule, TextRule } from '../types.ts';
import { deadAbstractionRules, deadAbstractionCrossFileRules } from './dead.ts';
import { unusedHelperRules, unusedHelperCrossFileRules } from './unused.ts';
import { cargoCultCommentRules } from './comments.ts';
import { genericNamingRules } from './naming.ts';
import { overEngineeringRules } from './over.ts';
import { commitTextRules } from './commit.ts';
import { aiPhrasingRules } from './ai.ts';
import type { ScannedFile } from '../types.ts';

export { commitTextRules, aiPhrasingRules };

export function allFileRules(): Rule[] {
  return [
    ...deadAbstractionRules(),
    ...unusedHelperRules(),
    ...cargoCultCommentRules(),
    ...genericNamingRules(),
    ...overEngineeringRules(),
  ];
}

export function allTextRules(): TextRule[] {
  return [...commitTextRules(), ...aiPhrasingRules()];
}

export function allCrossFileRules(files: ScannedFile[]): Finding[] {
  return [...deadAbstractionCrossFileRules(files), ...unusedHelperCrossFileRules(files)];
}

export function ruleCount(): number {
  return allFileRules().length + allTextRules().length;
}
