# Ecosystem demos — one-liners for each product (Phase 7)

From the repo root:

| Demo | Command | Shows |
|------|---------|-------|
| BenchKit | `node scripts/demos/benchkit-demo.mjs` | dataset rows (DEC-0006 sources) + will-it-run verdicts for kimi-k3 and gemma-4-26b |
| SkillHub | `bash scripts/demos/skillhub-install-demo.sh` | publish → search → install into a temp harness → verify --quality |
| SlopGate | `bash scripts/demos/slopgate-gate-demo.sh` | score clean/mild/heavy fixtures + CI gate exit codes at threshold 50 |
| DeskAgent | `bash scripts/demos/deskagent-approval-demo.sh` | capture → extract → approve/reject → sandbox + undo → skill install → citations |

All four run offline and end-to-end (the SkillHub demo starts its own registry on an
ephemeral port). The live DeskAgent model path is opt-in:
`cargo test -p deskagent-core -- --ignored ollama_live` with a running Ollama.
