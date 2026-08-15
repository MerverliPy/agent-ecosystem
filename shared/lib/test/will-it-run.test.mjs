// Calculator edge-case tests. Run: node --test shared/lib/test/
import { test } from "node:test";
import assert from "node:assert/strict";
import { estimate } from "../will-it-run.mjs";

test("dense model fits comfortably", () => {
  const r = estimate(
    { name: "gemma-26b", totalParamsB: 26, activeParamsB: 26, bitsPerWeight: 4 },
    { ramGb: 32, contextTokens: 4096 }
  );
  assert.equal(r.verdict, "fits");
  // weights = 26e9 * 0.5 = 13 GB; ramNeeded = 13 + ~9.8 + 1.5
  assert.ok(r.ramNeededGb > 13 && r.ramNeededGb < 26);
});

test("MoE active routing scales the speed estimate with ACTIVE params", () => {
  const moe = estimate(
    { name: "gemma4-26b-a4b", totalParamsB: 26, activeParamsB: 4, bitsPerWeight: 4 },
    { ramGb: 32, contextTokens: 2048 }
  );
  assert.equal(moe.verdict, "fits"); // 13 GB weights + ~4.9 GB KV + 1.5 GB overhead < 32
  const dense = estimate(
    { name: "dense-26b", totalParamsB: 26, activeParamsB: 26, bitsPerWeight: 4 },
    { ramGb: 32, contextTokens: 2048 }
  );
  assert.ok(moe.estTokensPerSec > dense.estTokensPerSec * 5); // ~6.5x fewer decode bytes
});

test("Kimi K3 needs streaming on consumer RAM", () => {
  const r = estimate(
    { name: "kimi-k3", totalParamsB: 2780, activeParamsB: 104, bitsPerWeight: 4 },
    { ramGb: 64, streamingSupported: true, contextTokens: 4096 }
  );
  assert.equal(r.verdict, "streams-needed"); // ~1390 GB weights cannot fit in 64 GB
  assert.ok(r.weightsGb > 1300 && r.weightsGb < 1450);
});

test("Kimi K3 without streaming support is no-fit", () => {
  const r = estimate(
    { name: "kimi-k3", totalParamsB: 2780, activeParamsB: 104, bitsPerWeight: 4 },
    { ramGb: 128, streamingSupported: false }
  );
  assert.equal(r.verdict, "no-fit");
});

test("KDA constant KV cache does not grow with context", () => {
  const small = estimate(
    { name: "kimi-k3", totalParamsB: 2780, activeParamsB: 104, bitsPerWeight: 4, constantKvBytesGb: 0.626 },
    { ramGb: 64, streamingSupported: true, contextTokens: 10 }
  );
  const large = estimate(
    { name: "kimi-k3", totalParamsB: 2780, activeParamsB: 104, bitsPerWeight: 4, constantKvBytesGb: 0.626 },
    { ramGb: 64, streamingSupported: true, contextTokens: 1_000_000 }
  );
  assert.equal(small.kvGb, large.kvGb); // 0.626 in both cases
  assert.equal(small.kvGb, 0.63);
});

test("int8 weights are twice int4 weights", () => {
  const q4 = estimate({ name: "m", totalParamsB: 10, bitsPerWeight: 4 }, { ramGb: 64, contextTokens: 100 });
  const q8 = estimate({ name: "m", totalParamsB: 10, bitsPerWeight: 8 }, { ramGb: 64, contextTokens: 100 });
  assert.ok(Math.abs(q8.weightsGb - 2 * q4.weightsGb) < 0.001);
});

test("invalid inputs throw", () => {
  assert.throws(() => estimate({ totalParamsB: 0 }, { ramGb: 8 }));
  assert.throws(() => estimate({ totalParamsB: 10, activeParamsB: 20 }, { ramGb: 8 }));
  assert.throws(() => estimate({ totalParamsB: 10 }, { ramGb: 0 }));
  assert.throws(() => estimate({ totalParamsB: 10 }, { ramGb: 8, contextTokens: -1 }));
  assert.throws(() => estimate(null, { ramGb: 8 }));
});

test("estimate returns assumptions for transparency", () => {
  const r = estimate(
    { name: "kimi-k3", totalParamsB: 2780, activeParamsB: 104, bitsPerWeight: 4, constantKvBytesGb: 0.626 },
    { ramGb: 64, streamingSupported: true }
  );
  assert.ok(Array.isArray(r.assumptions) && r.assumptions.length >= 3);
  assert.ok(r.estTokensPerSec > 0);
});
