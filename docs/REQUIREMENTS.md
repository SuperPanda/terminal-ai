# Terminal-AI Requirements Documentation

## Overview

Terminal-AI is a CLI tool designed for terminal-first and pipeline-safe AI interaction. It separates conversational output into distinct channels so that machine workflows and human-readable transcript archives do not conflict.

## Core Capabilities

### 1. AI Communication

- **Backend Compatibility**: Communicates with OpenAI-compatible AI endpoints
- **Default Configuration**: Uses `gpt-5.4` model at `https://api.openai.com` unless overridden
- **Authentication**: Supports bearer token authentication via `OPENAI_API_KEY` environment variable
- **System Prompt**: Injects a fixed system prompt defining the tool's identity and behavior

### 2. Input Handling

- **Interactive Mode**: When stdin is a TTY, prompts are read interactively
- **Piped Input**: When stdin is piped, stdin content becomes the prompt
- **Default Prompt**: Uses a provided prompt if neither interactive nor piped input is available
- **File Attachment**: Supports `--input-file` to attach file contents to the prompt

### 3. Output Architecture

Terminal-AI intentionally distinguishes **three output paths**:

#### stdout (Pipeline Output)
- Intended for pipelines, chaining, `head`, `sed`, `parallel`, `xargs`, etc.
- Defaults to a clean render suitable for shell composition
- Handles `BrokenPipe` gracefully, exiting with code 0 when pipe is closed

#### transcript (Archival Log)
- An append-only archival log in a stable transcript format
- Defaults to a date-based Markdown file under XDG data storage
- Preserves grep-friendly transcript shape:
  ```
  [timestamp, speaker, message_id]
  <indented content>
  ---
  ```
- Can be disabled with `--no-transcript`

#### output file (Artifact Sink)
- A separate explicit artifact sink for collatable or downstream-processed output
- Distinct from the transcript and may be overwritten if requested
- Intended for workflows like multi-pass prompting, intermediate materialization, or pipeline fan-out
- Configurable via `-O`, `--output-file` flags

### 4. Message ID System

Terminal-AI implements a branching conversation graph with explicit threading:

```
SpeechID := HASH8(Speaker, Timestamp)
AudienceID := parent SpeechID, or HASH8("NULL") for roots
MessageID := "<SpeechID>-<AudienceID>"
```

- **Threading**: `--parent-id` allows attaching a new user turn to an existing prior speech
- **Branching**: Supports multi-agent or multi-pass workflows
- **Root Identity**: Empty input normalizes to `"NULL"` for root-identity behavior

### 5. Configuration Resolution

Unless otherwise noted, values resolve through the following hierarchy:

```
CLI flag > process env > env file > default
```

#### Environment Variables

| Variable | Purpose | Default |
|----------|---------|---------|
| `TAI_MODEL` | Model name override | `gpt-5.4` |
| `TAI_BASE_URL` | API base URL override | `https://api.openai.com` |
| `OPENAI_API_KEY` | Bearer token | `None` |
| `TAI_APP_NAME` | XDG namespace root | `terminal-ai` |
| `TAI_ENV_FILE` | Env file path | `~/.config/terminal-ai/.default.env` |
| `TAI_TRANSCRIPT_DIR` | Transcript directory | `~/.local/share/terminal-ai/transcripts/default` |
| `TAI_TRANSCRIPT_FILE` | Transcript filename | Date-based `.md` |
| `TAI_OUTPUT_FILE` | Explicit output artifact file | `None` |
| `TAI_OVERWRITE_OUTPUT_FILE` | Truncate output file before write | `false` |
| `TAI_SPEAKER_USER` | User speaker label | `USER` |
| `TAI_SPEAKER_AI` | Fixed AI speaker label | `None` (uses model name) |

#### Per-Channel Format Variables

Each output channel has its own set of format variables:

