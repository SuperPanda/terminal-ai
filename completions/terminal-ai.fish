# completions/terminal-ai.fish
#
# Optional fish lists for additional parameter suggestions:
#   TAI_COMPLETION_MODELS
#   TAI_COMPLETION_BASE_URLS
#   TAI_COMPLETION_PARENT_IDS
#   TAI_COMPLETION_TRANSCRIPT_FILES
#
# These are suggestions, not restrictions on accepted values.
# Completion never invokes terminal-ai or makes network requests.

function __terminal_ai_models
    printf '%s\n' gpt-5.4 $TAI_MODEL $TAI_COMPLETION_MODELS |
        string match -r '.+' |
        sort -u
end

function __terminal_ai_base_urls
    printf '%s\n' https://api.openai.com $TAI_BASE_URL $TAI_COMPLETION_BASE_URLS |
        string match -r '.+' |
        sort -u
end

function __terminal_ai_output_control
    # Preserve previously entered tokens in a comma-separated specification:
    #   FULL_TRANSCRIPT,NO_H<TAB>
    # becomes:
    #   FULL_TRANSCRIPT,NO_HEADER...
    #
    # Fish handles the option prefix for --output-control=VALUE.
    set -l value (commandline -ct)
    if not set -q value[1]
        set value ''
    end

    set value (string replace -r '^(-o|--output-control)=' '' -- "$value")

    set -l prefix ''
    if string match -q '*,*' -- "$value"
        set prefix (string replace -r '[^,]*$' '' -- "$value")
    end

    # Keep these operations synchronized with apply_format_token().
    set -l operations \
        CONTENT_ONLY PROMPT_ONLY METADATA_ONLY HEADER_ONLY \
        FULL SILENT DISABLE ENABLE \
        NO_HEADER NO_HEADER_FLAG NO_METADATA NO_PROMPT NO_CONTENT \
        NO_INDENT NO_SEP NO_SEPARATOR NO_SPEAKER \
        ADD_HEADER WITH_HEADER ADD_METADATA WITH_METADATA \
        ADD_PROMPT WITH_CONTENT ADD_INDENT WITH_INDENT \
        ADD_SEP WITH_SEP ADD_SPEAKER WITH_SPEAKER

    # FILE is the legacy alias for TRANSCRIPT.
    set -l channels STDOUT TRANSCRIPT ARTIFACT FILE

    for operation in $operations
        printf '%s%s\tAll output channels\n' "$prefix" "$operation"

        for channel in $channels
            printf '%s%s_%s\tFormat %s channel\n' \
                "$prefix" "$operation" "$channel" "$channel"
        end
    end

    # Speaker suppression requires a channel suffix.
    for speaker in USER AI
        for channel in $channels
            printf '%sNO_%s_%s\tSuppress %s turns in %s\n' \
                "$prefix" "$speaker" "$channel" "$speaker" "$channel"
        end
    end
end

# Positional arguments are prompt text, not filenames.
complete -c terminal-ai -f

# General switches.
complete -c terminal-ai -s h -l help \
    -d 'Show help'
complete -c terminal-ai -s V -l version \
    -d 'Show version'
complete -c terminal-ai -l dry-run \
    -d 'Skip API call and emit placeholder response'
complete -c terminal-ai -l verbose \
    -d 'Emit raw API response JSON to stderr'

# Backend parameters: required arguments, no filename completion.
complete -c terminal-ai -s m -l model -x \
    -a '(__terminal_ai_models)' \
    -d 'Model name'
complete -c terminal-ai -s b -l base-url -x \
    -a '(__terminal_ai_base_urls)' \
    -d 'API base URL'

# File parameters: explicitly enable filename completion.
complete -c terminal-ai -s i -l input-file -r -F \
    -d 'Attach file to prompt; repeatable'
complete -c terminal-ai -l env-file -r -F \
    -d 'Configuration env file'
complete -c terminal-ai -s O -l output-file -r -F \
    -d 'Artifact output path'
complete -c terminal-ai -l overwrite-output-file \
    -d 'Overwrite artifact instead of appending'

# Transcript parameters.
complete -c terminal-ai -l transcript-dir -x \
    -a '(__fish_complete_directories)' \
    -d 'Transcript directory'
complete -c terminal-ai -l output-dir -x \
    -a '(__fish_complete_directories)' \
    -d 'Alias for --transcript-dir'
complete -c terminal-ai -l transcript-file -x \
    -a '$TAI_TRANSCRIPT_FILE $TAI_COMPLETION_TRANSCRIPT_FILES' \
    -d 'Filename within the transcript directory'
complete -c terminal-ai -l no-transcript \
    -d 'Disable transcript logging'

# Threading: IDs are free-form, with optional user-maintained suggestions.
complete -c terminal-ai -l parent-id -x \
    -a '$TAI_COMPLETION_PARENT_IDS' \
    -d 'Parent message ID for branching'

# Formatting: repeatable, optionally comma-separated.
complete -c terminal-ai -s o -l output-control -x \
    -a '(__terminal_ai_output_control)' \
    -d 'Output-control tokens; comma-separated or repeatable'

# ### Install
# 
# Run in fish from the project directory:
# 
# ```fish
# mkdir -p "$__fish_config_dir/completions"
# cp completions/terminal-ai.fish "$__fish_config_dir/completions/terminal-ai.fish"
# 
# # Load into the current shell.
# source "$__fish_config_dir/completions/terminal-ai.fish"
# ```
# 
# ### Configure known parameter values
# 
# Use fish universal variables to persist suggestions across sessions:
# 
# ```fish
# set -U TAI_COMPLETION_MODELS gpt-5.4 my-local-model another-model
# 
# set -U TAI_COMPLETION_BASE_URLS \
#     https://api.openai.com \
#     http://localhost:11434 \
#     http://localhost:8080
# 
# set -U TAI_COMPLETION_TRANSCRIPT_FILES default.md work.md notes.md
# 
# set -U TAI_COMPLETION_PARENT_IDS 12345678-abcdef01
# ```
# 
# Model and URL suggestions also include the current process values of `TAI_MODEL` and `TAI_BASE_URL`. The script deliberately does not source env files or query the backend.
# 
# ### Check completion results
# 
# ```fish
# complete -C 'terminal-ai --model '
# complete -C 'terminal-ai --base-url='
# complete -C 'terminal-ai --input-file '
# complete -C 'terminal-ai --transcript-dir '
# complete -C 'terminal-ai -o NO_HEADER'
# complete -C 'terminal-ai --output-control=FULL_TRANSCRIPT,NO_USER_'
# ```
# 
# The last example preserves `FULL_TRANSCRIPT,` while completing the next token.
