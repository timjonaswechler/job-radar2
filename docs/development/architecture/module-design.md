# Module design and naming

Job Radar favors deep modules: substantial behavior behind a small interface,
placed at a clear seam and testable through that interface. Module structure and
naming should make ownership visible without repeating the same context in every
identifier.

## Let the module path carry context

Group concepts that belong together in a logically named module. Within that
module, use the shortest name that remains precise. Do not repeat ancestor module
names in descendants merely to make an identifier globally descriptive.

Prefer:

```rust
source::onboarding::Proposal
source::onboarding::Error
source::onboarding::detect(...)
```

over:

```rust
source_onboarding::SourceOnboardingProposal
source_onboarding::SourceOnboardingError
source_onboarding::detect_source_onboarding(...)
```

The same principle applies in TypeScript: import paths and owning modules carry
context; exported names describe the role within that context.

Language-required separators are not the concern. Rust `snake_case`, Cargo
package names, and an underscore used for an intentionally unused binding remain
idiomatic. The smell is a chain of prefix or suffix words used to compensate for
missing ownership or an unclear module structure.

## Keep the interface small

Nesting alone does not create a deep module. A directory tree with many public
leaf modules can still expose a shallow interface. The owning module should:

- expose only the operations and types callers need;
- keep orchestration, helpers, and adapters private where possible;
- use deliberate re-exports for its canonical interface;
- concentrate invariants, error modes, and ordering constraints at its seam; and
- be testable through the same interface used by production callers.

Before introducing several similarly named public types or functions, ask
whether one deeper operation can hide their coordination.

## Prefix and suffix rule

Names do not repeat the context of their module path. Related concepts are
grouped by modules and receive short, precise local names. A prefix or suffix is
appropriate only when it:

- carries domain meaning that distinguishes two concepts in the same scope;
- communicates a stable role such as `Id`, `Input`, `Config`, or `Error`;
- is needed at an external seam where the module path is not retained; or
- is required by Rust, Cargo, Tauri, an operating system, or another tool.

Do not create siblings whose names differ only by a small generic prefix or
suffix when a module can express the relationship more clearly. Treat repeated
prefixes and suffixes as a prompt to inspect the seam, not as an automatic rename.

## Review questions

For every new module or architecture candidate, check:

1. Does the module own one coherent behavior or domain responsibility?
2. Does its path provide enough context for shorter local names?
3. Does every public name add information not already present in that path?
4. Can callers use the module without learning its internal module tree?
5. Would deleting the module spread its complexity across multiple callers?
6. Are prefixes and suffixes carrying meaning rather than compensating for a
   weak seam?

Apply these questions to code being added or materially restructured. Avoid
repository-wide renaming churn that does not improve a module's interface,
leverage, or locality.

## Exceptions

Document non-obvious technical exceptions next to the affected declaration. For
example, the desktop package uses `job_radar_lib` because its library target must
not collide with the binary target on supported platforms. Such a constraint is
not evidence that redundant affixes should become the general naming style.
