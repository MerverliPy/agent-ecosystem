// A grab-bag of small helpers.
export function double(value: number): number {
  return value * 2;
}

export function capitalize(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}

// This helper was written for an earlier feature and is no longer called.
export function legacyHelper(value: number): string {
  return `legacy:${value}`;
}
