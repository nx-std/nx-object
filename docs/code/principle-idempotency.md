---
name: "principle-idempotency"
description: "Idempotency — make operations safe to re-run and retry. Load when writing build or bundle commands that produce artifacts, discovery and transfer retry logic, resource startup/teardown, or anything a user re-invokes"
type: "principle"
scope: "global"
---

# Idempotency (Safe Retries and Replays)

**MANDATORY for ALL code in the workspace**

## Rule

Design state-altering operations so that running them twice has the same observable effect as running them
once. Whether explicit machinery is needed depends on who calls the operation:

1. **CLI command re-invocation** (`cargo nx build`, `cargo nx bundle`) → required. A user re-runs a command
   after every edit, after a `Ctrl-C`, and after a failed deploy; the same inputs _will_ be packed again. The
   same sources must produce the same artifact, and a command interrupted halfway must not leave a half-written
   output where the next step will pick it up.
2. **Network retries** (discovery broadcast, chunk transfer) → required. Discovery re-broadcasts on a timeout
   and UDP delivers duplicates, so the same console answers more than once; a transfer re-attempted after a
   dropped connection must not leave a partial NRO on the console.
3. **Resource acquisition and release** (`connect`, `spawn`, `shutdown`, `close`) → required. Two concurrent
   callers must not open two connections to one console; a double `shutdown()` must not error.
4. **Pure in-process functions** → nothing to do. `plan_layout(entries)` is idempotent by construction: same
   input, same output, no effects.

If re-running an operation produces a truncated file, a second connection, a duplicate console entry, or a
different artifact, it is not idempotent. Key results on a natural identity, write outputs atomically, share
the in-flight future, and guard "already done" state.

## Examples

1. **Write the artifact atomically, and identify it by its content**
   A build packs an image and writes it where the deploy step will read it. An interruption between the two is
   the ordinary case, not the rare one.

```rust
// ❌ Bad — `create` truncates the previous good artifact before a single byte of the new
// one exists. Interrupted mid-write, the output path holds a truncated NRO that still
// looks like a file, and the deploy step happily uploads it. The build id is recorded in
// a second file, so a re-run that reads the stamp first sees an id the bytes no longer match.
fn write_artifact(out: &Path, nro: &[u8], stamp: &Path, build_id: BuildId)
    -> Result<(), WriteArtifactError>
{
    let mut file = std::fs::File::create(out)?;
    file.write_all(nro)?;
    std::fs::write(stamp, build_id.to_string())?;
    Ok(())
}
```

```rust
// ✅ Good — the image is packed in full in memory, written to a temp file beside the
// destination, and renamed into place, so the output path only ever holds a complete
// artifact. The build id is derived from the bytes rather than recorded beside them, so
// there is no second fact to keep in sync and a re-run is an exact no-op.
fn write_artifact(out: &Path, nro: &[u8]) -> Result<BuildId, WriteArtifactError> {
    let tmp = out.with_extension("nro.tmp");
    let mut file = std::fs::File::create(&tmp)?;
    file.write_all(nro)?;
    file.sync_all()?;
    std::fs::rename(&tmp, out)?;
    Ok(BuildId::from_bytes(nro))
}
```

The identity must come from the inputs, not from the invocation: the same sources pack to the same bytes,
whichever run produced them.

2. **Key retried results on a natural identity**
   Discovery broadcasts a ping and collects pongs, re-broadcasting until the timeout expires.

```rust
// ❌ Bad — every pong is pushed. A retry that fires before the first reply lands, or a
// duplicated datagram, records the same console twice; the command then reports two
// consoles, prompts the user to choose between an address and itself, and a caller
// counting the results concludes the network has more devices on it than it does.
pub async fn discover(sock: &UdpSocket, retries: u32, timeout: Duration)
    -> io::Result<Vec<ConsoleAddr>>
{
    let mut found = Vec::new();
    for _ in 0..retries {
        sock.send_to(NXBOOT_PING, BROADCAST_ADDR).await?;
        while let Ok(Ok((len, addr))) = time::timeout(timeout, sock.recv_from(&mut buf)).await {
            if &buf[..len] == BOOTNX_PONG {
                found.push(ConsoleAddr::from(addr));
            }
        }
    }
    Ok(found)
}
```

