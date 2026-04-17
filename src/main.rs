//! terminal-ai — pipeline-safe AI client with structured transcript logging.
//!
//! `terminal-ai` is a small CLI for sending a prompt to a chat-completions style
//! backend while keeping its output channels deliberately separate:
//!
//! - `stdout` for pipe-safe AI response content
//! - transcript files for append-only archival logging
//! - optional artifact files for persistent response output
//! - `stderr` for diagnostics, metadata, and prompts only
//!
//! This separation allows shell-friendly usage such as:
//!
//! ```text
//! terminal-ai "question" | fzf
//! ```
//!
//! without contaminating piped content with prompts, warnings, or metadata.
//!
//! # The three output channels
//!
//! ```text
//! stdin / --input-file(s)
//!        │
//!        ▼
//!  [ call_ai_endpoint ]
//!        │
//!        ├──► stdout          AI response content. Clean for pipes.
//!        │                    terminal-ai "question" | fzf
//!        │
//!        ├──► transcript      Both USER and AI turns, with full metadata headers.
//!        │                    Append-only dated log. Disabled: --no-transcript
//!        │
//!        └──► artifact        AI response written to --output-file.
//!                             Persistent artifact sink. Separate from transcript.
//! ```
//!
//! `stderr` is a fourth diagnostic channel with optional ANSI color.
//! It carries metadata, errors, and interactive prompts — never conversation content.
//!
//! # Configuration resolution
//!
//! ```text
//! CLI flag  >  TAI_ENV_FILE / --env-file  >  config.env  >  .default.env  >  defaults
//! ```
//!
//! The process environment is never mutated. All env files are parsed into local maps.
//!
//! # Runtime dependencies
//!
//! - `curl`       HTTP transport
//! - `sha256sum`  Message ID derivation
//! - `date`       RFC-3339 timestamps

use std::collections::HashMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Deserialize;

// ── Constants ─────────────────────────────────────────────────────────────────

/// Application version
const VERSION: &str = "0.2.0";

/// Default model name used when no CLI, environment, or config-file override is present.
const DEFAULT_MODEL:    &str = "gpt-5.4";

/// Default API base URL used when no override is present.
const DEFAULT_BASE_URL: &str = "https://api.openai.com";

/// Fixed system prompt sent with every request.
///
/// This is intentionally compile-time constant so the tool always presents itself
/// to the backend in a predictable way unless the source is modified.
const SYSTEM_PROMPT:    &str =
    "You are a Terminal Based Assistant, which provides \
     Responses Compliant to CommonMark Spec (unless otherwise specified).";

// ── ANSI color codes ──────────────────────────────────────────────────────────
//
// Used only by the stderr channel. Never written to stdout or transcript.

/// ANSI reset sequence.
const ANSI_RESET:   &str = "\x1b[0m";
/// ANSI bold sequence.
const ANSI_BOLD:    &str = "\x1b[1m";
/// ANSI dim sequence.
const ANSI_DIM:     &str = "\x1b[2m";
/// ANSI red foreground sequence.
const ANSI_RED:     &str = "\x1b[31m";
/// ANSI yellow foreground sequence.
const ANSI_YELLOW:  &str = "\x1b[33m";
/// ANSI cyan foreground sequence.
const ANSI_CYAN:    &str = "\x1b[36m";

// ── Channel defaults ──────────────────────────────────────────────────────────
//
// Edit this block to change compile-time defaults per channel.
// Runtime overrides: TAI_{CHANNEL}_{FLAG} env vars, or -o tokens.
// All defaults are grouped here so changes are visible at a glance.

/// Compile-time default rendering flags for a single output channel.
///
/// These values seed [`FormatOptions`] before environment-variable overrides and
/// `-o/--output-control` tokens are applied.
struct ChannelDefaults {
    /// Whether the channel is enabled at all.
    enabled:   bool,
    /// Whether to emit the `[timestamp, speaker, message_id]` header line.
    header:    bool,
    /// Whether content lines should be indented.
    indent:    bool,
    /// Whether to emit a trailing `---` separator.
    separator: bool,
    /// Whether inline metadata comments should be emitted into this channel.
    metadata:  bool,
    /// Whether the speaker label should be emitted on its own line.
    speaker:   bool,
    /// Whether the message body itself should be emitted.
    prompt:    bool,
}

/// `stdout`: content-only, safe for pipes.
const STDOUT_DEFAULTS: ChannelDefaults = ChannelDefaults {
    enabled: true, header: false, indent: false, separator: false,
    metadata: false, speaker: false, prompt: true,
};

/// transcript: full archival format.
const TRANSCRIPT_DEFAULTS: ChannelDefaults = ChannelDefaults {
    enabled: true, header: true, indent: true, separator: true,
    metadata: false, speaker: false, prompt: true,
};

/// artifact (`--output-file`): same lean defaults as stdout.
const ARTIFACT_DEFAULTS: ChannelDefaults = ChannelDefaults {
    enabled: true, header: false, indent: false, separator: false,
    metadata: false, speaker: false, prompt: true,
};

// ── Pipe-safe I/O ─────────────────────────────────────────────────────────────
//
// Both stdout and stderr treat BrokenPipe as a clean exit.
// This allows: terminal-ai "hello" | head -n1
// to terminate with exit code 0 when head closes the pipe.

/// Write text to `stdout`, treating `BrokenPipe` as a successful termination.
///
/// This is important for shell pipelines where downstream tools may stop reading
/// early, for example:
///
/// ```text
/// terminal-ai "hello" | head -n1
/// ```
///
/// In that case, the process exits with code `0` instead of reporting an error.
fn write_stdout_safely(text: &str) {
    let mut stdout_handle = io::stdout().lock();
    match stdout_handle.write_all(text.as_bytes()) {
        Ok(())                                                      => {}
        Err(write_error) if write_error.kind() == io::ErrorKind::BrokenPipe => {
            std::process::exit(0);
        }
        Err(write_error) => {
            let _ = writeln!(io::stderr(), "error: stdout write failed: {write_error}");
            std::process::exit(1);
        }
    }
    match stdout_handle.flush() {
        Ok(())                                                       => {}
        Err(flush_error) if flush_error.kind() == io::ErrorKind::BrokenPipe => {
            std::process::exit(0);
        }
        Err(flush_error) => {
            let _ = writeln!(io::stderr(), "error: stdout flush failed: {flush_error}");
            std::process::exit(1);
        }
    }
}

/// Write text to `stderr`, treating `BrokenPipe` as a successful termination.
fn write_stderr_safely(text: &str) {
    let mut stderr_handle = io::stderr().lock();
    match stderr_handle.write_all(text.as_bytes()) {
        Ok(())                                                       => {}
        Err(write_error) if write_error.kind() == io::ErrorKind::BrokenPipe => {
            std::process::exit(0);
        }
        Err(_) => { std::process::exit(1); }
    }
    let _ = stderr_handle.flush();
}

// ── stderr channel ────────────────────────────────────────────────────────────
//
// The fourth channel. Carries diagnostics, metadata, and interactive prompts.
// Never carries conversation content.
//
// Color is auto-detected from stderr.is_terminal(), overridable via
// TAI_STDERR_COLOR=1/0. This means 2>file automatically strips color.

/// Renderer for the dedicated `stderr` diagnostic channel.
///
/// Unlike `stdout`, transcript output, and artifact output, this channel is
/// reserved exclusively for:
///
/// - hard errors
/// - warnings
/// - diagnostics
/// - interactive prompts
/// - response metadata summaries
///
/// It must never carry user or AI conversation content.
struct StderrChannel {
    /// Whether ANSI color should be emitted.
    use_color: bool,
}

