#!/usr/bin/env bash

BASE_PATH="$(pwd)"
PREFIX=$HOME/.local
DESTDIR=""
DATADIR="${XDG_DATA_HOME:-$HOME/.local/share}"
BINDIR="$PREFIX/bin"
MANDIR=$DATADIR/man
FISH_COMPLETION_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/fish/completions"
ZSH_COMPLETION_DIR=$DATADIR/zsh/site-functions
BASH_COMPLETION_DIR=$DATADIR/bash-completion/completions
install_optional() {
  local source="$1"
  local destination="$2"

  if [[ ! -e "$source" && ! -L "$source" ]]; then
    printf 'Skipping missing optional resource: %s\n' "$source" >&2
    return 0
  fi

  if [[ ! -f "$source" || ! -r "$source" ]]; then
    printf 'Cannot read resource: %s\n' "$source" >&2
    return 1
  fi

  install -Dm0644 -- "$source" "${DESTDIR}${destination}"
}

RUNTIME_VERSION="$(terminal-ai --version || echo "Not Found")"
# Should probably use the git commited cargo version.
CARGO_VERSION="$(grep -P '^version = ' $BASE_PATH/Cargo.toml | grep -oP '".*"' | tr -d '"')"
DEV_BUILD_VERSION="$($BASE_PATH/target/debug/terminal-ai --version)"
RELEASE_BUILD_VERSION="$($BASE_PATH/target/release/terminal-ai --version)"

#
# cargo build --release
#
printf "%s: %s\n" "Run time version" "$RUNTIME_VERSION"
printf "%s: %s\n" "Cargo.toml version" "$CARGO_VERSION"
printf "%s: %s\n" "DEV BUILD VERSION" "$DEV_BUILD_VERSION"
printf "%s: %s\n" "RELEASE_BUILD_VERSION" "$RELEASE_BUILD_VERSION"

if [[ ! "$CARGO_VERSION"="$DEV_BUILD_VERSION" ]]; then
  echo "Dev Build Required"
  exit
fi
if [[ ! "$DEV_BUILD_VERSION"="$RELEASE_BUILD_VERSION" ]]; then
  echo "RELEASE Build Required"
  exit
fi
if [[ ! "$RELEASE_BUILD_VERSION"="$RUNTIME_VERSION" ]]; then
  echo "Already Up to date"
  exit
fi
if [ "$CARGO_VERSION"="$RELEASE_BUILD_VERSION" ]; then echo "Updating $RUNTIME_VERSION to $RELEASE_BUILD_VERSION"; fi

install_optional \
  "$BASE_DIR/man/terminal-ai.1" \
  "$MANDIR/man1/terminal-ai.1"

install_optional \
  "$BASE_DIR/completions/terminal-ai.fish" \
  "$FISH_COMPLETION_DIR/terminal-ai.fish"

install_optional \
  "$BASE_DIR/completions/terminal-ai.zsh" \
  "$ZSH_COMPLETION_DIR/_terminal-ai"

install_optional \
  "$BASE_DIR/completions/terminal-ai.bash" \
  "$BASH_COMPLETION_DIR/terminal-ai"

install "$BASE_PATH/target/release/terminal-ai" "$BINDIR/terminal-ai"
# install "$BASE_PATH/bin/terminal-ai" "$HOME/.local/bin/terminal-ai"
install "$BASE_PATH/utils/tai-today" "$BINDIR/tai-today"
install "$BASE_PATH/utils/tai-get-message $BINDIR/tai-get-message"

echo "installed"
