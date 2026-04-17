# terminal-ai

**Version:** 0.2.0

A terminal AI client that records conversations as structured Markdown transcripts.
Each exchange is recorded independently to stdout and a log file, with per-channel
control over headers, indentation, and separators — so the same binary pipes cleanly
between tools while archiving full metadata for later replay or audit.

Compile:

```
cargo build --release
```

In workspace (`terminal-ai/Cargo.toml`):

```toml
[package]
name    = "terminal-ai"
version = "0.2.0"
edition = "2024"

[dependencies]
serde      = { version = "1", features = ["derive"] }
serde_json = "1"
```

Runtime dependency: `curl` (HTTP), `sha256sum` (message IDs), `date` (timestamps).
All standard on Linux.

---

## Design

### Output channels

Each recorded turn is rendered independently to two channels:

- **stdout** — intended for piping. Defaults to content only (no header, no indent,
  no separator), so `terminal-ai "question" | wc -w` counts only response words.

- **file** — the persistent transcript. Defaults to full format: header line,
  indented body, `---` separator between turns. Suitable for later `grep`, replay,
  or feeding back as context.

Each channel's format is controlled by three boolean env vars:

| Env var               | Channel | Effect                              | Default |
|-----------------------|---------|-------------------------------------|---------|
| `TAI_STDOUT_HEADER`   | stdout  | emit `[ts, SPEAKER, msg_id]` line   | `0`     |
| `TAI_STDOUT_INDENT`   | stdout  | prefix each content line with `" "` | `0`     |
| `TAI_STDOUT_SEP`      | stdout  | emit `---` after the entry          | `0`     |
| `TAI_FILE_HEADER`     | file    | emit `[ts, SPEAKER, msg_id]` line   | `1`     |
| `TAI_FILE_INDENT`     | file    | prefix each content line with `" "` | `1`     |
| `TAI_FILE_SEP`        | file    | emit `---` after the entry          | `1`     |

### Speaker labels

The `SPEAKER` field in a header names the participant. User turns are always labelled
`USER` (overridable via `TAI_SPEAKER_USER`). AI turns are labelled with the model
name from the API response — so transcripts record *which* model answered, not just
"AI". When shimming DeepSeek, Claude, a local llama.cpp model, or any other backend
into the same role, the transcript distinguishes them without any other change.

`TAI_SPEAKER_AI` overrides the label when you want a stable alias regardless of the
reported model name (e.g. `TAI_SPEAKER_AI=assistant` for anonymous playback).

### Model shimming

The request body is OpenAI-compatible `/v1/chat/completions`. Switching backends is
a single env var:

```sh
TAI_BASE_URL=http://localhost:8080   terminal-ai "…"   # llama.cpp
TAI_BASE_URL=https://api.deepseek.com OPENAI_API_KEY=sk-... terminal-ai "…"
TAI_BASE_URL=https://api.openai.com   OPENAI_API_KEY=sk-... terminal-ai "…"
```

The bearer header is sent only when `OPENAI_API_KEY` is set, so keyless local
servers work without any config.

---