impl StderrChannel {
    /// Construct a `StderrChannel` from process environment plus parsed env-file map.
    ///
    /// Color selection precedence is:
    ///
    /// 1. `TAI_STDERR_COLOR` from process environment
    /// 2. `TAI_STDERR_COLOR` from merged env-file map
    /// 3. auto-detection from `stderr.is_terminal()`
    fn from_env(file_map: &HashMap<String, String>) -> Self {
        let stderr_is_terminal = io::stderr().is_terminal();
        let color_override = env::var("TAI_STDERR_COLOR").ok()
            .or_else(|| file_map.get("TAI_STDERR_COLOR").cloned());
        let use_color = color_override
            .map(|value| matches!(value.trim(), "1" | "true" | "yes" | "on"))
            .unwrap_or(stderr_is_terminal);
        Self { use_color }
    }

    /// Emit response metadata.
    ///
    /// The format is a stable HTML comment string suitable for logging or copy/paste.
    /// Colorized output uses dim cyan.
    fn emit_metadata(&self, metadata: &ResponseMetadata) {
        let metadata_text = metadata.to_html_comment();
        if self.use_color {
            write_stderr_safely(&format!("{ANSI_DIM}{ANSI_CYAN}{metadata_text}{ANSI_RESET}\n"));
        } else {
            write_stderr_safely(&format!("{metadata_text}\n"));
        }
    }

    /// Emit a hard error and terminate the process with exit code `1`.
    ///
    /// Colorized output uses bold red.
    fn emit_error_and_exit(&self, message: &str) -> ! {
        if self.use_color {
            write_stderr_safely(
                &format!("{ANSI_BOLD}{ANSI_RED}error:{ANSI_RESET} {message}\n")
            );
        } else {
            write_stderr_safely(&format!("error: {message}\n"));
        }
        std::process::exit(1);
    }

    /// Emit a non-fatal warning.
    ///
    /// Colorized output uses yellow.
    fn emit_warning(&self, message: &str) {
        if self.use_color {
            write_stderr_safely(
                &format!("{ANSI_YELLOW}warning:{ANSI_RESET} {message}\n")
            );
        } else {
            write_stderr_safely(&format!("warning: {message}\n"));
        }
    }

    /// Emit a diagnostic line intended for debugging or verbose output.
    ///
    /// Colorized output uses dim text.
    fn emit_diagnostic(&self, message: &str) {
        if self.use_color {
            write_stderr_safely(&format!("{ANSI_DIM}{message}{ANSI_RESET}\n"));
        } else {
            write_stderr_safely(&format!("{message}\n"));
        }
    }

    /// Emit an interactive prompt without appending a trailing newline.
    ///
    /// Colorized output uses dim text.
    fn emit_interactive_prompt(&self, message: &str) {
        if self.use_color {
            write_stderr_safely(&format!("{ANSI_DIM}{message}{ANSI_RESET}"));
        } else {
            write_stderr_safely(message);
        }
    }
}

// ── Data structures ───────────────────────────────────────────────────────────

/// The logical origin of a recorded speech turn.
#[derive(Clone, Debug, PartialEq, Eq)]
enum SpeechKind {
    /// A user-authored turn.
    User,
    /// An AI-authored turn.
    Ai,
}

/// Response metadata extracted from backend JSON.
///
/// All fields are optional to tolerate partial or backend-specific responses.
/// When absent, the metadata renderer substitutes empty strings or zeros.
#[derive(Clone, Debug, Default)]
struct ResponseMetadata {
    /// Model name reported by the backend.
    model:             Option<String>,
    /// Backend response identifier.
    id:                Option<String>,
    /// Total token count, if reported.
    total_tokens:      Option<u64>,
    /// Prompt token count, if reported.
    prompt_tokens:     Option<u64>,
    /// Completion token count, if reported.
    completion_tokens: Option<u64>,
}

impl ResponseMetadata {
    /// Render metadata as a stable HTML comment string.
    ///
    /// This mirrors the shell-era convention and is suitable for transcript
    /// embedding or diagnostic output.
    fn to_html_comment(&self) -> String {
        format!(
            r#"<!--{{"model":"{}","id":"{}","usage":{{"total":{},"prompt":{},"completion":{}}}}}-->"#,
            self.model.as_deref().unwrap_or(""),
            self.id.as_deref().unwrap_or(""),
            self.total_tokens.unwrap_or(0),
            self.prompt_tokens.unwrap_or(0),
            self.completion_tokens.unwrap_or(0),
        )
    }
}

/// One fully materialized speech turn, ready for rendering to any channel.
///
/// `content` holds what was said — the composed user message, or the AI response.
/// Named `content` rather than `prompt` because AI turns contain responses.
#[derive(Clone, Debug)]
struct SpeechRecord {
    /// Whether this turn originated from the user or the AI.
    kind:       SpeechKind,
    /// Human-readable speaker label.
    speaker:    String,
    /// The message body. User: composed prompt + attachments. AI: response text.
    content:    String,
    /// RFC-3339 timestamp.
    timestamp:  String,
    /// Deterministic message identifier encoding speaker/timestamp/parentage.
    message_id: String,
    /// Optional backend metadata, generally present for AI turns only.
    metadata:   Option<ResponseMetadata>,
}

// ── API response deserialization ──────────────────────────────────────────────

/// Top-level response shape expected from a chat-completions style backend.
#[derive(Deserialize)]
struct ChatResponse {
    /// Backend response identifier.
    id:      Option<String>,
    /// Model name reported by the backend.
    model:   Option<String>,
    /// Optional usage metadata block.
    usage:   Option<UsageMetadata>,
    /// Returned choices, typically at least one.
    choices: Vec<ResponseChoice>,
}

/// Usage block from backend JSON.
#[derive(Deserialize)]
struct UsageMetadata {
    /// Total token count.
    total_tokens:      Option<u64>,
    /// Prompt token count.
    prompt_tokens:     Option<u64>,
    /// Completion token count.
    completion_tokens: Option<u64>,
}

/// One completion choice from backend JSON.
#[derive(Deserialize)]
struct ResponseChoice {
    /// Message payload for this choice.
    message: ResponseMessage,
}

/// Assistant message payload from backend JSON.
#[derive(Deserialize)]
struct ResponseMessage {
    /// Response text content.
    content: String,
}

// ── Env file parsing ──────────────────────────────────────────────────────────

/// Parse a simple `KEY=VALUE` environment file into a map.
///
/// Behavior:
///
/// - strips inline `#` comments
/// - trims surrounding single or double quotes
/// - skips malformed or empty lines
/// - returns an empty map on I/O failure
///
/// This function is intentionally forgiving for user-maintained config files.
fn parse_env_file(env_file_path: &Path) -> HashMap<String, String> {
    let Ok(file_text) = fs::read_to_string(env_file_path) else {
        return HashMap::new();
    };
    file_text.lines().filter_map(|line| {
        let uncommented = line.split('#').next()?.trim();
        if uncommented.is_empty() { return None; }
        let (key, value) = uncommented.split_once('=')?;
        let value_trimmed = value.trim().trim_matches('"').trim_matches('\'');
        let key_trimmed = key.trim();
        if key_trimmed.is_empty() { return None; }
        Some((key_trimmed.to_string(), value_trimmed.to_string()))
    })
    .collect()
}

