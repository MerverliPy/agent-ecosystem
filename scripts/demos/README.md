# Ecosystem demos — one-liners for each product (Phase 7)

From the repo root:

| Demo | Command | Shows |
|------|---------|-------|
| BenchKit | `node scripts/demos/benchkit-demo.mjs` | dataset rows (DEC-0006 sources) + will-it-run verdicts for kimi-k3 and gemma-4-26b |
| SkillHub | `bash scripts/demos/skillhub-install-demo.sh` | publish → search → install into a temp harness → verify --quality |
| SlopGate | `bash scripts/demos/slopgate-gate-demo.sh` | score clean/mild/heavy fixtures + CI gate exit codes at threshold 50 |
| DeskAgent | `bash scripts/demos/deskagent-approval-demo.sh` | capture → extract → approve/reject → sandbox + undo → skill install → citations |
| DeskAgent TUI GIF | `vhs scripts/demos/deskagent-tui-demo.tape` | records the four-pane TUI walkthrough (chat → approvals → models → tasks) to `apps/deskagent/docs/assets/deskagent-tui-demo.gif` |
| DeskAgent TUI GIF (mobile) | `vhs scripts/demos/deskagent-tui-mobile-demo.tape` | records the narrow portrait walkthrough (compact layout, mobile keys) to `apps/deskagent/docs/assets/deskagent-tui-mobile-demo.gif` |

All four run offline and end-to-end (the SkillHub demo starts its own registry on an
ephemeral port). The live DeskAgent model path is opt-in:
`cargo test -p deskagent-core -- --ignored ollama_live` with a running Ollama.

The TUI GIF needs [vhs](https://github.com/charmbracelet/vhs) and an isolated seeded store
(`DESKAGENT_DATA_DIR=/tmp/deskagent-gif-demo`, 5+ turns in one session so the extraction
pass queues approval cards); without Ollama the chat turn falls back offline (DEC-0005).
