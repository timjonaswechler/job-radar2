# Agent Instructions

## UI and Design Work

Before changing UI, read:

- `docs/design/README.md`
- `docs/design/product-audience.md`
- `docs/design/information-architecture.md`
- `docs/design/ui-data-rules.md`
- `docs/design/domain-ui-contract.md`
- `docs/design/visual-direction.md`
- `docs/design/inspiration.md`

Core rules:

- Do not mirror backend schemas directly into the UI.
- Keep UI states semantically faithful to backend/domain states; derived labels need explicit source data and derivation.
- Show user-facing decision data first; put technical data behind progressive disclosure.
- Prefer Data Grid/List + Detail Panel patterns for comparable work items.
- Use Timeline/Activity patterns for chronological events.
- Use Modals only for blocking or complex actions; use Popovers/Dropdowns for secondary actions.
- Every new screen needs Empty, Loading, Error, Disabled, Hover/Focus states.
- Use shadcn/reui components and semantic Tailwind tokens.
- Do not use raw colors like `text-lime-500`, `bg-blue-50`, or arbitrary brand colors in product UI.
- Treat `primary` as a true main-action color, not a general decorative brand color.
- Use brand/accent color sparingly for small highlights, glows, active markers, and selected states.

For UI generated from backend data, create a visibility matrix before implementation:

```txt
Field | Visible to user? | Why? | Disclosure level
```

Fields without a clear user-facing purpose stay hidden, advanced, or debug-only.
