// Generic helpers file.
export function formatPrice(cents: number): string {
  return `$${(cents / 100).toFixed(2)}`;
}

// Converts the value to a string.  // restates the code? no: value.toString
export function unusedExportedHelper(value: string): string {
  return value.toString();
}

export function addTwo(n: number): number {
  return n + 2;
}
