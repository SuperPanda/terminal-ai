#!/usr/bin/env sh
# File: tools/create-completions.sh
#
# Description:
#   Creates completion files for fish, zsh, and bash using the
#   configurations from .env
#
# See also: terminal-ai(1)

GENERATED_RESOURCE_PREFIX="complete"
TAI_INPUT_ARGS="--input-file=Cargo.toml --input-file=src/main.rs"

REPO_ROOT="${REPO_ROOT:-"$(git rev-parse --show-toplevel)"}"
BASE_PATH="${BASE_PATH:-$REPO_ROOT}"
export BASE_PATH

if [ -f "$BASE_PATH/.env" ]; then
  . "$BASE_PATH/.env"
fi

DEFAULT_TAI_MODEL="${DEFAULT_TAI_MODEL:?error: DEFAULT_TAI_MODEL not configured in Environment Variables (use .env file)}"
DEFAULT_APP_NAME="${DEFAULT_APP_NAME:?error: DEFAULT_APP_NAME not configured in Environment Variables (use .env file)}"
DEFAULT_TAI_OUTPUT_CONTROLS="${DEFAULT_TAI_OUTPUT_CONTROLS:?error: DEFAULT_TAI_OUTPUT_CONTROLS not configured in Environment Variables (use .env file)}"

# The .env file for terminal-ai.
model="${TAI_MODEL:-$DEFAULT_TAI_MODEL}"

# The app this project is expected to build.
appname="${APP_NAME:-$DEFAULT_APP_NAME}"

output_controls="${TAI_OUTPUT_CONTROLS:-$DEFAULT_TAI_OUTPUT_CONTROLS}"

completion_instructions="${DEFAULT_COMPLETION_INSTRUCTIONS:-provide the capability to provide parameter completions for known values}"

# PATH .generated/
generated_output_directory="$BASE_PATH/.generated"
mkdir -p "$generated_output_directory"

# PATH completions/
completions_directory="$BASE_PATH/completions"
mkdir -p "$completions_directory"

# This should be moved into .env.
export TAI_TRANSCRIPT_DIR="${DEFAULT_TAI_TRANSCRIPT_DIR:-${generated_output_directory}/transcripts}"
mkdir -p "$TAI_TRANSCRIPT_DIR"

for completion_shell in fish zsh bash; do
  # uuidv7 is a locally managed tool.
  UUID="$(uuidv7 || uuidgen -7)"

  # Example:
  # .generated/complete-fish-019fc356-4ee3-7e22-ae3a-2d80ed4d01c2
  output_file="$generated_output_directory/$GENERATED_RESOURCE_PREFIX-$completion_shell-$UUID"

  terminal-ai \
    --model="$model" \
    -o "$output_controls" \
    --output-file="$output_file" \
    $TAI_INPUT_ARGS \
    -- "create $completion_shell completions for $appname; $completion_instructions" |
    tee "$completions_directory/$appname.$completion_shell.new"
done
