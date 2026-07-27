#!/usr/bin/env bash
# File: tools/fix-doc.sh
#
# Generate an AI-assisted documentation fix report.
#
# Usage:
#   tools/fix-doc.sh <document> <file-or-directory>...
#
# Example:
#   tools/fix-doc.sh docs/TOOLS.md tools

set -euo pipefail

export REPO_ROOT="${REPO_ROOT:-"$(git rev-parse --show-toplevel)"}"
export GENERATED_FILE_PREFIX="${GENERATED_FILE_PREFIX:-fix-doc}"
export TAI_TRANSCRIPT_DIR="${REPO_ROOT}/.generated/transcripts"
export TAI_BIN="${TAI_BIN:-"${REPO_ROOT}/target/release/terminal-ai"}"
export TAI_MODEL="${TAI_MODEL:-gpt-5.5}"
REPORT_PATH="${REPO_ROOT}/.generated/${GENERATED_FILE_PREFIX}-$(uuidgen -7)"
export REPORT_PATH
export PROMPT="${PROMPT:-"This is a one shot prompt, do not ask followups. Fix the provided documentation file using the referenced files and directories as the source of truth. Only propose changes to the documentation document. Provide an extensive and complete solution, preferably as a unified diff or complete replacement content."}"

usage() {
  cat >&2 <<EOF
Usage:
  $(basename "$0") <document> <file-or-directory>...

Arguments:
  document             Documentation file to fix.
  file-or-directory    One or more source files or directories to use as context.

Environment:
  REPO_ROOT            Repository root. Defaults to git top-level.
  TAI_BIN              terminal-ai binary. Defaults to target/release/terminal-ai.
  TAI_MODEL            Model name. Defaults to ${TAI_MODEL}.
  PROMPT               Override the default documentation-fix prompt.
EOF
}

resolve_path() {
  local path="$1"

  if [[ -e "${path}" ]]; then
    realpath "${path}"
  elif [[ -e "${REPO_ROOT}/${path}" ]]; then
    realpath "${REPO_ROOT}/${path}"
  else
    printf 'error: path does not exist: %s\n' "${path}" >&2
    return 1
  fi
}

render_file() {
  local file="$1"
  local relative_path

  relative_path="$(realpath --relative-to="${REPO_ROOT}" "${file}" 2>/dev/null || realpath "${file}")"

  printf '\n─────┬──────────────────────────────────────────────────────────────────────────\n'
  printf '     │ File: %s\n' "${relative_path}"
  printf '─────┼──────────────────────────────────────────────────────────────────────────\n'
  sed '=' "${file}" | sed 'N;s/\n/\t/' | sed 's/^/ /'
  printf '─────┴──────────────────────────────────────────────────────────────────────────\n'
}

render_context() {
  local doc_path="$1"
  shift

  printf '# Documentation file to fix\n'
  render_file "${doc_path}"

  printf '\n# Reference files\n'

  while IFS= read -r file; do
    render_file "${file}"
  done < <(
    for path in "$@"; do
      if [[ -f "${path}" ]]; then
        realpath "${path}"
      elif [[ -d "${path}" ]]; then
        fd . "${path}" -t f -x realpath {}
      fi
    done | sort -u
  )
}

if [[ "$#" -lt 2 ]]; then
  usage
  exit 2
fi

DOC_PATH="$(resolve_path "$1")"
shift

if [[ ! -f "${DOC_PATH}" ]]; then
  printf 'error: document must be a file: %s\n' "${DOC_PATH}" >&2
  exit 1
fi

CONTEXT_PATHS=()
for path in "$@"; do
  CONTEXT_PATHS+=("$(resolve_path "${path}")")
done

mkdir -p "${REPO_ROOT}/.generated"

"${TAI_BIN}" \
  -o NO_USER_STDOUT \
  --input-file=<(render_context "${DOC_PATH}" "${CONTEXT_PATHS[@]}") \
  --model="${TAI_MODEL}" \
  -- "${PROMPT}" >>"${REPORT_PATH}"

GENERATED_FILE_PREFIX="${GENERATED_FILE_PREFIX}" "${REPO_ROOT}/tools/show-fix.sh"
