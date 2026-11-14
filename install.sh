#!/bin/sh
set -eu

REPO="slashome/wally"
INSTALL_DIR="${WALLY_INSTALL_DIR:-$HOME/.local/bin}"
BIN="$INSTALL_DIR/wally"

setup_colors() {
  if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
    BOLD=$(printf '\033[1m') YELLOW=$(printf '\033[38;5;214m') RUST=$(printf '\033[38;5;130m')
    RED=$(printf '\033[31m') RESET=$(printf '\033[0m')
  else
    BOLD="" YELLOW="" RUST="" RED="" RESET=""
  fi
}

say() { printf '%s%s%s\n' "$YELLOW" "$*" "$RESET"; }
fail() { printf '%sError: %s%s\n' "$RED" "$*" "$RESET" >&2; exit 1; }

check_system() {
  [ "$(uname -s)" = Darwin ] || fail "wally only runs on macOS for now."
  command -v curl >/dev/null 2>&1 || fail "curl is required."
}

latest_tag() {
  curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" 2>/dev/null \
    | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1
}

download() {
  tag="$1" work="$2"
  url="https://github.com/$REPO/releases/download/$tag/wally-$tag-universal-apple-darwin.tar.gz"
  say "Downloading wally $tag…"
  curl -fsSL "$url" -o "$work/wally.tar.gz" || return 1
  tar -xzf "$work/wally.tar.gz" -C "$work" || return 1
  [ -x "$work/wally" ]
}

build() {
  work="$1"
  command -v cargo >/dev/null 2>&1 \
    || fail "no release to download and no cargo to build from source: install Rust from https://rustup.rs"
  say "Building wally from source with cargo (a minute or two)…"
  cargo install --quiet --git "https://github.com/$REPO" --root "$work/cargo" wally
  cp "$work/cargo/bin/wally" "$work/wally"
}

ask_source() {
  if [ -n "${WALLY_SOURCE:-}" ]; then
    printf '%s' "$WALLY_SOURCE"
    return
  fi
  default="$HOME/Pictures/Wallpapers"
  if [ -r /dev/tty ] && [ -w /dev/tty ]; then
    printf '%sFolder of wallpapers%s [%s]: ' "$BOLD" "$RESET" "$default" > /dev/tty
    read -r answer < /dev/tty || answer=""
    printf '%s' "${answer:-$default}"
  else
    printf '%s' "$default"
  fi
}

configure() {
  source_dir=$(ask_source)
  case "$source_dir" in "~"*) source_dir="$HOME${source_dir#\~}" ;; esac
  "$BIN" settings path >/dev/null
  if [ -d "$source_dir" ]; then
    "$BIN" settings source add "$source_dir" >/dev/null 2>&1 || true
    "$BIN" settings source remove "$HOME/Pictures/Wallpapers" >/dev/null 2>&1 || true
  else
    printf '%s%s does not exist yet: add it later with `wally settings source add <folder>`.%s\n' \
      "$RUST" "$source_dir" "$RESET"
  fi
}

main() {
  setup_colors
  check_system
  work=$(mktemp -d "${TMPDIR:-/tmp}/wally.XXXXXX")
  trap 'rm -rf "$work"' EXIT

  tag="${WALLY_VERSION:-$(latest_tag)}"
  if [ -z "$tag" ] || ! download "$tag" "$work"; then
    build "$work"
  fi

  mkdir -p "$INSTALL_DIR"
  install -m 755 "$work/wally" "$BIN"
  xattr -d com.apple.quarantine "$BIN" 2>/dev/null || true

  configure
  if [ -z "${WALLY_NO_DAEMON:-}" ]; then
    "$BIN" daemon install >/dev/null
    daemon_note="The daemon is running and starts at login."
  else
    daemon_note="Start the daemon with: wally daemon install"
  fi

  "$BIN" banner
  printf '%s%s is now installed.%s %s\n\n' "$BOLD" "$("$BIN" --version)" "$RESET" "$daemon_note"
  case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *) printf '%sAdd %s to your PATH:%s\n  export PATH="%s:$PATH"\n\n' "$RUST" "$INSTALL_DIR" "$RESET" "$INSTALL_DIR" ;;
  esac
  printf '  wally status     is it running, where do images come from\n'
  printf '  wally show       which image is on which screen\n'
  printf '  wally next       change now\n'
  printf '  wally help       everything else\n\n'
}

main "$@"
