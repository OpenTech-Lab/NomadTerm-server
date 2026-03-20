#!/usr/bin/env bash
# NomadTerm Server Installer
#
# Usage:
#   sudo bash install.sh init       – First-time install (download, register & start)
#   sudo bash install.sh update     – Update to latest release and restart
#   sudo bash install.sh uninstall  – Remove binary and service
#
# After install, manage with systemctl:
#   systemctl status  nomadterm
#   systemctl start   nomadterm
#   systemctl stop    nomadterm
#   systemctl restart nomadterm
#
# Environment variables:
#   NOMADTERM_PORT   Listening port (default: 7681)

set -euo pipefail

REPO="__REPO__"   # Substituted by CI with github.repository
BINARY="nomadterm"
BIN_PATH="/usr/local/bin/${BINARY}"
SERVICE_NAME="${BINARY}"
SERVICE_FILE="/etc/systemd/system/${SERVICE_NAME}.service"
PORT="${NOMADTERM_PORT:-7681}"

# ── Helpers ──────────────────────────────────────────────────────────────────

die()  { echo "error: $*" >&2; exit 1; }
info() { echo "==> $*"; }

need_root() {
  [[ $EUID -eq 0 ]] || die "Run as root:  sudo bash $0 ${1:-}"
}

detect_platform() {
  local os arch
  os=$(uname -s | tr '[:upper:]' '[:lower:]')
  arch=$(uname -m)
  case "$arch" in
    x86_64|amd64)  arch="x86_64" ;;
    aarch64|arm64) arch="arm64"  ;;
    *) die "Unsupported CPU architecture: $arch" ;;
  esac
  case "$os" in
    linux)  echo "linux-${arch}" ;;
    darwin) die "macOS detected — use Homebrew or build from source (systemd not available)" ;;
    *)      die "Unsupported OS: $os" ;;
  esac
}

latest_tag() {
  curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep '"tag_name"' | head -1 \
    | sed 's/.*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/'
}

download_binary() {
  local tag="$1" platform="$2"
  local url="https://github.com/${REPO}/releases/download/${tag}/${BINARY}-${platform}"
  info "Downloading ${BINARY} ${tag} (${platform}) ..."
  curl -fsSL --output "${BIN_PATH}" "${url}"
  chmod +x "${BIN_PATH}"
}

write_service() {
  local user="${1}"
  local home
  home=$(getent passwd "$user" 2>/dev/null | cut -d: -f6) || home="/home/${user}"

  cat > "${SERVICE_FILE}" <<EOF
[Unit]
Description=NomadTerm — Secure Remote PTY Daemon for Multi-AI CLI
After=network.target
Wants=network-online.target

[Service]
Type=simple
User=${user}
WorkingDirectory=${home}
ExecStart=${BIN_PATH} --ws --port ${PORT}
Restart=always
RestartSec=3
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
EOF
  systemctl daemon-reload
  systemctl enable "${SERVICE_NAME}"
}

# ── Commands ─────────────────────────────────────────────────────────────────

cmd_init() {
  need_root "init"

  local platform tag
  platform=$(detect_platform)
  tag=$(latest_tag)
  [[ -n "$tag" ]] || die "Could not determine latest release tag"

  local service_user="${SUDO_USER:-}"
  [[ -n "$service_user" ]] || die "Could not detect calling user — set SUDO_USER or run via sudo"

  info "Installing NomadTerm ${tag} for user '${service_user}'"
  download_binary "${tag}" "${platform}"
  write_service "${service_user}"
  systemctl start "${SERVICE_NAME}"

  echo ""
  echo "NomadTerm ${tag} installed and running."
  echo ""
  echo "  systemctl status  ${SERVICE_NAME}"
  echo "  systemctl stop    ${SERVICE_NAME}"
  echo "  systemctl start   ${SERVICE_NAME}"
  echo "  systemctl restart ${SERVICE_NAME}"
  echo ""
  echo "To update later:"
  echo "  sudo bash $0 update"
}

cmd_update() {
  need_root "update"

  [[ -x "${BIN_PATH}" ]] || die "${BINARY} is not installed — run: sudo bash $0 init"

  local platform tag
  platform=$(detect_platform)
  tag=$(latest_tag)
  [[ -n "$tag" ]] || die "Could not determine latest release tag"

  info "Updating NomadTerm to ${tag} ..."
  systemctl stop "${SERVICE_NAME}" 2>/dev/null || true
  download_binary "${tag}" "${platform}"
  systemctl daemon-reload
  systemctl start "${SERVICE_NAME}"

  echo "NomadTerm updated to ${tag} and restarted."
}

cmd_uninstall() {
  need_root "uninstall"

  info "Uninstalling NomadTerm ..."
  systemctl stop    "${SERVICE_NAME}" 2>/dev/null || true
  systemctl disable "${SERVICE_NAME}" 2>/dev/null || true
  rm -f "${SERVICE_FILE}" "${BIN_PATH}"
  systemctl daemon-reload

  echo "NomadTerm removed."
}

# ── Dispatch ─────────────────────────────────────────────────────────────────

case "${1:-help}" in
  init)      cmd_init      ;;
  update)    cmd_update    ;;
  uninstall) cmd_uninstall ;;
  *)
    echo "Usage: sudo bash $(basename "$0") <command>"
    echo ""
    echo "Commands:"
    echo "  init       Download binary, install & start service (first-time setup)"
    echo "  update     Update to latest release and restart service"
    echo "  uninstall  Stop service and remove binary"
    echo ""
    echo "Service management (after init):"
    echo "  systemctl start   ${SERVICE_NAME}"
    echo "  systemctl stop    ${SERVICE_NAME}"
    echo "  systemctl status  ${SERVICE_NAME}"
    echo "  systemctl restart ${SERVICE_NAME}"
    echo ""
    echo "Environment:"
    echo "  NOMADTERM_PORT   Listening port (default: 7681)"
    exit 1
    ;;
esac
