#!/usr/bin/env sh
# File: tools/create-manual.sh
#
# Description:
#   Creates man/<APP_NAME>.1.new using the configurations from .env
#
# See also: terminal-ai(1)

GENERATED_RESOURCE_PREFIX="man"
TAI_INPUT_ARGS="--input-file=Cargo.toml --input-file=src/main.rs"

REPO_ROOT="${REPO_ROOT:-"$(git rev-parse --show-toplevel)"}"
BASE_PATH="${BASE_PATH:-$REPO_ROOT}"
export BASE_PATH
if [ -f $BASE_PATH/.env ]; then . "$BASE_PATH/.env"; fi

DEFAULT_TAI_MODEL="${DEFAULT_TAI_MODEL:?error: DEFAULT_TAI_MODEL not configured in Environment Variables (use .env file)}"
DEFAULT_APP_NAME="${DEFAULT_APP_NAME:?error: DEFAULT_APP_NAME not configured in the Environment Variables (use .env file)}"
DEFAULT_TAI_OUTPUT_CONTROLS="${DEFAULT_TAI_OUTPUT_CONTROLS:?error: DEFAULT_TAI_OUTPUT_CONTROLS not configured in the Environment Variables (use .env file)}"

# The .env file for terminal-ai
model="${TAI_MODEL:-$DEFAULT_TAI_MODEL}"
# The app this project is expected to build for.
appname="${APP_NAME:-$DEFAULT_APP_NAME}"
output_controls="${TAI_OUTPUT_CONTROLS:-$DEFAULT_TAI_OUTPUT_CONTROLS}"

generate_manual_instructions="${DEFAULT_MAN_INSTRUCTIONS:-generate a $appname(1) man artifact covering environment variables, synopsis, defaults, examples, and other relevant section}"
# The UUID tool. uuidv7 is a locally managed tool..
UUID="$(uuidv7 || uuidgen -7)"

# PATH .generated/
generated_output_directory="$BASE_PATH/.generated"
mkdir -p "$generated_output_directory"

# Example Output Path .generated/man-019fc356-4ee3-7e22-ae3a-2d80ed4d01c2
output_file="$generated_output_directory/$GENERATED_RESOURCE_PREFIX-$UUID"

# This should be moved into .env
export TAI_TRANSCRIPT_DIR="${DEFAULT_TAI_TRANSCRIPT_DIR:-${generated_output_directory}/transcripts}"
mkdir -p "$TAI_TRANSCRIPT_DIR"

# ~/repos/<PROJECT>/man
mkdir -p "$BASE_PATH/$GENERATED_RESOURCE_PREFIX"

# ~/repos/<PROJECT>/.generated
mkdir -p "$generated_output_directory"

terminal-ai --model="$model" -o "$output_controls" --output-file="$output_file" $TAI_INPUT_ARGS -- "$generate_manual_instructions" | tee "man/$appname.1.new"
