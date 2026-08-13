---
name: "rust-async-tokio"
description: "Tokio runtime: blocking pool for CPU work, no std lock across an await, abort-on-drop, cancellation safety, deadlines. Load when writing async code or using select!"
type: "core"
scope: "global"
---

# Tokio Runtime Rules

**MANDATORY for ALL Rust code in the workspace**

The shape of concurrency — who owns a task, what waits for it — is owned by [rust-async](rust-async.md). This
document is about the runtime underneath it: what blocks a worker thread, what gets cancelled and when, and
what the executor cannot rescue you from. The rules share one root cause. A tokio worker thread runs many
tasks by moving between them at `.await` points, so **anything that occupies a worker without awaiting starves
every other task on that thread** — and with a work-stealing runtime and a bounded worker count, a handful of
such tasks stalls the process.

## 1. CPU-Bound Work Goes to the Blocking Pool

Reading stays async; the CPU-bound step is a **synchronous function** run through `spawn_blocking`. Deflate
and BLZ compression, RomFS image assembly, SHA-256 build IDs, large serde, and anything that runs for
milliseconds without awaiting all qualify.

It is not permission to call `block_on` inside an async context — that parks a worker waiting on the runtime it
is part of, and deadlocks it outright when the pool is saturated — nor an invitation to make the CPU step
`async`: an `async fn` that never awaits is a synchronous function wearing a future, and blocks exactly as hard.

```rust
// ❌ Bad — the compression runs on a worker thread. Every other task scheduled
// there stops for the duration, so a chunk that takes 40ms to compress adds 40ms
// of latency to the stdio forwarding sharing that worker.
pub async fn send_chunk(sock: &mut TcpStream, chunk: Vec<u8>) -> Result<(), SendNroError> {
    let compressed = deflate(&chunk);
    write_chunk(sock, &compressed).await
}

// ✅ Good — the socket work stays async, the compression moves to the blocking
// pool, and the sync fn is directly unit-testable because it is not a future at all.
pub async fn send_chunk(sock: &mut TcpStream, chunk: Vec<u8>) -> Result<(), SendNroError> {
    let compressed = tokio::task::spawn_blocking(move || deflate(&chunk))
        .await
        .map_err(SendNroError::CompressPanicked)?;
    write_chunk(sock, &compressed).await
}
```

A `spawn_blocking` task **cannot be aborted once it has started running**: the runtime has no way to interrupt
a synchronous call. Bound the work itself rather than expecting cancellation to save you.

## 2. Never Hold a `std` Lock Across an `await`

A `std::sync::MutexGuard` held across an `.await` blocks the worker thread for as long as the future is
pending, and deadlocks if the task that would release it is scheduled on the same thread. It is also not
`Send`, so the compiler rejects it in a spawned task — which means the version that compiles is the one in a
non-spawned future, where the failure is silent.

The fix is almost always to shorten the critical section rather than to change the lock type: take what you
need, drop the guard, then await.

```rust
// ❌ Bad — the guard is alive across the await, so the worker is held for the
// length of a network call, and any task needing this lock waits behind it.
let discovered = self.discovered.lock().unwrap();
let console = discovered.get(&name).copied();
let client = NetloaderClient::connect(console).await?;

// ✅ Good — lock, extract, drop, then await.
let console = {
    let discovered = self.discovered.lock().unwrap();
    discovered.get(&name).copied()
};
let client = NetloaderClient::connect(console).await?;
```

Reach for `tokio::sync::Mutex` only when the critical section genuinely must span an await — and treat that as
a design question first, because it usually means the lock is protecting an operation rather than data.

## 3. Handles Abort on Drop

Dropping a `JoinHandle` **detaches** the task; it does not stop it. Any handle a type owns is therefore wrapped
so that dropping the owner stops the work:

```rust
// ✅ Good — the wrapper is applied at the spawn, so the handle cannot be stored bare.
let compressor = AbortOnDropHandle::new(tokio::spawn(async move { compress_chunks(nro, tx).await }));
```

This is the mechanical half of the ownership rule in [rust-async](rust-async.md). `JoinSet` does the same for a
group (dropping the set aborts every task in it), so a set needs no extra wrapper.

## 4. `select!` Cancels the Losers

`tokio::select!` drops every branch that did not complete. That is the point, and it is also the trap: a future
dropped mid-poll loses whatever it had buffered, and starts over if it is recreated on the next iteration.

In a loop, every branch must be **cancellation-safe** — safe to drop mid-poll and re-create without losing
data. `JoinSet::join_next`, `mpsc::Receiver::recv`, and `tokio::time::sleep` are, so a branch written as
`Some(chunk) = chunks.recv()` costs nothing when it loses the race; a hand-written socket read that has
consumed half a length-prefixed frame into a local buffer is not. Where a branch is not cancel-safe, hold the future outside the loop and
poll the same one each iteration rather than constructing it in the `select!`.

```rust
// ❌ Bad — the read future is rebuilt each iteration, so every time the shutdown
// branch loses the race the partially-read frame is discarded. The console's
// stdout then silently loses lines, and the loop looks correct.
loop {
    tokio::select! {
        result = stdio.read_frame() => print(result?),
        _ = shutdown.recv() => break,
    }
}
```

## 5. Every Remote Call Has a Deadline

A call to something outside the process — the console's TCP socket, a discovery broadcast, a spawned `cargo`
— is wrapped in `tokio::time::timeout`. A console that accepts a connection and then goes quiet otherwise
holds a task and a socket indefinitely; the symptom is a deploy that never returns and prints nothing. The
timeout names what it bounds, so the error says which call gave up rather than that something did:

```rust
// ✅ Good — a named deadline, and a typed error that says what expired.
let ack = tokio::time::timeout(ACK_LIMIT, read_ack(&mut sock))
    .await
    .map_err(|_| SendNroError::AckTimeout { limit: ACK_LIMIT })??;
```

A deadline is the outer bound on a retry budget, not a replacement for one: the discovery loop bounds each
broadcast with a timeout *and* caps the number of attempts.

## Checklist

Before committing code, verify:

- [ ] CPU-bound work runs in `spawn_blocking` as a synchronous function, not inline in a task
- [ ] No `block_on` inside an async context
- [ ] No `std::sync` guard is alive across an `.await`; the critical section ends before the await
- [ ] `tokio::sync::Mutex` is used only where the critical section must span an await, and the reason holds
- [ ] Every owned `JoinHandle` aborts on drop, or lives in a `JoinSet`
- [ ] Every `select!` branch in a loop is cancellation-safe, or the future is held across iterations
- [ ] Every call leaving the process is wrapped in a named `timeout`

## References

- [rust-async](rust-async.md) - Extends: The ownership and scope rules these runtime mechanics implement
- [principle-single-responsibility](principle-single-responsibility.md) - Foundation: Separating the sync
  CPU step from the async shell is what makes the pure part testable
- [logging](logging.md) - Related: What a timeout or a starved worker should report
