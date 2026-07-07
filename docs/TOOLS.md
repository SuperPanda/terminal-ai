# Tools

A small collection of repository-local helper scripts for linting, AI-assisted remediation, documentation repair, and commit-message generation.

These tools are intentionally lightweight and repository-local. Most of them generate artifacts under `.generated/` and several invoke the local `terminal-ai` binary.

## Overview

```text
tools/
├─ fix-doc.sh
├─ fix-tools.sh
├─ fix.sh
├─ generate-commit-msg.sh
├─ list-generated-files.sh
├─ show-fix.sh
└─ view-last-commit-msg.sh
```

## Tool summary

| Tool                            | Purpose                                                         |                         Output prefix | Default AI binary            |
| ------------------------------- | --------------------------------------------------------------- | ------------------------------------: | ---------------------------- |
| `tools/fix-doc.sh`              | Generate an AI-assisted documentation fix report                |                             `fix-doc` | `target/release/terminal-ai` |
| `tools/fix-tools.sh`            | Run `shellcheck` against shell scripts and ask AI for fixes     |                          `shellcheck` | `target/release/terminal-ai` |
| `tools/fix.sh`                  | Run `cargo clippy` and ask AI for fixes                         |                                 `fix` | `target/release/terminal-ai` |
| `tools/generate-commit-msg.sh`  | Generate a conventional commit message from `git diff --cached` |                             `changes` | `target/debug/terminal-ai`   |
| `tools/list-generated-files.sh` | List generated files with timestamp, prefix, and path           |                                   n/a | n/a                          |
| `tools/show-fix.sh`             | Show the latest generated report for a prefix                   | inherited via `GENERATED_FILE_PREFIX` | n/a                          |
| `tools/view-last-commit-msg.sh` | Print the latest generated commit message                       |                             `changes` | n/a                          |

---

## `tools/fix-tools.sh`

Runs `shellcheck` over shell scripts and sends the diagnostics plus tool context to `terminal-ai`.

### Behavior

- Resolves `REPO_ROOT` with `git rev-parse --show-toplevel`.
- Sets:
  - `GENERATED_FILE_PREFIX="shellcheck"`
  - `TARGET_PATH="${REPO_ROOT}/tools"`
  - `REPORT_PATH="${REPO_ROOT}/.generated/shellcheck-<UUIDv7>"`
  - `TAI_BIN="${REPO_ROOT}/target/release/terminal-ai"`
  - `PROMPT` from the first positional argument, if provided
- Creates `TARGET_PATH` with:

  ```bash
  mkdir -p "${TARGET_PATH}"
  ```

- Runs `shellcheck` across shell scripts discovered from the repository root:

  ```bash
  fd . -e sh "${REPO_ROOT}" -x realpath {}
  ```

- Renders shell-script context from `tools/` using `bat` through `parallel`.
- Appends the AI response to `.generated/shellcheck-<UUIDv7>`.
- Invokes:

  ```bash
  "${REPO_ROOT}/tools/show-fix.sh"
  ```

### Usage

```sh
./tools/fix-tools.sh
```

With a custom prompt:

```sh
./tools/fix-tools.sh "Provide precise shell fixes and safer alternatives."
```

### Notes

- This script uses the release `terminal-ai` binary by default.
- Although `TARGET_PATH` is `tools/`, the `shellcheck` discovery command scans from `${REPO_ROOT}`.
- The script writes reports under `.generated/`, but creates `${TARGET_PATH}` rather than `.generated/`; the documentation here reflects the script as written.

---

## `tools/fix.sh`

Runs `cargo clippy` for the Rust project and sends the diagnostics to `terminal-ai`.

### Behavior

- Resolves `REPO_ROOT` with `git rev-parse --show-toplevel`.
- Sets:
  - `GENERATED_FILE_PREFIX="fix"`
  - `TAI_BIN="${REPO_ROOT}/target/release/terminal-ai"`
  - `PROMPT` from the first positional argument, if provided
  - `REPORT_PATH="${REPO_ROOT}/.generated/fix-<UUIDv7>"`
- Creates:

  ```sh
  mkdir -p "${REPO_ROOT}/.generated"
  ```

