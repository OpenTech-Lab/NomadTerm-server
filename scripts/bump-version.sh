#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: bump-version.sh [x.y.z]

Updates server version, writes release note, commits, tags, and pushes.
Example:
  ./scripts/bump-version.sh 0.2.0
  ./scripts/bump-version.sh   # auto bump patch from latest version
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

if [[ $# -gt 1 ]]; then
  usage
  exit 1
fi

NEW_VERSION="${1:-}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CARGO_TOML="$REPO_ROOT/Cargo.toml"
CARGO_LOCK="$REPO_ROOT/Cargo.lock"
DESKTOP_CARGO_TOML="$REPO_ROOT/desktop/src-tauri/Cargo.toml"
DESKTOP_CARGO_LOCK="$REPO_ROOT/desktop/src-tauri/Cargo.lock"
DESKTOP_TAURI_CONF="$REPO_ROOT/desktop/src-tauri/tauri.conf.json"
VERSION_DIR="$SCRIPT_DIR/version"
REMOTE="${REMOTE:-origin}"

if [[ ! -f "$CARGO_TOML" ]]; then
  echo "Error: Cargo.toml not found at $CARGO_TOML" >&2
  exit 1
fi

mkdir -p "$VERSION_DIR"

cd "$REPO_ROOT"
git rev-parse --is-inside-work-tree >/dev/null

REMOTE_URL="$(git remote get-url "$REMOTE" 2>/dev/null || true)"
case "$REMOTE_URL" in
  https://github.com/*)
    REPO_SLUG="${REMOTE_URL#https://github.com/}"
    ;;
  git@github.com:*)
    REPO_SLUG="${REMOTE_URL#git@github.com:}"
    ;;
  ssh://git@github.com/*)
    REPO_SLUG="${REMOTE_URL#ssh://git@github.com/}"
    ;;
  *)
    echo "Error: unsupported remote URL for release notes: $REMOTE_URL" >&2
    exit 1
    ;;
esac
REPO_SLUG="${REPO_SLUG%.git}"

