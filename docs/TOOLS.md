# Tools

A small collection of repository-local helper scripts for linting, AI-assisted remediation, and commit-message generation.

These tools are intentionally lightweight and may evolve over time. They share a few conventions:

- generated outputs are written to `./.generated/`
- file names follow `<prefix>-<UUIDv7>`
- most tools discover the repository root with `git rev-parse --show-toplevel`
- AI-backed tools call the local `terminal-ai` binary

## Overview

    tools/
    ├─ fix-tools.sh
    ├─ fix.sh
    ├─ generate-commit-msg.sh
    ├─ list-generated-files.sh
    ├─ show-fix.sh
    └─ view-last-commit-msg.sh

### Tool summary

| Tool | Purpose | Output prefix | Default AI binary |
|---|---|---:|---|
| `tools/fix-tools.sh` | Run `shellcheck` against shell scripts and ask AI for fixes | `shellcheck` | `target/release/terminal-ai` |
| `tools/fix.sh` | Run `cargo clippy` and ask AI for fixes | `fix` | `target/release/terminal-ai` |
| `tools/generate-commit-msg.sh` | Generate a conventional commit message from `git diff --cached` | `changes` | `target/debug/terminal-ai` |
| `tools/list-generated-files.sh` | List generated files with timestamp, prefix, and path | n/a | n/a |
| `tools/show-fix.sh` | Show the latest generated fix-style report | inherited via `GENERATED_FILE_PREFIX` | n/a |
| `tools/view-last-commit-msg.sh` | Print the latest generated commit message | `changes` | n/a |

---

## Common conventions and best practices

These are the patterns already used by the scripts and should be preserved or extended when adding new tools.

### 1. Resolve paths from the repository root

Most scripts derive `REPO_ROOT` using:

```bash
git rev-parse --show-toplevel
```

This is the correct default because it allows tools to be run from anywhere inside the repository.

**Best practice:**
- always anchor file paths from `REPO_ROOT`
- prefer `"${REPO_ROOT}/..."` over relative paths once root is known

### 2. Store generated artifacts under `.generated/`

AI outputs and other generated artifacts belong under:

```text
${REPO_ROOT}/.generated/
```

**Best practice:**
- keep generated output out of source directories
- use a stable prefix plus `uuidgen -7`
- create the directory before writing:
  ```bash
  mkdir -p "${REPO_ROOT}/.generated"
  ```

### 3. Use environment variables for overrides

`generate-commit-msg.sh` already supports external invocation via environment variables such as:

- `REPO_ROOT`
- `TAI_BIN`
- `PROMPT`

This is a strong pattern.

**Best practice:**
- use `${VAR:-default}` when a tool may be reused across repositories
- export only what child processes need
- document override points in the script header and in this file

### 4. Separate human-viewing tools from generation tools

The current split is good:

- generation tools write artifacts
- viewer tools render or print the latest artifact

This keeps generation composable and scriptable.

**Best practice:**
- generator scripts should not require interactive viewing to succeed
- viewer scripts should support piping/plain output where possible

### 5. Prefer stable output naming conventions

Current prefixes:

- `fix`
- `shellcheck`
- `changes`

These prefixes make generated files easy to enumerate and filter.

> **Best practice:**
>
> - one tool, one prefix
> - keep prefixes short and descriptive
> - do not reuse prefixes for different artifact types

### 6. Preserve script composability

Several tools chain standard utilities such as:

- `fd`
- `xargs`
- `parallel`
- `realpath`
- `cat`

This is useful, but requires careful handling of quoting and null-delimited data.

**Best practice:**
- prefer null-delimited pipelines when arbitrary file names are involved
- consistently quote variable expansions
- use `set -euo pipefail` in scripts that should fail fast

### 7. Distinguish “dogfooding” from “stable” execution paths

The current tooling uses:

- `release` binary for fix workflows
- `debug` binary for commit generation

That distinction appears intentional:
- fix flows prioritize stability
- commit message generation prioritizes day-to-day dogfooding

**Best practice:**
- document why a tool prefers `debug` or `release`
- allow overrides via `TAI_BIN`

---

## Tool details

## `tools/fix-tools.sh`

Runs `shellcheck` over shell scripts and sends the diagnostics plus tool context to `terminal-ai`.

### Behavior

- sets:
  - `GENERATED_FILE_PREFIX="shellcheck"`
  - `TARGET_PATH="${REPO_ROOT}/tools"`
  - `REPORT_PATH="${REPO_ROOT}/.generated/shellcheck-<UUIDv7>"`
  - `TAI_BIN="${REPO_ROOT}/target/release/terminal-ai"`
