# Bash completion for terminal-ai.
# Requires Bash >= 4 and bash-completion.
#
# Optional customization in ~/.bashrc:
#
#   TAI_COMPLETION_MODELS=(my-local-model another-model)
#   TAI_COMPLETION_BASE_URLS=(http://localhost:11434 http://localhost:8080)
#   TAI_COMPLETION_PARENT_IDS=(12345678-abcdef01)
#
# These arrays provide suggestions, not validation.

_terminal_ai_value_option() {
  # Normalize value-taking options to their canonical long names.
  case $1 in
  -m | --model) printf '%s\n' --model ;;
  -b | --base-url) printf '%s\n' --base-url ;;
  -i | --input-file) printf '%s\n' --input-file ;;
  -O | --output-file) printf '%s\n' --output-file ;;
  -o | --output-control) printf '%s\n' --output-control ;;
  --env-file) printf '%s\n' --env-file ;;
  --parent-id) printf '%s\n' --parent-id ;;
  --transcript-dir | --output-dir) printf '%s\n' --transcript-dir ;;
  --transcript-file) printf '%s\n' --transcript-file ;;
  *) return 1 ;;
  esac
}

_terminal_ai_output_tokens() {
  local operation channel speaker
  local -a operations=(
    CONTENT_ONLY PROMPT_ONLY METADATA_ONLY HEADER_ONLY
    FULL SILENT DISABLE ENABLE
    NO_HEADER NO_HEADER_FLAG NO_METADATA NO_PROMPT NO_CONTENT
    NO_INDENT NO_SEP NO_SEPARATOR NO_SPEAKER
    ADD_HEADER WITH_HEADER ADD_METADATA WITH_METADATA
    ADD_PROMPT WITH_CONTENT ADD_INDENT WITH_INDENT
    ADD_SEP WITH_SEP ADD_SPEAKER WITH_SPEAKER
  )
  local -a channels=(STDOUT TRANSCRIPT ARTIFACT FILE)

  for operation in "${operations[@]}"; do
    printf '%s\n' "$operation"
    for channel in "${channels[@]}"; do
      printf '%s_%s\n' "$operation" "$channel"
    done
  done

  # Speaker suppression requires a channel suffix.
  for speaker in USER AI; do
    for channel in "${channels[@]}"; do
      printf 'NO_%s_%s\n' "$speaker" "$channel"
    done
  done
}

_terminal_ai_known_values() {
  # Add cases here to supply known values for other parameters.
  # Emit one candidate per line; do not emit diagnostics to stdout.
  case $1 in
  --model)
    printf '%s\n' \
      "${TAI_MODEL:-gpt-5.4}" \
      "${TAI_COMPLETION_MODELS[@]}"
    ;;
  --base-url)
    printf '%s\n' \
      "${TAI_BASE_URL:-https://api.openai.com}" \
      "${TAI_COMPLETION_BASE_URLS[@]}"
    ;;
  --parent-id)
    printf '%s\n' "${TAI_COMPLETION_PARENT_IDS[@]}"
    ;;
  --output-control)
    _terminal_ai_output_tokens
    ;;
  esac
}