```rust
// ✅ Good — the reply's own address is the key, so a duplicate pong and an extra retry
// are both no-ops. Broadcasting one more time can only ever confirm what is already known.
pub async fn discover(sock: &UdpSocket, retries: u32, timeout: Duration)
    -> io::Result<BTreeSet<ConsoleAddr>>
{
    let mut found = BTreeSet::new();
    for _ in 0..retries {
        sock.send_to(NXBOOT_PING, BROADCAST_ADDR).await?;
        while let Ok(Ok((len, addr))) = time::timeout(timeout, sock.recv_from(&mut buf)).await {
            if &buf[..len] == BOOTNX_PONG {
                found.insert(ConsoleAddr::from(addr));
            }
        }
    }
    Ok(found)
}
```

The same shape governs acquisition and teardown. A client that lazily connects shares one in-flight future, so
N concurrent callers await one `connect` instead of opening N sockets to one console, and a failed connect
leaves nothing cached so the next call retries rather than replaying the failure. Teardown runs from a
cancellation, from a signal handler, and from the failure path of startup: `shutdown()` takes its state
(`std::mem::take`, `Option::take`) before awaiting, so the second call has nothing left to do instead of
awaiting an already-joined stdio forwarder and returning a `JoinError` that masks the original failure.
Wherever a resource guards "already done" state the rule is the same: aborting a transfer that already
finished is a no-op, and a detached forwarder drops the bytes instead of erroring.

## Why It Matters

Every path a user touches here is retried by hand. The edit-build-deploy loop is a command re-run dozens of
times an hour, often after an interrupt, so a build step that truncates its output before producing the
replacement leaves the next deploy uploading a file that was never finished — and the failure surfaces on the
console, as a title that will not launch, rather than in the build that caused it.

The same logic governs anything the network can deliver twice. Discovery is at-least-once by construction: the
broadcast is repeated on a timeout and the transport duplicates freely, so results that are appended rather
than keyed count one console as several. And a transfer re-attempted after a dropped connection has to be able
to overwrite what the previous attempt left, or the console keeps a partial NRO that the loader reads as a
valid one.

## Pragmatism Caveat

Some operations genuinely cannot be idempotent: scaffolding a project into a directory twice is not the same
as doing it once. The response is not machinery but honesty — `cargo nx new` refuses an existing, non-empty
directory rather than silently re-scaffolding over it, and says so in its `--help` and its docs. An
undocumented non-idempotent re-run path is always wrong.

Equally, do not bolt identity keys onto pure functions or onto in-process calls where the caller controls
execution. A layout planner, a formatter, and an encoder need nothing.

## Checklist

Before committing code, verify:

- [ ] Every artifact is written to a temp path and renamed into place; nothing is truncated before its
      replacement exists in full
- [ ] Outputs are identified by their content, not by the run that produced them; the same inputs pack to the
      same bytes
- [ ] Results collected from a retried network operation key on a natural identity, so a duplicate reply is a
      no-op
- [ ] An interrupted transfer leaves the console with no file rather than a partial one
- [ ] Lazily-created resources share one in-flight future, and a failed creation leaves nothing cached
- [ ] `shutdown()`/`close()` take their state before awaiting, and are safe to call twice
- [ ] Cleanup on a failure path joins every spawned task so one hung forwarder cannot block the rest
- [ ] Deliberately non-idempotent commands say so in their help text and their docs

## References

- [principle-validate-at-edge](principle-validate-at-edge.md) - Related: The boundary that accepts an input is
  where the identity of the work it causes is established
- [principle-least-surprise](principle-least-surprise.md) - Related: `connect`/`shutdown`, `start`/`stop` carry
  an implied "safe to call again" contract
- [principle-type-driven-design](principle-type-driven-design.md) - Related: Model "already done" as state the
  type carries, not as a fact the caller must remember

## External References

- [Idempotency in Depth (Luca Palmieri)](https://lpalmieri.com/posts/idempotency/)
- [Atomic file writes and `rename(2)` (LWN)](https://lwn.net/Articles/457667/)