/// Load and merge the configuration file chain into one map.
///
/// Priority, lowest to highest:
///
/// 1. XDG `.default.env` (legacy compatibility)
/// 2. XDG `config.env`
/// 3. explicit session override from `--env-file` or `TAI_ENV_FILE`
///
/// Missing default files are ignored. A missing explicit file is a hard error.
fn load_merged_config_map(
    xdg_config_app_dir: &Path,
    explicit_env_file:  Option<&Path>,
    stderr:             &StderrChannel,
) -> HashMap<String, String> {
    let mut merged: HashMap<String, String> = HashMap::new();

    // Layer 1: legacy .default.env (lowest priority)
    let legacy_env_path = xdg_config_app_dir.join(".default.env");
    if legacy_env_path.exists() {
        merged.extend(parse_env_file(&legacy_env_path));
    }

    // Layer 2: primary config.env
    let primary_config_path = xdg_config_app_dir.join("config.env");
    if primary_config_path.exists() {
        merged.extend(parse_env_file(&primary_config_path));
    }

    // Layer 3: explicit override (--env-file or TAI_ENV_FILE)
    if let Some(explicit_path) = explicit_env_file {
        if !explicit_path.exists() {
            stderr.emit_error_and_exit(
                &format!("env file not found: {}", explicit_path.display())
            );
        }
        merged.extend(parse_env_file(explicit_path));
    }

    merged
}

// ── Value resolution ──────────────────────────────────────────────────────────
//
// Single resolution point for all configurable values.
// Priority: CLI flag > process env > merged file map > default.

/// Resolve a required string-valued setting.
///
/// Precedence:
///
/// 1. CLI-provided value
/// 2. process environment
/// 3. merged env-file map
/// 4. lazy default
fn resolve_value(
    cli_value: Option<String>,
    env_key:   &str,
    file_map:  &HashMap<String, String>,
    default:   impl FnOnce() -> String,
) -> String {
    cli_value
        .or_else(|| env::var(env_key).ok().filter(|value| !value.is_empty()))
        .or_else(|| file_map.get(env_key).filter(|value| !value.is_empty()).cloned())
        .unwrap_or_else(default)
}

/// Resolve an optional string-valued setting.
///
/// Empty values are treated as unset.
fn resolve_optional_value(
    cli_value: Option<String>,
    env_key:   &str,
    file_map:  &HashMap<String, String>,
) -> Option<String> {
    cli_value
        .or_else(|| env::var(env_key).ok().filter(|value| !value.is_empty()))
        .or_else(|| file_map.get(env_key).filter(|value| !value.is_empty()).cloned())
}

/// Resolve a boolean setting.
///
/// `cli_flag` acts as a force-enable switch. If it is `true`, the result is
/// immediately `true`. Otherwise resolution proceeds via process environment,
/// then env-file map, then the provided default.
///
/// Truthy values are: `1`, `true`, `yes`, `on` (case-insensitive).
fn resolve_bool_value(
    cli_flag: bool,
    env_key:  &str,
    file_map: &HashMap<String, String>,
    default:  bool,
) -> bool {
    if cli_flag { return true; }
    let raw = env::var(env_key).ok()
        .or_else(|| file_map.get(env_key).cloned());
    raw.map(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
       .unwrap_or(default)
}

// ── XDG paths ─────────────────────────────────────────────────────────────────

/// Return the current user's home directory, or `/tmp` as a last resort.
fn home_directory() -> PathBuf {
    PathBuf::from(env::var("HOME").unwrap_or_else(|_| "/tmp".into()))
}

/// Return the XDG config home directory.
fn xdg_config_home() -> PathBuf {
    env::var("XDG_CONFIG_HOME").map(PathBuf::from)
        .unwrap_or_else(|_| home_directory().join(".config"))
}

/// Return the XDG data home directory.
fn xdg_data_home() -> PathBuf {
    env::var("XDG_DATA_HOME").map(PathBuf::from)
        .unwrap_or_else(|_| home_directory().join(".local/share"))
}

/// Return the application name used for XDG directory layout.
///
/// This is overridable through `TAI_APP_NAME`.
fn app_name() -> String {
    env::var("TAI_APP_NAME").unwrap_or_else(|_| "terminal-ai".into())
}

// ── Timestamps and message IDs ────────────────────────────────────────────────
//
// MessageID := "<SpeechID>-<AudienceID>"
// SpeechID  := HASH8(speaker, timestamp)
// AudienceID := parent SpeechID, or HASH8("NULL") for roots
//
// AI turn IDs encode the user turn ID as parent — forming a causal chain
// that allows transcript replay and conversation branching without a database.

/// Invoke `date(1)` with the supplied flag and return trimmed stdout.
///
/// # Panics
///
/// Panics if `date` is not available in `PATH`.
fn run_date_command(format_flag: &str) -> String {
    let date_output = Command::new("date")
        .arg(format_flag)
        .output()
        .expect("date(1) not found in PATH");
    String::from_utf8_lossy(&date_output.stdout).trim().to_string()
}

/// Return today's date as `YYYY-MM-DD` using `date --rfc-3339=date`.
fn today_as_date_string()     -> String { run_date_command("--rfc-3339=date")    }

/// Return the current RFC-3339 timestamp using `date --rfc-3339=seconds`.
fn current_timestamp_string() -> String { run_date_command("--rfc-3339=seconds") }

/// Hash the provided text with `sha256sum` and return the first 8 hex characters.
///
/// Empty input is normalized to `"NULL"`.
///
/// # Panics
///
/// Panics if `sha256sum` is not available in `PATH`, or if process execution fails.
fn sha256_first_8_chars(input_text: &str) -> String {
    let source_text = if input_text.is_empty() { "NULL" } else { input_text };
    let mut sha256_process = Command::new("sha256sum")
        .stdin(Stdio::piped()).stdout(Stdio::piped())
        .spawn().expect("sha256sum not found in PATH");
    if let Some(mut stdin_pipe) = sha256_process.stdin.take() {
        let _ = stdin_pipe.write_all(source_text.as_bytes());
    }
    let sha256_output = sha256_process.wait_with_output().expect("sha256sum failed");
    String::from_utf8_lossy(&sha256_output.stdout)[..8].to_string()
}

/// Generate a message ID from speaker, timestamp, and optional parent.
///
/// The resulting format is:
///
/// ```text
/// <self_hash>-<parent_hash>
/// ```
///
/// where:
///
/// - `self_hash = HASH8(timestamp, speaker)`
/// - `parent_hash` is the first 8 characters of the parent ID, or `HASH8("NULL")` for roots
fn generate_message_id(
    speaker_label:     &str,
    timestamp:         &str,
    parent_message_id: Option<&str>,
) -> String {
    let self_hash = sha256_first_8_chars(&format!("{timestamp},{speaker_label}"));
    let parent_hash = parent_message_id
        .map(|parent_id| parent_id[..parent_id.len().min(8)].to_string())
        .unwrap_or_else(|| sha256_first_8_chars("NULL"));
    format!("{self_hash}-{parent_hash}")
}

// ── Output format options ─────────────────────────────────────────────────────
//
// FormatOptions is the rendering configuration for a single channel.
// render_record() is pure — no I/O, no side effects.
//
// Three instances exist: stdout_format, transcript_format, artifact_format.
// Each is constructed from its ChannelDefaults, then env vars, then -o tokens.

/// Rendering configuration for one output channel.
///
/// This type is pure formatting state: it does not perform any I/O itself.
/// Rendering is done by [`FormatOptions::render_record`].
#[derive(Clone, Debug)]
struct FormatOptions {
    /// Master switch: when false, `render_record` returns an empty string.
    enabled:        bool,
    /// Emit `[timestamp, speaker, message_id]` header.
    emit_header:    bool,
    /// Prefix each content line with `indent_string`.
    indent_content: bool,
    /// Emit `---` thematic break after the content.
    emit_separator: bool,
    /// Emit the metadata HTML comment inline in this channel.
    ///
    /// Metadata is also independently emitted to `stderr`.
    emit_metadata:  bool,
    /// Emit speaker label as a standalone line.
    emit_speaker:   bool,
    /// Emit the content body.
    emit_content:   bool,
    /// String used when indenting content lines. Defaults to a single space.
    indent_string:  String,
}

impl FormatOptions {
    /// Construct channel formatting from compile-time defaults plus env overrides.
    ///
    /// `env_prefix` is expected to be one of:
    ///
    /// - `"STDOUT_"`
    /// - `"TRANSCRIPT_"`
    /// - `"ARTIFACT_"`
    fn from_defaults_and_env(
        env_prefix: &str,
        file_map:   &HashMap<String, String>,
        defaults:   &ChannelDefaults,
    ) -> Self {
        let resolve_bool = |flag_suffix: &str, default_value: bool| {
            let full_key = format!("TAI_{env_prefix}{flag_suffix}");
            env::var(&full_key).ok()
                .or_else(|| file_map.get(&full_key).cloned())
                .map(|value| matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                ))
                .unwrap_or(default_value)
        };

        let indent_key = format!("TAI_{env_prefix}INDENT_STR");
        let indent_string = env::var(&indent_key).ok()
            .or_else(|| file_map.get(&indent_key).cloned())
            .unwrap_or_else(|| " ".to_string());

        Self {
            enabled:        resolve_bool("ENABLE",   defaults.enabled),
            emit_header:    resolve_bool("HEADER",   defaults.header),
            indent_content: resolve_bool("INDENT",   defaults.indent),
            emit_separator: resolve_bool("SEP",      defaults.separator),
            emit_metadata:  resolve_bool("METADATA", defaults.metadata),
            emit_speaker:   resolve_bool("SPEAKER",  defaults.speaker),
            emit_content:   resolve_bool("PROMPT",   defaults.prompt),
            indent_string,
        }
    }

    /// Render one [`SpeechRecord`] according to this channel's configuration.
    ///
    /// Returns an empty string when the channel is disabled.
    fn render_record(&self, speech: &SpeechRecord) -> String {
        if !self.enabled { return String::new(); }

        let mut rendered = String::new();

        if self.emit_header {
            rendered.push_str(&format!(
                "[{}, {}, {}]\n",
                speech.timestamp, speech.speaker, speech.message_id
            ));
        }

        if self.emit_speaker {
            rendered.push_str(&speech.speaker);
            rendered.push('\n');
        }

        if self.emit_content {
            if self.indent_content {
                for line in speech.content.lines() {
                    rendered.push_str(&self.indent_string);
                    rendered.push_str(line);
                    rendered.push('\n');
                }
                if speech.content.is_empty() {
                    rendered.push_str(&self.indent_string);
                    rendered.push('\n');
                }
            } else {
                rendered.push_str(&speech.content);
                if !speech.content.ends_with('\n') {
                    rendered.push('\n');
                }
            }
        }

        if self.emit_metadata
            && let Some(metadata) = &speech.metadata {
                rendered.push_str(&metadata.to_html_comment());
                rendered.push('\n');
            }

        if self.emit_separator {
            rendered.push_str("\n---\n");
        }

        rendered
    }
}

