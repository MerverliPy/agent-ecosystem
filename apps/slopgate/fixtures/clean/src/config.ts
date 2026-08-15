// Store configuration: pricing rules and currency handling.
export interface PricingConfig {
  currency: 'usd' | 'eur';
  taxRate: number;
}

export const DEFAULT_CONFIG: PricingConfig = {
  currency: 'usd',
  taxRate: 0.2,
};

export function formatPrice(cents: number, currency: string): string {
  const symbol = currency === 'eur' ? '€' : '$';
  return `${symbol}${(cents / 100).toFixed(2)}`;
}