- gathers shell files
- runs `shellcheck`
- provides tool file context to AI
- appends AI output to the generated report
- invokes `tools/show-fix.sh`

### Best practices used here

- scopes the target to `tools/`
- uses a dedicated prefix for shellcheck-oriented output
- reuses `show-fix.sh` for presentation

### Recommended usage

```bash
./tools/fix-tools.sh
```

With a custom prompt:

```bash
./tools/fix-tools.sh "Provide precise shell fixes and safer alternatives."
```

### Notes

This script relies on external tools such as:

- `fd`
- `realpath`
- `shellcheck`
- `parallel`
- `bat`
- `uuidgen`

Because this is a pipeline-heavy script, robust null-delimited handling and fail-fast shell settings would be good future improvements.

---

## `tools/fix.sh`

Runs `cargo clippy` for the Rust project and sends the results to `terminal-ai`.

### Behavior

- sets:
  - `GENERATED_FILE_PREFIX="fix"`
  - `TAI_BIN="${REPO_ROOT}/target/release/terminal-ai"`
  - `REPORT_PATH="${REPO_ROOT}/.generated/fix-<UUIDv7>"`
- runs:
  ```bash
  cargo -Z unstable-options -C "${REPO_ROOT}" clippy
  ```
- sends clippy output plus `src/main.rs` as context to AI
- writes report to `.generated/`
- invokes `tools/show-fix.sh`

### Best practices used here

- stable output location
- predictable prefix
- uses release AI binary for consistency

### Recommended usage

```bash
./tools/fix.sh
```

With a custom prompt:

```bash
./tools/fix.sh "Suggest minimal, idiomatic Rust fixes with alternatives where useful."
```

### Notes

This script currently passes `src/main.rs` as the input context. If the codebase grows into multiple modules, this tool may need broader source-context selection.

---

## `tools/generate-commit-msg.sh`

Generates a commit message from the staged diff.

### Behavior

- uses `set -euo pipefail`
- supports external overrides:
  - `REPO_ROOT`
  - `TAI_BIN`
  - `PROMPT`
- defaults:
  - `GENERATED_FILE_PREFIX="changes"`
  - `TAI_BIN="${REPO_ROOT}/target/debug/terminal-ai"`
  - `REPORT_PATH="${REPO_ROOT}/.generated/changes-<UUIDv7>"`
- runs:
  ```bash
  git -C "${REPO_ROOT}" diff --cached
  ```
- asks AI to write a conventional commit message in plain text
- writes the result to `.generated/`
- invokes `tools/view-last-commit-msg.sh`

### Best practices used here

This is currently the strongest script from a shell-hygiene perspective.

It already demonstrates several good practices:

- `set -euo pipefail`
- override-friendly environment variables
- clearly documented external invocation
- plain-text output mode for AI-generated commit messages
- explicit AI args grouped into `TAI_ARGS`

### Recommended usage

```bash
./tools/generate-commit-msg.sh
```

Override the binary when called from another repository:

```bash
TAI_BIN="/path/to/terminal-ai" REPO_ROOT="$(pwd)" ./tools/generate-commit-msg.sh
```

Override the prompt:

```bash
PROMPT="Write a concise conventional commit message with an optional body if needed." \
  ./tools/generate-commit-msg.sh
```

### Notes

This tool intentionally defaults to the `debug` binary to encourage dogfooding during normal development.

---

## `tools/list-generated-files.sh`

Lists generated files in a TSV-like format.

### Behavior

Outputs rows containing:

1. timestamp derived from UUIDv7
2. generated prefix/type
3. relative path from repository root

### Best practices used here

- centralizes discovery of generated artifacts
- makes generated output easier to sort and inspect from the command line
- uses UUIDv7 timestamps instead of file mtimes

### Recommended usage

```bash
./tools/list-generated-files.sh
```

Example use in pipelines:

```bash
./tools/list-generated-files.sh | sort
```

### Notes

This script depends on:

- `fd`
- `parallel`
- `realpath`
- `uuidparse`

Because it processes paths and shell text heavily, careful quoting should continue to be a priority.

---

## `tools/show-fix.sh`

Shows the latest generated fix report for a given prefix.

### Behavior

- discovers the latest generated file matching `GENERATED_FILE_PREFIX`
- by default renders it with `mdless --ansi`
- with `--pipe`, prints raw file contents

### Environment

- `GENERATED_FILE_PREFIX` defaults to `fix`

### Recommended usage

Show latest `fix` report:

```bash
./tools/show-fix.sh
```

Show latest shellcheck report:

```bash
GENERATED_FILE_PREFIX=shellcheck ./tools/show-fix.sh
```

Pipe raw output:

