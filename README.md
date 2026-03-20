# NomadTerm Server

## What Is This?

This repo contains the NomadTerm daemon and CLI. It is the backend part of the project that runs on the machine where your AI CLI sessions live.

When started in WebSocket mode, the daemon:

- exposes a `/ws` endpoint for the mobile client
- lists and manages PTY-backed AI sessions such as Claude, Codex, Copilot, and Gemini
- sends terminal output to the phone in real time
- forwards input, resize, kill, and approval actions from the phone back to the running session
- prints the connection address, token, and QR code used by the mobile app

This codebase was originally forked from `aannoo/hcom` and extended into NomadTerm.

## How To Use It

### 1. Build the binary

**CLI / headless daemon** (default):

```bash
cargo build --release
```

**Desktop GUI** (requires a display — Linux, macOS, Windows):

```bash
cargo build --release --features gui
```

The compiled binary will be available at `target/release/nomadterm`.

---

### Option A — Desktop GUI (recommended)

The GUI lets you register multiple repos, generate per-repo QR codes, and start/stop the daemon with one click.

```bash
./target/release/nomadterm gui
```

What you get:

- **Left panel**: add/remove repo folders; each repo gets a stable bearer token
- **Right panel**: Start/Stop button, live session count, QR code, and a copyable `nomadterm://` URL
- Scan the QR from the mobile app to connect — the token is saved on the phone for 30 days and auto-renewed on each connect

> Requires a build with `--features gui`. Running `nomadterm gui` without it prints an error asking you to rebuild.

---

### Option B — Headless daemon (original CLI mode)

For the intended phone-to-machine flow over Tailscale:

```bash
./target/release/nomadterm --ws --bind-tailscale
```

Useful flags:

- `--bind-tailscale`: bind to the machine's Tailscale IP when available
- `--port <PORT>`: change the default port (`7681`)
- `--no-tls`: run in local or trusted-network mode

If Tailscale is not available, the daemon falls back to localhost. For local testing you can also start it without `--bind-tailscale`.

### 3. Copy the connection info (headless mode)

At startup the daemon prints:

- the WebSocket address
- the generated bearer token
- a QR code that the mobile app can scan

The token is persisted in `~/.hcom/nomadterm.token`.

### 4. Connect from the mobile app

Open the NomadTerm mobile app and either:

- scan the QR code shown by the daemon or GUI, or
- enter the host, port, and token manually
- tap a saved repo from the list (after the first scan)

### 5. Use the remote session manager

After connecting from mobile, you can:

- view the current session list
- spawn a new AI CLI session
- open a live terminal view
- approve or reject tool calls
- kill a running session

## License

[MIT](LICENSE)
