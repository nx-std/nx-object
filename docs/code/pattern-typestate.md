---
name: "pattern-typestate"
description: "Typestate pattern — model state machines with distinct types to enforce valid transitions at compile time. Load when designing workflows, pipelines, or objects with lifecycle states"
type: core
scope: "global"
---

# Typestate Pattern (State Machines with Types)

**MANDATORY for ALL Rust code in the workspace**

## Rule

Use distinct types for each state to prevent invalid transitions at compile time. When an object has a lifecycle (created → started → completed), each phase is a separate type, so only the operations valid in that state exist.

A struct that guards transitions with a status enum and runtime assertions catches invalid transitions only at runtime. Replace the enum with distinct types that consume `self` on transition, making an invalid transition a compile error.

## Examples

```rust
// ❌ Bad — runtime state checking, panics on invalid transition
pub struct Transfer {
    phase: TransferPhase,
    // ...
}

impl Transfer {
    pub fn send_name(&mut self) {
        assert_eq!(self.phase, TransferPhase::Connected); // Runtime panic!
        self.phase = TransferPhase::Named;
    }

    pub fn send_data(&mut self) {
        assert_eq!(self.phase, TransferPhase::Named); // Runtime panic!
        self.phase = TransferPhase::Sent;
    }
}

// Nothing prevents sending the payload before the console has acknowledged the name
```

```rust
// ✅ Good — type system enforces the protocol's order
pub struct ConnectedTransfer { sock: TcpStream }
pub struct NamedTransfer { sock: TcpStream, file_length: u32 }
pub struct SentTransfer { sock: TcpStream, file_length: u32 }

impl ConnectedTransfer {
    pub async fn send_name(self, name: &str, length: u32) -> Result<NamedTransfer, SendNroError> {
        // ...writes the name and length, then waits for the acknowledgement
    }
}

impl NamedTransfer {
    pub async fn send_data(self, nro: &[u8]) -> Result<SentTransfer, SendNroError> {
        // ...
    }
}

// Usage:
let transfer = ConnectedTransfer::connect(console).await?;
let transfer = transfer.send_name(name, length).await?;  // Connected -> Named
let transfer = transfer.send_data(&nro).await?;          // Named -> Sent
// transfer.send_name(..);                               // Compile error — SentTransfer has no send_name()
```

```rust
// ✅ Good — typestate with shared data via a generic parameter
pub struct Transfer<S> {
    sock: TcpStream,
    console: ConsoleAddr,
    state: S,
}

pub struct Connected;
pub struct Named { file_length: u32 }
pub struct Sent { file_length: u32 }

impl Transfer<Connected> {
    pub async fn send_name(self, name: &str, length: u32) -> Result<Transfer<Named>, SendNroError> {
        Ok(Transfer { sock: self.sock, console: self.console, state: Named { file_length: length } })
    }
}

impl Transfer<Named> {
    pub async fn send_data(self, nro: &[u8]) -> Result<Transfer<Sent>, SendNroError> {
        Ok(Transfer {
            sock: self.sock,
            console: self.console,
            state: Sent { file_length: self.state.file_length },
        })
    }
}
```

## Why It Matters

Runtime state assertions are invisible to the compiler: they fail only when the wrong code path executes, which may happen first on a user's console under conditions no test reproduced. Typestate turns an invalid transition into a compile error, eliminating an entire class of logic bugs, and the type signature documents which operations are valid in each state — enforcement and documentation in one.

## Pragmatism Caveat

Not every stateful object needs typestate. An object with two states, or simple well-tested transitions, may be simpler as a status enum with clear documentation. Apply typestate when invalid transitions would cause serious bugs, when the state machine is complex enough that runtime assertions are easy to forget, or when multiple callers might not know the correct transition order. For values stored in a collection or deserialized from a file (where a single concrete type is needed), a status enum is often the practical choice — typestate works best for in-memory, linear workflows such as a wire protocol's handshake.

## Checklist

Before committing code, verify:

- [ ] State transitions consume `self` (move semantics) to prevent reuse of the old state
- [ ] Each state type only exposes operations valid for that state
- [ ] No runtime assertions (`assert!`, `panic!`) for state validity that the type system could enforce
- [ ] State-specific data is only present in the types where it exists (e.g., `file_length` only after the name is acknowledged)
- [ ] Simple two-state objects or deserialized entities use status enums when typestate adds unnecessary complexity

## References

- [principle-type-driven-design](principle-type-driven-design.md) - Foundation: Design principle this pattern implements
- [pattern-builder](pattern-builder.md) - Related: Builder pattern can use typestate for compile-time required field enforcement
