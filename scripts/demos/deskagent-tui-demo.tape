# DeskAgent TUI demo tape (vhs — https://github.com/charmbracelet/vhs)
#
# Renders apps/deskagent/docs/assets/deskagent-tui-demo.gif at 16:9.
# Prereq: a seeded store in /tmp/deskagent-gif-demo (5+ turns in one session so
# the extraction pass has queued approval cards, model remembered as
# qwen2.5-coder:7b) and a built binary (cargo build -p deskagent-cli). The chat
# turn talks to a local Ollama; without one it falls back offline (DEC-0005).
#
#   vhs scripts/demos/deskagent-tui-demo.tape
#
Output apps/deskagent/docs/assets/deskagent-tui-demo.gif
Set Width 1280
Set Height 720
Set Padding 24
Set FontFamily "Ubuntu Mono"
Set FontSize 16
Set Theme "Dracula"
Set Framerate 50
Set PlaybackSpeed 1.25

# Launch against an isolated store (binary dir first on PATH).
Env PATH "/home/calvin/agent-ecosystem/apps/deskagent/target/debug:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
Type "export DESKAGENT_DATA_DIR=/tmp/deskagent-gif-demo"
Enter
Sleep 300ms
Type "deskagent"
Enter
Sleep 3s

# Chat tab: one live turn against the remembered local model.
Type "What makes a great terminal demo?"
Enter
Sleep 10s

# Memory+Approvals: approve the pending extraction cards inline.
Tab
Sleep 1s
Type "y"
Sleep 1.5s
Type "j"
Sleep 800ms
Type "y"
Sleep 1.5s

# Models tab: load from the local runtime, browse the list.
Tab
Sleep 800ms
Type "r"
Sleep 2s
Type "j"
Sleep 500ms
Type "j"
Sleep 800ms

# Tasks tab: informational placeholder pane.
Tab
Sleep 1.5s

# Quit.
Escape
Sleep 1s
