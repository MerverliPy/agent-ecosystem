# DeskAgent TUI — mobile portrait demo tape (vhs — https://github.com/charmbracelet/vhs)
#
# Renders apps/deskagent/docs/assets/deskagent-tui-mobile-demo.gif at a narrow
# terminal size (~46x27) approximating the Moshi iOS app in portrait over SSH.
# Shows the compact layout (short tab labels, condensed citations), the
# mobile-first status bar (hints + transient status always visible), and the
# number-key pane switching (1-4) plus Tab.
#
# Prereq: a seeded store in /tmp/deskagent-gif-demo (see deskagent-tui-demo.tape)
# and a built binary (cargo build -p deskagent-cli). The chat turn talks to a
# local Ollama; without one it falls back offline (DEC-0005).
#
#   vhs scripts/demos/deskagent-tui-mobile-demo.tape
#
Output apps/deskagent/docs/assets/deskagent-tui-mobile-demo.gif
Set Width 520
Set Height 600
Set Padding 12
Set FontFamily "Ubuntu Mono"
Set FontSize 17
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

# Chat tab in compact mode: one live turn against the remembered local model.
Type "hello from my phone"
Enter
Sleep 10s

# Leave Chat with Tab, then switch panes with number keys (no Shift+Tab needed).
Tab
Sleep 1s
Type "3"
Sleep 1s
Type "r"
Sleep 2s
Type "4"
Sleep 1.2s
Type "1"
Sleep 1.2s

# Quit.
Escape
Sleep 800ms