- Runs:

  ```sh
  cargo -Z unstable-options -C "${REPO_ROOT}" clippy
  ```

- Sends `cargo clippy` output to `terminal-ai` via `xargs -0`.
- Provides `src/main.rs` as the input file context:

  ```sh
  --input-file="$REPO_ROOT/src/main.rs"
  ```

- Appends the AI response to `.generated/fix-<UUIDv7>`.
- Invokes `tools/show-fix.sh` afterward.

### Usage

```sh
./tools/fix.sh
```

With a custom prompt:

```sh
./tools/fix.sh "Suggest minimal, idiomatic Rust fixes with alternatives where useful."
```

### Notes

- Default prompt:

  ```text
  This is a one shot prompt, do not ask followups and extensive and complete solutions and alternatives.
  ```

- The script does not use `set -euo pipefail`.
- The documentation here reflects the current implementation, including its direct use of `src/main.rs` as context.

---

## `tools/generate-commit-msg.sh`

Generates a commit message from the staged diff in plain text suitable for direct use as a Git commit message.

### Behavior

- Uses:

  ```sh
  set -euo pipefail
  ```

- Sets:
  - `GENERATED_FILE_PREFIX="changes"`
  - `REPO_ROOT="${REPO_ROOT:-$(git rev-parse --show-toplevel)}"`
  - `TAI_BIN="${TAI_BIN:-${REPO_ROOT}/target/debug/terminal-ai}"`
  - `PROMPT="${PROMPT:-...}"`
  - `TAI_ARGS="-o NO_USER_STDOUT,CONTENT_ONLY --model=gpt-5.5"`
  - `TAI_TOOLS_PATH="${TAI_TOOLS_PATH:-${REPO_ROOT}/tools}"`
  - `REPORT_PATH="${REPO_ROOT}/.generated/changes-<UUIDv7>"`
- Creates:

  ```sh
  mkdir -p "${REPO_ROOT}/.generated"
  ```

- Runs:

  ```sh
  git -C "${REPO_ROOT}" diff --cached
  ```

- Invokes `terminal-ai` with:
  - `TAI_STDOUT_HEADER=0`
  - `TAI_STDOUT_INDENT=0`
  - `TAI_STDOUT_SEP=0`
- Writes output to `.generated/changes-<UUIDv7>`.
- Invokes:

  ```sh
  "${TAI_TOOLS_PATH}/view-last-commit-msg.sh"
  ```

### Usage

```sh
./tools/generate-commit-msg.sh
```

Override the binary and repository root:

```sh
TAI_BIN="/path/to/terminal-ai" REPO_ROOT="$(pwd)" ./tools/generate-commit-msg.sh
```

Override the prompt:

```sh
PROMPT="Write a concise conventional commit message with an optional body if needed." \
  ./tools/generate-commit-msg.sh
```

### Notes

- This script defaults to the debug `terminal-ai` binary:

  ```sh
  ${REPO_ROOT}/target/debug/terminal-ai
  ```

- It also supports overriding `TAI_TOOLS_PATH`, which is used when invoking `view-last-commit-msg.sh`.
- The prompt is a long inline instruction string oriented around conventional commits and plain-text output.

---

## `tools/list-generated-files.sh`

Lists generated files under `.generated/`.

### Behavior

- Resolves `REPO_ROOT` with `git rev-parse --show-toplevel`.
- Discovers files from:

  ```sh
  "$REPO_ROOT/.generated/"
  ```

- Treats generated files as following the pattern:

  ```text
  <GENERATED_TYPE>-<UUIDv7>
  ```

- Outputs tab-delimited rows combining:
  1. time derived from the UUIDv7 suffix
  2. generated type / prefix
  3. relative path from repository root
- Sorts the final output.

### Usage

```sh
./tools/list-generated-files.sh
```

Example:

```sh
./tools/list-generated-files.sh | sort
```

### Notes

- This script uses `fd`, `parallel`, `realpath`, and `uuidparse`.
- The implementation stores the discovered file list in a shell variable and then derives timestamp, type, and path columns from it.

---

## `tools/show-fix.sh`