// ── -o token grammar ──────────────────────────────────────────────────────────
//
// apply_format_token mutates a single FormatOptions.
// apply_output_control_token dispatches to the right channel(s) and policy.
//
// Token grammar:
//   {PRESET}_{CHANNEL}             — e.g. CONTENT_ONLY_STDOUT
//   {PRESET}                       — e.g. CONTENT_ONLY  (applies to all channels)
//   {NO|ADD}_{FLAG}_{CHANNEL}      — e.g. NO_HEADER_STDOUT
//   {NO|ADD}_{FLAG}                — e.g. NO_METADATA  (applies to all channels)
//   NO_{USER|AI}_{CHANNEL}         — speaker suppression

/// Apply a preset or flag token to a single [`FormatOptions`] instance.
///
/// Returns an error string if the token is not recognized.
fn apply_format_token(operation: &str, format: &mut FormatOptions) -> Result<(), String> {
    match operation {
        // Presets
        "CONTENT_ONLY" | "PROMPT_ONLY" => {
            format.emit_header    = false;
            format.emit_metadata  = false;
            format.emit_speaker   = false;
            format.emit_content   = true;
            format.indent_content = false;
            format.emit_separator = false;
        }
        "METADATA_ONLY" => {
            format.emit_header    = false;
            format.emit_metadata  = true;
            format.emit_speaker   = false;
            format.emit_content   = false;
            format.emit_separator = false;
        }
        "HEADER_ONLY" => {
            format.emit_header    = true;
            format.emit_metadata  = false;
            format.emit_speaker   = false;
            format.emit_content   = false;
            format.emit_separator = false;
        }
        "FULL" => {
            format.enabled        = true;
            format.emit_header    = true;
            format.indent_content = true;
            format.emit_separator = true;
            format.emit_metadata  = true;
            format.emit_speaker   = true;
            format.emit_content   = true;
        }
        "SILENT" | "DISABLE" => { format.enabled = false; }
        "ENABLE"             => { format.enabled = true; }

        // Individual flag negations
        "NO_HEADER"   | "NO_HEADER_FLAG"  => { format.emit_header    = false; }
        "NO_METADATA"                      => { format.emit_metadata  = false; }
        "NO_PROMPT"   | "NO_CONTENT"       => { format.emit_content   = false; }
        "NO_INDENT"                        => { format.indent_content = false; }
        "NO_SEP"      | "NO_SEPARATOR"     => { format.emit_separator = false; }
        "NO_SPEAKER"                       => { format.emit_speaker   = false; }

        // Individual flag additions
        "ADD_HEADER"  | "WITH_HEADER"      => { format.emit_header    = true; }
        "ADD_METADATA"| "WITH_METADATA"    => { format.emit_metadata  = true; }
        "ADD_PROMPT"  | "WITH_CONTENT"     => { format.emit_content   = true; }
        "ADD_INDENT"  | "WITH_INDENT"      => { format.indent_content = true; }
        "ADD_SEP"     | "WITH_SEP"         => { format.emit_separator = true; }
        "ADD_SPEAKER" | "WITH_SPEAKER"     => { format.emit_speaker   = true; }

        other => return Err(format!("unknown format operation: {other}")),
    }
    Ok(())
}

/// Apply one `-o/--output-control` token.
///
/// Tokens may target:
///
/// - a single channel via `_STDOUT`, `_TRANSCRIPT`, `_ARTIFACT`
/// - all channels when no suffix is present
/// - output policy via `NO_USER_*` / `NO_AI_*` suppression tokens
fn apply_output_control_token(
    token:               &str,
    stdout_format:       &mut FormatOptions,
    transcript_format:   &mut FormatOptions,
    artifact_format:     &mut FormatOptions,
    output_policy:       &mut OutputPolicy,
) -> Result<(), String> {
    let upper = token.trim().to_ascii_uppercase();
    if upper.is_empty() { return Ok(()); }

    // Speaker suppression tokens — handled before channel dispatch.
    match upper.as_str() {
        "NO_USER_STDOUT"                          => { output_policy.suppress_user_stdout     = true; return Ok(()); }
        "NO_USER_TRANSCRIPT" | "NO_USER_FILE"     => { output_policy.suppress_user_transcript = true; return Ok(()); }
        "NO_USER_ARTIFACT"                        => { output_policy.suppress_user_artifact   = true; return Ok(()); }
        "NO_AI_STDOUT"                            => { output_policy.suppress_ai_stdout       = true; return Ok(()); }
        "NO_AI_TRANSCRIPT"   | "NO_AI_FILE"       => { output_policy.suppress_ai_transcript   = true; return Ok(()); }
        "NO_AI_ARTIFACT"                          => { output_policy.suppress_ai_artifact     = true; return Ok(()); }
        _ => {}
    }

    // Channel-suffixed tokens — dispatch to one channel.
    if upper.ends_with("_STDOUT") {
        let operation = &upper[..upper.len() - 7];
        return apply_format_token(operation, stdout_format);
    }
    if upper.ends_with("_TRANSCRIPT") {
        let operation = &upper[..upper.len() - 11];
        return apply_format_token(operation, transcript_format);
    }
    if upper.ends_with("_FILE") {
        // _FILE is an alias for _TRANSCRIPT
        let operation = &upper[..upper.len() - 5];
        return apply_format_token(operation, transcript_format);
    }
    if upper.ends_with("_ARTIFACT") {
        let operation = &upper[..upper.len() - 9];
        return apply_format_token(operation, artifact_format);
    }

    // No channel suffix — apply to all format channels.
    apply_format_token(&upper, stdout_format)?;
    apply_format_token(&upper, transcript_format)?;
    apply_format_token(&upper, artifact_format)?;
    Ok(())
}

