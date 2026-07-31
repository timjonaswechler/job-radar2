# TypeScript and React engineering

Job Radar uses correctness-first TypeScript and predictable React state.
Complexity belongs at external adapters, orchestration stays testable, and UI
modules render explicit state. Prefer the smallest model that makes correct
behavior unsurprising.

Apply these rules to new and materially changed TypeScript and React code.
Existing code does not need unrelated migration merely to conform to this
document.

## Contracts at seams

Everything crossing a runtime seam has an explicit transport contract.

- Treat values from Tauri, JSON, storage, browser APIs, and other external
  systems as untrusted runtime data.
- Receive uncertain values as `unknown` and validate them at the owning seam.
- Keep Tauri transport types and runtime decoding in `src/lib/api/`.
- Let feature modules consume validated values rather than transport details.
- Review the Rust and TypeScript sides of a transport contract together.
- Keep transport types, domain models, form state, and view models distinct
  when they have different responsibilities.

A generic annotation such as `invoke<Result>(...)` describes the expected
result to TypeScript; it does not validate the value at runtime. Match the
strength of runtime validation to the risk and variability of the seam.

## Module roles

Follow [Module design and naming](module-design.md) for module seams, interface
depth, and naming. Within the frontend, use these ownership defaults:

- `src/lib/api/` owns Tauri commands, transport types, and runtime decoding.
- `src/features/<feature>/model/` owns pure domain, form, and view-model logic.
- Feature hooks own UI-adjacent orchestration and lifecycle.
- Feature components own rendering and user interaction.
- `src/components/ui/` and `src/components/reui/` own reusable UI primitives.
- `src/app/` owns application-wide composition, navigation, and providers.

Move logic into a shared module when it has a clear owner or a second concrete
caller. Keep feature-specific behavior with its feature. Use deliberate feature
entry points where callers need a stable interface; prefer direct imports over
catch-all barrels that obscure ownership and dependencies.

## Complexity at adapters

Keep Tauri, browser, storage, and library-specific behavior at the adapter that
owns it.

- Present small, stable operations to orchestration code.
- Keep transport details out of UI components.
- Translate external failures into stable results at the adapter seam.
- Keep domain transformations deterministic where practical.
- Accept external dependencies at the owning seam instead of creating them
  inside model logic.

An abstraction earns its interface by removing knowledge or coordination from
callers. Future possibilities alone do not require another layer.

## Predictable state

Model related states so that invalid combinations cannot be constructed.

- Use discriminated unions for mutually exclusive states and async lifecycles.
- Include reverse states and actions: opening has closing, activation has
  deactivation, and starting work has completion or cancellation.
- Store genuinely mutable UI state and derive values that can be computed from
  existing state.
- Give each async operation one owner responsible for its lifecycle.
- Prevent stale responses from replacing newer state.
- Define cleanup for timers, listeners, subscriptions, and polling.
- Model cancellation, failure, and partial success when callers or users can
  observe them.
- Make spinners, labels, disabled actions, and Diagnostics reflect the actual
  state.

Prefer:

```ts
type LoadState<T> =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "loaded"; data: T }
  | { status: "failed"; error: string };
```

over independent booleans and nullable values whose combinations callers must
interpret.

## React responsibilities

Keep business rules and data transformations outside rendering code.

- Express validation, normalization, and state transitions as pure functions.
- Use Effects to synchronize React with external systems, not to replace model
  functions.
- Derive values during rendering; use `useMemo` when calculation cost or stable
  identity makes it relevant.
- Use `useCallback` when function identity is part of a child or Effect
  interface, not as a default wrapper.
- Move multi-step async workflows into a hook or orchestration module with a
  small interface.
- Let component props describe the component's role rather than expose its
  internal mechanics.
- Treat accessibility as part of an interactive component's interface.
- Route product-facing text through the existing localization system when the
  surrounding feature is localized.

Component size is a design signal, not a fixed limit. Split a component when
doing so clarifies state ownership, behavior, or its interface.

## Type precision

Use inference inside implementations and explicit types where they make a seam
or domain distinction clearer.

- Use `unknown` for values that have not been validated.
- Keep unavoidable `any` local to a third-party seam and explain why the safer
  type cannot be expressed there.
- Use literal and discriminated unions for finite domain states.
- Introduce distinct types when they protect identity, units, validation, or
  domain meaning.
- Use optional properties only when absence is part of the contract.
- Give `null` and `undefined` deliberate, non-overlapping meanings at a seam.
- Treat casts such as `as SomeType` as a prompt to inspect validation and
  ownership of the value.

## Performance and reliability

Design normal and degraded operation together.

- Transfer only the data a caller needs across the Tauri seam.
- Consider large lists, repeated parsing, avoidable copying, unnecessary
  renders, and continuously repainting animations.
- Let one module own normalized or decoded data instead of repeating work in
  several callers.
- Add memoization in response to concrete identity or computation needs.
- Base optimizations on measurement or a concrete resource and complexity
  analysis.
- Treat errors, slow responses, cancellation, and partial results as normal
  operating states rather than exceptional UI afterthoughts.
- Include performance costs that may surprise callers in the module's
  interface.

## Tests and focused proof

Use the smallest proof that exercises the changed behavior while iterating,
then follow the repository's `quick -> focused -> full` handoff loop in
[`validation.md`](../validation.md).

- Test pure model functions without React.
- Test components through visible behavior and user interaction.
- Cover relevant loading, success, empty, error, cancellation, and partial
  states.
- Fake Tauri behavior at the API seam instead of reproducing transport mocks in
  every component.
- Test regressions at the seam where the incorrect behavior was observable.
- Keep tests independent of internal hook calls and incidental DOM structure.

A successful TypeScript compilation proves type consistency, not the complete
runtime behavior of a change. Run `just verify` before handoff.

## Completion criteria

A TypeScript or React change is architecturally reviewed when every new or
materially changed feature seam accounts for:

1. the module that owns its contract;
2. validation of external runtime data;
3. the valid states its types permit;
4. ownership of pure model logic and async orchestration;
5. the adapter containing external complexity;
6. errors, cancellation, cleanup, stale responses, and partial results when
   relevant;
7. affected command, feature, and UI entry points;
8. performance characteristics that may surprise callers or users;
9. a focused test of its observable behavior; and
10. accessibility, localization, and documentation when relevant.

Mark an item as not applicable only when the seam genuinely does not have that
concern.