Shows the latest generated report for a prefix.

### Behavior

- Resolves `REPO_ROOT` with `git rev-parse --show-toplevel`.
- Uses:

  ```sh
  GENERATED_FILE_PREFIX="${GENERATED_FILE_PREFIX:-fix}"
  ```

- Finds the latest matching generated file under:

  ```sh
  "${REPO_ROOT}/.generated"
  ```

- If called with `--pipe`, prints raw contents with `cat`.
- Otherwise renders the file with:

  ```sh
  mdless --ansi
  ```

### Environment

- `GENERATED_FILE_PREFIX` defaults to `fix`

### Usage

```sh
./tools/show-fix.sh
```

Show latest shellcheck report:

```sh
GENERATED_FILE_PREFIX=shellcheck ./tools/show-fix.sh
```

Pipe raw output:

```sh
./tools/show-fix.sh --pipe
```

### Notes

- This tool is also used by `fix.sh`, `fix-tools.sh`, and `fix-doc.sh`.
- It is not limited to the `fix` prefix; any generated prefix can be supplied through `GENERATED_FILE_PREFIX`.
- The current implementation directly references `${1}` and assumes the latest matching file exists.

---

## `tools/view-last-commit-msg.sh`

Prints the latest generated commit-message artifact.

### Behavior

- Resolves `REPO_ROOT` with `git rev-parse --show-toplevel`.
- Finds the latest file under `.generated/` matching `changes`.
- Pipes that file to `cat` via `xargs`.

### Usage

```sh
./tools/view-last-commit-msg.sh
```

### Notes

- This script is stdout-oriented and intended for composable shell use.
- It is invoked automatically by `tools/generate-commit-msg.sh`.

---

## `tools/fix-doc.sh`

Generates an AI-assisted documentation fix report from one documentation file plus one or more source-of-truth files or directories.

### Behavior

- Uses:

  ```sh
  set -euo pipefail
  ```

- Supports:
  - a required documentation file argument
  - one or more required file-or-directory context arguments
- Sets:
  - `REPO_ROOT="${REPO_ROOT:-$(git rev-parse --show-toplevel)}"`
  - `GENERATED_FILE_PREFIX="${GENERATED_FILE_PREFIX:-fix-doc}"`
  - `TAI_BIN="${TAI_BIN:-${REPO_ROOT}/target/release/terminal-ai}"`
  - `TAI_MODEL="${TAI_MODEL:-gpt-5.5}"`
  - `REPORT_PATH="${REPO_ROOT}/.generated/fix-doc-<UUIDv7>"`
  - `PROMPT="${PROMPT:-...}"`
- Validates arguments and prints a usage message when fewer than two arguments are provided.
- Resolves both the document path and context paths via a helper that accepts either:
  - the path as provided, or
  - the path relative to `REPO_ROOT`
- Renders the documentation file and all referenced context files into a structured input document with:
  - file headers
  - relative paths
  - line-numbered file contents
- For directory context arguments, expands files using:

  ```sh
  fd . "${path}" -t f -x realpath {}
  ```

- Creates:

  ```sh
  mkdir -p "${REPO_ROOT}/.generated"
  ```

- Invokes `terminal-ai` with:
  - `-o NO_USER_STDOUT`
  - `--input-file=<(render_context ...)`
  - `--model="${TAI_MODEL}"`
- Appends output to `.generated/fix-doc-<UUIDv7>`.
- Invokes:

  ```sh
  GENERATED_FILE_PREFIX="${GENERATED_FILE_PREFIX}" "${REPO_ROOT}/tools/show-fix.sh"
  ```

### Usage

```sh
tools/fix-doc.sh <document> <file-or-directory>...
```

Example:

```sh
tools/fix-doc.sh docs/TOOLS.md tools
```

### Environment

- `REPO_ROOT` — repository root. Defaults to `git rev-parse --show-toplevel`.
- `GENERATED_FILE_PREFIX` — output prefix. Defaults to `fix-doc`.
- `TAI_BIN` — `terminal-ai` binary. Defaults to `target/release/terminal-ai`.
- `TAI_MODEL` — model name. Defaults to `gpt-5.5`.
- `PROMPT` — overrides the default documentation-fix prompt.

