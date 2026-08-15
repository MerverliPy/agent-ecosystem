// Shipping cost estimation for the store.
export function formatShipping(zip: string, weightGrams: number): string {
  const fee = weightGrams > 1000 ? 7.99 : 4.99;
  return `${fee.toFixed(2)} (${zip})`;
}