```
TAI_STDOUT_{HEADER,INDENT,SPEAKER,PROMPT,METADATA,SEP,ENABLE}
TAI_FILE_{HEADER,INDENT,SPEAKER,PROMPT,METADATA,SEP,ENABLE}
TAI_ARTIFACT_{HEADER,INDENT,SPEAKER,PROMPT,METADATA,SEP,ENABLE}
```

### 6. Output Control

Terminal-AI supports granular output control via `-o`, `--output-control` flags:

#### Control Tokens

```
NO_USER_STDOUT        - Suppress user output to stdout
NO_USER_FILE          - Suppress user output to transcript
NO_USER_ARTIFACT      - Suppress user output to output file
NO_AI_STDOUT          - Suppress AI output to stdout
NO_AI_FILE            - Suppress AI output to transcript
NO_AI_ARTIFACT        - Suppress AI output to output file
```

Multiple tokens can be comma-separated or repeated:
```
-o NO_USER_TRANSCRIPT,NO_AI_STDOUT
-o NO_USER_TRANSCRIPT
-o NO_AI_STDOUT
-o NO_AI_STDOUT
```

### 7. Runtime Dependencies

External commands expected to be available on standard Linux systems:

| Command | Purpose |
|---------|---------|
| `curl` | HTTP transport for API requests |
| `sha256sum` | Message ID derivation |
| `date` | RFC-3339 timestamp generation |

### 8. Usage Examples

```bash
# Interactive session
terminal-ai

# Piped input
echo "Hello" | terminal-ai

# With custom model
terminal-ai --model gpt-4 --prompt "Explain quantum mechanics"

# Disable transcript, capture to file
terminal-ai --no-transcript --output-file ./response.md

# Attach file to prompt
terminal-ai --input-file ./context.txt "Summarize this"

# Control output channels
terminal-ai -o NO_AI_STDOUT -o NO_USER_TRANSCRIPT

# Dry-run without API call
terminal-ai --dry-run "Test prompt"

# Verbose mode showing raw API response
terminal-ai --verbose "Debug request"
```

### 9. Transcript Format

The transcript format is designed to be:

- **Stable**: Consistent structure across versions
- **Grep-friendly**: Structured with clear delimiters
- **Append-only**: Never overwrites existing content
- **Date-stamped**: Uses current date for organization

```
[2024-01-15T10:30:45Z, USER, abc123-def456]
Here is my question...
---

[2024-01-15T10:30:47Z, USER, def456-ghi789]
<thinking and context>
---

[2024-01-15T10:30:48Z, GPT-5.4, ghi789-jkl012]
This is the AI's response.
---
```

### 10. Pipeline-Safe Behavior

- **BrokenPipe Handling**: Both stdout and stderr explicitly handle `BrokenPipe` errors
- **Graceful Exit**: Exits with code 0 when downstream consumer closes pipe early
- **Example**:
  ```bash
  terminal-ai "hello" | head -n1  # Terminates cleanly with exit code 0
  ```

### 11. Security Considerations

- **Environment Variables**: Process environment is never mutated
- **File Reading**: Env file reading fails gracefully (returns empty map)
- **API Key Handling**: Only loaded from environment or explicit file; never logged
- **Path Validation**: Transcript directory existence is verified before use

### 12. Extensibility Points

- **Backend Agnostic**: Designed to work with any OpenAI-compatible endpoint
- **Format Customizable**: Per-channel format variables allow detailed output control
- **Output Control**: Granular suppression policies enable complex workflows
- **Threading Support**: Message ID system supports branching conversation graphs

## Summary

Terminal-AI is a robust, pipeline-safe tool for AI interaction that:

1. Separates output into three distinct channels (stdout, transcript, output file)
2. Implements a sophisticated message ID system for conversation threading
3. Supports granular output control via command-line flags
4. Maintains configuration through a well-defined resolution hierarchy
5. Handles edge cases like broken pipes gracefully
6. Provides extensive customization through environment variables
7. Maintains stable, grep-friendly transcript format for archival purposes
