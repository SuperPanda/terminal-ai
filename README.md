# terminal-ai

A pipeline-safe terminal AI client that records conversations as structured Markdown transcripts.
Each exchange is written independently to three output channels, each with its own rendering policy,
so the same binary pipes cleanly between shell tools while archiving full metadata for later replay.

```sh
cargo build --release
```

Runtime dependencies: `curl` (HTTP), `sha256sum` (message IDs), `date` (timestamps). All standard on Linux.

---

## What it does

### Three output channels

Every turn is rendered independently to each enabled sink:

| Channel      | Purpose                          | Default format          |
|--------------|----------------------------------|-------------------------|
| `stdout`     | Clean AI response for pipelines  | Content only            |
| `transcript` | Append-only archival log         | Header + indent + `---` |
| `artifact`   | Explicit per-run output file     | Configurable            |

`stderr` is a fourth channel reserved for metadata, errors, and interactive prompts.
It never carries conversation content.

```sh
# Count words in a response without contamination
terminal-ai "list ten unix commands" | wc -w

# Full transcript format on stdout for inspection
TAI_STDOUT_HEADER=1 TAI_STDOUT_INDENT=1 TAI_STDOUT_SEP=1 terminal-ai --dry-run "test"
```

### Message IDs and conversation threading

Every turn is identified by a deterministic hash of who spoke and when, paired with
the ID of the turn it responds to. This forms a directed graph across the transcript
without a database.

```
SpeechID   := HASH8(Speaker, Timestamp)
AudienceID := parent SpeechID  |  HASH8("NULL") for roots
MessageID  := "<SpeechID>-<AudienceID>"
```

`--parent-id` attaches a new turn to an existing prior speech, enabling branching
and multi-pass workflows across separate invocations.

See [`docs/concepts/message-ids.md`](docs/concepts/message-ids.md) for the full graph model.

---

## Build

```sh
cargo build --release          # production binary
cargo build                    # debug binary (used by generate-commit-msg.sh for dogfooding)
```

Cargo workspace: `Cargo.toml` at repo root. Single binary: `terminal-ai`.

---

## Use

### Prompt sources

Prompts are resolved in order:

1. Trailing CLI words: `terminal-ai "explain monads"`
2. `--` separator: `terminal-ai -- explain monads`
3. Piped stdin: `echo "explain monads" | terminal-ai`
4. Interactive: invoked with no arguments in a TTY

Multi-line input via `xargs -0`:

```sh
cat context.txt | xargs -0 terminal-ai "summarise this"
```

### Pipeline usage

`stdout` is content-only by default. Pipe freely:

```sh
terminal-ai "list ten unix commands" | fzf
terminal-ai "generate a config" | tee ./out.conf
terminal-ai "question" | head -n1   # exits cleanly with code 0
```

### Backend switching

Any OpenAI-compatible endpoint. Bearer header is sent only when `OPENAI_API_KEY` is set:

```sh
TAI_BASE_URL=http://localhost:8080                           terminal-ai "…"   # llama.cpp, keyless
TAI_BASE_URL=https://api.deepseek.com OPENAI_API_KEY=sk-… terminal-ai "…"
TAI_BASE_URL=https://api.openai.com   OPENAI_API_KEY=sk-… terminal-ai "…"
```

Transcripts record the model name reported by the backend, so switching backends
is attributable without any other change.

### Output control

Fine-grained suppression and formatting via `-o`:

```sh
terminal-ai -o NO_USER_TRANSCRIPT "question"      # suppress user turn from log
terminal-ai -o CONTENT_ONLY_STDOUT "question"     # no header/sep on stdout
terminal-ai -o NO_METADATA "question"             # drop usage metadata from all channels
terminal-ai -o NO_AI_STDOUT,NO_USER_TRANSCRIPT …  # combined, comma-separated
```

Full token grammar: [`docs/CONFIGURATION.md#output-control`](docs/CONFIGURATION.md).

### Transcript inspection

Transcripts are append-only dated Markdown files under `$XDG_DATA_HOME/terminal-ai/transcripts/default/`.

```sh
grep '^\[' ~/.local/share/terminal-ai/transcripts/default/$(date --rfc-3339=date).md
```

---

## Configure

Resolution order, highest to lowest:

```
CLI flag  →  process env  →  TAI_ENV_FILE / --env-file  →  config.env  →  .default.env  →  default
```

Core variables:

| Variable            | CLI flag              | Default                        | Purpose                         |
|---------------------|-----------------------|--------------------------------|---------------------------------|
| `TAI_MODEL`         | `-m, --model`         | `gpt-5.4`                      | Model name                      |
| `TAI_BASE_URL`      | `-b, --base-url`      | `https://api.openai.com`       | API base URL                    |
| `OPENAI_API_KEY`    | —                     | —                              | Bearer token                    |
| `TAI_TRANSCRIPT_DIR`| `--transcript-dir`    | `~/.local/share/…/default`     | Transcript directory            |
| `TAI_OUTPUT_FILE`   | `-O, --output-file`   | —                              | Artifact output path            |
| `TAI_SPEAKER_USER`  | —                     | `USER`                         | User turn label                 |
| `TAI_SPEAKER_AI`    | —                     | model name from response       | AI turn label override          |
| `TAI_NO_TRANSCRIPT` | `--no-transcript`     | `false`                        | Disable transcript writing      |

Full reference: [`docs/CONFIGURATION.md`](docs/CONFIGURATION.md).

---

## Develop

### Commit workflow

1. Stage a self-contained unit with GitUI
2. `./tools/generate-commit-msg.sh` — pipes `git diff --cached` to debug binary
3. Review with `./tools/view-last-commit-msg.sh`
4. Commit `git commit -m $(./tools/view-last-commit-msg.sh`

Generated files land in `.generated/` with UUIDv7 names. Never commit `.generated/`.

### LLM-assisted fixup

```sh
./tools/fix.sh            # clippy → AI → .generated/fix-<uuid>
./tools/fix-tools.sh      # shellcheck on tools/ → AI → .generated/shellcheck-<uuid>
./tools/show-fix.sh       # view latest fix report (mdless)
./tools/show-fix.sh --pipe  # or pipe it
```

Both fixup tools use the release binary. The commit message tool uses the debug binary
to surface regressions early.
