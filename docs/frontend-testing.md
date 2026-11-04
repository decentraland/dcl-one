# Frontend testing standards

Normative standards for tests of UI code in every current and future app under `apps/` and in `rendering/bevy-explorer/source/react-web`, the same [scope as the web app architecture standards](web-app-standards.md#scope-and-relation-to-the-contributor-guides). The apps named here are evidence of current practice, not an allowlist, and `apps/ui3` is being retired. The Explorer HUD is in scope; the mapping of its existing harness has no decision yet. Each section records a decision that has been made; areas without a decision say so instead of guessing. These standards replace the UI parts of the contributor guides' testing standards (`testing-uis`, and the Jest and `redux-saga-test-plan` rules in `writing-tests`). Architecture rules are in [Web app architecture standards](web-app-standards.md). The Explorer's console end-to-end suite is covered by `apps/bevy-explorer/e2e/README.md`.

## Contents

- [Tooling](#tooling)
- [Layout and naming](#layout-and-naming)
- [Components](#components)
- [Query hooks](#query-hooks)
- [Network doubles](#network-doubles)
- [Stores](#stores)
- [Server rendering validation](#server-rendering-validation)
- [Selecting elements](#selecting-elements)
- [Reporting](#reporting)

## Tooling

- **Decision:** tests run on Vitest, and components are tested with Testing Library in jsdom. Do not add Jest, Enzyme or `redux-saga-test-plan`.
- `apps/ui3` (`apps/ui3/vitest.config.ts`) and `react-web` (`rendering/bevy-explorer/source/react-web/vitest.config.ts`) set jsdom for the whole app. `apps/sites` runs Vitest in node by default and opts into jsdom per test file with the `@vitest-environment jsdom` comment.
- `apps/ui3` configures the runner in `apps/ui3/vitest.config.ts`, with the shared setup in `apps/ui3/src/test/setup.ts`. `apps/sites` merges its own config over the Vite config (`apps/sites/vitest.config.ts`). End-to-end route tests for Sites run through `apps/sites/vitest.e2e.config.ts` (`pnpm test:e2e`).
- Mock with `vi.fn`, `vi.spyOn` and `vi.stubGlobal`, and restore in `afterEach`.

## Layout and naming

- Tests sit next to the code they cover, named `<module>.test.ts` or `<module>.test.tsx`.
- Describe behavior from the user's or caller's side: given this input or action, this observable result. Do not assert call order or private state.
- Keep one scenario per test, with its own setup. Tests never share a client, store or double across tests, so they do not depend on order: create a fresh instance per test (from the store factory where there is one), or reset the shared instance in `beforeEach`. A reset of a module-level store clears its listeners and pending work as well as its values, not only the values.

## Components

- Render with Testing Library and query the way a user finds the element: role, label, text. Drive interaction with `@testing-library/user-event`.
- A component that reads server data is rendered inside a `QueryClientProvider` with a client created for that test (see below).
- Test a component's behavior with different props and states (loading, error, empty, loaded), not the source of its data.
- Verify visual and responsive changes in a browser as [UI standards](ui-standards.md#verification) require; unit tests do not prove visual parity.

## Query hooks

- **Decision:** test a hook with `renderHook` and a wrapper that provides a `QueryClientProvider`. Create the client with `new QueryClient({ defaultOptions: { queries: { retry: false } } })` so a failing request fails the test at once instead of retrying. Current example: `apps/ui3/src/data/hooks/useCommunityProfiles.test.tsx`.
- Wait on the observable result (`waitFor`, `findBy*`), not on timers.
- Test the key factory and the fetcher as plain functions: every input that changes the response changes the key (see [Query keys and options](web-app-standards.md#query-keys-and-options)).
- For a mutation, assert the request, the resulting cache invalidation (the queries that refetch) and the error state. For an optimistic update, assert the rollback on error.

## Network doubles

- Stub `fetch` with `vi.spyOn(globalThis, "fetch")` or `vi.stubGlobal("fetch", ...)` when a test needs one or two responses. Build a small domain harness when many tests share the same endpoints (current example: the Shop API fixtures in `apps/ui3/src/shop/api/fixtures.ts`).
- MSW is allowed and not required. Current use is limited to `apps/ui3/src/shop/api/client.test.ts` and its fixtures. No decision yet on making it the default.
- A test never reaches the real network.

## Stores

- A store (`useSyncExternalStore` or other) is unit tested as a plain module: its state transitions, its scoping to the account (and reset on sign-out when it has one), and its subscription cleanup. Current example of testing a store as a plain module: `apps/ui3/src/shop/cart/store.test.ts`, which covers the per-account carts; it has no reset or subscription-cleanup test.
- Test the hook that reads the store through `renderHook` when it adds behavior of its own.
- No coverage threshold is set; no decision yet.

## Server rendering validation

Applies to a route that hands server data to TanStack Query hooks, and to the migration of any [exception](web-app-standards.md#exceptions-and-migration-gates). Run these cases against the route's real loader and handler, not a mock of them.

| Case | What it proves |
| --- | --- |
| Concurrent requests | Two interleaved requests for different accounts, locales or parameters return different data and share no cache or module state |
| First hydration | After the first document, the browser cache holds the expected keys and the page makes no duplicate request for them |
| Client navigation | A client navigation delivers no dehydrated state and fetches through the browser client; the route's own server loader is not re-run (the root loader revalidates only as its `shouldRevalidate` decides) |
| Required-data errors on a document request | A failure of required data returns the intended status (404 for a missing entity, 503 for an unavailable upstream) from the status-bearing response the loader throws or returns, through the real handler, not a rendered loading page; the `ErrorBoundary` is not the source of the status |
| Required-data errors on a client navigation | Navigating to a missing or unavailable required entity renders the route's not-found or unavailable UI through the route `ErrorBoundary`, not a blank page or an endless loading state; a background refetch error does not replace a page that is already rendered (the data stays and the error shows inline); retry from the boundary (`reset()` plus a revalidation or navigation) renders the route again instead of rethrowing the cached error, and navigating away and back after a route error renders the route again only after a reset |
| Native mount | A host that mounts the pages (for example `mountShop` in `apps/sites/src/routes/shop/native.client.tsx`) renders with a provider |
| Account switch | After the switch, entries of the previous account in both key shapes (`["private", account, ...]` and the account-scoped keys not yet migrated) are gone, observers and in-flight requests of the old account do not repopulate them so no old entry reappears after the removal, and one account's data is not shown to the next |
| Sign-out | After sign-out no private entry remains in the cache, in either key shape (`["private", ...]` and the account-scoped keys not yet migrated) |

Current example of a test that calls a route's real loader: `apps/sites/test/e2e/places-loader.e2e.test.ts`.

## Selecting elements

Role, label and text queries are the most used in current tests, and many components also carry a `data-testid`. No decision yet on a `data-testid` rule. Until one is made, prefer a role or label query and add a `data-testid` only when no accessible query identifies the element. Do not select by styling class names.

## Reporting

A change reports which checks ran and their results, and which did not run and why. A documentation-only change needs link, structure and content checks, not an application build.