```rust
// terminal-ai.rs

use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde::Deserialize;

// ── Constants ─────────────────────────────────────────────────────────────────

const DEFAULT_MODEL:    &str = "gpt-5.4";
const DEFAULT_BASE_URL: &str = "http://localhost:8080";
const SYSTEM_PROMPT:    &str =
    "You are a Terminal Based Assistant, which provides \
     Responses Compliant to CommonMark Spec (unless otherwise specified).";

// ── API response types ────────────────────────────────────────────────────────
//
// Mirrors the OpenAI /v1/chat/completions response shape.
// All fields are Option because llama.cpp and third-party backends vary
// in which fields they populate — missing fields deserialise to None
// rather than failing the parse.

#[derive(Deserialize)]
struct ChatResponse {
    id:      Option<String>,
    model:   Option<String>,
    usage:   Option<Usage>,
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Usage {
    total_tokens:      Option<u64>,
    prompt_tokens:     Option<u64>,
    completion_tokens: Option<u64>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Deserialize)]
struct Message {
    content: String,
}

// ── Output format ─────────────────────────────────────────────────────────────
//
// FormatOptions describes how a single channel renders a recorded turn.
// The two instances (stdout, file) are independent — each is configured
// from its own env var prefix and renders without consulting the other.

struct FormatOptions {
    /// Emit `[timestamp, SPEAKER, msg_id]` before the content.
    header:    bool,
    /// Prefix every content line with a single space.
    /// Preserves the block-indent convention of the original shell script
    /// while remaining trivially strippable (`sed 's/^ //'`).
    indent:    bool,
    /// Emit `---` (Markdown thematic break) after the content.
    separator: bool,
}

impl FormatOptions {
    fn from_env_prefix(prefix: &str, default_header: bool, default_indent: bool, default_sep: bool) -> Self {
        let flag = |suffix, default: bool| -> bool {
            env::var(format!("TAI_{}{}", prefix, suffix))
                .map(|v| v == "1")
                .unwrap_or(default)
        };
        Self {
            header:    flag("HEADER", default_header),
            indent:    flag("INDENT", default_indent),
            separator: flag("SEP",    default_sep),
        }
    }

    /// Render one turn into a String.
    ///
    /// `header_line` is pre-formatted as `[ts, SPEAKER, msg_id]` by the caller;
    /// separating construction from rendering keeps this function pure and testable.
    fn render(&self, header_line: &str, content: &str) -> String {
        let mut out = String::new();
        if self.header {
            out.push_str(header_line);
            out.push('\n');
        }
        if self.indent {
            for line in content.lines() {
                out.push(' ');
                out.push_str(line);
                out.push('\n');
            }
        } else {
            out.push_str(content);
            if !content.ends_with('\n') {
                out.push('\n');
            }
        }
        if self.separator {
            out.push_str("\n---\n");
        }
        out
    }
}

struct OutputFormat {
    stdout: FormatOptions,
    file:   FormatOptions,
}

impl OutputFormat {
    fn from_env() -> Self {
        Self {
            // stdout: content-only by default — safe for piping
            stdout: FormatOptions::from_env_prefix("STDOUT_", false, false, false),
            // file: full format by default — archival transcript
            file:   FormatOptions::from_env_prefix("FILE_",   true,  true,  true),
        }
    }
}

// ── Config ────────────────────────────────────────────────────────────────────

struct Config {
    model:        String,
    base_url:     String,
    api_key:      Option<String>,
    output_dir:   PathBuf,
    output_file:  String,
    dry_run:      bool,
    verbose:      bool,
    input_file:   Option<PathBuf>,
    format:       OutputFormat,
    /// Label for user turns in the transcript header.
    speaker_user: String,
    /// Optional fixed label for AI turns.
    /// When None, the model name from the API response is used,
    /// attributing each response to its actual source model.
    speaker_ai:   Option<String>,
}

// ── XDG helpers ───────────────────────────────────────────────────────────────

fn home() -> PathBuf {
    PathBuf::from(env::var("HOME").unwrap_or_else(|_| "/tmp".into()))
}

fn xdg_config_home() -> PathBuf {
    env::var("XDG_CONFIG_HOME").map(PathBuf::from)
        .unwrap_or_else(|_| home().join(".config"))
}

fn xdg_data_home() -> PathBuf {
    env::var("XDG_DATA_HOME").map(PathBuf::from)
        .unwrap_or_else(|_| home().join(".local/share"))
}

fn app_name() -> String {
    env::var("TAI_APP_NAME").unwrap_or_else(|_| "terminal-ai".into())
}

// ── Time / hashing ────────────────────────────────────────────────────────────
//
// Message IDs are derived from (timestamp, speaker) via sha256, truncated to
// 8 hex chars. A turn's ID incorporates its parent ID as a causal pointer,
// forming a lightweight chain across the transcript without a database.

fn date_cmd(fmt: &str) -> String {
    let out = Command::new("date").arg(fmt).output().expect("date failed");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn today()  -> String { date_cmd("--rfc-3339=date")    }
fn now_ts() -> String { date_cmd("--rfc-3339=seconds") }

fn hash8(input: &str) -> String {
    let src = if input.is_empty() { "NULL" } else { input };
    let mut child = Command::new("sha256sum")
        .stdin(Stdio::piped()).stdout(Stdio::piped())
        .spawn().expect("sha256sum failed");
    child.stdin.take().unwrap().write_all(src.as_bytes()).ok();
    let out = child.wait_with_output().expect("sha256sum wait failed");
    String::from_utf8_lossy(&out.stdout)[..8].to_string()
}

fn gen_message_id(speaker: &str, ts: &str, parent: Option<&str>) -> String {
    let speech_id   = hash8(&format!("{},{}", ts, speaker));
    let parent_part = parent
        .map(|p| p[..p.len().min(8)].to_string())
        .unwrap_or_else(|| hash8("NULL"));
    format!("{}-{}", speech_id, parent_part)
}

// ── Env-file loader ───────────────────────────────────────────────────────────

fn load_env_file(path: &std::path::Path) {
    let Ok(text) = fs::read_to_string(path) else { return };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        if let Some((k, v)) = line.split_once('=') {
            let v = v.trim_matches('"').trim_matches('\'');
            // Never overwrite a value already in the environment.
            if env::var(k.trim()).is_err() {
                env::set_var(k.trim(), v);
            }
        }
    }
}

// ── JSON escaping ─────────────────────────────────────────────────────────────

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
     .replace('"',  "\\\"")
     .replace('\n', "\\n")
     .replace('\r', "\\r")
     .replace('\t', "\\t")
}

// ── AI request ────────────────────────────────────────────────────────────────
//
// Returns (content, model_label).
//
// model_label is the model name as reported by the backend — the actual string
// that went into the SPEAKER field when TAI_SPEAKER_AI is not set. This lets
// the transcript record "deepseek-r1" or "claude-3-5-sonnet" rather than
// a generic "AI", which is what makes model shimming attributable.

fn ai_speak(prompt: &str, cfg: &Config) -> (String, String) {
    if cfg.dry_run {
        return ("<AI Response>".into(), cfg.model.clone());
    }

    let body = format!(
        r#"{{"model":"{m}","messages":[{{"role":"system","content":"{s}"}},{{"role":"user","content":"{p}"}}]}}"#,
        m = escape_json(&cfg.model),
        s = escape_json(SYSTEM_PROMPT),
        p = escape_json(prompt),
    );

    let url = format!("{}/v1/chat/completions", cfg.base_url);
    let mut cmd = Command::new("curl");
    cmd.arg("-s").arg(&url)
       .arg("-H").arg("Content-Type: application/json");
    if let Some(key) = &cfg.api_key {
        cmd.arg("-H").arg(format!("Authorization: Bearer {}", key));
    }
    let out = cmd.arg("-d").arg(&body).output().expect("curl failed");
    let raw = String::from_utf8_lossy(&out.stdout);

    if cfg.verbose {
        return (raw.into_owned(), cfg.model.clone());
    }

    match serde_json::from_str::<ChatResponse>(&raw) {
        Err(e) => {
            eprintln!("parse error: {}\nraw: {}", e, raw);
            (String::new(), cfg.model.clone())
        }
        Ok(resp) => {
            // Usage metadata goes to stderr — stdout stays clean for piping.
            if let Some(u) = &resp.usage {
                eprintln!(
                    r#"<!--{{"model":"{}","id":"{}","usage":{{"total":{},"prompt":{},"completion":{}}}}}-->"#,
                    resp.model.as_deref().unwrap_or(""),
                    resp.id.as_deref().unwrap_or(""),
                    u.total_tokens.unwrap_or(0),
                    u.prompt_tokens.unwrap_or(0),
                    u.completion_tokens.unwrap_or(0),
                );
            }
            let model_label = resp.model
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| cfg.model.clone());
            let content = resp.choices.into_iter()
                .next()
                .map(|c| c.message.content)
                .unwrap_or_default();
            (content, model_label)
        }
    }
}

// ── Record ────────────────────────────────────────────────────────────────────
//
// Writes one turn to both channels with their independent format options.
// The header line is constructed once and passed to both renderers —
// render() is pure and has no side effects of its own.

fn record(speaker: &str, content: &str, ts: &str, msg_id: &str, cfg: &Config) {
    let header_line = format!("[{}, {}, {}]", ts, speaker, msg_id);

    let stdout_text = cfg.format.stdout.render(&header_line, content);
    print!("{}", stdout_text);

    let file_text = cfg.format.file.render(&header_line, content);
    let path = cfg.output_dir.join(&cfg.output_file);
    OpenOptions::new().create(true).append(true).open(&path)
        .and_then(|mut f| f.write_all(file_text.as_bytes()))
        .expect("log write failed");
}

// ── Converse ──────────────────────────────────────────────────────────────────
//
// One round-trip: record the user turn, call the backend, record the AI response.
// The AI speaker label resolves as: TAI_SPEAKER_AI override > reported model name.

fn converse(prompt: &str, attachment: Option<&str>, cfg: &Config) {
    let full = match attachment {
        Some(a) => format!("{}\n--- (@attachment) ---\n{}", prompt, a),
        None    => prompt.to_string(),
    };

    let user_ts     = now_ts();
    let user_msg_id = gen_message_id(&cfg.speaker_user, &user_ts, None);
    record(&cfg.speaker_user, &full, &user_ts, &user_msg_id, cfg);

    let ai_ts                  = now_ts();
    let (content, model_label) = ai_speak(&full, cfg);
    let ai_speaker             = cfg.speaker_ai.as_deref().unwrap_or(&model_label);
    let ai_msg_id              = gen_message_id(ai_speaker, &ai_ts, Some(&user_msg_id));
    record(ai_speaker, &content, &ai_ts, &ai_msg_id, cfg);
}

// ── Argument parsing ──────────────────────────────────────────────────────────

fn parse_args() -> (Config, String) {
    let app          = app_name();
    let default_log  = xdg_data_home().join(&app).join("transcripts/default");
    let default_file = format!("{}.md", today());
    let default_env  = xdg_config_home().join(&app).join(".default.env");

    let mut output_dir    = default_log.clone();
    let mut output_file   = default_file;
    let mut dry_run       = false;
    let mut verbose       = false;
    let mut input_file:   Option<PathBuf> = None;
    let mut env_file:     Option<PathBuf> = None;
    let mut model_ovr:    Option<String>  = None;
    let mut base_url_ovr: Option<String>  = None;
    let mut prompt_parts: Vec<String>     = vec![];

    let args: Vec<String> = env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--dry-run" => dry_run  = true,
            "--verbose" => verbose  = true,
            "--help"    => { print_help(); std::process::exit(0); }
            "--"        => { prompt_parts.extend_from_slice(&args[i+1..]); break; }
            a if a.starts_with("--output-dir=")  => output_dir   = PathBuf::from(&a[13..]),
            a if a.starts_with("--output-file=") => output_file  = a[14..].to_string(),
            a if a.starts_with("--input-file=")  => input_file   = Some(PathBuf::from(&a[13..])),
            a if a.starts_with("--env-file=")    => env_file     = Some(PathBuf::from(&a[11..])),
            a if a.starts_with("--model=")       => model_ovr    = Some(a[8..].to_string()),
            a if a.starts_with("--base-url=")    => base_url_ovr = Some(a[11..].to_string()),
            a if a.starts_with("--") => {
                eprintln!("Unknown option: {}", a);
                std::process::exit(1);
            }
            a => prompt_parts.push(a.to_string()),
        }
        i += 1;
    }

    // Env file priority: TAI_ENV_FILE > --env-file > XDG default
    match (env::var("TAI_ENV_FILE").ok(), env_file) {
        (Some(p), _) => {
            let p = PathBuf::from(&p);
            if !p.exists() { eprintln!("TAI_ENV_FILE not found: {:?}", p); std::process::exit(1); }
            load_env_file(&p);
        }
        (None, Some(p)) => {
            if !p.exists() { eprintln!("--env-file not found: {:?}", p); std::process::exit(1); }
            load_env_file(&p);
        }
        (None, None) => { load_env_file(&default_env); }
    }

    if output_dir != default_log && !output_dir.is_dir() {
        eprintln!("OUTPUT PATH IS NOT A VALID DIRECTORY: {:?}", output_dir);
        std::process::exit(1);
    }
    fs::create_dir_all(&default_log).expect("cannot create log dir");

    let model = model_ovr
        .or_else(|| env::var("TAI_MODEL").ok())
        .unwrap_or_else(|| DEFAULT_MODEL.into());

    let base_url = base_url_ovr
        .or_else(|| env::var("TAI_BASE_URL").ok())
        .unwrap_or_else(|| DEFAULT_BASE_URL.into());

    let cfg = Config {
        model,
        base_url,
        api_key:      env::var("OPENAI_API_KEY").ok(),
        output_dir,
        output_file,
        dry_run,
        verbose,
        input_file,
        format:       OutputFormat::from_env(),
        speaker_user: env::var("TAI_SPEAKER_USER").unwrap_or_else(|_| "USER".into()),
        speaker_ai:   env::var("TAI_SPEAKER_AI").ok(),
    };

    let prompt = if prompt_parts.is_empty() {
        eprint!("Enter prompt: ");
        io::stderr().flush().ok();
        let line = io::stdin().lock().lines().next()
            .and_then(|l| l.ok()).unwrap_or_default();
        if line.is_empty() { eprintln!("Error: no prompt provided."); std::process::exit(1); }
        line
    } else {
        prompt_parts.join(" ")
    };

    (cfg, prompt)
}

fn print_help() {
    print!(concat!(
        "Usage: terminal-ai [OPTIONS] [PROMPT]\n",
        "\n",
        "Options:\n",
        "  --output-dir=PATH   log directory\n",
        "  --output-file=FILE  log filename (default: today.md)\n",
        "  --input-file=FILE   attach file contents to prompt\n",
        "  --env-file=FILE     load env vars from file\n",
        "  --model=MODEL       model name (env: TAI_MODEL)\n",
        "  --base-url=URL      API base URL (env: TAI_BASE_URL)\n",
        "  --dry-run           skip API call, record placeholder\n",
        "  --verbose           emit raw API response instead of content\n",
        "  --help              this message\n",
        "\n",
        "Output format (per channel, 0=off 1=on):\n",
        "  TAI_STDOUT_HEADER   header line on stdout     (default: 0)\n",
        "  TAI_STDOUT_INDENT   indent content on stdout  (default: 0)\n",
        "  TAI_STDOUT_SEP      separator on stdout       (default: 0)\n",
        "  TAI_FILE_HEADER     header line in log file   (default: 1)\n",
        "  TAI_FILE_INDENT     indent content in log     (default: 1)\n",
        "  TAI_FILE_SEP        separator in log          (default: 1)\n",
        "\n",
        "Speaker labels:\n",
        "  TAI_SPEAKER_USER    user turn label           (default: USER)\n",
        "  TAI_SPEAKER_AI      AI turn label override    (default: model name from response)\n",
        "\n",
        "Backend:\n",
        "  TAI_BASE_URL        API base URL              (default: http://localhost:8080)\n",
        "  TAI_MODEL           model name                (default: llama3)\n",
        "  OPENAI_API_KEY      bearer token (omit for keyless local servers)\n",
    ));
}

// ── Entry ─────────────────────────────────────────────────────────────────────

fn main() {
    let (cfg, prompt) = parse_args();

    let attachment = cfg.input_file.as_ref().map(|p| {
        fs::read_to_string(p).unwrap_or_else(|e| {
            eprintln!("Cannot read {:?}: {}", p, e);
            std::process::exit(1);
        })
    });

    converse(&prompt, attachment.as_deref(), &cfg);
}
```

