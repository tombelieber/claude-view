#!/usr/bin/env bash
set -euo pipefail

# claude-view quick launcher
# Downloads pre-built binary from GitHub Releases and runs it.
# No Rust, Node, or Bun required — just curl and tar.

REPO="anonymous-dev/claude-view"
CACHE_DIR="${HOME}/.cache/claude-view"
BIN_DIR="${CACHE_DIR}/bin"
VERSION_FILE="${CACHE_DIR}/version"

# --- Resolve version ---

resolve_version() {
  if [[ -n "${CLAUDE_VIEW_VERSION:-}" ]]; then
    echo "$CLAUDE_VIEW_VERSION"
    return
  fi
  # Fetch latest release tag from GitHub API
  local tag
  tag=$(curl -sI "https://github.com/${REPO}/releases/latest" \
    | grep -i '^location:' \
    | sed 's|.*/tag/||' \
    | tr -d '[:space:]')
  echo "${tag#v}"
}

# --- Detect platform ---

detect_platform() {
  local os arch
  os=$(uname -s | tr '[:upper:]' '[:lower:]')
  arch=$(uname -m)

  case "$os" in
    darwin) os="darwin" ;;
    linux)  os="linux" ;;
    *)
      echo "Error: Unsupported OS: $os" >&2
      exit 1
      ;;
  esac

  case "$arch" in
    arm64|aarch64) arch="arm64" ;;
    x86_64|amd64)  arch="x64" ;;
    *)
      echo "Error: Unsupported architecture: $arch" >&2
      exit 1
      ;;
  esac

  echo "${os}-${arch}"
}

# --- Main ---

main() {
  local version platform artifact url

  version=$(resolve_version)
  if [[ -z "$version" ]]; then
    echo "Error: Could not resolve version. Set CLAUDE_VIEW_VERSION=x.y.z" >&2
    exit 1
  fi

  platform=$(detect_platform)
  artifact="claude-view-${platform}.tar.gz"
  url="https://github.com/${REPO}/releases/download/v${version}/${artifact}"

  # Check cache
  if [[ -f "$VERSION_FILE" && -f "${BIN_DIR}/claude-view" ]]; then
    local cached
    cached=$(cat "$VERSION_FILE")
    if [[ "$cached" == "$version" ]]; then
      echo "claude-view v${version} (cached)"
      STATIC_DIR="${BIN_DIR}/dist" exec "${BIN_DIR}/claude-view" "$@"
    fi
  fi

  # Download
  echo "Downloading claude-view v${version} for ${platform}..."
  rm -rf "$BIN_DIR"
  mkdir -p "$BIN_DIR"

  if ! curl -fSL "$url" | tar xz -C "$BIN_DIR"; then
    echo "Error: Download failed." >&2
    echo "  URL: $url" >&2
    echo "  Check https://github.com/${REPO}/releases for available versions." >&2
    exit 1
  fi

  chmod +x "${BIN_DIR}/claude-view"

  # macOS Gatekeeper: remove quarantine flag from downloaded binary
  if [[ "$(uname -s)" == "Darwin" ]]; then
    xattr -dr com.apple.quarantine "$BIN_DIR" 2>/dev/null || true
  fi
  mkdir -p "$CACHE_DIR"
  echo "$version" > "$VERSION_FILE"
  echo "Installed to ${BIN_DIR}"

  # Run
  STATIC_DIR="${BIN_DIR}/dist" exec "${BIN_DIR}/claude-view" "$@"
}

main "$@"
