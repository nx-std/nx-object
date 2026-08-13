---
name: "rust-async"
description: "Structured concurrency: every task has an owner, and buffered streams are not concurrency. Load when spawning a task, fanning out work, or designing shutdown"
type: "core"
scope: "global"
---

# Structured Concurrency

**MANDATORY for ALL Rust code in the workspace**

**Every task has an owner, and no task outlives the scope that started it.** Concurrency is expressed by
scopes that wait for their children, not by launching work that continues after the launching function
returns.

The runtime mechanics — the blocking pool, lock discipline, cancellation safety — are owned by
[rust-async-tokio](rust-async-tokio.md). This document is about the shape of the concurrency itself.

## 1. Spawn Is a `goto`

A bare `tokio::spawn` severs the relationship between a caller and the work it caused, which is what makes it
the concurrency equivalent of `goto`. The consequence is a rule about **reading code**: if any function may
spawn, then no function's return tells you it is done. `let sent = send_nro(addr, nro).await?;` looks complete,
and it is a lie if `send_nro` spawned a stdio forwarder on the way out — a doubt every caller inherits, that no
signature records.

Three things are lost, and each has to be rebuilt by hand once it is gone:

- **Failure has no path back.** A panicking or erroring detached task disappears into a `JoinHandle` nobody
  holds, so failures are recovered with out-of-band machinery — a channel, a shared flag, a log line someone
  greps later.
- **Cancellation has no path down.** Dropping the future that started the work does not stop the work.
- **Shutdown has nothing to wait for.** The process exits with the task mid-write, or hangs because nobody
  can tell whether it is finished.

## 2. Concurrency Is a Scope, Not a Statement

Fan-out is expressed with a construct that **owns** its children and cannot be left before they finish:

