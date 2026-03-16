#!/bin/sh
set -eu

# claude-view installer
# Usage: curl -fsSL https://get.claudeview.ai/install.sh | sh
#
# Environment variables:
#   CLAUDE_VIEW_VERSION   - Install a specific version (default: latest)
#   CLAUDE_VIEW_INSTALL_DIR - Override install location (default: ~/.claude-view)

REPO="tombelieber/claude-view"
INSTALL_DIR="${CLAUDE_VIEW_INSTALL_DIR:-$HOME/.claude-view}"
BIN_DIR="${INSTALL_DIR}/bin"

# --- Colors (disabled if not a terminal) ---

if [ -t 1 ]; then
  BOLD='\033[1m'
  DIM='\033[2m'
  GREEN='\033[32m'
  RED='\033[31m'
  RESET='\033[0m'
else
  BOLD='' DIM='' GREEN='' RED='' RESET=''
fi

info()    { printf "${BOLD}%s${RESET}\n" "$1"; }
success() { printf "${GREEN}${BOLD}%s${RESET}\n" "$1"; }
error()   { printf "${RED}error: %s${RESET}\n" "$1" >&2; exit 1; }
dim()     { printf "${DIM}%s${RESET}\n" "$1"; }

# --- Platform detection ---

detect_platform() {
  os=$(uname -s | tr '[:upper:]' '[:lower:]')
  arch=$(uname -m)

  case "$os" in
    darwin) os="darwin" ;;
    linux)  os="linux" ;;
    *)      error "Unsupported OS: $os. Only macOS and Linux are supported." ;;
  esac

  case "$arch" in
    arm64|aarch64) arch="arm64" ;;
    x86_64|amd64)  arch="x64" ;;
    *)             error "Unsupported architecture: $arch" ;;
  esac

  echo "${os}-${arch}"
}

# --- Version resolution ---

resolve_version() {
  if [ -n "${CLAUDE_VIEW_VERSION:-}" ]; then
    echo "$CLAUDE_VIEW_VERSION"
    return
  fi

  tag=$(curl -sI "https://github.com/${REPO}/releases/latest" \
    | grep -i '^location:' \
    | sed 's|.*/tag/v||' \
    | tr -d '[:space:]')

  if [ -z "$tag" ]; then
    error "Could not resolve latest version. Set CLAUDE_VIEW_VERSION=x.y.z or check https://github.com/${REPO}/releases"
  fi

  echo "$tag"
}

# --- Checksum verification ---

verify_checksum() {
  file="$1"
  version="$2"
  artifact="$3"

  checksums_url="https://github.com/${REPO}/releases/download/v${version}/checksums.txt"
  checksums=$(curl -fsSL "$checksums_url" 2>/dev/null || true)

  if [ -z "$checksums" ]; then
    dim "  (checksum file not available, skipping verification)"
    return
  fi

  expected=$(echo "$checksums" | grep "$artifact" | awk '{print $1}')
  if [ -z "$expected" ]; then
    dim "  (no checksum for $artifact, skipping verification)"
    return
  fi

  if command -v sha256sum >/dev/null 2>&1; then
    actual=$(sha256sum "$file" | awk '{print $1}')
  elif command -v shasum >/dev/null 2>&1; then
    actual=$(shasum -a 256 "$file" | awk '{print $1}')
  else
    dim "  (sha256sum not found, skipping verification)"
    return
  fi

  if [ "$actual" != "$expected" ]; then
    error "Checksum verification failed.\n  Expected: $expected\n  Actual:   $actual"
  fi

  dim "  Checksum verified."
}

# --- Shell profile detection ---

detect_profile() {
  shell_name=$(basename "${SHELL:-/bin/sh}")

  case "$shell_name" in
    zsh)
      echo "${HOME}/.zshrc"
      ;;
    bash)
      if [ -f "${HOME}/.bashrc" ]; then
        echo "${HOME}/.bashrc"
      elif [ -f "${HOME}/.bash_profile" ]; then
        echo "${HOME}/.bash_profile"
      else
        echo "${HOME}/.bashrc"
      fi
      ;;
    fish)
      echo "${HOME}/.config/fish/config.fish"
      ;;
    *)
      echo "${HOME}/.profile"
      ;;
  esac
}

# --- PATH setup ---

setup_path() {
  # Check if already in PATH
  case ":${PATH}:" in
    *":${BIN_DIR}:"*) return ;;
  esac

  profile=$(detect_profile)
  shell_name=$(basename "${SHELL:-/bin/sh}")

  if [ "$shell_name" = "fish" ]; then
    path_line="fish_add_path \"${BIN_DIR}\""
  else
    path_line="export PATH=\"${BIN_DIR}:\$PATH\""
  fi

  # Check if line already exists in profile
  if [ -f "$profile" ] && grep -qF "$BIN_DIR" "$profile" 2>/dev/null; then
    return
  fi

  printf "\n# claude-view\n%s\n" "$path_line" >> "$profile"
  dim "  Added to PATH in $profile"
}

# --- Main ---

main() {
  info "Installing claude-view..."
  echo ""

  platform=$(detect_platform)
  version=$(resolve_version)
  artifact="claude-view-${platform}.tar.gz"
  url="https://github.com/${REPO}/releases/download/v${version}/${artifact}"

  info "  Platform: ${platform}"
  info "  Version:  v${version}"
  echo ""

  # Download to temp file for checksum verification
  tmp_dir=$(mktemp -d)
  tmp_file="${tmp_dir}/${artifact}"
  trap 'rm -rf "$tmp_dir"' EXIT

  printf "Downloading..."
  if ! curl -fSL --progress-bar "$url" -o "$tmp_file" 2>&1; then
    echo ""
    error "Download failed.\n  URL: $url\n  Check https://github.com/${REPO}/releases for available versions."
  fi
  echo ""

  # Verify checksum
  verify_checksum "$tmp_file" "$version" "$artifact"

  # Clean and extract
  rm -rf "$BIN_DIR"
  mkdir -p "$BIN_DIR"
  tar xzf "$tmp_file" -C "$BIN_DIR"

  # Make binary executable
  chmod +x "${BIN_DIR}/claude-view"

  # macOS: remove quarantine flag
  if [ "$(uname -s)" = "Darwin" ]; then
    xattr -dr com.apple.quarantine "$BIN_DIR" 2>/dev/null || true
  fi

  # Write version marker
  echo "$version" > "${INSTALL_DIR}/version"

  # Install sidecar dependencies if Node.js is available
  if [ -d "${BIN_DIR}/sidecar" ] && command -v npm >/dev/null 2>&1; then
    if [ ! -d "${BIN_DIR}/sidecar/node_modules" ]; then
      dim "  Installing sidecar dependencies..."
      (cd "${BIN_DIR}/sidecar" && npm install --omit=dev --silent 2>/dev/null) || true
    fi
  fi

  # Set up PATH
  setup_path

  echo ""
  success "claude-view v${version} installed successfully!"
  echo ""
  dim "  Installed to: ${BIN_DIR}/claude-view"

  # Check if the bin dir is in current PATH
  case ":${PATH}:" in
    *":${BIN_DIR}:"*)
      echo ""
      info "Run 'claude-view' to get started."
      ;;
    *)
      echo ""
      info "Restart your shell, then run 'claude-view' to get started."
      dim "  Or run now: ${BIN_DIR}/claude-view"
      ;;
  esac
}

main
