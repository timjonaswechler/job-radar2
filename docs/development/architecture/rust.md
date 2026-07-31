# Rust engineering

Job Radar uses correctness-first, type-driven Rust. Model states,
invariants, ownership, and error modes before implementing behavior.
Prefer straightforward code whose correctness is visible from its types
and control flow.

Apply these rules to new and materially changed Rust code. Existing code
does not need unrelated migration merely to conform to this document.

## Types and invariants

Use types to express domain distinctions and prevent invalid states.

- Represent mutually exclusive states with enums.
- Group values that form one invariant into one type.
- Validate values when they enter the type that owns their invariant.
- Keep fields private when unrestricted construction could violate an
  invariant.
- Introduce a newtype when it protects identity, units, validation, or
  domain meaning. A wrapper without such a responsibility does not earn
  its interface.

Prefer:

```rust
enum Completion {
    Completed,
    CompletedWithErrors,
    Cancelled,
    Failed,
}
```

over several booleans whose valid combinations callers must remember.

## Interfaces and ownership

Follow [Module design and naming](module-design.md) for module seams,
interface depth, visibility, and naming.

Let ownership reflect responsibility:

- Take a value when the module retains or consumes it.
- Borrow a value when the operation only observes it.
- Return owned results when ownership passes to the caller.
- Make meaningful cloning visible at the point where ownership branches.
- Introduce shared ownership only when values genuinely have multiple
  owners.
- Accept external dependencies at the owning seam instead of constructing
  them inside domain logic.

Keep visibility as narrow as the callers permit. Introduce traits when a
seam has real variation or a concrete testing need, not solely for
hypothetical substitution.

## Errors

Error behavior is part of a module's interface.

- Use structured error types when callers need to distinguish causes or
  respond differently.
- Preserve relevant source errors and add operational context at the layer
  that owns that context.
- Translate internal errors into the serializable application transport
  error at the Tauri command seam.
- A private leaf helper may return a simple error when no caller needs
  structured recovery.
- Represent expected failure with `Result` or another explicit outcome.
- Reserve panics for violated internal invariants that the program has
  already established.

Production `expect` calls state the invariant they rely on. Tests may use
`unwrap` and `expect` when setup failure should abort the test and does not
hide the behavior under test.

## Effects, async, and concurrency

Keep domain decisions deterministic where practical. Concentrate database,
filesystem, network, clock, browser, and process interactions at explicit
seams.

- Bound external work with the relevant request, item, byte, page, action,
  or duration limits.
- Treat cancellation and partial completion as modeled outcomes where the
  caller can observe them.
- Return structured Diagnostics for recoverable or partial failures.
- Keep lock scopes small and never hold a synchronous lock guard across an
  `.await`.
- Give spawned work an explicit owner and a defined completion,
  cancellation, or cleanup path.
- Keep retries bounded and expose exhaustion through the module's normal
  error or diagnostic interface.

For Source Profiles and Search Runs, implement these capabilities through
the generic DSL or pipeline rather than source-specific Rust branches.

## Clarity and performance

Prefer boring code: conventional control flow, locally visible data
transformations, and abstractions that remove knowledge from callers.

- Use iterators when they make a transformation clearer.
- Use loops when mutation, early exit, or error handling becomes clearer.
- Introduce generics when they provide leverage to concrete callers.
- Keep internal complexity behind a smaller interface instead of exporting
  intermediate mechanics.
- Explain non-obvious invariants, safety arguments, and tradeoffs; let the
  code explain its syntax.
- Base performance work on measurement or a concrete complexity and
  resource analysis.
- Include surprising runtime, allocation, blocking, or I/O costs in the
  interface callers must understand.

## Tests

Test behavior through the narrowest stable interface that owns it.

Follow the test-location rules in the repository
[`AGENTS.md`](../../../AGENTS.md) and the ownership map in
[`validation.md`](../validation.md).

Cover the observable contract:

- valid behavior and state transitions;
- rejected inputs and preserved invariants;
- relevant error distinctions;
- cancellation, limits, and partial results where applicable;
- persistence or adapter contracts at their actual seam.

Use in-module tests for private helpers and narrow implementation
invariants. Avoid making implementation details public solely to test them.

## Completion criteria

A Rust change is architecturally reviewed when every new or materially
changed public seam accounts for:

1. the invariant or responsibility it owns;
2. the valid states its types permit;
3. ownership of inputs, retained dependencies, and returned values;
4. errors callers can observe or distinguish;
5. external effects and their seam;
6. cancellation, bounds, and partial completion when relevant;
7. tests of its observable behavior; and
8. performance characteristics that may surprise callers.

Mark an item as not applicable only when the seam genuinely does not have
that concern.
