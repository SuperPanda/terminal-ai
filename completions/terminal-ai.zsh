#compdef terminal-ai

# Complete known values without restricting what users can enter.
#
# Arguments:
#   1: zstyle name
#   2: description
#   remaining arguments: built-in/environment suggestions
_terminal_ai_known_values() {
  local style_name=$1 description=$2
  shift 2

  local -aU values
  local -a configured expl
  local value

  values=()
  for value in "$@"; do
    [[ -n $value ]] && values+=("$value")
  done

  if zstyle -a ":completion:${curcontext}:" "$style_name" configured; then
    for value in "${configured[@]}"; do
      [[ -n $value ]] && values+=("$value")
    done
  fi

  if (( ${#values} )); then
    _wanted "$style_name" expl "$description" compadd -a values
  else
    _message "$description"
  fi
}

_terminal_ai_models() {
  _terminal_ai_known_values models 'model name' \
    gpt-5.4 "${TAI_MODEL-}"
}

_terminal_ai_base_urls() {
  _terminal_ai_known_values base-urls 'API base URL' \
    https://api.openai.com "${TAI_BASE_URL-}"
}

_terminal_ai_parent_ids() {
  _terminal_ai_known_values parent-ids 'parent message ID'
}

_terminal_ai_transcript_files() {
  # These are filenames relative to --transcript-dir, not necessarily
  # paths relative to the shell's current directory.
  _terminal_ai_known_values transcript-files 'transcript filename' \
    "${TAI_TRANSCRIPT_FILE-}"
}

_terminal_ai_output_control() {
  local -a operations channels tokens
  local operation channel speaker

  # Exact operations accepted by apply_format_token(), including aliases.
  operations=(
    CONTENT_ONLY
    PROMPT_ONLY
    METADATA_ONLY
    HEADER_ONLY
    FULL
    SILENT
    DISABLE
    ENABLE

    NO_HEADER
    NO_HEADER_FLAG
    NO_METADATA
    NO_PROMPT
    NO_CONTENT
    NO_INDENT
    NO_SEP
    NO_SEPARATOR
    NO_SPEAKER

    ADD_HEADER
    WITH_HEADER
    ADD_METADATA
    WITH_METADATA
    ADD_PROMPT
    WITH_CONTENT
    ADD_INDENT
    WITH_INDENT
    ADD_SEP
    WITH_SEP
    ADD_SPEAKER
    WITH_SPEAKER
  )

  # FILE is the legacy alias for TRANSCRIPT.
  channels=(STDOUT TRANSCRIPT ARTIFACT FILE)

  tokens=()
  for operation in "${operations[@]}"; do
    # Unsuffixed operations apply to all channels.
    tokens+=("$operation")

    for channel in "${channels[@]}"; do
      tokens+=("${operation}_${channel}")
    done
  done

  # Speaker suppression requires a channel suffix.
  for speaker in USER AI; do
    for channel in "${channels[@]}"; do
      tokens+=("NO_${speaker}_${channel}")
    done
  done

  # Complete individual tokens inside a comma-separated argument.
  _values -s , 'output-control token' "${tokens[@]}"
}

# Do not use _arguments -s: the application does not parse bundled
# short options. The trailing "=" in value-taking option specifications
# supports either a separate argument or an =VALUE argument.
#
# -S stops option completion after "--".
_arguments -S \
  '(-h --help -V --version)'{-h,--help}'[Show help and exit]' \
  '(-h --help -V --version)'{-V,--version}'[Show version and exit]' \
  '--dry-run[Skip the API call and emit a placeholder response]' \
  '--verbose[Emit raw API response JSON to stderr]' \
  '--no-transcript[Disable transcript writing]' \
  '--overwrite-output-file[Overwrite artifact output instead of appending]' \
  '(-m --model)'{-m,--model}'=[Model name]:model:_terminal_ai_models' \
  '(-b --base-url)'{-b,--base-url}'=[API base URL]:URL:_terminal_ai_base_urls' \
  '*'{-i,--input-file}'=[Attach a file to the prompt]:input file:_files' \
  '(-O --output-file)'{-O,--output-file}'=[Artifact output path]:output file:_files' \
  '*'{-o,--output-control}'=[Output-control tokens, comma-separated or repeated]:output control:_terminal_ai_output_control' \
  '--env-file=[Configuration environment file]:environment file:_files' \
  '--parent-id=[Parent message ID for threading]:parent ID:_terminal_ai_parent_ids' \
  '(--transcript-dir --output-dir)--transcript-dir=[Transcript directory]:transcript directory:_files -/' \
  '(--transcript-dir --output-dir)--output-dir=[Alias for --transcript-dir]:transcript directory:_files -/' \
  '--transcript-file=[Filename within the transcript directory]:transcript filename:_terminal_ai_transcript_files' \
  '*:prompt:_message "prompt text"'
```

### Install

From the project directory:

```sh
mkdir -p ~/.local/share/zsh/site-functions
cp completions/_terminal-ai ~/.local/share/zsh/site-functions/
```

Add this to `~/.zshrc`, placing the `fpath` modification **before** your existing `compinit` invocation:

```zsh
fpath=("$HOME/.local/share/zsh/site-functions" $fpath)

autoload -Uz compinit
compinit
```

If a shell framework already runs `compinit`, only add the `fpath` modification before the framework initializes.

To register the completion immediately in your current shell, after `compinit` has run:

```zsh
fpath=("$HOME/.local/share/zsh/site-functions" $fpath)
autoload -Uz _terminal-ai
compdef _terminal-ai terminal-ai
```

### Configure known parameter values

Add suggestions using `zstyle` in `~/.zshrc`:

```zsh
zstyle ':completion:*:*:terminal-ai:*' models \
  'gpt-5.4' \
  'my-local-model' \
  'my-custom-deployment'

# terminal-ai appends /v1/chat/completions to the base URL.
zstyle ':completion:*:*:terminal-ai:*' base-urls \
  'https://api.openai.com' \
  'http://localhost:11434' \
  'http://localhost:8080'

zstyle ':completion:*:*:terminal-ai:*' parent-ids \
  '01234567-89abcdef' \
  '89abcdef-01234567'

zstyle ':completion:*:*:terminal-ai:*' transcript-files \
  'notes.md' \
  'development.md' \
  'research.md'

# These suggestions supplement the built-in defaults and relevant process-environment values; they do not restrict accepted input. Completion does not run `terminal-ai`, contact the API, or source environment files.
# 
# ### Try it
# 
# Press **Tab** at these positions:
# 
# ```text
# terminal-ai --model <Tab>
# terminal-ai --model=<Tab>
# terminal-ai -m=<Tab>
# terminal-ai --base-url <Tab>
# terminal-ai --input-file <Tab>
# terminal-ai --transcript-dir <Tab>
# terminal-ai -o CONTENT_<Tab>
# terminal-ai -o CONTENT_ONLY_STDOUT,NO_USER_<Tab>
# terminal-ai --output-control=FULL_TRANSCRIPT,NO_METADATA_<Tab>
# ```
# 
# You can check the completion file’s syntax with:
# 
# ```sh
# zsh -n completions/_terminal-ai
# ```
