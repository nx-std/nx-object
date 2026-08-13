---
name: "logging"
description: "Structured logging with tracing: levels, field taxonomy, spans. Load when adding a log line or choosing a level"
type: core
scope: "global"
---

# Logging Patterns

**MANDATORY for ALL logging in this workspace**

A log line is a machine-parseable event, not a sentence: values go in fields, the message names what happened,
and the field taxonomy below is shared by every crate so one query works across all of them.

## Logger Configuration

Log levels come from two environment variables:

- **`WORKSPACE_LOG`** sets the baseline level for all workspace crates. Default `info`. Values: `error`,
  `warn`, `info`, `debug`, `trace`.
- **`RUST_LOG`** sets per-crate directives and overrides `WORKSPACE_LOG`. External dependencies default to
  `error` to reduce noise.

```bash
export WORKSPACE_LOG=info                          # baseline for every workspace crate
export RUST_LOG="nx_netloader=debug,tokio=warn"    # per-crate override, wins over WORKSPACE_LOG
```

Use `WORKSPACE_LOG=info` by default, `WORKSPACE_LOG=debug` for local development, and `RUST_LOG` for targeted
debugging of a module.

### User-Facing CLI Output

A command-line tool has **two distinct output channels**, and they MUST NOT be conflated:

- **Diagnostic logging** — `tracing`, gated by `RUST_LOG` / `WORKSPACE_LOG`. Opt-in output for debugging and
  observability. Governed by every rule in this document.
- **User-facing output** — the progress lines, status, warnings, and error reports a user runs the command to
  see. This is the program's **product**, written to stdout/stderr unconditionally.

User-facing output MUST go through a dedicated `ui` module that wraps `println!` / `eprintln!` behind a small,
Cargo-style API (e.g. `ui::status`, `ui::warning`, `ui::error`). Calling `println!` / `eprintln!` directly —
scattered across command code — remains **forbidden**: the `ui` module is the single sanctioned wrapper, so
output style stays consistent and testable. This mirrors Cargo's own split between its `Shell` abstraction and
its `tracing` / `log` diagnostics.

## Core Principles

### 1. Use `tracing` Crate Exclusively

Always use the fully qualified `tracing::<macro>!()` form, and `#[tracing::instrument]` for the attribute.
Never use `println!`, `eprintln!`, the `log` crate, or imported tracing macros.

```rust
// ✅ Good — fully qualified, so nothing in scope can shadow the macro or the attribute
tracing::info!(console_addr = %console_addr, "transfer started");

#[tracing::instrument(skip_all, fields(console_addr = %console_addr))]
pub async fn send_nro(console_addr: SocketAddr, nro: &[u8]) -> Result<(), SendNroError> {
    // ...
}

// ❌ Bad — an imported `info!`/`instrument` collides with any other macro of that name and hides which
// logging facade a module is on
use tracing::{info, instrument};

#[instrument(skip_all)]
pub async fn send_nro(console_addr: SocketAddr, nro: &[u8]) -> Result<(), SendNroError> {
    info!(console_addr = %console_addr, "transfer started");
}

// ❌ Bad — bypasses the subscriber: no level filter, no fields, no span context, and it is not the `ui`
// module either, so it is neither diagnostics nor product output
println!("Sending {} to console", file_name);
eprintln!("Error: {}", err);
log::info!("transfer started");
```

### 2. Structured Logging is Mandatory

Always use field-based structured logging. Never interpolate or format values into the message.

```rust
// ✅ Good — every value is a field that can be filtered, grouped, and aggregated
tracing::info!(
    console_addr = %console_addr,
    file_name = %file_name,
    duration_ms = elapsed.as_millis(),
    "transfer completed"
);

// ❌ Bad — the values are baked into a string, so nothing can select on console_addr or sum duration_ms
tracing::info!("Sent {} to {} in {}ms", file_name, console_addr, elapsed.as_millis());
tracing::info!(format!("Sent {}", file_name));
tracing::info!(file_name = %file_name, "Sent {}", file_name);
```

### 3. Line Length and Multiline Formatting

Split any call over 100 characters, or with 3 or more fields, across lines.

