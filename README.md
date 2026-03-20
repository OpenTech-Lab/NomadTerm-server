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

```bash
cargo build --release
```

The compiled binary will be available at `target/release/nomadterm`.

### 2. Start the daemon

For the intended phone-to-machine flow over Tailscale:

```bash
./target/release/nomadterm --ws --bind-tailscale
```

Useful flags:

- `--bind-tailscale`: bind to the machine's Tailscale IP when available
- `--port <PORT>`: change the default port (`7681`)
- `--no-tls`: run in local or trusted-network mode

If Tailscale is not available, the daemon falls back to localhost. For local testing you can also start it without `--bind-tailscale`.

### 3. Copy the connection info

At startup the daemon prints:

- the WebSocket address
- the generated bearer token
- a QR code that the mobile app can scan

The token is persisted in `~/.hcom/nomadterm.token`.

### 4. Connect from the mobile app

Open the NomadTerm mobile app and either:

- scan the QR code shown by the daemon, or
- enter the host, port, and token manually

### 5. Use the remote session manager

After connecting from mobile, you can:

- view the current session list
- spawn a new AI CLI session
- open a live terminal view
- approve or reject tool calls
- kill a running session

## License

[MIT](LICENSE)
