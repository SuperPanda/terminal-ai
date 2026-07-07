#!/usr/bin/env bash
# File : tools/generate-commit-msg.sh
#
# If use tool from a different workspaces external to the tool, use environment variables,
# from the tool's execution target repo.
# TAI_BIN="$(realpath <PATH_TO_TAI_BINARY_TU_USE>/<debug|release>/terminal-ai)" REPO_ROOT=$(pwd) ../terminal-ai-rs.draft/tools/generate-commit-msg.sh
#
set -euo pipefail

export GENERATED_FILE_PREFIX="changes"
export REPO_ROOT=${REPO_ROOT:-"$(git rev-parse --show-toplevel)"}
export TAI_BIN="${TAI_BIN:-${REPO_ROOT}/target/debug/terminal-ai}"
export PROMPT="${PROMPT:-"<INSTRUCTION> Write a git commit message for the provided git diff --cached output using conventional git commit messages respond suitable for direct inclusion as a git commit multi-line message (plain text not markdown). Extra information is provided in the body structured for navigation, discoverability and changelogs. Using syntax to improve readability, such as single quotes around filepaths, and configuration env variables, parameters and arguments. <END INSTRUCTION> \n"}"
export TAI_MODEL="${TAI_MODEL:-gpt-5.5}"
export TAI_ARGS="-o NO_USER_STDOUT,CONTENT_ONLY --model=${TAI_MODEL}"
export TAI_TOOLS_PATH="${TAI_TOOLS_PATH:-"${REPO_ROOT}/tools"}"
export REPORT_PATH="${REPO_ROOT}/.generated/${GENERATED_FILE_PREFIX}-$(uuidgen -7)"
mkdir -p "${REPO_ROOT}/.generated"
# xargs -0 is just how terminal-ai works to allow it work handle multiple lines of input (until streaming implementation)
git -C ${REPO_ROOT} diff --cached | TAI_STDOUT_HEADER=0 TAI_STDOUT_INDENT=0 TAI_STDOUT_SEP=0 xargs -I{} -0 $TAI_BIN $TAI_ARGS -- "${PROMPT}\n{}" >"${REPORT_PATH}"
"${TAI_TOOLS_PATH}/view-last-commit-msg.sh"
