// LLM review layer — bring-your-own-key. Scores prose/commit/PR-description slop with a
// pattern catalog, cross-checked by an LLM over an OpenAI-compatible chat completions API.
// Deterministic core always works; this layer is disabled when no key is present (DEC-0005).
import type { Finding, LlmReviewResult } from './types.ts';
import { aiPhrasingRules, commitTextRules } from './rules/index.ts';

export interface LlmConfig {
  key: string;
  url: string;
  model: string;
}

export function llmConfig(): LlmConfig | null {
  const key = process.env.SLOPGATE_LLM_KEY ?? process.env.OPENAI_API_KEY ?? '';
  if (!key) return null;
  return {
    key,
    url: process.env.SLOPGATE_LLM_URL ?? 'https://api.openai.com/v1/chat/completions',
    model: process.env.SLOPGATE_LLM_MODEL ?? 'gpt-4o-mini',
  };
}

/** The deterministic pattern catalog (shared with the text rules). */
export function patternCatalog(): Array<{ id: string; description: string }> {
  return [
    ...commitTextRules().map((r) => ({ id: r.id, description: r.description })),
    ...aiPhrasingRules().map((r) => ({ id: r.id, description: r.description })),
  ];
}

export interface ProseItem {
  source: string;
  text: string;
}

/**
 * Run the deterministic catalog over prose items (always available, no key needed).
 * Returns the same Finding shape the rest of the pipeline uses.
 */
export function catalogReview(items: ProseItem[]): Finding[] {
  const textRules = [...commitTextRules(), ...aiPhrasingRules()];
  const findings: Finding[] = [];
  for (const item of items) {
    for (const rule of textRules) {
      findings.push(...rule.check(item.text, item.source));
    }
  }
  return findings;
}

type FetchLike = (url: string, init: unknown) => Promise<{ ok: boolean; status: number; json(): Promise<unknown> }>;

/**
 * Ask the configured model to score each prose item 0–100 for slop and name the slop
 * category. `fetchImpl` is injectable for tests. Returns `enabled:false` when no key.
 */
export async function reviewProseWithLlm(
  items: ProseItem[],
  cfg: LlmConfig,
  fetchImpl: FetchLike = fetch as unknown as FetchLike
): Promise<LlmReviewResult> {
  if (items.length === 0) {
    return { enabled: true, reviews: [] };
  }
  const catalog = patternCatalog()
    .map((r) => `${r.id}: ${r.description}`)
    .join('\n');

  const system =
    'You are SlopGate, a strict reviewer that detects AI-slop in prose (commit messages, PR descriptions, docs). ' +
    'You score each input text 0-100 where higher means more sloppy/AI-generated boilerplate. ' +
    'Respond with ONLY a JSON array, one object per input, in order, with fields: ' +
    '{"source": string, "slopScore": number 0-100, "slopType": string, "notes": string[]}. ' +
    'Use the pattern catalog to anchor your judgement:\n' + catalog;

  const user = JSON.stringify(
    items.map((it) => ({ source: it.source, text: it.text.slice(0, 4000) }))
  );

  const res = await fetchImpl(cfg.url, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${cfg.key}`,
    },
    body: JSON.stringify({
      model: cfg.model,
      temperature: 0,
      response_format: { type: 'json_object' },
      messages: [
        { role: 'system', content: system },
        { role: 'user', content: user },
      ],
    }),
  });

  if (!res.ok) {
    throw new Error(`LLM review failed: HTTP ${res.status}`);
  }

  const data = (await res.json()) as {
    choices?: Array<{ message?: { content?: string } }>;
  };
  const raw = data.choices?.[0]?.message?.content ?? '';
  const parsed = parseLlmJson(raw);
  return { enabled: true, reviews: parsed };
}

/** Parse the model's JSON reply robustly: accept array or {reviews:[...]}, strip markdown fences. */
export function parseLlmJson(raw: string): LlmReviewResult['reviews'] {
  let text = raw.trim();
  text = text.replace(/^```(?:json)?\s*/i, '').replace(/```$/, '').trim();
  let arr: Array<{ source?: string; slopScore?: number; slopType?: string; notes?: string[] }> = [];
  try {
    const parsed = JSON.parse(text) as unknown;
    if (Array.isArray(parsed)) arr = parsed as typeof arr;
    else if (parsed && typeof parsed === 'object' && Array.isArray((parsed as { reviews?: unknown }).reviews)) {
      arr = (parsed as { reviews: typeof arr }).reviews;
    }
  } catch {
    // fall through to the markdown-list heuristic below
    const m = text.match(/\[[\s\S]*\]/);
    if (m) {
      try {
        const parsed = JSON.parse(m[0]) as unknown;
        if (Array.isArray(parsed)) arr = parsed as typeof arr;
      } catch {
        arr = [];
      }
    }
  }
  return arr
    .filter((r) => typeof r?.source === 'string')
    .map((r) => ({
      source: r.source as string,
      slopScore: typeof r.slopScore === 'number' ? Math.max(0, Math.min(100, r.slopScore)) : 0,
      slopType: typeof r.slopType === 'string' ? r.slopType : 'unknown',
      notes: Array.isArray(r.notes) ? r.notes.map(String) : [],
    }));
}

/** Merge deterministic catalog findings with LLM reviews into a combined report. */
export function mergeReviews(catalog: Finding[], llm: LlmReviewResult): {
  catalog: Finding[];
  llm: LlmReviewResult;
  combinedScore: number;
} {
  const llmAvg =
    llm.reviews.length > 0
      ? llm.reviews.reduce((a, r) => a + r.slopScore, 0) / llm.reviews.length
      : 0;
  // Catalog weight: up to 100 from high(10)/med(5)/low(2); blend 60/40 catalog/LLM.
  const catalogScore = Math.min(100, catalog.reduce((a, f) => a + (f.severity === 'high' ? 10 : f.severity === 'medium' ? 5 : 2), 0));
  const combinedScore = llm.enabled ? Math.round(catalogScore * 0.6 + llmAvg * 0.4) : catalogScore;
  return { catalog, llm, combinedScore };
}