```bash
./tools/show-fix.sh --pipe
```

### Best practices used here

- presentation is separated from generation
- supports both human-readable and pipeline-friendly output

### Notes

A future-safe version should guard argument access more defensively, since `${1}` may be unset.

---

## `tools/view-last-commit-msg.sh`

Prints the latest generated commit message.

### Behavior

- finds the latest file under `.generated/` matching `changes`
- prints it to stdout

### Recommended usage

```bash
./tools/view-last-commit-msg.sh
```

### Best practices used here

- minimal and composable
- stdout-oriented by default, which is ideal for shell use

---

## Expected dependencies

Depending on the tool, the following commands may be required:

- `git`
- `cargo`
- `uuidgen`
- `uuidparse`
- `fd`
- `xargs`
- `parallel`
- `realpath`
- `shellcheck`
- `bat`
- `mdless`

And for AI-backed workflows:

- `target/debug/terminal-ai` or `target/release/terminal-ai`

---

## Recommended standards for future tools

When adding a new script under `tools/`, prefer the following template-level practices:

### Shell safety

Use:

```bash
set -euo pipefail
```

and quote all variable expansions:

```bash
"${REPO_ROOT}"
"${REPORT_PATH}"
```

### Repository-root discovery

Use:

```bash
REPO_ROOT="${REPO_ROOT:-$(git rev-parse --show-toplevel)}"
```

### Generated output

Use:

```bash
GENERATED_FILE_PREFIX="your-prefix"
REPORT_PATH="${REPO_ROOT}/.generated/${GENERATED_FILE_PREFIX}-$(uuidgen -7)"
mkdir -p "${REPO_ROOT}/.generated"
```

### Override-friendly binaries and prompts

Use:

```bash
TAI_BIN="${TAI_BIN:-${REPO_ROOT}/target/release/terminal-ai}"
PROMPT="${PROMPT:-Default prompt text}"
```

### Viewer pairing

If a tool generates a new artifact class, also add a lightweight viewer script or make it discoverable via `list-generated-files.sh`.

### Output mode

When a tool is intended for reuse in pipelines:
- prefer stdout for final output, or
- write to `.generated/` and provide a `--pipe` or dedicated viewer command

---

## Typical workflows

## AI-assisted Rust fixes

1. Run:
   ```bash
   ./tools/fix.sh
   ```
2. Review the latest generated report:
   ```bash
   ./tools/show-fix.sh
   ```
3. For raw output:
   ```bash
   ./tools/show-fix.sh --pipe
   ```

## AI-assisted shell fixes for repo tools

1. Run:
   ```bash
   ./tools/fix-tools.sh
   ```
2. Review the generated shellcheck-based suggestions:
   ```bash
   GENERATED_FILE_PREFIX=shellcheck ./tools/show-fix.sh
   ```

## Git commit message generation

1. Stage a coherent change set.
2. Run:
   ```bash
   ./tools/generate-commit-msg.sh
   ```
3. Review:
   ```bash
   ./tools/view-last-commit-msg.sh
   ```

---

## Manual invocation guidance

For ad hoc AI usage, prefer either:

- stdout-only output for immediate use, or
- a generated file under `.generated/assistant-<UUIDv7>`

Suggested pattern:

```bash
mkdir -p "${REPO_ROOT}/.generated"
"${TAI_BIN}" ... > "${REPO_ROOT}/.generated/assistant-$(uuidgen -7)"
```

---

## Known inconsistencies corrected in this document

The previous version of this document had a few mismatches relative to the scripts. This version reflects the scripts as written:

- `fix-tools.sh` writes `shellcheck-<UUIDv7>`, not `fix-<UUIDv7>`
- `fix.sh` writes `fix-<UUIDv7>`, not `shellcheck-<UUIDv7>`
- `generate-commit-msg.sh` writes to `.generated/changes-<UUIDv7>`
- `show-fix.sh` uses `mdless` by default and raw output with `--pipe`
- `generate-commit-msg.sh` defaults to the `debug` binary
- `fix.sh` and `fix-tools.sh` default to the `release` binary

---

## Future Work

- Implement `tools/fix-docs.md`
- Expected tool renaming
- Fixes to existing issues
- Makefile / build script integrations
- Tests, Man Pages, Completions

## Appendix: links

- [Tool: fix tools](tools/fix-tools.sh)
- [Tool: fix code](tools/fix.sh)
- [Tool: commit message generator](tools/generate-commit-msg.sh)
- [Tool: list generated files](tools/list-generated-files.sh)
- [Tool: show latest fix](tools/show-fix.sh)
- [Tool: view generated commit message](tools/view-last-commit-msg.sh)
