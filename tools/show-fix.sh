#!/usr/bin/env bash
export REPO_ROOT=$(git rev-parse --show-toplevel)
export GENERATED_FILE_PREFIX="${GENERATED_FILE_PREFIX:-fix}"
export LATEST_FIX_FILE=$(fd "${GENERATED_FILE_PREFIX}" "${REPO_ROOT}/.generated" -x realpath --relative-to="${REPO_ROOT}" "{}" | sort | tail -n1)
if [ "${1}" == "--pipe" ]; then
  cat "${REPO_ROOT}/${LATEST_FIX_FILE}"
else
  mdless --ansi "${REPO_ROOT}/${LATEST_FIX_FILE}"
fi
