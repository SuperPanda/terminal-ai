#!/usr/bin/env bash
REPO_ROOT=$(git rev-parse --show-toplevel)
fd changes "${REPO_ROOT}/.generated" | sort | tail -n1 | xargs cat 
