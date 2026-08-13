---
name: "logging-errors"
description: "Error logging: mandatory error and error_source fields, chain preservation, context before the error. Load when logging a Result or an error value"
type: core
scope: "global"
---

# Error Logging Patterns

**MANDATORY for ALL error logging in this workspace**

An error log is only useful if it carries the whole failure — the message the caller saw and the chain of
causes beneath it — in the same shape in every crate, so a reader can follow it without knowing which crate
emitted it.

## The Mandatory Error Pattern

**ABSOLUTELY MANDATORY**: every log that includes a `std::error::Error` value carries both fields, exactly:

```rust
error = %err, error_source = logging::error_source(&err)
```

- `error = %err` is the Display form: the immediate error message.
- `error_source = logging::error_source(&err)` is the complete source chain, the Debug form of a
  `Vec<String>`, produced by the workspace's `logging::error_source()` utility.

**Field ordering**: `error` and `error_source` are the **last fields before the message string**. Context
fields (`console_addr`, `file_name`, and so on) come first, then the error fields, then the message.

```rust
// ✅ Good — context, error pair, message: one shape every reader and filter can parse
tracing::error!(
    console_addr = %console_addr,
    error = %err,
    error_source = logging::error_source(&err),
    "failed to send the file name"
);

// ❌ Bad — without error_source the chain stops at the outermost message, so "failed to send the file
// name" is all anyone ever sees; the refused connection underneath it is lost
tracing::error!(console_addr = %console_addr, error = %err, "failed to send the file name");

// ❌ Bad — Debug on the top-level error logs the struct literal instead of the message it was written to carry
tracing::error!(error = ?err, error_source = logging::error_source(&err), "transfer failed");

// ❌ Bad — hand-rolled source, and `unwrap()` panics on any error without one
tracing::error!(error = %err, source = %err.source().unwrap(), "transfer failed");

// ❌ Bad — renamed fields silently break every saved filter keyed on `error`/`error_source`
tracing::error!(err = %err, error_chain = logging::error_source(&err), "transfer failed");

// ❌ Bad — context after the error fields, so the message is no longer the last thing on the line and the
// field order stops being predictable across crates
tracing::error!(
    error = %err,
    error_source = logging::error_source(&err),
    console_addr = %console_addr,
    "transfer failed"
);
```

## Never Log an Error Without Context

Include the operational identifiers that say which unit of work failed, and a message that says what was
being attempted.

```rust
// ✅ Good — names the work, so the failure can be reproduced from the log line alone
tracing::error!(
    console_addr = %console_addr,
    file_name = %file_name,
    file_length = file_length,
    error = %err,
    error_source = logging::error_source(&err),
    "failed to send the compressed file data"
);

// ❌ Bad — no identifiers, and a message that fits every failure in the crate; nothing to act on
tracing::error!(error = %err, error_source = logging::error_source(&err), "operation failed");
tracing::error!(error = %err, error_source = logging::error_source(&err), "error");
```

## Error Variables Are Named `err`

Always bind error values as `err`, in match arms and closures alike. Never `e`.

```rust
// ✅ Good — one name for errors everywhere, so the pattern is greppable and reads unambiguously
let nro = std::fs::read(&nro_path).map_err(|err| {
    tracing::error!(
        file_name = %file_name,
        error = %err,
        error_source = logging::error_source(&err),
        "failed to read the nro"
    );
    Error::ReadNro(err)
})?;

// ❌ Bad — `e` collides with element and entry bindings in the same scope, and hides which value is the error
let nro = std::fs::read(&nro_path).map_err(|e| {
    tracing::error!(file_name = %file_name, error = %e, error_source = logging::error_source(&e), "failed to read the nro");
    Error::ReadNro(e)
})?;
```

## Error Chain Preservation

Always use the workspace's `logging::error_source()` utility. Never assemble the chain by hand from
`err.source()`, which reaches only the immediate cause.

```rust
// ✅ Good — walks the whole chain to the root cause
tracing::error!(
    console_addr = %console_addr,
    error = %err,
    error_source = logging::error_source(&err),
    "transfer failed"
);

// ❌ Bad — one level deep, so a connection refusal three layers down never reaches the log
tracing::error!(console_addr = %console_addr, error = %err, source = ?err.source(), "transfer failed");
```

Given a nested error:

```rust
#[derive(Debug, thiserror::Error)]
#[error("failed to deploy the nro")]
struct DeployNroError(#[source] SendNroError);

#[derive(Debug, thiserror::Error)]
#[error("failed to send the compressed file data")]
struct SendNroError(#[source] std::io::Error);
```

logging `error = %err, error_source = logging::error_source(&err)` emits:

```text
error = "failed to deploy the nro"
error_source = ["failed to send the compressed file data", "connection reset by peer"]
```

## Retry Logging

A retry line is still an error log: the mandatory pattern and the field ordering above apply to it unchanged,
with `retry_attempt` as one of the context fields ahead of the error pair. An attempt that will be retried
logs at `warn`; the one that exhausts the budget logs at `error`.

## Checklist

Before committing code with error logging, verify:

- [ ] All error logs include `error = %err` (Display format)
- [ ] All error logs include `error_source = logging::error_source(&err)`
- [ ] Error and error_source fields are the last fields before the message
- [ ] Context fields (`console_addr`, `file_name`, etc.) come before error fields
- [ ] No use of `error = ?err` (Debug format for top-level error)
- [ ] No hand-assembled source chain via `err.source()`
- [ ] Error variable named `err` (not `e`)
- [ ] Every error log carries operational context and a specific message
- [ ] The workspace `logging` module is in scope where errors are logged

## References

- [logging](logging.md) - Related: General structured logging patterns
