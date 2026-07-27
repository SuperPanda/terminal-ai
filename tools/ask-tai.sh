#!/usr/bin/env sh
mkdir -p .generated/transcripts
terminal-ai --transcript-dir=.generated/transcripts --output-file=.generated/tai-message-$(uuidgen -7) --model=gpt-5.5 --input-file=src/main.rs -- "${1}"
