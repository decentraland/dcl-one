# Sites page components

- Status: Accepted
- Date: 2026-11-20

## Decision

Keep React Router entries, loaders, actions, headers, metadata and error boundaries in `apps/sites/src/routes/`. A route can render a library view directly. When a screen needs a separate component to compose library views with Sites data or behavior, put that component in `apps/sites/src/pages/<domain>/`. Routes can share the same page.

Domain hooks, providers and interaction logic in `src/features/` can serve several pages. The dependency runs from pages to features. Fetchers stay in the data layer. Visual components and their styles follow the [UI standards](../ui-standards.md#app-specific-ui).

## Rationale

A page used by multiple route modules needs a predictable home outside the router's discovered routes. Keeping it above features also allows features to be reused without importing a screen or its route contract.
