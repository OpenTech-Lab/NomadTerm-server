---
name: nomadterm-agent-messaging
description: |
  Let AI agents message, watch, and spawn each other across terminals. Claude Code, Gemini CLI, Codex, OpenCode. Use this skill when the human user needs help, status, or reference about nomadterm - when user asks questions like "how to setup nomadterm", "nomadterm not working", "explain nomadterm", or any nomadterm troubleshooting.

---

# nomadterm — Let AI agents message, watch, and spawn each other across terminals. Claude Code, Gemini CLI, Codex, OpenCode.

AI agents running in separate terminals are isolated from each other. Context doesn't transfer, decisions get repeated, file edits collide. nomadterm connects them.

```
pip install nomadterm
nomadterm claude
nomadterm gemini
nomadterm codex
nomadterm opencode
nomadterm                            # TUI dashboard
```

---

## What humans can do

Tell any agent:

> send a message to claude

> when codex goes idle send it the next task

> watch gemini's file edits, review each and send feedback if any bugs

> fork yourself to investigate the bug and report back

> find which agent worked on terminal_id code, resume them and ask why it sucks

---

## What agents can do 

- Message each other (@mentions, intents, threads, broadcast)
- Read each other's transcripts (ranges, detail levels)
- View agent terminal screens, inject text/enter for approvals
- Query event history (file edits, commands, status, lifecycle)
- Subscribe and react to each other's activity in real-time
- Spawn, fork, resume, kill agents in new terminal panes
- Build context bundles (files, transcript, events) for handoffs
- Collision detection — 2 agents edit same file within 20s, both notified
- Cross-device — connect agents across machines via MQTT relay

---

## Setup

If the user invokes this skill without arguments:

1. Run `nomadterm status` — if "command not found", run `pip install nomadterm` first
2. Tell user to run `nomadterm claude` or `nomadterm gemini` or `nomadterm codex` or `nomadterm opencode` in a new terminal (auto installs hooks on first run)

| Status Output | Meaning | Action |
|--------|---------|--------|
| command not found | nomadterm not installed | `pip install nomadterm` |
| `[~] claude` | Tool exists, hooks not installed | `nomadterm hooks add` then restart tool (or just `nomadterm claude`) |
| `[✓] claude` | Hooks installed | Ready — use `nomadterm claude` or `nomadterm start` |
| `[✗] claude` | Tool not found | Install the AI tool first |

After adding hooks or installing nomadterm you must restart the current AI tool for nomadterm to activate.

---

## Tool Support

| Tool | Message Delivery |
|------|------------------|
| Claude Code (incl. subagents) | automatic |
| Gemini CLI | automatic |
| Codex | automatic |
| OpenCode | automatic |
| Any AI tool | manual - via `nomadterm start` |


---

## Troubleshooting

### "nomadterm not working"

```bash
nomadterm status          # Check installation
nomadterm hooks status    # Check hooks specifically
nomadterm daemon status
nomadterm relay status
```

**Hooks missing?** `nomadterm hooks add` then restart tool.

**Still broken?**
```bash
nomadterm reset all && nomadterm hooks add
# Close all claude/codex/gemini/opencode/nomadterm windows
nomadterm claude          # Fresh start
```

### "messages not arriving"

1. **Check recipient:** `nomadterm list` — are they `listening` or `active`?
2. **Check message sent:** `nomadterm events --sql "type='message'" --last 5`
3. **Recipient shows `[claude*]`?** Restart the AI tool

### Sandbox / Permission Issues

```bash
export NOMADTERM_DIR="$PWD/.nomadterm"     # Project-local mode
nomadterm hooks add                   # Installs to project dir
```

---

## Files

| What | Location |
|------|----------|
| Database | `~/.nomadterm/nomadterm.db` |
| Config | `~/.nomadterm/config.toml` |
| Logs | `~/.nomadterm/.tmp/logs/nomadterm.log` |

With `NOMADTERM_DIR` set, uses that path instead of `~/.nomadterm`.

---

## More Info

```bash
nomadterm --help              # All commands
nomadterm <command> --help    # Command details
nomadterm run docs            # Full CLI + config + API reference
```

GitHub: https://github.com/aannoo/nomadterm