### Notes

- The default prompt is:

  ```text
  This is a one shot prompt, do not ask followups. Fix the provided documentation file using the referenced files and directories as the source of truth. Only propose changes to the documentation document. Provide an extensive and complete solution, preferably as a unified diff or complete replacement content.
  ```

- This is the only current tool that renders a custom structured context document from both a target file and arbitrary file/directory inputs.

---

## Generated artifacts

Generated artifacts are written under:

```text
${REPO_ROOT}/.generated/
```

Current prefixes used by the scripts are:

- `fix`
- `fix-doc`
- `shellcheck`
- `changes`

Generated file names follow:

```text
<prefix>-<UUIDv7>
```

The scripts use `uuidgen -7` when creating artifact paths.

---

## Environment and defaults

Common environment variables used across the tools include:

- `REPO_ROOT`
- `GENERATED_FILE_PREFIX`
- `TAI_BIN`
- `TAI_MODEL`
- `PROMPT`
- `TAI_ARGS`
- `TAI_TOOLS_PATH`
- `REPORT_PATH`

Repository root is generally discovered with:

```sh
git rev-parse --show-toplevel
```

AI-backed tools currently default to:

- `target/release/terminal-ai` for `fix.sh`, `fix-tools.sh`, and `fix-doc.sh`
- `target/debug/terminal-ai` for `generate-commit-msg.sh`

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
- `sed`
- `sort`
- `tail`
- `cut`
- `paste`
- `tr`
- `cat`

---

## Typical workflows

### AI-assisted Rust fixes

1. Run:

   ```sh
   ./tools/fix.sh
   ```

2. Review:

   ```sh
   ./tools/show-fix.sh
   ```

3. Or pipe raw output:

   ```sh
   ./tools/show-fix.sh --pipe
   ```

### AI-assisted shell fixes

1. Run:

   ```sh
   ./tools/fix-tools.sh
   ```

2. Review:

   ```sh
   GENERATED_FILE_PREFIX=shellcheck ./tools/show-fix.sh
   ```

### AI-assisted documentation fixes

1. Run:

   ```sh
   ./tools/fix-doc.sh docs/TOOLS.md tools
   ```

2. Review:

   ```sh
   GENERATED_FILE_PREFIX=fix-doc ./tools/show-fix.sh
   ```

3. Or pipe raw output:

   ```sh
   GENERATED_FILE_PREFIX=fix-doc ./tools/show-fix.sh --pipe
   ```

### Commit message generation

1. Stage changes.
2. Run:

   ```sh
   ./tools/generate-commit-msg.sh
   ```

3. Print the latest generated message:

   ```sh
   ./tools/view-last-commit-msg.sh
   ```

---

## Source-of-truth notes

This document reflects the current scripts as written, including implementation details that are not necessarily idealized. In particular:

- `fix-doc.sh` exists and is part of the current tool set.
- `fix-doc.sh` defaults `GENERATED_FILE_PREFIX` to `fix-doc`.
- `fix-tools.sh` uses `shellcheck-<UUIDv7>`.
- `fix.sh` uses `fix-<UUIDv7>`.
- `generate-commit-msg.sh` writes to `.generated/changes-<UUIDv7>`
- `show-fix.sh` defaults `GENERATED_FILE_PREFIX` to `fix`
- `show-fix.sh` renders with `mdless --ansi` unless called with `--pipe`
- `generate-commit-msg.sh` defaults to the debug binary
- `fix.sh`, `fix-tools.sh`, and `fix-doc.sh` default to the release binary

---

## Appendix: links

- [Tool: fix documentation](../tools/fix-doc.sh)
- [Tool: fix tools](../tools/fix-tools.sh)
- [Tool: fix code](../tools/fix.sh)
- [Tool: commit message generator](../tools/generate-commit-msg.sh)
- [Tool: list generated files](../tools/list-generated-files.sh)
- [Tool: show the latest fix/report](../tools/show-fix.sh)
- [Tool: view generated commit message](../tools/view-last-commit-msg.sh)