Formatting rules:

- Opening parenthesis on the macro line: `tracing::info!(`
- One field per line, indented 4 spaces
- **The message string is the last parameter**, after all fields
- Closing parenthesis and semicolon together: `);`
- Single-line form only for simple calls under 100 characters

```rust
// ✅ Good — multiline once the call passes 100 chars or 3 fields
tracing::error!(
    console_addr = %console_addr,
    error = %err,
    error_source = logging::error_source(&err),
    "failed to send the file name"
);

// ✅ Good — short and simple, so one line stays readable
tracing::info!(console_addr = %console_addr, "console discovered");

// ❌ Bad — runs past the margin, so the fields are unreadable in a diff or a review comment
tracing::error!(console_addr = %console_addr, error = %err, error_source = logging::error_source(&err), "failed to send the file name");
```

### 4. Consistent Log Levels

Choose the level from operational significance. See [Log Level Rules](#log-level-rules).

```rust
// ✅ Good — level matches significance
tracing::error!(error = %err, error_source = logging::error_source(&err), "transfer failed");
tracing::info!(console_addr = %console_addr, "console discovered");
tracing::debug!(attempt = attempt, "sending ping message");

// ❌ Bad — a routine event at error level trains a user to ignore the level that matters, and a real
// failure at info level is never seen
tracing::error!("console discovered");
tracing::info!(error = %err, "transfer failed");
tracing::debug!(console_addr = %console_addr, "console discovered");
```

### 5. Field Naming Conventions

Always use `snake_case`, and always use the same field name for the same thing across the whole codebase.

```rust
// ✅ Good — snake_case, and the names match the taxonomy below
tracing::info!(
    console_addr = %console_addr,
    file_name = %file_name,
    file_length = file_length,
    duration_ms = elapsed.as_millis(),
    "transfer completed"
);

// ❌ Bad — camelCase, abbreviations, and a name that drifts between call sites all break the one filter
// someone writes to follow a transfer through the log
tracing::info!(consoleAddr = %console_addr, fileName = %file_name, "transfer completed");
tracing::info!(addr = %console_addr, fname = %file_name, "sending");
tracing::info!(file = %file_name, "transfer started");
tracing::info!(file_name = %file_name, "transfer completed");
```

**Standard field names:**

| Resource         | Field Name      | Example                                                  |
| ---------------- | --------------- | -------------------------------------------------------- |
| Console address  | `console_addr`  | `console_addr = %console_addr`                           |
| Transferred file | `file_name`     | `file_name = %file_name`                                 |
| File length      | `file_length`   | `file_length = file_length`                              |
| Target triple    | `target`        | `target = %target`                                       |
| Output format    | `format`        | `format = %format`                                       |
| RomFS entry path | `entry_path`    | `entry_path = %entry_path`                               |
| Segment index    | `segment_index` | `segment_index = idx`                                    |
| Duration         | `duration_ms`   | `duration_ms = elapsed.as_millis()`                      |
| Retry attempt    | `retry_attempt` | `retry_attempt = 3`                                      |
| Error            | `error`         | `error = %err` (MANDATORY format)                        |
| Error Source     | `error_source`  | `error_source = logging::error_source(&err)` (MANDATORY) |

## Field Formatting

### 1. Display Formatting (`%`)

Use `%` for values with a human-readable `Display` representation: identifiers, names, enums, and the
top-level error message.

```rust
// ✅ Good — Display renders the value as a reader reads it
tracing::info!(console_addr = %console_addr, file_name = %file_name, format = %format, "bundle packed");

// ❌ Bad — Debug on a type that has Display logs `EntryPath("...")` instead of the path
tracing::info!(entry_path = ?entry_path, "asset added");
```

### 2. Debug Formatting (`?`)

Use `?` for complex types, collections, and error source chains. Primitive numeric types take no prefix.

```rust
// ✅ Good — `?` for structured values, no prefix for numbers
tracing::debug!(
    spec = ?npdm_spec,
    segments = ?segment_bounds,
    error_source = logging::error_source(&err), // returns DebugValue<Vec<String>>
    retry_attempt = 3,
    duration_ms = elapsed.as_millis(),
    entry_count = entries.len(),
    "image assembled"
);
```

### 3. Avoid Redundant Context

Set shared context once on the span. Do not repeat a field the enclosing span already carries.

```rust
// ✅ Good — console_addr enters every event through the span, so each line stays about what happened
#[tracing::instrument(skip_all, fields(console_addr = %console_addr))]
pub async fn send_nro(console_addr: SocketAddr, nro: &[u8]) -> Result<(), SendNroError> {
    tracing::info!("transfer started");

    match transfer(console_addr, nro).await {
        Ok(()) => tracing::info!("transfer completed"),
        Err(err) => tracing::error!(
            error = %err,
            error_source = logging::error_source(&err),
            "transfer failed"
        ),
    }

    Ok(())
}

// ❌ Bad — console_addr is duplicated on every event under the span, doubling the payload and inviting
// the two copies to disagree
#[tracing::instrument(skip_all, fields(console_addr = %console_addr))]
pub async fn send_nro(console_addr: SocketAddr, nro: &[u8]) -> Result<(), SendNroError> {
    tracing::info!(console_addr = %console_addr, "transfer started");
    // ...
}
```

## Log Level Rules

### 1. Error Level

`tracing::error!` is for unrecoverable failures, corrupted output, and anything that leaves the user without
the artifact they asked for: an image that failed to assemble, a transfer abandoned mid-write, an unexpected
error the run cannot continue past.

```rust
// ✅ Good — a failure the user must act on
tracing::error!(
    console_addr = %console_addr,
    error = %err,
    error_source = logging::error_source(&err),
    "transfer abandoned mid-write"
);

// ❌ Bad — routine events at error level train a reader to ignore the level that matters
tracing::error!("netloader client started");
tracing::error!(retry_attempt = 1, "retrying discovery");
```

### 2. Warn Level

`tracing::warn!` is for recoverable failures and degraded operation: performance degradation, resource limits
approaching, deprecated functionality in use.

A retry that is still within its budget is a `warn`: it reports degraded-but-self-correcting behavior, and it
carries `retry_attempt` so the trend is visible. The attempt that exhausts the budget is the error.

```rust
// ✅ Good — degraded but self-correcting, and the fields show the trend
tracing::warn!(retry_attempt = attempt, "discovery ping went unanswered");
tracing::warn!(
    file_length = file_length,
    duration_ms = elapsed.as_millis(),
    "transfer slower than expected"
);

// ❌ Bad — a success and a routine step at warn level bury the degradations worth reading
tracing::warn!("transfer completed");
tracing::warn!("starting romfs scan");
```

### 3. Info Level

`tracing::info!` is for important state changes and lifecycle events: a console discovered, a transfer
started and completed, a bundle written, a build finished.

```rust
// ✅ Good — a lifecycle event worth one line at the default level
tracing::info!(
    console_addr = %console_addr,
    file_name = %file_name,
    duration_ms = elapsed.as_millis(),
    "transfer completed"
);

// ❌ Bad — per-chunk and per-step detail at info level floods the default stream
tracing::info!(chunk_len = 0x4000, "sending chunk");
tracing::info!("checking output directory");
```

### 4. Debug Level

`tracing::debug!` is for detailed execution flow and diagnostics: each discovery attempt, intermediate
packaging steps, expected error conditions during normal operation, buffer sizing.

```rust
// ✅ Good — diagnostic detail, off by default
tracing::debug!(attempt = attempt, "sending ping message");
tracing::debug!(
    entry_count = entries.len(),
    segments = ?segment_bounds,
    "assembling image"
);

// ❌ Bad — a lifecycle event and a real failure disappear when debug is filtered out
tracing::debug!(console_addr = %console_addr, "console discovered");
tracing::debug!(error = %err, error_source = logging::error_source(&err), "transfer failed");
```

### 5. Trace Level

`tracing::trace!` is for extremely verbose debugging, disabled in production: function entry and exit where
`#[tracing::instrument]` is not used, per-iteration logging, low-level protocol and allocation detail.

```rust
// ✅ Good — per-chunk detail nobody runs by default
tracing::trace!(chunk_index = i, chunk_len = chunk.len(), "compressed chunk sent");
tracing::trace!(buffer_size = buffer.len(), capacity = buffer.capacity(), "buffer allocated");

// ❌ Bad — anything a user needs is invisible at the level nobody enables
tracing::trace!(console_addr = %console_addr, "transfer completed");
tracing::trace!(error = %err, error_source = logging::error_source(&err), "transfer failed");
```

## Message Formatting

### 1. Descriptive Messages, Not snake_case

Messages are human-readable phrases. Data belongs in fields, never interpolated into the message.

```rust
// ✅ Good — a phrase a reader understands, with the data beside it
tracing::info!(console_addr = %console_addr, "console discovered");
tracing::info!(file_name = %file_name, console_addr = %console_addr, "transfer started");

// ❌ Bad — an identifier-shaped message reads as a code symbol, and interpolated data cannot be filtered
tracing::info!(console_addr = %console_addr, "console_discovered");
tracing::info!("console {} discovered", console_addr);
```

### 2. Brief and Clear

Messages are concise. Avoid full sentences and narration.

```rust
// ✅ Good — says what happened in as few words as carry it
tracing::info!(file_name = %file_name, file_length = len, "transfer completed");

// ❌ Bad — a sentence costs bytes on every event and says nothing the fields do not
tracing::info!(file_name = %file_name, "The file transfer has now completed");
```

### 3. Action-Oriented Past Tense

Use past-tense verbs describing what happened. No present progressive, no editorial.

```rust
// ✅ Good — the event already happened when the line is written
tracing::info!(console_addr = %console_addr, "console discovered");
tracing::info!(file_name = %file_name, "nro deployed");
tracing::warn!(retry_attempt = attempt, "discovery retry scheduled");

// ❌ Bad — progressive tense claims work that may never finish, and editorial adds no signal
tracing::info!(console_addr = %console_addr, "discovering console");
tracing::info!(file_name = %file_name, "successfully deployed nro");
tracing::error!(error = %err, error_source = logging::error_source(&err), "oh no connection problem");
```

### 4. No Punctuation

Never end a message with a period, exclamation mark, or question mark.

```rust
// ✅ Good — no trailing punctuation
tracing::info!(console_addr = %console_addr, "console discovered");

// ❌ Bad — punctuation varies per author and breaks grouping on the message string
tracing::info!(console_addr = %console_addr, "console discovered.");
tracing::error!(error = %err, error_source = logging::error_source(&err), "transfer failed!");
tracing::warn!(retry_attempt = 3, "retrying discovery?");
```

## Checklist

Before committing code with logging, verify:

- [ ] All logging uses fully qualified `tracing::<macro>!()` form
- [ ] Instrument attribute uses fully qualified `#[tracing::instrument]` form
- [ ] No use of `println!`, `eprintln!`, or `log` crate; user-facing output goes through the `ui` module
- [ ] All logs use structured field-based logging, with no string interpolation in messages
- [ ] Appropriate log level used (error/warn/info/debug/trace)
- [ ] Multiline format used for calls exceeding 100 characters or with 3+ fields
- [ ] Message string is the last parameter (after all fields)
- [ ] Display formatting (`%`) used for human-readable values
- [ ] Debug formatting (`?`) used for complex types and collections
- [ ] No prefix for primitive numeric types
- [ ] `logging::error_source()` returns `DebugValue<Vec<String>>`
- [ ] All field names use `snake_case`
- [ ] Consistent field names used (e.g., `console_addr`, not `addr` or `console`)
- [ ] Standard field names followed (see the taxonomy table)
- [ ] No abbreviated field names
- [ ] Messages are descriptive and human-readable (not snake_case)
- [ ] Data is in fields, not interpolated in messages
- [ ] Messages are brief and action-oriented, in past tense
- [ ] No punctuation, editorial comments, or vague descriptions in messages
- [ ] Relevant context included in all error logs
- [ ] `#[tracing::instrument]` used for important functions
- [ ] Redundant context avoided in nested spans
- [ ] Resource identifiers included where relevant

## References

- [logging-errors](logging-errors.md) - Related: Error-specific logging patterns
