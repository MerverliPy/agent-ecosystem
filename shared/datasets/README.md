# shared/datasets/

Seeded local-inference benchmark data. Seeded in Phase 2 (BenchKit) from published sources:

- kimi-k3-in-c — 2.78T params, ~3.7% active (~104B), 8.24GB RAM, 1.56TB disk, MXFP4; int8 ≈ 1% error, int4 ≈ 17%
- sqliteai/warp — expert streaming from NVMe, bounded cache
- turbo-fieldfare — Gemma 4 26B-A4B, ~2GB RAM, M-series
- MiniMax-H3

Every row must validate against `../schemas/benchmark-result.schema.json` and carry `source_url` (DEC-0006).