---

## Env var reference

| Variable           | Purpose                                              | Default                 |
|--------------------|------------------------------------------------------|-------------------------|
| `TAI_BASE_URL`     | API base URL                                         | `http://localhost:8080` |
| `TAI_MODEL`        | Model name sent in request                           | `llama3`                |
| `OPENAI_API_KEY`   | Bearer token; omit for keyless servers               | —                       |
| `TAI_SPEAKER_USER` | Label for user turns in transcript header            | `USER`                  |
| `TAI_SPEAKER_AI`   | Fixed label for AI turns; unset = use reported model | —                       |
| `TAI_STDOUT_HEADER`| Include header on stdout                             | `0`                     |
| `TAI_STDOUT_INDENT`| Indent content on stdout                             | `0`                     |
| `TAI_STDOUT_SEP`   | Separator on stdout                                  | `0`                     |
| `TAI_FILE_HEADER`  | Include header in log file                           | `1`                     |
| `TAI_FILE_INDENT`  | Indent content in log file                           | `1`                     |
| `TAI_FILE_SEP`     | Separator in log file                                | `1`                     |
| `TAI_APP_NAME`     | App name for XDG paths                               | `terminal-ai`           |
| `TAI_ENV_FILE`     | Env file path (highest priority)                     | —                       |

## Usage examples

Pipe response directly into another tool — stdout is clean content by default:

```sh
terminal-ai "list ten unix commands" | fzf
```

Show full transcript format on stdout (inspection / debug):

```sh
TAI_STDOUT_HEADER=1 TAI_STDOUT_INDENT=1 TAI_STDOUT_SEP=1 terminal-ai "hello"
```

Transcripts attributed by model when switching backends:

```sh
# Each response recorded as e.g. [2025-01-01 12:00:00+00, deepseek-r1, abc12345-00000000]
TAI_BASE_URL=https://api.deepseek.com OPENAI_API_KEY=sk-... terminal-ai "explain monads"

# Stable alias regardless of reported model name
TAI_SPEAKER_AI=assistant terminal-ai "explain monads"
```

Dry run to inspect format without hitting any backend:

```sh
TAI_STDOUT_HEADER=1 TAI_STDOUT_SEP=1 terminal-ai --dry-run "test"
```
