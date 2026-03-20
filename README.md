# NomadTerm Server

## What Is This?

NomadTerm is a secure remote PTY daemon for AI CLI tools (Claude Code, Codex, Copilot, Gemini). It runs on the machine where your AI sessions live and lets you monitor and control them from your phone or desktop.

```
Mobile app ──WebSocket──► nomadterm daemon ◄── manages ── nomadterm-desktop (GUI)
```

- **Daemon** (`nomadterm`) — runs in the background, exposes a `/ws` endpoint, manages PTY sessions, streams output to your phone in real time
- **Desktop GUI** (`nomadterm-desktop`) — optional Tauri app on your Linux desktop to manage repos, generate QR codes, and start/stop the daemon with one click

---

## Install (Linux — recommended)

Download both `.deb` files from the [latest release](../../releases/latest):

| File | What it installs |
|------|-----------------|
| `nomadterm_X.Y.Z_amd64.deb` | `/usr/bin/nomadterm` + systemd service + app launcher entry |
| `nomadterm-desktop_X.Y.Z_amd64.deb` | `/usr/bin/nomadterm-desktop` (the Tauri GUI) |

```bash
sudo apt install ./nomadterm_X.Y.Z_amd64.deb
sudo apt install ./nomadterm-desktop_X.Y.Z_amd64.deb
```

After installing both, **NomadTerm** appears in your Ubuntu app launcher. Open it to get started.

> **Tip:** copy the `.deb` files to `/tmp/` before installing to avoid an apt sandbox warning:
> ```bash
> cp ~/Downloads/nomadterm*.deb /tmp/
> sudo apt install /tmp/nomadterm_X.Y.Z_amd64.deb /tmp/nomadterm-desktop_X.Y.Z_amd64.deb
> ```

---

## Usage

### Option A — Desktop GUI (recommended for desktop Linux)

1. Install both `.deb` packages above
2. Open **NomadTerm** from your app launcher
3. Click **+** to add a repo folder
4. Click **Start** — the daemon starts and a QR code appears
5. Scan the QR from the NomadTerm mobile app to connect

The GUI manages tokens, ports, and the daemon lifecycle automatically.

---

### Option B — Headless daemon (servers, SSH, no display)

```bash
nomadterm --ws --bind-tailscale --port 7681
```

Useful flags:

| Flag | Description |
|------|-------------|
| `--bind-tailscale` | bind to the Tailscale IP (falls back to LAN IP if unavailable) |
| `--port <PORT>` | WebSocket port (default `7681`) |
| `--no-tls` | disable TLS for local/trusted networks |
| `--token <TOKEN>` | set a fixed bearer token instead of generating one |
| `--workspace <PATH>` | set the working directory for the session |

At startup the daemon prints the WebSocket address, bearer token, and a QR code. Scan it from the mobile app or enter the details manually.

#### Install as a systemd service

```bash
# First-time setup
sudo bash install.sh init

# Update later
sudo bash install.sh update
```

The `install.sh` script is included in `nomadterm-server-linux-x86_64.tar.gz` on the releases page, or downloadable directly:

```bash
curl -Lo install.sh https://github.com/YOUR_ORG/nomadterm/releases/latest/download/install.sh
sudo bash install.sh init
```

---

## Connect from mobile

Open the NomadTerm mobile app and:

- **Scan the QR code** shown by the daemon or desktop GUI, or
- **Enter manually**: host, port, and token

After connecting you can:

- view and spawn AI CLI sessions (Claude Code, Codex, Copilot, Gemini)
- open a live terminal view
- approve or reject tool calls
- kill a running session

---

## Build from source

```bash
# Daemon only
cargo build --release
./target/release/nomadterm --ws

# Desktop GUI
cd desktop
npm install
npm run tauri dev   # requires nomadterm binary in PATH
```

---

## License

[MIT](LICENSE)
