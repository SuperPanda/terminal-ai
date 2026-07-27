#!/usr/bin/env bash
REPO_ROOT=$(git rev-parse --show-toplevel)
export REPO_ROOT
export GENERATED_FILE_PREFIX="shellcheck"
export TARGET_PATH="${REPO_ROOT}/tools"
REPORT_PATH="${REPO_ROOT}/.generated/${GENERATED_FILE_PREFIX}-$(uuidgen -7)"
export REPORT_PATH
export TAI_BIN="${REPO_ROOT}/target/release/terminal-ai"
export PROMPT="${1:-"This is a one shot prompt, do not ask followups and extensive and complete solutions and alternatives."}"
mkdir -p "${TARGET_PATH}"
fd . -e sh "${REPO_ROOT}" -x realpath {} | xargs shellcheck 2>&1 | xargs -I{} -0 "${TAI_BIN}" -o NO_USER_STDOUT --input-file=<(fd . -e sh "${TARGET_PATH}" -x realpath {} | parallel bat --decorations=always) --model=gpt-5.4 -- "${PROMPT}\n{}" >>"${REPORT_PATH}"
"${REPO_ROOT}/tools/show-fix.sh"