// ── Output policy ─────────────────────────────────────────────────────────────

/// Per-speaker, per-channel suppression policy.
///
/// This is evaluated before rendering, allowing a turn to be excluded from one
/// sink while still being sent to others.
#[derive(Clone, Debug)]
struct OutputPolicy {
    /// Suppress user turns from stdout.
    suppress_user_stdout:     bool,
    /// Suppress user turns from transcript output.
    suppress_user_transcript: bool,
    /// Suppress user turns from artifact output.
    suppress_user_artifact:   bool,
    /// Suppress AI turns from stdout.
    suppress_ai_stdout:       bool,
    /// Suppress AI turns from transcript output.
    suppress_ai_transcript:   bool,
    /// Suppress AI turns from artifact output.
    suppress_ai_artifact:     bool,
}

impl OutputPolicy {
    /// Return a policy that suppresses nothing.
    fn none() -> Self {
        Self {
            suppress_user_stdout:     false,
            suppress_user_transcript: false,
            suppress_user_artifact:   false,
            suppress_ai_stdout:       false,
            suppress_ai_transcript:   false,
            suppress_ai_artifact:     false,
        }
    }

    /// Return whether this speech kind may be emitted to stdout.
    fn allow_stdout(&self, kind: &SpeechKind) -> bool {
        match kind {
            SpeechKind::User => !self.suppress_user_stdout,
            SpeechKind::Ai   => !self.suppress_ai_stdout,
        }
    }

    /// Return whether this speech kind may be emitted to transcript output.
    fn allow_transcript(&self, kind: &SpeechKind) -> bool {
        match kind {
            SpeechKind::User => !self.suppress_user_transcript,
            SpeechKind::Ai   => !self.suppress_ai_transcript,
        }
    }

    /// Return whether this speech kind may be emitted to artifact output.
    fn allow_artifact(&self, kind: &SpeechKind) -> bool {
        match kind {
            SpeechKind::User => !self.suppress_user_artifact,
            SpeechKind::Ai   => !self.suppress_ai_artifact,
        }
    }
}

// ── Config ────────────────────────────────────────────────────────────────────

/// Fully resolved runtime configuration.
struct Config {
    /// Requested model name.
    model:    String,
    /// API base URL.
    base_url: String,
    /// Optional bearer token.
    api_key:  Option<String>,

    /// Zero or more files whose contents are appended to the prompt.
    input_files: Vec<PathBuf>,

    /// Whether transcript writing is disabled.
    no_transcript:          bool,
    /// Directory that receives transcript files.
    transcript_directory:   PathBuf,
    /// Transcript filename within `transcript_directory`.
    transcript_filename:    String,

    /// Optional artifact output file path.
    output_file:            Option<PathBuf>,
    /// Whether artifact output overwrites instead of appending.
    overwrite_output_file:  bool,

    /// Whether API calls should be skipped and replaced with a placeholder.
    dry_run: bool,
    /// Whether verbose diagnostics should be emitted to stderr.
    verbose: bool,

    /// Rendering options for stdout.
    stdout_format:     FormatOptions,
    /// Rendering options for transcript output.
    transcript_format: FormatOptions,
    /// Rendering options for artifact output.
    artifact_format:   FormatOptions,
    /// Per-speaker channel suppression rules.
    output_policy:     OutputPolicy,

    /// User speaker label.
    speaker_user_label: String,
    /// Optional AI speaker label. `None` means use backend-reported model name.
    speaker_ai_label:   Option<String>,

    /// Optional explicit parent message ID for threading or branching.
    parent_id: Option<String>,
}

// ── Attachment handling ───────────────────────────────────────────────────────
//
// Multiple --input-file arguments are each appended to the base prompt,
// labelled with their filename so the model has provenance for each block.

/// Build the final prompt by appending each input file as a labelled attachment block.
///
/// Each attachment is appended in the form:
///
/// ```text
/// --- (@attachment: filename.ext) ---
/// <file contents>
/// ```
///
/// A read failure is a hard error.
fn build_prompt_with_attachments(
    base_prompt:      &str,
    input_file_paths: &[PathBuf],
    stderr:           &StderrChannel,
) -> String {
    if input_file_paths.is_empty() {
        return base_prompt.to_string();
    }
    let mut full_prompt = base_prompt.to_string();
    for input_file_path in input_file_paths {
        let filename_display = input_file_path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "attachment".to_string());
        let file_content = fs::read_to_string(input_file_path).unwrap_or_else(|read_error| {
            stderr.emit_error_and_exit(
                &format!("cannot read {}: {}", input_file_path.display(), read_error)
            );
        });
        full_prompt.push_str(&format!(
            "\n--- (@attachment: {filename_display}) ---\n{file_content}"
        ));
    }
    full_prompt
}

// ── JSON escaping ─────────────────────────────────────────────────────────────

