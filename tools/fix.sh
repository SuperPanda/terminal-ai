#!/usr/bin/env bash
export REPO_ROOT=$(git rev-parse --show-toplevel)
export GENERATED_FILE_PREFIX="fix"
export TAI_BIN="${REPO_ROOT}/target/release/terminal-ai"
export PROMPT="${1:-"This is a one shot prompt, do not ask followups and extensive and complete solutions and alternatives."}"
export REPORT_PATH="${REPO_ROOT}/.generated/${GENERATED_FILE_PREFIX}-$(uuidgen -7)"
mkdir -p "${REPO_ROOT}/.generated"
# xargs -0 is just how terminal-ai works to allow it work handle multiple lines of input.
cargo -Z unstable-options -C "${REPO_ROOT}" clippy 2>&1 | xargs -I{} -0 "${TAI_BIN}" -o NO_USER_STDOUT --input-file="$REPO_ROOT/src/main.rs" --model=gpt-5.4 -- "${PROMPT}\n{}" >> "$REPORT_PATH"
${REPO_ROOT}/tools/show-fix.sh