| Shape                                    | Use                                    |
|------------------------------------------|----------------------------------------|
| A fixed set of differently-typed futures | `tokio::try_join!` / `futures::join!`  |
| A dynamic set of same-typed tasks        | `tokio::task::JoinSet`                 |
| A pipeline stage feeding another         | A spawned producer and a bounded channel ([§3](#3-a-single-task-is-owned-by-a-handle)) |
| First one wins, rest are cancelled       | `tokio::select!`                       |

`StreamExt::buffered`, `buffer_unordered`, and `for_each_concurrent` are **not** on that list: they look like
bounded concurrency and behave differently, in a way that stalls a transfer without anything in the types
saying so ([§5](#5-buffer-data-not-code)).

`JoinSet` is the workhorse. It owns every task it spawned, yields their results as they complete, and **aborts
all of them when dropped** — so an early return or a `?` in the middle of the loop cannot leave orphans behind.

```rust
// ❌ Bad — three orphans. If `report` fails, the transfers keep running against
// consoles nobody is watching; if the caller is cancelled, they keep running
// against nothing. The `?` below the loop is what makes this unsafe, and nothing
// about the spawn says so.
for console in consoles {
    let nro = nro.clone();
    tokio::spawn(async move { send_nro(console, &nro).await });
}
report(&consoles).await?;

// ✅ Good — the set owns the transfers. Every path out of this function, including
// the `?`, drops the set and aborts whatever is still running; and the results
// are ordinary values, so a failed transfer is a returned error.
pub async fn deploy_all(nro: Arc<[u8]>, consoles: Vec<SocketAddr>) -> Result<(), DeployError> {
    let mut transfers = JoinSet::new();
    for console in &consoles {
        let (nro, console) = (Arc::clone(&nro), *console);
        transfers.spawn(async move { send_nro(console, &nro).await });
    }
    while let Some(joined) = transfers.join_next().await {
        joined.map_err(DeployError::TransferPanicked)??;
    }
    report(&consoles).await?;
    Ok(())
}
```

## 3. A Single Task Is Owned by a Handle

One long-lived background task — a read-ahead producer, a flusher, a heartbeat — is owned by the struct that
needs it, through a handle that **aborts on drop**. Owning a bare `JoinHandle` is not enough: dropping a
`JoinHandle` detaches the task rather than stopping it, which is the default this rule exists to prevent.

```rust
// ❌ Bad — the handle is dropped at the end of `new`, so the compressor is detached.
// Dropping the transfer leaves it filling a channel nobody reads, holding the NRO
// open until the process exits.
pub fn new(nro: File) -> Self {
    let (tx, rx) = mpsc::channel(READ_AHEAD);
    tokio::spawn(async move { compress_chunks(nro, tx).await });
    Self { rx }
}

// ✅ Good — the transfer owns the compressor. Dropping the transfer aborts it, so
// the task's lifetime is exactly the lifetime of the thing that needs it.
pub fn new(nro: File) -> Self {
    let (tx, rx) = mpsc::channel(READ_AHEAD);
    let compressor =
        AbortOnDropHandle::new(tokio::spawn(async move { compress_chunks(nro, tx).await }));
    Self { rx, compressor }
}
```

## 4. Errors Return; They Do Not Get Announced

Because a scope joins its children before returning, a child's failure is an ordinary `Result` the parent
returns, the same as in sync code. Detached work cannot do this, so it grows a second, parallel error path.

```rust
// ❌ Bad — a channel exists only to carry errors back from tasks that should
// have been able to return them. Every caller now has two failure modes to
// handle, and the one on the channel arrives after the function has returned Ok.
let (errors_tx, mut errors_rx) = mpsc::channel(16);
for console in consoles {
    let errors_tx = errors_tx.clone();
    tokio::spawn(async move {
        if let Err(err) = send_nro(console, &nro).await {
            let _ = errors_tx.send(err).await;
        }
    });
}
```

The `JoinSet` loop in [§2](#2-concurrency-is-a-scope-not-a-statement) is the fix: one failure path, and it is
the function's return type.

## 5. Buffer Data, Not Code

`buffered(n)` and `for_each_concurrent(n)` read as "run up to `n` of these at once". What they do is hold `n`
futures inside **one** task and advance them only while that task polls the stream: the concurrency is
scheduled by the consumer's loop, not by the runtime. Two consequences follow, neither visible at the call:

- **Nothing progresses while the consumer works.** The moment the loop body does anything substantial —
  encoding, writing, awaiting a full channel — every buffered future stops. Throughput collapses into a
  sawtooth: fetch a batch, stall the fetches while writing it, resume. A bigger buffer never fixes it, because
  the buffer was never the bottleneck.
- **It can deadlock.** The stream waits for buffer space while a buffered future waits for something the
  consumer holds — a lock, channel capacity, a permit. Control of execution has become a shared resource that
  nothing in the types mentions.

The rule is **buffer data, not code**: give the concurrent work its own task, and connect the stages with a
bounded channel. The producer is then scheduled by the runtime, keeps running while the consumer writes, and
the only shared resource is the channel — visible, typed, with an obvious capacity.

```rust
// ❌ Bad — the asset reads only advance while this loop is polling. Every write
// into the image stalls all eight of them, so the disk sits idle for the duration
// of the encode and the bundle runs at a fraction of its apparent concurrency.
let mut assets = stream::iter(paths).map(|path| read_asset(path)).buffered(8);
while let Some(asset) = assets.try_next().await? {
    image.write(encode(asset)).await?;
}

// ✅ Good — the reader is its own task, so asset reads continue during the write.
// The channel bounds memory instead of the buffer bounding progress, and the
// handle ties the reader's lifetime to this function.
let (tx, mut rx) = mpsc::channel(8);
let reader = AbortOnDropHandle::new(tokio::spawn(async move {
    let mut assets = stream::iter(paths).map(|path| read_asset(path)).buffered(8);
    while let Some(asset) = assets.try_next().await? {
        if tx.send(asset).await.is_err() {
            break;
        }
    }
    Ok::<_, ReadAssetError>(())
}));

while let Some(asset) = rx.recv().await {
    image.write(encode(asset)).await?;
}
reader.await.map_err(BundleError::ReaderPanicked)??;
```

`buffered` still has a place **inside** such a producer, where the task does nothing but drive the stream. What
is forbidden is a `buffered` stream whose consumer does the real work in the same task.

## 6. The Narrow Case for a Detached Task

A task may be detached when its lifetime genuinely **is** the process's: a signal handler, a metrics exporter,
a supervisor started at boot. Two conditions apply, and both are checked in review:

1. It is started at the composition root — the binary's `main` or its runtime builder — never deep in a
   library call where a caller cannot see it.
2. Its docs say it is detached and why nothing waits for it.

Anything else is an orphan. A task that must outlive its starting function but not the process moves ownership
up: return the `JoinSet` or the handle to a caller that lives long enough, rather than detaching and hoping.

## Checklist

Before committing code, verify:

- [ ] No bare `tokio::spawn` whose `JoinHandle` is dropped
- [ ] Fan-out uses `JoinSet`, `try_join!`, or a buffered stream inside a dedicated producer task — a construct
      that owns its children
- [ ] Every early return and `?` in a fan-out path leaves no task running
- [ ] A single background task is owned by an abort-on-drop handle held by the type that needs it
- [ ] Task failures are returned as `Result`, not delivered over a side channel
- [ ] No `buffered`/`for_each_concurrent` stream whose consumer does substantial work in the same task;
      pipeline stages are separate tasks joined by a bounded channel
- [ ] Any detached task is started at the composition root and documents why nothing waits for it
- [ ] No function returns while work it started is still running, unless its docs say so

## References

- [rust-async-tokio](rust-async-tokio.md) - Extends: The runtime rules these shapes rely on — the blocking
  pool, lock discipline, and cancellation safety
- [principle-idempotency](principle-idempotency.md) - Foundation: Teardown that is safe to call twice, and why
  shutdown must be able to wait
- [rust-errors-handling](rust-errors-handling.md) - Related: Propagation rules a joined task's error follows
- [principle-single-responsibility](principle-single-responsibility.md) - Foundation: A task's owner is the
  type whose lifetime it shares

## External References

- [Notes on structured concurrency, or: Go statement considered harmful — Nathaniel J. Smith](https://vorpus.org/blog/notes-on-structured-concurrency-or-go-statement-considered-harmful/)
- [Futures Unordered — withoutboats](https://without.boats/blog/futures-unordered/) — why buffered streams
  hide sequencing points, and the "buffer data, not code" rule
- [`tokio::task::JoinSet`](https://docs.rs/tokio/latest/tokio/task/struct.JoinSet.html)
- [RFC: structured concurrency — tokio#2596](https://github.com/tokio-rs/tokio/issues/2596)
- [`async_nursery` — a structured-concurrency primitive for Rust](https://github.com/najamelan/async_nursery)