/// Escape text for safe interpolation into a JSON string literal.
///
/// This is a minimal string escaper used to construct the request body without
/// a JSON serializer.
fn escape_for_json_string(raw_text: &str) -> String {
    raw_text
        .replace('\\', "\\\\")
        .replace('"',  "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

// ── AI endpoint call ──────────────────────────────────────────────────────────
//
// Returns (response_content, reported_model_label, metadata).
//
// Verbose mode: raw JSON is emitted to stderr as a diagnostic;
// the function still returns the parsed content so the transcript stays clean.
// If parsing also fails, the raw JSON is returned as content and flagged.

/// Result of a backend AI call.
struct AiCallResult {
    /// Extracted response content.
    content:     String,
    /// Effective model label to use for speaker naming.
    model_label: String,
    /// Optional parsed response metadata.
    metadata:    Option<ResponseMetadata>,
}

/// Call the configured AI endpoint and extract content plus metadata.
///
/// In `dry_run` mode, no subprocess is launched and a placeholder response is returned.
///
/// In verbose mode, raw response JSON is echoed to `stderr`.
///
/// If JSON parsing fails, a diagnostic is emitted and an empty content string is returned.
fn call_ai_endpoint(prompt: &str, config: &Config, stderr: &StderrChannel) -> AiCallResult {
    if config.dry_run {
        return AiCallResult {
            content:     "<AI Response>".into(),
            model_label: config.model.clone(),
            metadata:    None,
        };
    }

    let request_body = format!(
        r#"{{"model":"{model}","messages":[{{"role":"system","content":"{system}"}},{{"role":"user","content":"{prompt}"}}]}}"#,
        model  = escape_for_json_string(&config.model),
        system = escape_for_json_string(SYSTEM_PROMPT),
        prompt = escape_for_json_string(prompt),
    );

    let endpoint_url = format!(
        "{}/v1/chat/completions",
        config.base_url.trim_end_matches('/')
    );

    let mut curl_command = Command::new("curl");
    curl_command
        .arg("-s")
        .arg(&endpoint_url)
        .arg("-H").arg("Content-Type: application/json");
    if let Some(api_key) = &config.api_key {
        curl_command.arg("-H").arg(format!("Authorization: Bearer {api_key}"));
    }

    let curl_output = curl_command
        .arg("-d").arg(&request_body)
        .output()
        .expect("curl not found in PATH");

    let raw_response_body = String::from_utf8_lossy(&curl_output.stdout);

    // Verbose: emit raw JSON to stderr as diagnostic, then parse normally.
    // The transcript receives parsed content, not the raw JSON blob.
    if config.verbose {
        stderr.emit_diagnostic(&format!("--- raw response ---\n{raw_response_body}--- end ---"));
    }

    match serde_json::from_str::<ChatResponse>(&raw_response_body) {
        Err(parse_error) => {
            stderr.emit_diagnostic(
                &format!("response parse failed: {parse_error}\nraw: {raw_response_body}")
            );
            AiCallResult {
                content:     String::new(),
                model_label: config.model.clone(),
                metadata:    None,
            }
        }
        Ok(response) => {
            let metadata = response.usage.as_ref().map(|usage| ResponseMetadata {
                model:             response.model.clone(),
                id:                response.id.clone(),
                total_tokens:      usage.total_tokens,
                prompt_tokens:     usage.prompt_tokens,
                completion_tokens: usage.completion_tokens,
            });

            // Metadata always goes to stderr. Channels may also render it
            // inline via emit_metadata flag — that is independent.
            if let Some(meta) = &metadata {
                stderr.emit_metadata(meta);
            }

            let model_label = response.model
                .filter(|model_name| !model_name.is_empty())
                .unwrap_or_else(|| config.model.clone());
            
            let response_content = match response.choices.is_empty() {
                true => {
                    stderr.emit_warning("backend response contained no choices");
                    String::new()
                }
                false => {
                            response.choices
                            .into_iter()
                            .next()
                            .map(|choice| choice.message.content)
                            .unwrap_or_default()
                        }
            };

            AiCallResult {
                content: response_content,
                model_label,
                metadata,
            }
        }
    }
}

// ── File writing helpers ──────────────────────────────────────────────────────

/// Append text to a file, creating it if necessary.
///
/// Any write error is fatal.
fn append_to_file(path: &Path, text: &str, stderr: &StderrChannel) {
    let result = OpenOptions::new()
        .create(true).append(true)
        .open(path)
        .and_then(|mut file| file.write_all(text.as_bytes()));
    if let Err(write_error) = result {
        stderr.emit_error_and_exit(
            &format!("write failed for {}: {write_error}", path.display())
        );
    }
}

/// Overwrite a file with text, creating it if necessary.
///
/// Any write error is fatal.
fn overwrite_file(path: &Path, text: &str, stderr: &StderrChannel) {
    let result = OpenOptions::new()
        .create(true).truncate(true).write(true)
        .open(path)
        .and_then(|mut file| file.write_all(text.as_bytes()));
    if let Err(write_error) = result {
        stderr.emit_error_and_exit(
            &format!("write failed for {}: {write_error}", path.display())
        );
    }
}

// ── Recording ─────────────────────────────────────────────────────────────────
//
// Writes one SpeechRecord to all enabled sinks, respecting OutputPolicy.
// Each channel is independent; a failure on one does not affect others.

/// Record one speech turn to all enabled sinks.
///
/// The same [`SpeechRecord`] may be written to:
///
/// - `stdout`
/// - transcript file
/// - artifact file
///
/// subject to both [`OutputPolicy`] and per-channel [`FormatOptions`].
fn record_speech(speech: &SpeechRecord, config: &Config, stderr: &StderrChannel) {
    // stdout
    if config.output_policy.allow_stdout(&speech.kind) {
        let stdout_entry = config.stdout_format.render_record(speech);
        if !stdout_entry.is_empty() {
            write_stdout_safely(&stdout_entry);
        }
    }

    // transcript
    if !config.no_transcript && config.output_policy.allow_transcript(&speech.kind) {
        let transcript_entry = config.transcript_format.render_record(speech);
        if !transcript_entry.is_empty() {
            let transcript_path = config
                .transcript_directory
                .join(&config.transcript_filename);
            append_to_file(&transcript_path, &transcript_entry, stderr);
        }
    }

    // artifact (--output-file)
    if let Some(output_file_path) = &config.output_file
        && config.output_policy.allow_artifact(&speech.kind) {
            let artifact_entry = config.artifact_format.render_record(speech);
            if !artifact_entry.is_empty() {
                if config.overwrite_output_file {
                    overwrite_file(output_file_path, &artifact_entry, stderr);
                } else {
                    append_to_file(output_file_path, &artifact_entry, stderr);
                }
            }
        }
}

// ── Converse ──────────────────────────────────────────────────────────────────
//
// One full round-trip:
//   1. Build full prompt from base prompt + attachments
//   2. Record USER turn (transcript only — never stdout by default)
//   3. Call backend
//   4. Record AI turn (stdout + transcript + artifact)
//
// AI speaker label: TAI_SPEAKER_AI override > model name from response.

/// Execute one full user→AI round-trip.
///
/// Steps:
///
/// 1. Build the full prompt from CLI/stdin text plus attachments
/// 2. Record the user turn
/// 3. Call the backend
/// 4. Record the AI turn
fn converse(base_prompt: &str, config: &Config, stderr: &StderrChannel) {
    let full_prompt = build_prompt_with_attachments(base_prompt, &config.input_files, stderr);

    let user_timestamp  = current_timestamp_string();
    let user_message_id = generate_message_id(
        &config.speaker_user_label,
        &user_timestamp,
        config.parent_id.as_deref(),
    );

    let user_speech = SpeechRecord {
        kind:       SpeechKind::User,
        speaker:    config.speaker_user_label.clone(),
        content:    full_prompt.clone(),
        timestamp:  user_timestamp,
        message_id: user_message_id.clone(),
        metadata:   None,
    };
    record_speech(&user_speech, config, stderr);

    let ai_result   = call_ai_endpoint(&full_prompt, config, stderr);
    let ai_timestamp = current_timestamp_string();
    let ai_speaker   = config.speaker_ai_label
        .as_deref()
        .unwrap_or(&ai_result.model_label)
        .to_string();
    let ai_message_id = generate_message_id(
        &ai_speaker,
        &ai_timestamp,
        Some(&user_message_id),
    );

    let ai_speech = SpeechRecord {
        kind:       SpeechKind::Ai,
        speaker:    ai_speaker,
        content:    ai_result.content,
        timestamp:  ai_timestamp,
        message_id: ai_message_id,
        metadata:   ai_result.metadata,
    };
    record_speech(&ai_speech, config, stderr);
}

// ── Argument parsing ──────────────────────────────────────────────────────────

/// Consume the next CLI argument as the value for `option_name`.
///
/// Exits with an error if the value is missing.
fn require_value(arguments: &[String], index: &mut usize, option_name: &str, stderr: &StderrChannel) -> String {
    *index += 1;
    if *index >= arguments.len() {
        stderr.emit_error_and_exit(&format!("{option_name} requires a value"));
    }
    arguments[*index].clone()
}

/// Parse CLI arguments, resolve configuration, and obtain the final prompt.
///
/// Returns `(Config, prompt_text)`.
///
/// The prompt is sourced from, in order:
///
/// - trailing CLI words, if present
/// - interactive stdin prompt if stdin is a TTY
/// - piped stdin content otherwise
fn parse_args(stderr: &StderrChannel) -> (Config, String, StderrChannel) {
    let application_name = app_name();

    let xdg_config_app_dir = xdg_config_home().join(&application_name);
    let default_transcript_directory = xdg_data_home()
        .join(&application_name)
        .join("transcripts/default");
    let default_transcript_filename = format!("{}.md", today_as_date_string());

    // CLI state
    let mut cli_transcript_directory: Option<PathBuf> = None;
    let mut cli_transcript_filename:  Option<String>  = None;
    let mut cli_input_files:          Vec<PathBuf>    = vec![];
    let mut cli_env_file:             Option<PathBuf> = None;
    let mut cli_model:                Option<String>  = None;
    let mut cli_base_url:             Option<String>  = None;
    let mut cli_output_file:          Option<PathBuf> = None;
    let mut cli_parent_id:            Option<String>  = None;
    let mut cli_dry_run               = false;
    let mut cli_verbose               = false;
    let mut cli_no_transcript         = false;
    let mut cli_overwrite_output_file = false;
    let mut cli_output_control_tokens: Vec<String>   = vec![];
    let mut prompt_words:              Vec<String>    = vec![];

    let arguments: Vec<String> = env::args().skip(1).collect();
    let mut argument_index = 0;

    while argument_index < arguments.len() {
        let argument = &arguments[argument_index];

        // Helper: extract =VALUE from --flag=VALUE or return Err for separate arg form.
        let inline_value = |prefix: &str| -> Option<String> {
            argument.strip_prefix(prefix).map(|value| value.to_string())
        };

        match argument.as_str() {
            "--dry-run"              => cli_dry_run               = true,
            "--verbose"              => cli_verbose                = true,
            "--no-transcript"        => cli_no_transcript          = true,
            "--overwrite-output-file"=> cli_overwrite_output_file  = true,
            "--help" | "-h"          => { print_help(); std::process::exit(0); },
            "--version" | "-V"          => { print_version(); std::process::exit(0); },
            "--"                     => {
                prompt_words.extend_from_slice(&arguments[argument_index + 1..]);
                break;
            }

            "-m" | "--model" => {
                cli_model = Some(require_value(&arguments, &mut argument_index, argument, stderr));
            }
            "-b" | "--base-url" => {
                cli_base_url = Some(require_value(&arguments, &mut argument_index, argument, stderr));
            }
            "-i" | "--input-file" => {
                let value = require_value(&arguments, &mut argument_index, argument, stderr);
                cli_input_files.push(PathBuf::from(value));
            }
            "-O" | "--output-file" => {
                let value = require_value(&arguments, &mut argument_index, argument, stderr);
                cli_output_file = Some(PathBuf::from(value));
            }
            "-o" | "--output-control" => {
                let value = require_value(&arguments, &mut argument_index, argument, stderr);
                cli_output_control_tokens.push(value);
            }
            "--env-file" => {
                let value = require_value(&arguments, &mut argument_index, argument, stderr);
                cli_env_file = Some(PathBuf::from(value));
            }
            "--parent-id" => {
                cli_parent_id = Some(require_value(&arguments, &mut argument_index, argument, stderr));
            }
            "--transcript-dir" | "--output-dir" => {
                let value = require_value(&arguments, &mut argument_index, argument, stderr);
                cli_transcript_directory = Some(PathBuf::from(value));
            }
            "--transcript-file" => {
                cli_transcript_filename = Some(require_value(&arguments, &mut argument_index, argument, stderr));
            }

            // = forms
            _ if argument.starts_with("-m=")                  => { cli_model              = inline_value("-m="); }
            _ if argument.starts_with("--model=")             => { cli_model              = inline_value("--model="); }
            _ if argument.starts_with("-b=")                  => { cli_base_url           = inline_value("-b="); }
            _ if argument.starts_with("--base-url=")          => { cli_base_url           = inline_value("--base-url="); }
            _ if argument.starts_with("-O=")                  => { cli_output_file        = inline_value("-O=").map(PathBuf::from); }
            _ if argument.starts_with("--output-file=")       => { cli_output_file        = inline_value("--output-file=").map(PathBuf::from); }
            _ if argument.starts_with("-i=")                  => { cli_input_files.push(PathBuf::from(inline_value("-i=").unwrap())); }
            _ if argument.starts_with("--input-file=")        => { cli_input_files.push(PathBuf::from(inline_value("--input-file=").unwrap())); }
            _ if argument.starts_with("-o=")                  => { cli_output_control_tokens.push(inline_value("-o=").unwrap()); }
            _ if argument.starts_with("--output-control=")    => { cli_output_control_tokens.push(inline_value("--output-control=").unwrap()); }
            _ if argument.starts_with("--env-file=")          => { cli_env_file           = inline_value("--env-file=").map(PathBuf::from); }
            _ if argument.starts_with("--parent-id=")         => { cli_parent_id          = inline_value("--parent-id="); }
            _ if argument.starts_with("--transcript-dir=")    => { cli_transcript_directory = inline_value("--transcript-dir=").map(PathBuf::from); }
            _ if argument.starts_with("--output-dir=")        => { cli_transcript_directory = inline_value("--output-dir=").map(PathBuf::from); }
            _ if argument.starts_with("--transcript-file=")   => { cli_transcript_filename = inline_value("--transcript-file="); }

            _ if argument.starts_with("--") => {
                stderr.emit_error_and_exit(&format!("unknown option: {argument}"));
            }

            _ => prompt_words.push(argument.to_string()),
        }

        argument_index += 1;
    }

    // Explicit env file: TAI_ENV_FILE > --env-file
    let explicit_env_file: Option<PathBuf> = env::var("TAI_ENV_FILE")
        .map(PathBuf::from)
        .ok()
        .or(cli_env_file);

    // Load merged config map from all sources.
    let file_env_map = load_merged_config_map(
        &xdg_config_app_dir,
        explicit_env_file.as_deref(),
        stderr,
    );

    // Validate input files exist before making any API call.
    for input_file_path in &cli_input_files {
        if !input_file_path.exists() {
            stderr.emit_error_and_exit(
                &format!("input file not found: {}", input_file_path.display())
            );
        }
    }

    // Transcript directory
    let transcript_directory: PathBuf = cli_transcript_directory
        .or_else(|| env::var("TAI_TRANSCRIPT_DIR").ok().map(PathBuf::from))
        .or_else(|| env::var("TAI_OUTPUT_DIR").ok().map(PathBuf::from)) // legacy alias
        .or_else(|| file_env_map.get("TAI_TRANSCRIPT_DIR").map(PathBuf::from))
        .or_else(|| file_env_map.get("TAI_OUTPUT_DIR").map(PathBuf::from))
        .unwrap_or_else(|| default_transcript_directory.clone());

    if transcript_directory != default_transcript_directory && !transcript_directory.is_dir() {
        stderr.emit_error_and_exit(
            &format!("transcript directory does not exist: {}", transcript_directory.display())
        );
    }
    fs::create_dir_all(&default_transcript_directory)
        .expect("cannot create default transcript directory");

    let transcript_filename = resolve_value(
        cli_transcript_filename,
        "TAI_TRANSCRIPT_FILE",
        &file_env_map,
        || default_transcript_filename,
    );

    let model    = resolve_value(cli_model,    "TAI_MODEL",    &file_env_map, || DEFAULT_MODEL.into());
    let base_url = resolve_value(cli_base_url, "TAI_BASE_URL", &file_env_map, || DEFAULT_BASE_URL.into());
    let api_key  = resolve_optional_value(None, "OPENAI_API_KEY", &file_env_map);

    let no_transcript = resolve_bool_value(cli_no_transcript, "TAI_NO_TRANSCRIPT", &file_env_map, false);

    let output_file = cli_output_file
        .or_else(|| env::var("TAI_OUTPUT_FILE").ok().map(PathBuf::from))
        .or_else(|| file_env_map.get("TAI_OUTPUT_FILE").map(PathBuf::from));

    let overwrite_output_file = resolve_bool_value(
        cli_overwrite_output_file, "TAI_OVERWRITE_OUTPUT_FILE", &file_env_map, false,
    );

    let speaker_user_label = resolve_value(None, "TAI_SPEAKER_USER", &file_env_map, || "USER".into());
    let speaker_ai_label   = resolve_optional_value(None, "TAI_SPEAKER_AI", &file_env_map);

    // Build FormatOptions from per-channel defaults + env overrides.
    let mut stdout_format     = FormatOptions::from_defaults_and_env("STDOUT_",     &file_env_map, &STDOUT_DEFAULTS);
    let mut transcript_format = FormatOptions::from_defaults_and_env("TRANSCRIPT_", &file_env_map, &TRANSCRIPT_DEFAULTS);
    let mut artifact_format   = FormatOptions::from_defaults_and_env("ARTIFACT_",   &file_env_map, &ARTIFACT_DEFAULTS);
    let mut output_policy     = OutputPolicy::none();

    // Apply -o tokens, which override the env-resolved FormatOptions.
    for control_spec in &cli_output_control_tokens {
        for token in control_spec.split(',') {
            if let Err(token_error) = apply_output_control_token(
                token,
                &mut stdout_format,
                &mut transcript_format,
                &mut artifact_format,
                &mut output_policy,
            ) {
                stderr.emit_error_and_exit(&token_error);
            }
        }
    }

    let config = Config {
        model, base_url, api_key,
        input_files:            cli_input_files,
        no_transcript,
        transcript_directory,
        transcript_filename,
        output_file,
        overwrite_output_file,
        dry_run:                cli_dry_run,
        verbose:                cli_verbose,
        stdout_format,
        transcript_format,
        artifact_format,
        output_policy,
        speaker_user_label,
        speaker_ai_label,
        parent_id:              cli_parent_id,
    };

    // Prompt: CLI words joined, or stdin (interactive or piped).
    let prompt = if prompt_words.is_empty() {
        if io::stdin().is_terminal() {
            stderr.emit_interactive_prompt("Enter prompt: ");
            let mut interactive_input = String::new();
            io::stdin().read_line(&mut interactive_input).ok();
            let interactive_input = interactive_input.trim_end_matches('\n').to_string();
            if interactive_input.trim().is_empty() {
                stderr.emit_error_and_exit("no prompt provided.");
            }
            interactive_input
        } else {
            let mut piped_input = String::new();
            io::stdin().read_to_string(&mut piped_input).ok();
            if piped_input.trim().is_empty() {
                stderr.emit_error_and_exit("no prompt provided on stdin.");
            }
            piped_input
        }
    } else {
        prompt_words.join(" ")
    };

    let stderr = StderrChannel::from_env(&file_env_map);
    (config, prompt, stderr)
}

// ── Version and Help text ─────────────────────────────────────────────────────────────────

// Print application version  to `stdout`.
fn print_version() {
    write_stdout_safely(VERSION)
}

// Print CLI help text to `stdout`.
fn print_help() {
    write_stdout_safely(concat!(
        "Usage: terminal-ai [OPTIONS] [PROMPT]\n",
        "\n",
        "If PROMPT is omitted:\n",
        "  - stdin is a TTY → interactive prompt on stderr\n",
        "  - stdin is piped → stdin content becomes the prompt\n",
        "\n",
        "Options:\n",
        "  -h, --help                   Show this help\n",
        "      --dry-run                Skip API call, emit placeholder response\n",
        "      --verbose                Emit raw API JSON to stderr (transcript unaffected)\n",
        "\n",
        "  -m, --model MODEL            Model name (env: TAI_MODEL)\n",
        "  -b, --base-url URL           API base URL (env: TAI_BASE_URL)\n",
        "  -i, --input-file FILE        Attach file to prompt; repeatable\n",
        "      --env-file FILE          Env file path override (env: TAI_ENV_FILE)\n",
        "\n",
        "Transcript:\n",
        "      --transcript-dir PATH    Transcript directory (env: TAI_TRANSCRIPT_DIR)\n",
        "      --output-dir PATH        Alias for --transcript-dir\n",
        "      --transcript-file FILE   Transcript filename (env: TAI_TRANSCRIPT_FILE)\n",
        "      --no-transcript          Disable transcript (env: TAI_NO_TRANSCRIPT=1)\n",
        "\n",
        "Artifact (separate from transcript):\n",
        "  -O, --output-file FILE       Artifact output path (env: TAI_OUTPUT_FILE)\n",
        "      --overwrite-output-file  Truncate artifact before writing (env: TAI_OVERWRITE_OUTPUT_FILE=1)\n",
        "\n",
        "Threading:\n",
        "      --parent-id ID           Explicit parent message ID for branching\n",
        "\n",
        "Output control:\n",
        "  -o, --output-control SPEC    Token(s), comma-separated or repeated\n",
        "\n",
        "  Suppression tokens:\n",
        "    NO_{USER,AI}_{STDOUT,TRANSCRIPT,ARTIFACT}\n",
        "\n",
        "  Format preset tokens (append _{STDOUT,TRANSCRIPT,ARTIFACT} or omit for all):\n",
        "    CONTENT_ONLY   — content only, no header/metadata/sep\n",
        "    METADATA_ONLY  — metadata comment only\n",
        "    HEADER_ONLY    — header line only\n",
        "    FULL           — all flags on\n",
        "    SILENT         — disable channel entirely\n",
        "\n",
        "  Individual flag tokens:\n",
        "    {NO,ADD}_{HEADER,METADATA,PROMPT,INDENT,SEP,SPEAKER}[_{CHANNEL}]\n",
        "\n",
        "  Examples:\n",
        "    -o CONTENT_ONLY_STDOUT          stdout: content only\n",
        "    -o NO_HEADER_STDOUT             stdout: remove header\n",
        "    -o NO_USER_TRANSCRIPT           transcript: suppress user turns\n",
        "    -o FULL_TRANSCRIPT              transcript: all flags on\n",
        "    -o NO_METADATA                  all channels: no metadata\n",
        "    -o CONTENT_ONLY,NO_USER_STDOUT  combined\n",
        "\n",
        "Speaker labels:\n",
        "  TAI_SPEAKER_USER   user turn label (default: USER)\n",
        "  TAI_SPEAKER_AI     AI turn label; unset = model name from response\n",
        "\n",
        "stderr channel:\n",
        "  Carries metadata, errors, and interactive prompts. Never carries content.\n",
        "  TAI_STDERR_COLOR=1/0  — force color on/off (auto-detects TTY by default)\n",
        "  Metadata format: <!--{\"model\":\"...\",\"id\":\"...\",\"usage\":{...}}-->\n",
        "\n",
        "Per-channel format env vars (TAI_{CHANNEL}_{FLAG}, values: 1/0):\n",
        "  Channels: STDOUT, TRANSCRIPT, ARTIFACT\n",
        "  Flags:    ENABLE, HEADER, INDENT, SEP, METADATA, SPEAKER, PROMPT\n",
        "  Extra:    TAI_{CHANNEL}_INDENT_STR (indentation string, default: ' ')\n",
        "\n",
        "Config file load chain (higher wins):\n",
        "  $XDG_CONFIG_HOME/terminal-ai/.default.env  (legacy)\n",
        "  $XDG_CONFIG_HOME/terminal-ai/config.env   (primary)\n",
        "  --env-file / TAI_ENV_FILE                  (session override)\n",
        "\n",
        "MessageID format:\n",
        "  MessageID := <SpeechID>-<AudienceID>\n",
        "  SpeechID  := HASH8(speaker, timestamp)\n",
        "  AudienceID := parent SpeechID or HASH8(\"NULL\") for roots\n",
        "\n",
        "Runtime dependencies: curl, sha256sum, date\n",
    ));
}

// ── Entry ─────────────────────────────────────────────────────────────────────

/// Program entry point.
fn main() {
    // StderrChannel is constructed before config so that early errors are colored.
    // At this point the file map is not available yet, so we do a lightweight
    // pre-check of TAI_STDERR_COLOR from the process environment only.
    let early_stderr = StderrChannel {
        use_color: io::stderr().is_terminal(),
    };

    let (config, prompt, stderr) = parse_args(&early_stderr);

    // Re-construct with full file map now available inside config.
    // (config doesn't hold the file map, so we re-read the env var here.)

    converse(&prompt, &config, &stderr);
}
