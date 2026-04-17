#!/usr/bin/env bash

export REPO_ROOT=$(git rev-parse --show-toplevel)

# Generated files are <GENERATED_TYPE>-<UUIDv7>
FILES="$(fd . "$REPO_ROOT/.generated/" | parallel realpath --relative-to="${REPO_ROOT}" {})"

# (TIME,TYPE,RELATIVE_PATH_TO_REPO_ROOT) Tab Delimited
paste <(printf "$FILES\n"  | cut -f2- -d- | uuidparse --output TIME --noheading --raw | tr ' ' '\t' | sed 's/\\x20/ /' | sed 's/,[^\+-]+)//g' | sed -E 's/,[[:digit:]]+/ /')  <(printf "$FILES\n" | sed 's/[^\/]*\///g' | cut -f1 -d-) <(echo "$FILES"  | parallel realpath --relative-to=${REPO_ROOT} {})  | sort 