_terminal_ai_trim_wordbreaks() {
  # We reconstruct =, :, and , as part of the logical argument.
  # Readline may nevertheless replace only the portion after one of
  # those characters. Remove the retained prefix from our replies.
  local current=$1 character prefix trim_prefix=
  local i

  for character in '=' ':' ','; do
    if [[ $COMP_WORDBREAKS == *"$character"* &&
      $current == *"$character"* ]]; then
      prefix=${current%"$character"*}$character
      if ((${#prefix} > ${#trim_prefix})); then
        trim_prefix=$prefix
      fi
    fi
  done

  if [[ -n $trim_prefix ]]; then
    for i in "${!COMPREPLY[@]}"; do
      COMPREPLY[i]=${COMPREPLY[i]#"$trim_prefix"}
    done
  fi
}

_terminal_ai() {
  local cur prev cword
  local -a words
  COMPREPLY=()

  # Preserve URL colons, inline assignments, and comma-separated specs.
  _get_comp_words_by_ref -n '=,:' cur prev words cword

  local i word option= pending= inline_prefix=
  local value candidate match_value list_prefix= file_action=
  local -a options=(
    -h --help -V --version
    --dry-run --verbose --no-transcript --overwrite-output-file
    -m --model -b --base-url
    -i --input-file -O --output-file -o --output-control
    --env-file --parent-id
    --transcript-dir --output-dir --transcript-file
    --
  )

  # Track value-taking options and the end-of-options marker.
  # A "--" consumed as an option value is not an end marker.
  for ((i = 1; i < cword; i++)); do
    word=${words[i]}

    if [[ -n $pending ]]; then
      pending=
      continue
    fi

    [[ $word == -- ]] && return 0

    if [[ $word != *=* ]]; then
      if option=$(_terminal_ai_value_option "$word"); then
        pending=$option
      fi
    fi
  done

  option=$pending
  value=$cur

  # Recognize inline assignments only when not already consuming a value.
  if [[ -z $option && $cur == *=* ]]; then
    if option=$(_terminal_ai_value_option "${cur%%=*}"); then
      inline_prefix=${cur%%=*}=
      value=${cur#*=}
    else
      return 0
    fi
  fi

  if [[ -z $option ]]; then
    # Positional arguments are free-form prompts, not filenames.
    if [[ -z $cur || $cur == -* ]]; then
      for candidate in "${options[@]}"; do
        [[ $candidate == "$cur"* ]] &&
          COMPREPLY+=("$candidate")
      done
    fi
    return 0
  fi

  case $option in
  --input-file | --output-file | --env-file | --transcript-file)
    file_action=-f
    ;;
  --transcript-dir)
    file_action=-d
    ;;
  esac

  if [[ -n $file_action ]]; then
    # Preserve spaces in filenames; let Readline quote the results.
    compopt -o filenames
    while IFS= read -r candidate; do
      COMPREPLY+=("$inline_prefix$candidate")
    done < <(compgen "$file_action" -- "$value")

    _terminal_ai_trim_wordbreaks "$cur"
    return 0
  fi

  if [[ $option == --output-control && $value == *,* ]]; then
    # Complete only the final token, retaining previous tokens.
    list_prefix=${value%,*},
    value=${value##*,}
  fi

  match_value=$value
  if [[ $option == --output-control ]]; then
    # Rust accepts output-control tokens case-insensitively.
    match_value=${value^^}
  fi

  local -A seen=()
  while IFS= read -r candidate; do
    [[ -n $candidate ]] || continue
    [[ $candidate == "$match_value"* ]] || continue
    [[ ${seen["$candidate"]:-} == 1 ]] && continue
    seen["$candidate"]=1
    COMPREPLY+=("$inline_prefix$list_prefix$candidate")
  done < <(_terminal_ai_known_values "$option")

  _terminal_ai_trim_wordbreaks "$cur"
  return 0
}

# Deliberately omit -o default/-o bashdefault:
# unknown parameter values and free-form prompts should not suggest files.
complete -F _terminal_ai terminal-ai

# ### Install
#
# For automatic loading through `bash-completion`:
#
# ```bash
# completion_dir="${BASH_COMPLETION_USER_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/bash-completion}/completions"
#
# mkdir -p "$completion_dir"
# install -m 0644 completions/terminal-ai "$completion_dir/terminal-ai"
# ```
#
# Open a new Bash session, or load it into the current one:
#
# ```bash
# # bash-completion must already be loaded.
# source completions/terminal-ai
# ```
#
# ### Configure known parameter values
#
# Add these arrays to `~/.bashrc`:
#
# ```bash
# TAI_COMPLETION_MODELS=(
#     gpt-5.4
#     my-local-model
#     another-model
# )
#
# TAI_COMPLETION_BASE_URLS=(
#     https://api.openai.com
#     http://localhost:11434
#     http://localhost:8080
# )
#
# TAI_COMPLETION_PARENT_IDS=(
#     12345678-abcdef01
# )
# ```
#
# The current `TAI_MODEL` and `TAI_BASE_URL` environment values are also suggested. Configuration files are deliberately not sourced or executed.
#
# To add known values for another parameter, extend `_terminal_ai_known_values` using its canonical option name.
#
# ### Examples
#
# ```text
# terminal-ai --mo<TAB>
# terminal-ai --model <TAB>
# terminal-ai -m=my<TAB>
# terminal-ai --base-url=http://loc<TAB>
# terminal-ai --input-file ./src/<TAB>
# terminal-ai --transcript-dir ./<TAB>
# terminal-ai -o CONTENT_<TAB>
# terminal-ai -o CONTENT_ONLY_STDOUT,NO_USER_<TAB>
# terminal-ai --output-control=FULL_TRANS<TAB>
# ```
#
# `--transcript-file` uses filesystem completion relative to the shell’s current directory; the application still interprets that value within its configured transcript directory.
