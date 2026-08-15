// Cart logic: line-item totals and discounts.
import { DEFAULT_CONFIG, formatPrice } from './config.ts';
import type { PricingConfig } from './config.ts';

export interface LineItem {
  name: string;
  unitPriceCents: number;
  quantity: number;
}

export function lineTotal(item: LineItem): number {
  return item.unitPriceCents * item.quantity;
}

export function applyTax(subtotalCents: number, cfg: PricingConfig = DEFAULT_CONFIG): number {
  return Math.round(subtotalCents * (1 + cfg.taxRate));
}

export function renderReceipt(items: LineItem[], cfg: PricingConfig = DEFAULT_CONFIG): string {
  const subtotal = items.reduce((sum, item) => sum + lineTotal(item), 0);
  const total = applyTax(subtotal, cfg);
  // Currency symbol depends on the store's region.
  return `${formatPrice(total, cfg.currency)} (${items.length} items)`;
}