CURRENT_VERSION="$(awk '
  BEGIN{in_pkg=0}
  /^\[package\]$/ {in_pkg=1; next}
  /^\[/ && $0 !~ /^\[package\]$/ {in_pkg=0}
  in_pkg && /^version[[:space:]]*=/ {
    gsub(/^version[[:space:]]*=[[:space:]]*"/, "", $0)
    gsub(/".*$/, "", $0)
    print $0
    exit
  }
' "$CARGO_TOML")"

if [[ -z "$CURRENT_VERSION" ]]; then
  echo "Error: failed to read current package version from Cargo.toml" >&2
  exit 1
fi

LATEST_VERSION="$CURRENT_VERSION"
LATEST_TAG_VERSION="$(git -C "$REPO_ROOT" tag --list 'v[0-9]*.[0-9]*.[0-9]*' | sed 's/^v//' | sort -V | tail -n1)"
if [[ -n "$LATEST_TAG_VERSION" ]]; then
  LATEST_VERSION="$(printf '%s\n%s\n' "$LATEST_VERSION" "$LATEST_TAG_VERSION" | sort -V | tail -n1)"
fi

if [[ -z "$NEW_VERSION" ]]; then
  IFS='.' read -r latest_major latest_minor latest_patch <<< "$LATEST_VERSION"
  NEW_VERSION="${latest_major}.${latest_minor}.$((latest_patch + 1))"
  echo "No version supplied. Auto bumping patch: $LATEST_VERSION -> $NEW_VERSION"
elif [[ ! "$NEW_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "Error: version must match x.y.z (example: 1.4.2)." >&2
  exit 1
fi

LATEST="$(printf '%s\n%s\n' "$LATEST_VERSION" "$NEW_VERSION" | sort -V | tail -n1)"
if [[ "$LATEST" != "$NEW_VERSION" || "$NEW_VERSION" == "$LATEST_VERSION" ]]; then
  echo "Error: new version ($NEW_VERSION) must be greater than latest version ($LATEST_VERSION)." >&2
  exit 1
fi

TAG="v$NEW_VERSION"

if git rev-parse -q --verify "refs/tags/$TAG" >/dev/null; then
  echo "Error: tag $TAG already exists locally." >&2
  exit 1
fi

tmp_toml="$(mktemp)"
if ! awk -v ver="$NEW_VERSION" '
  BEGIN{in_pkg=0; done=0}
  /^\[package\]$/ {in_pkg=1; print; next}
  /^\[/ && $0 !~ /^\[package\]$/ {in_pkg=0}
  in_pkg && !done && /^version[[:space:]]*=/ {
    print "version = \"" ver "\""
    done=1
    next
  }
  {print}
  END {
    if (!done) {
      exit 42
    }
  }
' "$CARGO_TOML" >"$tmp_toml"; then
  rm -f "$tmp_toml"
  echo "Error: failed to update version in Cargo.toml" >&2
  exit 1
fi
mv "$tmp_toml" "$CARGO_TOML"

if [[ -f "$CARGO_LOCK" ]]; then
  tmp_lock="$(mktemp)"
  if ! awk -v ver="$NEW_VERSION" '
    BEGIN{in_target=0; updated=0}
    /^\[\[package\]\]$/ {in_target=0}
    /^name = "nomadterm"$/ {in_target=1}
    in_target && /^version = "/ && !updated {
      print "version = \"" ver "\""
      in_target=0
      updated=1
      next
    }
    {print}
    END {
      if (!updated) {
        exit 43
      }
    }
  ' "$CARGO_LOCK" > "$tmp_lock"; then
    rm -f "$tmp_lock"
    echo "Error: failed to update sync-server version in Cargo.lock" >&2
    exit 1
  fi
  mv "$tmp_lock" "$CARGO_LOCK"
fi

# --- desktop/src-tauri/Cargo.toml ---
if [[ -f "$DESKTOP_CARGO_TOML" ]]; then
  tmp_desktop_toml="$(mktemp)"
  if ! awk -v ver="$NEW_VERSION" '
    BEGIN{in_pkg=0; done=0}
    /^\[package\]$/ {in_pkg=1; print; next}
    /^\[/ && $0 !~ /^\[package\]$/ {in_pkg=0}
    in_pkg && !done && /^version[[:space:]]*=/ {
      print "version = \"" ver "\""
      done=1
      next
    }
    {print}
    END { if (!done) { exit 42 } }
  ' "$DESKTOP_CARGO_TOML" >"$tmp_desktop_toml"; then
    rm -f "$tmp_desktop_toml"
    echo "Error: failed to update version in desktop/src-tauri/Cargo.toml" >&2
    exit 1
  fi
  mv "$tmp_desktop_toml" "$DESKTOP_CARGO_TOML"
fi

# --- desktop/src-tauri/tauri.conf.json ---
if [[ -f "$DESKTOP_TAURI_CONF" ]]; then
  tmp_tauri_conf="$(mktemp)"
  if ! sed "s/\"version\": \"[^\"]*\"/\"version\": \"${NEW_VERSION}\"/" \
      "$DESKTOP_TAURI_CONF" >"$tmp_tauri_conf"; then
    rm -f "$tmp_tauri_conf"
    echo "Error: failed to update version in desktop/src-tauri/tauri.conf.json" >&2
    exit 1
  fi
  mv "$tmp_tauri_conf" "$DESKTOP_TAURI_CONF"
fi

# --- desktop/src-tauri/Cargo.lock ---
if [[ -f "$DESKTOP_CARGO_LOCK" ]]; then
  tmp_desktop_lock="$(mktemp)"
  if ! awk -v ver="$NEW_VERSION" '
    BEGIN{in_target=0; updated=0}
    /^\[\[package\]\]$/ {in_target=0}
    /^name = "nomadterm-desktop"$/ {in_target=1}
    in_target && /^version = "/ && !updated {
      print "version = \"" ver "\""
      in_target=0
      updated=1
      next
    }
    {print}
    END { if (!updated) { exit 43 } }
  ' "$DESKTOP_CARGO_LOCK" >"$tmp_desktop_lock"; then
    rm -f "$tmp_desktop_lock"
    echo "Error: failed to update nomadterm-desktop version in desktop/src-tauri/Cargo.lock" >&2
    exit 1
  fi
  mv "$tmp_desktop_lock" "$DESKTOP_CARGO_LOCK"
fi

RELEASE_NOTE="$VERSION_DIR/$NEW_VERSION.md"
if [[ -f "$RELEASE_NOTE" ]]; then
  echo "Error: release note already exists: $RELEASE_NOTE" >&2
  exit 1
fi

LAST_TAG="$(git -C "$REPO_ROOT" describe --tags --abbrev=0 2>/dev/null || true)"
if [[ -n "$LAST_TAG" ]]; then
  CHANGELOG_ENTRIES="$(git -C "$REPO_ROOT" log "${LAST_TAG}..HEAD" --pretty=format:'- `%h` %s' || true)"
else
  CHANGELOG_ENTRIES="$(git -C "$REPO_ROOT" log --max-count=20 --pretty=format:'- `%h` %s' || true)"
fi

if [[ -z "$CHANGELOG_ENTRIES" ]]; then
  CHANGELOG_ENTRIES="- (no changes)"
fi

RELEASE_DATE="$(date -u +"%Y-%m-%d")"
DOWNLOAD_BASE_URL="https://github.com/${REPO_SLUG}/releases/download/${TAG}"
if [[ -n "$LAST_TAG" ]]; then
  FULL_CHANGELOG_URL="https://github.com/${REPO_SLUG}/compare/${LAST_TAG}...${TAG}"
  FULL_CHANGELOG_TEXT="${LAST_TAG}...${TAG}"
else
  FULL_CHANGELOG_URL="https://github.com/${REPO_SLUG}/releases/tag/${TAG}"
  FULL_CHANGELOG_TEXT="Initial release"
fi

cat >"$RELEASE_NOTE" <<EOF
# Changelog - $TAG

Release date: $RELEASE_DATE

## What's Changed

$CHANGELOG_ENTRIES

## Install

Linux x64:
\`\`\`bash
curl -Lo nomadterm ${DOWNLOAD_BASE_URL}/nomadterm-linux-x86_64
chmod +x nomadterm
\`\`\`

Linux ARM64:
\`\`\`bash
curl -Lo nomadterm ${DOWNLOAD_BASE_URL}/nomadterm-linux-arm64
chmod +x nomadterm
\`\`\`

macOS Apple Silicon:
\`\`\`bash
curl -Lo nomadterm ${DOWNLOAD_BASE_URL}/nomadterm-darwin-arm64
chmod +x nomadterm
\`\`\`

macOS Intel:
\`\`\`bash
curl -Lo nomadterm ${DOWNLOAD_BASE_URL}/nomadterm-darwin-x86_64
chmod +x nomadterm
\`\`\`

Windows x64:
\`\`\`powershell
curl.exe -Lo nomadterm.exe ${DOWNLOAD_BASE_URL}/nomadterm-windows-x86_64.exe
\`\`\`

## Full Changelog

[$FULL_CHANGELOG_TEXT]($FULL_CHANGELOG_URL)
EOF

# Keep only latest version note file
find "$VERSION_DIR" -maxdepth 1 -type f -name '*.md' ! -name "$NEW_VERSION.md" -delete

git add -A

if git diff --cached --quiet; then
  echo "Error: no changes to commit." >&2
  exit 1
fi

git commit -m "chore(release): $TAG"
git tag -a "$TAG" -m "Release $TAG"
git push "$REMOTE" HEAD
git push "$REMOTE" "$TAG"

echo "Release complete: $TAG"
