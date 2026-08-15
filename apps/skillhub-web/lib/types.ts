// Pure types + formatting — safe to import from client components (no node builtins).

export interface SkillVersion {
  version: string;
  published_at: string;
  verified: boolean;
  harnesses: string[];
  permissions: string[];
}

export interface SkillPackage {
  name: string;
  description: string;
  license: string;
  repo: string;
  verified: boolean;
  high_risk: boolean;
  downloads: number;
  versions: SkillVersion[];
  /** SlopGate quality score 0-100 (higher = worse); present when the snapshot ran the scanner. */
  quality_score?: number;
}

export interface Snapshot {
  updated_at: string;
  packages: SkillPackage[];
}

export function slugify(s: string): string {
  return s
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

export function fmt(n: number): string {
  return n.toLocaleString();
}
