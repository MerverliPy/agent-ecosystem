// Pure types + formatting — safe to import from client components (no node builtins).

export interface Hardware {
  cpu: string;
  gpu?: string | null;
  ram_gb?: number | null;
  os?: string;
}

export interface BenchmarkRow {
  model: string;
  hardware: Hardware;
  runtime: string;
  quantization?: string | null;
  tokens_per_sec?: number | null;
  peak_ram_gb?: number | null;
  disk_size_gb?: number | null;
  active_params_b?: number | null;
  quality_delta?: number | null;
  fits?: "fits" | "streams-needed" | "no-fit" | null;
  source_url: string;
  submitted_at: string;
}

export function slugify(s: string): string {
  return s
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

export function fmt(n: number | null | undefined, digits = 2): string {
  if (n == null || !Number.isFinite(n)) return "—";
  return n.toLocaleString(undefined, { maximumFractionDigits: digits });
}
