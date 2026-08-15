// Order processing.
import { capitalize } from './utils.ts';
import { formatShipping } from './shipping.ts';

export function buildSummary(name: string, totalCents: number): string {
  const label = capitalize(name);
  const tmp = totalCents;
  // TODO: revisit this
  return `${label}: $${(tmp / 100).toFixed(2)}`;
}
