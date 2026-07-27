# Terminal AI Utilities

These utilities read Terminal AI transcript Markdown files from the command line.
They are installed from the repository's `utils/` directory.

[Desktop integrations (`terminal-ai-extra`)](https://code.pandasportal.net/panda/terminal-ai-extra)

## Transcript files

Unless otherwise noted, transcripts are read from:

```text
~/.local/share/terminal-ai/transcripts/default
```

Transcript files are Markdown files (`*.md`). Message boundaries are transcript
header lines that start with `[`; `tai-get-message` expects headers matching
`^\[[^]]+\]$`.

Use `tai-get-message` when you want the latest or a previous message by offset.
Use `tai-today` when you want line-oriented access to the most recent transcript
file in the default transcript directory.

## `tai-get-message`

`tai-get-message` extracts one Terminal AI message and writes the message body to
standard output. It removes the transcript message header from the output.

### Usage

```text
tai-get-message [OPTIONS] [OFFSET]
```

Options:

- `--offset OFFSET` - message offset. `0` means latest, `1` means previous.
- `--dir DIR` - search transcript Markdown files in `DIR`.
- `--file FILE` - search only `FILE`.
- `-h`, `--help` - show help.

Environment variables:

- `OFFSET` - default message offset.
- `TRANSCRIPT_FILE` - transcript file override.
- `TAI_TRANSCRIPT_FILE` - transcript file override.
- `TRANSCRIPT_DIR` - transcript directory override.
- `TAI_TRANSCRIPT_DIR` - transcript directory override.

Directory precedence:

1. `--dir DIR`
2. `TRANSCRIPT_DIR`
3. `TAI_TRANSCRIPT_DIR`
4. `~/.local/share/terminal-ai/transcripts/default`

File precedence:

1. `--file FILE`
2. `TRANSCRIPT_FILE`
3. `TAI_TRANSCRIPT_FILE`
4. If no file is given, all `*.md` files in the resolved directory are searched.

Offset meanings:

- `0` - latest message.
- `1` - previous message.
- `2` - two messages back.

When no file override is set, transcript files are sorted by path and searched
from the last sorted file backward. Within each transcript, messages are counted
from the last message backward.

### Examples

Print the latest message:

```sh
tai-get-message
```

Print the previous message:

```sh
tai-get-message 1
```

Print the message two messages back:

```sh
tai-get-message --offset 2
```

Read transcripts from another directory:

```sh
tai-get-message --dir .generated/transcripts
```

Read from one transcript file:

```sh
tai-get-message --file .generated/transcripts/2026-06-21.md
```

Open the latest message in an editor:

```sh
tai-get-message | nvim -
```

## `tai-today`

`tai-today` is a convenience helper for the most recent Markdown transcript in
the default transcript directory. It resolves the file with:

```sh
fd -e md . "$HOME/.local/share/terminal-ai/transcripts/default" | sort | tail -n1
```

Unlike `tai-get-message`, `tai-today` does not support transcript directory or
transcript file overrides.

### Usage

```text
tai-today [COMMAND] [ARGUMENTS]
```

Commands:

- No command: print the resolved transcript file path.
- `open`: display the resolved transcript with `bat`.
- `lines [START [END]]`: print an inclusive line range from the transcript.
  - `START` defaults to `1`; `END` defaults to the transcript's total line count.
- `index`: print transcript header lines with line numbers, followed by an
  `EOF` marker.
- `msg [MESSAGE_NUMBER]`: print a message from the `index` output. The number is
  one-based and defaults to `1`. Output includes the selected message header.

`tai-today msg` is implemented in terms of `tai-today index` and
`tai-today lines`; it prints from the selected header through the line before
the next indexed header. For the final message, the current implementation uses
the `EOF` marker's line number and then drops the last emitted line, so the last
line of the transcript is not printed.

### Examples

Print the path to the most recent default transcript:

```sh
tai-today
```

Open that transcript in Neovim:

```sh
tai-today | xargs nvim
```

Display it with `bat`:

```sh
tai-today open
```

Show the message/header index:

```sh
tai-today index
```

Print lines 25 through 80:

```sh
tai-today lines 25 80
```

Print the third indexed message:

```sh
tai-today msg 3
```

Dependencies:

- `tai-today` requires `fd`.
- `tai-today open` and `tai-today lines` require `bat`.
- `tai-get-message` requires Bash and standard Unix tools; `fd` is used when
  available, otherwise `find` is used to locate transcript files.
